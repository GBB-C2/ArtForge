//! Resolve OBJ/MTL and glTF image paths without crawling directories or fetching URLs.
use std::path::{Path, PathBuf};

pub(crate) struct Resource {
    pub path: PathBuf,
    pub relocated: bool,
}

/// Prefer the authored path. A moved export can keep its resources beside the
/// OBJ/MTL or in conventional immediate subfolders. Never probe UNC or URLs.
pub(crate) fn resolve(base: &Path, model_base: &Path, reference: &str) -> Result<Resource, String> {
    let normalized = normalize(reference)?;
    let windows_drive = drive_prefix(&normalized);
    if let Some(path) = direct_path(base, &normalized).filter(|p| p.is_file()) {
        return Ok(Resource {
            path,
            relocated: false,
        });
    }
    let filename = normalized.rsplit('/').next().unwrap_or("");
    let filename = if windows_drive && !normalized.contains('/') {
        &filename[2..]
    } else {
        filename
    };
    if filename.is_empty() || matches!(filename, "." | "..") || filename.contains(':') {
        return Err("некорректное имя локального ресурса".into());
    }
    // Fixed candidates keep missing assets cheap, including enormous folders.
    // Both roots matter when the MTL is in a materials/ subfolder and textures
    // were copied next to the OBJ or into its textures/ folder.
    for root in [base, model_base] {
        for folder in [
            "",
            "textures",
            "Textures",
            "maps",
            "Maps",
            "materials",
            "Materials",
        ] {
            let candidate = root.join(folder).join(filename);
            if candidate.is_file() {
                return Ok(Resource {
                    path: candidate,
                    relocated: true,
                });
            }
        }
    }
    Err("локальный файл не найден по исходному пути и рядом с моделью/MTL".into())
}

/// glTF references are URIs; OBJ paths deliberately keep literal '%' characters.
pub(crate) fn resolve_image_uri(base: &Path, uri: &str) -> Result<Resource, String> {
    // Reject network schemes before decoding/rebasing, including encoded UNC.
    // A file URI decodes itself exactly once in normalize().
    if uri
        .get(..5)
        .is_some_and(|s| s.eq_ignore_ascii_case("file:"))
    {
        return resolve(base, base, uri);
    }
    let decoded =
        urlencoding::decode(uri).map_err(|_| "некорректная кодировка URI текстуры".to_owned())?;
    resolve(base, base, &decoded)
}

fn direct_path(base: &Path, normalized: &str) -> Option<PathBuf> {
    let path = Path::new(normalized);
    if drive_prefix(normalized) {
        // Foreign drives must not become relative directories named "C:".
        // C:map.png depends on hidden per-drive cwd state, so only rebase it.
        (cfg!(windows) && normalized.as_bytes().get(2) == Some(&b'/')).then(|| path.to_path_buf())
    } else if path.is_absolute() {
        Some(path.to_path_buf())
    } else {
        Some(base.join(path))
    }
}

pub(crate) fn library_references(base: &Path, value: &str) -> Vec<String> {
    // A single unquoted filename can contain spaces. Prefer it when it exists;
    // otherwise distinguish the common multiple-filename form by .mtl suffixes.
    let mut items = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut quoted = false;
    for ch in value.chars() {
        if quote == Some(ch) {
            quote = None;
        } else if quote.is_none() && word.is_empty() && matches!(ch, '\'' | '"') {
            quote = Some(ch);
            quoted = true;
        } else if quote.is_none() && ch.is_whitespace() {
            if !word.is_empty() {
                items.push(std::mem::take(&mut word));
            }
        } else {
            word.push(ch);
        }
    }
    if !word.is_empty() {
        items.push(word);
    }
    if quoted
        || (items.len() > 1
            && items
                .iter()
                .all(|p| p.to_ascii_lowercase().ends_with(".mtl"))
            && !normalize(value)
                .ok()
                .and_then(|n| direct_path(base, &n))
                .is_some_and(|p| p.is_file()))
    {
        items
    } else {
        vec![value.trim().to_owned()]
    }
}

/// An exporter can quote names containing '#'; only unquoted '#' starts a comment.
pub(crate) fn without_comment(line: &str) -> &str {
    let mut quote = None;
    let mut boundary = true;
    for (index, ch) in line.char_indices() {
        if quote == Some(ch) {
            quote = None;
        } else if quote.is_none() && boundary && matches!(ch, '\'' | '"') {
            quote = Some(ch);
        } else if quote.is_none() && ch == '#' {
            return &line[..index];
        }
        boundary = ch.is_whitespace();
    }
    line
}

fn drive_prefix(path: &str) -> bool {
    path.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && path.as_bytes().get(1) == Some(&b':')
}

fn normalize(reference: &str) -> Result<String, String> {
    let reference = reference.trim();
    let reference = reference
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| {
            reference
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
        })
        .unwrap_or(reference);
    let mut path = reference.replace('\\', "/");
    if path
        .get(..5)
        .is_some_and(|s| s.eq_ignore_ascii_case("file:"))
    {
        let rest = &path[5..];
        let local = if let Some(rest) = rest.strip_prefix("//") {
            let (host, tail) = rest.split_once('/').unwrap_or((rest, ""));
            if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
                return Err(
                    "сетевой file URI пропущен; используйте локальную копию ресурса".into(),
                );
            }
            format!("/{tail}")
        } else {
            rest.to_owned()
        };
        path = urlencoding::decode(&local)
            .map_err(|_| "некорректная кодировка file URI".to_owned())?
            .into_owned();
        path = path.replace('\\', "/");
        if path.starts_with('/') && drive_prefix(&path[1..]) {
            path.remove(0);
        }
        if !path.starts_with('/') && !(drive_prefix(&path) && path.as_bytes().get(2) == Some(&b'/'))
        {
            return Err("file URI должен содержать абсолютный локальный путь".into());
        }
    }
    if path.is_empty() || path.contains('\0') {
        return Err("пустой путь или нулевой символ в имени ресурса".into());
    }
    if path.starts_with("//") {
        return Err("сетевой UNC-путь пропущен; используйте локальную копию ресурса".into());
    }
    if path.contains(':') && !drive_prefix(&path) {
        return Err("сетевая ссылка или неподдерживаемая схема ресурса пропущена".into());
    }
    Ok(path)
}
