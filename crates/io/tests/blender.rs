use poseforge_io::{
    import_blend_with_options, import_blend_with_progress, locate_blender, BlendImportOptions,
    IoError,
};
use std::{path::Path, sync::atomic::AtomicBool};

#[test]
fn source_animation_is_opt_in() {
    assert!(!BlendImportOptions::default().import_animations);
}

#[test]
fn cancellation_precedes_reading_and_launching_blender() {
    let result =
        import_blend_with_progress("missing.blend", None, &AtomicBool::new(true), |_, _| {});
    assert!(matches!(result, Err(IoError::Cancelled)));
}

#[test]
fn explicit_missing_executable_does_not_fall_back_to_another_installation() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    assert!(locate_blender(Some(&directory.path().join("missing-blender"))).is_err());
}

#[test]
fn wrong_file_type_is_rejected_without_launch() {
    let result = import_blend_with_progress(
        "model.glb",
        Some(Path::new("missing")),
        &AtomicBool::new(false),
        |_, _| {},
    );
    assert!(result.unwrap_err().to_string().contains(".blend"));
}

#[cfg(target_os = "windows")]
#[test]
fn windows_batch_files_are_not_accepted_as_blender_executables() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let path = directory.path().join("blender.cmd");
    std::fs::write(&path, "@echo off\r\nexit /b 0\r\n").unwrap();
    assert!(locate_blender(Some(&path)).is_err());
}

