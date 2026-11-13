//! Native path selection, independent of the application's file operations.
//!
//! Run `pick` on a worker thread: it blocks until the user accepts or cancels.
//! Keep the owner window alive until that worker returns. The caller applies the
//! selected path and performs its usual validation and overwrite confirmation.

use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerKind {
    OpenFile,
    SaveFile,
    Folder,
}

#[derive(Clone, Debug)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct PickerFilter {
    pub name: String,
    /// Extensions without a leading dot or wildcard. An empty list means all files.
    pub extensions: Vec<String>,
}

#[derive(Clone, Debug)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct PickerRequest {
    pub kind: PickerKind,
    pub title: String,
    pub initial_path: Option<PathBuf>,
    pub filters: Vec<PickerFilter>,
    /// Borrowed Windows HWND, when available; no ownership is transferred.
    pub owner: Option<isize>,
}

/// Returns the selected path, `None` on cancellation, or an actionable error.
/// No files or directories are written by this function.
pub fn pick(request: PickerRequest) -> Result<Option<PathBuf>, String> {
    #[cfg(windows)]
    {
        windows_backend::pick(request)
    }
    #[cfg(not(windows))]
    {
        let _ = request;
        Err("Системное окно доступно в Windows. Вставьте путь или выберите файл в списке этой панели.".into())
    }
}