#[cfg(unix)]
mod process_tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::PathBuf,
        sync::{atomic::Ordering, Mutex},
    };

    // A concurrent fork can briefly inherit another test's write handle to a
    // freshly authored script, before exec closes CLOEXEC descriptors. Executing
    // that script in the meantime fails with ETXTBSY. Serialize fixture creation
    // through child cleanup; the production importer must still report genuine
    // executable errors instead of silently retrying them.
    static PROCESS_FIXTURE: Mutex<()> = Mutex::new(());

    fn executable(dir: &Path, text: &str) -> PathBuf {
        let path = dir.join("fake blender with spaces");
        fs::write(&path, text).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    fn quote(path: &Path) -> String {
        format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
    }

    fn fixture(directory: &Path, animated: bool) -> PathBuf {
        let original =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/skinned_triangle.glb");
        if animated {
            return original;
        }
        let data = fs::read(original).unwrap();
        assert_eq!(&data[..4], b"glTF");
        let json_len = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
        let mut document: serde_json::Value =
            serde_json::from_slice(&data[20..20 + json_len]).unwrap();
        assert!(document
            .as_object_mut()
            .unwrap()
            .remove("animations")
            .is_some());
        let mut json = serde_json::to_vec(&document).unwrap();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let remainder = &data[20 + json_len..];
        let mut output = b"glTF".to_vec();
        output.extend_from_slice(&2u32.to_le_bytes());
        output.extend_from_slice(&((20 + json.len() + remainder.len()) as u32).to_le_bytes());
        output.extend_from_slice(&(json.len() as u32).to_le_bytes());
        output.extend_from_slice(b"JSON");
        output.extend_from_slice(&json);
        output.extend_from_slice(remainder);
        let path = directory.join("model only.glb");
        fs::write(&path, output).unwrap();
        path
    }

    #[test]
    fn converter_arguments_are_literal_and_temp_output_is_cleaned_after_import() {
        let _fixture_guard = PROCESS_FIXTURE.lock().unwrap();
        let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let source = directory.path().join("source; $HOME with spaces.BLEND");
        fs::write(&source, b"source bytes").unwrap();
        let fixture = fixture(directory.path(), false);
        let receipt = directory.path().join("output-path.txt");
        let script = format!(
            r#"#!/bin/sh
[ "$1" = --background ] || exit 11
[ "$2" = --factory-startup ] || exit 12
[ "$3" = --disable-autoexec ] || exit 13
[ "$4" = --python-exit-code ] || exit 14
[ "$5" = 1 ] || exit 15
[ "$6" = --python ] || exit 16
[ "$8" = -- ] || exit 18
shift 8
[ "$#" = 3 ] || exit 19
[ -f "$1" ] || exit 20
cp {} "$2"
printf '%s' '{{"fps":30,"blender_version":"fixture process","warnings":[],"import_profile":"MODEL","animations_exported":false}}' > "$3"
printf '%s' "$2" > {}
"#,
            quote(&fixture),
            quote(&receipt)
        );
        let fake = executable(directory.path(), &script);
        let imported =
            import_blend_with_progress(&source, Some(&fake), &AtomicBool::new(false), |_, _| {})
                .unwrap();
        assert_eq!(imported.report.joints, 2);
        assert_eq!(imported.project.fps, 30);
        assert!(imported.project.clips.is_empty());
        assert_eq!(imported.report.textures, 1);
        assert!(imported
            .report
            .warnings
            .iter()
            .any(|text| text.contains("Готовые анимационные клипы не запекались")));
        assert_eq!(fs::read(&source).unwrap(), b"source bytes");
        let temporary_output = PathBuf::from(fs::read_to_string(receipt).unwrap());
        assert!(!temporary_output.exists());
        assert!(!temporary_output.parent().unwrap().exists());
    }

    #[test]
    fn unsuccessful_process_reports_tail_and_keeps_source() {
        let _fixture_guard = PROCESS_FIXTURE.lock().unwrap();
        let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let source = directory.path().join("source with spaces.BLEND");
        fs::write(&source, b"unchanged source").unwrap();
        let fake = executable(
            directory.path(),
            "#!/bin/sh\nprintf 'controlled converter failure\\n' >&2\nexit 7\n",
        );
        let result =
            import_blend_with_progress(&source, Some(&fake), &AtomicBool::new(false), |_, _| {});
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("controlled converter failure"),
            "Expected converter stderr in the error; received: {error}"
        );
        assert_eq!(fs::read(source).unwrap(), b"unchanged source");
    }

    #[test]
    fn running_conversion_can_be_cancelled() {
        let _fixture_guard = PROCESS_FIXTURE.lock().unwrap();
        let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let source = directory.path().join("source.blend");
        fs::write(&source, b"unchanged source").unwrap();
        let fake = executable(
            directory.path(),
            "#!/bin/sh\nprintf 'POSEFORGE_STAGE:EXPORT_MODEL\\n'\nexec sleep 20\n",
        );
        let cancelled = AtomicBool::new(false);
        let result =
            import_blend_with_progress(&source, Some(&fake), &cancelled, |progress, message| {
                if message.starts_with("Экспорт модели") {
                    assert!(progress < 0.);
                    cancelled.store(true, Ordering::Relaxed);
                }
            });
        assert!(
            matches!(result, Err(IoError::Cancelled)),
            "Expected cancellation; received: {:?}",
            result.as_ref().err()
        );
        assert_eq!(fs::read(source).unwrap(), b"unchanged source");
    }

    #[test]
    fn animation_option_reaches_converter_and_preserves_existing_clip_import() {
        let _fixture_guard = PROCESS_FIXTURE.lock().unwrap();
        let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let source = directory.path().join("animated character.blend");
        fs::write(&source, b"unchanged source").unwrap();
        let fixture = fixture(directory.path(), true);
        let fake = executable(
            directory.path(),
            &format!(
                r#"#!/bin/sh
shift 8
[ "$#" = 4 ] || exit 21
[ "$4" = --animations ] || exit 22
cp {} "$2"
printf '%s' '{{"fps":24,"blender_version":"fixture process","warnings":[],"import_profile":"ANIMATION","animations_exported":true}}' > "$3"
"#,
                quote(&fixture)
            ),
        );
        let imported = import_blend_with_options(
            &source,
            Some(&fake),
            BlendImportOptions {
                import_animations: true,
            },
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap();
        assert_eq!(imported.report.joints, 2);
        assert_eq!(imported.report.textures, 1);
        assert!(!imported.project.clips.is_empty());
        assert!(imported
            .report
            .warnings
            .iter()
            .any(|text| text.contains("запечённый диапазон анимации")));
        assert_eq!(fs::read(source).unwrap(), b"unchanged source");
    }

    #[test]
    fn inconsistent_converter_profile_is_rejected_before_import() {
        let _fixture_guard = PROCESS_FIXTURE.lock().unwrap();
        let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let source = directory.path().join("source.blend");
        fs::write(&source, b"unchanged source").unwrap();
        let fixture = fixture(directory.path(), false);
        for metadata in [
            r#"{"import_profile":"ANIMATION","animations_exported":true}"#,
            r#"{"import_profile":"MODEL","animations_exported":true}"#,
            r#"{}"#,
        ] {
            let fake = executable(
                directory.path(),
                &format!(
                    r#"#!/bin/sh
shift 8
cp {} "$2"
printf '%s' '{}' > "$3"
"#,
                    quote(&fixture),
                    metadata
                ),
            );
            let result = import_blend_with_progress(
                &source,
                Some(&fake),
                &AtomicBool::new(false),
                |_, _| {},
            );
            let error = result.unwrap_err().to_string();
            assert!(
                error.contains("режиму импорта"),
                "Unexpected converter error: {error}"
            );
        }
        assert_eq!(fs::read(source).unwrap(), b"unchanged source");
    }

    #[test]
    fn model_only_profile_rejects_unexpected_animation_payload() {
        let _fixture_guard = PROCESS_FIXTURE.lock().unwrap();
        let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let source = directory.path().join("source.blend");
        fs::write(&source, b"unchanged source").unwrap();
        let fixture = fixture(directory.path(), true);
        let fake = executable(
            directory.path(),
            &format!(
                r#"#!/bin/sh
shift 8
cp {} "$2"
printf '%s' '{{"import_profile":"MODEL","animations_exported":false}}' > "$3"
"#,
                quote(&fixture)
            ),
        );
        let result =
            import_blend_with_progress(&source, Some(&fake), &AtomicBool::new(false), |_, _| {});
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("клипы при выключенном"),
            "Unexpected converter error: {error}"
        );
        assert_eq!(fs::read(source).unwrap(), b"unchanged source");
    }
}