#[cfg(windows)]
mod windows_backend {
    use super::{PickerKind, PickerRequest};
    use std::ffi::{c_void, OsStr, OsString};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};
    use windows::core::{HRESULT, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FileOpenDialog, FileSaveDialog, IFileDialog, IShellItem, SHCreateItemFromParsingName,
        FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, FOS_NOTESTFILECREATE,
        FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FOS_STRICTFILETYPES,
        SIGDN_FILESYSPATH,
    };

    struct ComApartment;

    impl ComApartment {
        fn new() -> Result<Self, String> {
            // The caller uses a dedicated worker. Both S_OK and S_FALSE require
            // a matching CoUninitialize on this same thread.
            unsafe {
                CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE)
                    .ok()
                    .map_err(|e| format!("Cannot initialize the system file chooser: {e}"))?;
            }
            Ok(Self)
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            // All dialog interfaces are dropped before the apartment guard.
            unsafe { CoUninitialize() }
        }
    }

    struct ShellPath(PWSTR);

    impl Drop for ShellPath {
        fn drop(&mut self) {
            // IShellItem::GetDisplayName allocates this string with the COM
            // task allocator. It is owned by this guard and freed exactly once.
            unsafe { CoTaskMemFree(Some(self.0 .0.cast::<c_void>())) }
        }
    }

    fn wide(value: &OsStr) -> Result<Vec<u16>, String> {
        let mut text: Vec<u16> = value.encode_wide().collect();
        if text.contains(&0) {
            return Err("A file chooser value contains a null character.".into());
        }
        text.push(0);
        Ok(text)
    }

    fn check<T>(result: windows::core::Result<T>, action: &str) -> Result<T, String> {
        result.map_err(|e| format!("{action}: {e}"))
    }

    fn extension_pattern(extensions: &[String]) -> Result<String, String> {
        if extensions.is_empty() {
            return Ok("*.*".into());
        }
        let mut patterns = Vec::with_capacity(extensions.len());
        for extension in extensions {
            if extension.is_empty()
                || extension.starts_with('.')
                || extension.ends_with('.')
                || !extension
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'))
            {
                return Err(format!("Invalid file chooser extension: {extension}"));
            }
            patterns.push(format!("*.{extension}"));
        }
        Ok(patterns.join(";"))
    }

    fn initial_location(path: &Path, kind: PickerKind) -> Option<(PathBuf, Option<OsString>)> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().ok()?.join(path)
        };
        let filename = if kind != PickerKind::Folder && !absolute.is_dir() {
            absolute.file_name().map(OsStr::to_os_string)
        } else {
            None
        };
        // An export destination can contain directories that do not exist yet.
        // Open at the nearest existing directory and keep its proposed filename.
        let folder = absolute.ancestors().find(|p| p.is_dir())?.to_path_buf();
        Some((folder, filename))
    }

    pub(super) fn pick(request: PickerRequest) -> Result<Option<PathBuf>, String> {
        let _apartment = ComApartment::new()?;
        let title = wide(OsStr::new(&request.title))?;
        let filter_strings: Vec<(Vec<u16>, Vec<u16>)> = if request.kind == PickerKind::Folder {
            Vec::new()
        } else {
            request
                .filters
                .iter()
                .map(|filter| {
                    Ok((
                        wide(OsStr::new(&filter.name))?,
                        wide(OsStr::new(&extension_pattern(&filter.extensions)?))?,
                    ))
                })
                .collect::<Result<_, String>>()?
        };
        let filters: Vec<COMDLG_FILTERSPEC> = filter_strings
            .iter()
            .map(|(name, pattern)| COMDLG_FILTERSPEC {
                pszName: PCWSTR(name.as_ptr()),
                pszSpec: PCWSTR(pattern.as_ptr()),
            })
            .collect();

        // COM interfaces remain on this STA thread. All string buffers passed
        // into the dialog are NUL terminated and live for the corresponding call;
        // filter buffers additionally stay alive through Show.
        unsafe {
            let class = if request.kind == PickerKind::SaveFile {
                &FileSaveDialog
            } else {
                &FileOpenDialog
            };
            let dialog: IFileDialog = check(
                CoCreateInstance(class, None, CLSCTX_INPROC_SERVER),
                "Cannot create the system file chooser",
            )?;
            let mut options = check(dialog.GetOptions(), "Cannot configure the file chooser")?
                | FOS_FORCEFILESYSTEM
                | FOS_NOCHANGEDIR
                | FOS_PATHMUSTEXIST;
            match request.kind {
                PickerKind::OpenFile => options |= FOS_FILEMUSTEXIST,
                PickerKind::Folder => options |= FOS_PICKFOLDERS,
                PickerKind::SaveFile => {
                    // Selection only: the application owns writing and its final
                    // overwrite prompt, after the path has been applied.
                    options = (options & !FOS_OVERWRITEPROMPT) | FOS_NOTESTFILECREATE;
                    if !filters.is_empty() {
                        options |= FOS_STRICTFILETYPES;
                    }
                }
            }
            check(
                dialog.SetOptions(options),
                "Cannot configure the file chooser",
            )?;
            check(
                dialog.SetTitle(PCWSTR(title.as_ptr())),
                "Cannot set the chooser title",
            )?;
            if !filters.is_empty() {
                check(dialog.SetFileTypes(&filters), "Cannot set the file filters")?;
                check(dialog.SetFileTypeIndex(1), "Cannot select the file filter")?;
            }
            if request.kind == PickerKind::SaveFile {
                if let Some(extension) = request.filters.first().and_then(|f| f.extensions.first())
                {
                    let extension = wide(OsStr::new(extension))?;
                    check(
                        dialog.SetDefaultExtension(PCWSTR(extension.as_ptr())),
                        "Cannot set the default file extension",
                    )?;
                }
            }
            if let Some((folder, filename)) = request
                .initial_path
                .as_deref()
                .and_then(|path| initial_location(path, request.kind))
            {
                let folder = wide(folder.as_os_str())?;
                // The starting directory is only a hint: a removed drive or a
                // shell namespace failure must not prevent choosing another path.
                let item: windows::core::Result<IShellItem> =
                    SHCreateItemFromParsingName(PCWSTR(folder.as_ptr()), None);
                if let Ok(item) = item {
                    let _ = dialog.SetFolder(&item);
                }
                if let Some(filename) = filename {
                    let filename = wide(&filename)?;
                    check(
                        dialog.SetFileName(PCWSTR(filename.as_ptr())),
                        "Cannot set the initial filename",
                    )?;
                }
            }
            let owner = HWND(request.owner.unwrap_or_default() as *mut c_void);
            match dialog.Show(owner) {
                Ok(()) => {}
                Err(error) if error.code() == HRESULT::from_win32(ERROR_CANCELLED.0) => {
                    return Ok(None);
                }
                Err(error) => return Err(format!("Cannot show the system file chooser: {error}")),
            }
            let item = check(dialog.GetResult(), "Cannot read the selected path")?;
            let path = ShellPath(check(
                item.GetDisplayName(SIGDN_FILESYSPATH),
                "Cannot read the selected filesystem path",
            )?);
            if path.0.is_null() {
                return Err("The system file chooser returned an empty path.".into());
            }
            // Preserve all Windows filename code units, including non-ASCII and
            // non-UTF-8-representable names; no shell command or lossy conversion.
            let selected = OsString::from_wide(path.0.as_wide());
            if selected.is_empty() {
                return Err("The system file chooser returned an empty path.".into());
            }
            Ok(Some(PathBuf::from(selected)))
        }
    }
}
