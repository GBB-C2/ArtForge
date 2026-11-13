//! Workspace preferences and scene navigation, independent of the document.
use crate::ui::{Accent, Palette, Theme};
use poseforge_core::Project;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static RECOVERY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Separate unsaved documents and application sessions without writing to the
/// current directory. Keep this ID for the lifetime of one untitled document.
pub fn new_recovery_id() -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{}-{stamp:x}-{:x}",
        std::process::id(),
        RECOVERY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

fn recovery_directory_with(
    windows: bool,
    mut variable: impl FnMut(&str) -> Option<OsString>,
) -> Result<PathBuf, String> {
    let (base, suffix) = if let Some(path) = variable("POSEFORGE_RECOVERY_DIR") {
        (PathBuf::from(path), None)
    } else if windows {
        let path = variable("LOCALAPPDATA")
            .filter(|path| !path.is_empty())
            .ok_or("Не задан LOCALAPPDATA. Укажите абсолютный POSEFORGE_RECOVERY_DIR.")?;
        (PathBuf::from(path), Some("PoseForge/recovery"))
    } else if let Some(path) = variable("XDG_STATE_HOME").filter(|path| !path.is_empty()) {
        (PathBuf::from(path), Some("PoseForge/recovery"))
    } else {
        let path = variable("HOME")
            .filter(|path| !path.is_empty())
            .ok_or("Не задан HOME. Укажите абсолютный POSEFORGE_RECOVERY_DIR.")?;
        (PathBuf::from(path), Some(".local/state/PoseForge/recovery"))
    };
    if !base.is_absolute() || base.components().any(|part| part == Component::ParentDir) {
        return Err("Папка восстановления должна иметь абсолютный путь без '..'. Проверьте POSEFORGE_RECOVERY_DIR и каталог данных пользователя.".into());
    }
    Ok(suffix.map_or(base.clone(), |suffix| base.join(suffix)))
}

fn absolute_document_path(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| format!("Не удалось определить путь проекта: {e}"))?
            .join(path)
    };
    // Existing documents and aliases of the same path share a recovery file.
    if let Ok(canonical) = std::fs::canonicalize(&absolute) {
        return Ok(canonical);
    }
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    Ok(normalized)
}

fn recovery_file_path(directory: &Path, document: Option<&Path>, untitled_id: &str) -> PathBuf {
    // Fixed FNV-1a makes the name stable across launches and distinguishes equal
    // basenames in different folders. Only the digest enters the destination;
    // document paths, including parent components, cannot escape this folder.
    let identity = document.map_or_else(
        || untitled_id.as_bytes(),
        |path| path.as_os_str().as_encoded_bytes(),
    );
    let digest = identity.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    let kind = if document.is_some() {
        "project"
    } else {
        "untitled"
    };
    directory.join(format!("{kind}-{digest:016x}.autosave.poseforge"))
}

/// Resolve and create the per-user recovery directory. Never fall back to the
/// project folder or working directory; failures leave manual saving available.
pub fn recovery_path(document: Option<&Path>, untitled_id: &str) -> Result<PathBuf, String> {
    let directory = recovery_directory_with(cfg!(windows), |name| std::env::var_os(name))?;
    prepare_recovery_path(&directory, document, untitled_id)
}

fn prepare_recovery_path(
    directory: &Path,
    document: Option<&Path>,
    untitled_id: &str,
) -> Result<PathBuf, String> {
    if document.is_none() && untitled_id.is_empty() {
        return Err("Не задан идентификатор восстановления новой сцены.".into());
    }
    let identity = document.map(absolute_document_path).transpose()?;
    let path = recovery_file_path(directory, identity.as_deref(), untitled_id);
    std::fs::create_dir_all(directory).map_err(|e| {
        format!(
            "Не удалось создать папку восстановления {}: {e}",
            directory.display()
        )
    })?;
    Ok(path)
}

// This marker migrates layout defaults independently of the settings schema.
const INTERFACE_VERSION: u64 = 3;

#[derive(Clone, Debug)]
pub struct Preferences {
    pub language: crate::localization::Language,
    pub recent_projects: Vec<PathBuf>,
    pub theme: Theme,
    pub accent: Accent,
    pub left_width: Option<f32>,
    pub right_width: Option<f32>,
    pub creation_panel_position: Option<[f32; 2]>,
    pub left_panel: bool,
    pub animation_visible: bool,
    pub animation_layers_open: bool,
    pub tools_expanded: bool,
    pub collapsed_sections: BTreeSet<u16>,
    pub right_panel: bool,
    pub material_grid: bool,
    pub material_thumbnail: f32,
    pub material_favorites: bool,
    pub material_category: String,
    pub adaptive_preview: bool,
    pub performance_overlay: bool,
    pub grid: bool,
    pub viewport_shading: u8,
    pub show_backfaces: bool,
    pub skeleton: bool,
    pub blender_path: Option<PathBuf>,
    pub blend_import_animations: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            language: Default::default(),
            recent_projects: Vec::new(),
            theme: Theme::Dark,
            accent: Accent::default(),
            left_width: None,
            right_width: None,
            creation_panel_position: None,
            left_panel: true,
            animation_visible: true,
            animation_layers_open: false,
            tools_expanded: true,
            collapsed_sections: BTreeSet::new(),
            right_panel: false,
            material_grid: false,
            material_thumbnail: 36.,
            material_favorites: false,
            material_category: String::new(),
            adaptive_preview: true,
            performance_overlay: false,
            grid: true,
            viewport_shading: 3,
            show_backfaces: true,
            skeleton: true,
            blender_path: None,
            blend_import_animations: false,
        }
    }
}
impl Preferences {
    pub fn palette(&self) -> Palette {
        self.accent.palette(self.theme)
    }

    fn path() -> Option<PathBuf> {
        if let Some(p) = std::env::var_os("POSEFORGE_SETTINGS") {
            return Some(p.into());
        }
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))?;
        Some(base.join("PoseForge").join("workspace.json"))
    }
    pub fn from_json(value: &serde_json::Value) -> Self {
        let d = Self::default();
        // Earlier releases saved both always-open panels. Start their first
        // simplified workspace with the canvas visible, preserving other settings.
        let current_layout = value["interface_version"]
            .as_u64()
            .is_some_and(|version| version >= INTERFACE_VERSION);
        Self {
            language: crate::localization::Language::from_name(value["language"].as_str().unwrap_or("ru")),
            recent_projects: value["recent_projects"].as_array().into_iter().flatten().filter_map(|v|v.as_str())
                .filter(|p|!p.is_empty()).take(6).map(PathBuf::from).collect(),
            theme: Theme::from_name(value["theme"].as_str().unwrap_or("dark")),
            accent: Accent::from_name(value["accent"].as_str().unwrap_or("blue")),
            left_width: value["left_width"]
                .as_f64()
                .filter(|v| v.is_finite() && (220. ..=520.).contains(v))
                .map(|v| v as f32),
            right_width: value["right_width"]
                .as_f64()
                .filter(|v| v.is_finite() && (280. ..=600.).contains(v))
                .map(|v| v as f32),
            creation_panel_position: value["creation_panel_position"].as_array().and_then(|v| {
                if v.len() != 2 { return None; }
                let point = [v[0].as_f64()? as f32, v[1].as_f64()? as f32];
                point.iter().all(|v| v.is_finite()).then_some(point)
            }),
            material_grid: value["material_grid"].as_bool().unwrap_or(false),
            material_thumbnail: match value["material_thumbnail"].as_u64() {
                Some(52) => 52.,
                Some(72) => 72.,
                _ => 36.,
            },
            material_favorites: value["material_favorites"].as_bool().unwrap_or(false),
            material_category: value["material_category"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(64)
                .collect(),
            adaptive_preview: value["adaptive_preview"].as_bool().unwrap_or(true),
            performance_overlay: value["performance_overlay"].as_bool().unwrap_or(false),
            left_panel: !current_layout || value["left_panel"].as_bool().unwrap_or(d.left_panel),
            animation_visible: value["animation_visible"].as_bool().unwrap_or(true),
            animation_layers_open: value["animation_layers_open"].as_bool().unwrap_or(false),
            tools_expanded: value["tools_expanded"].as_bool().unwrap_or(true),
            collapsed_sections: value["collapsed_sections"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_u64().and_then(|n| u16::try_from(n).ok()))
                .collect(),
            right_panel: current_layout && value["right_panel"].as_bool().unwrap_or(d.right_panel),
            grid: value["grid"].as_bool().unwrap_or(d.grid),
            viewport_shading: value["viewport_shading"].as_u64().filter(|v| *v <= 3).unwrap_or(3) as u8,
            show_backfaces: value["show_backfaces"].as_bool().unwrap_or(true),
            skeleton: value["skeleton"].as_bool().unwrap_or(d.skeleton),
            blender_path: value["blender_path"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .map(PathBuf::from),
            blend_import_animations: value["blend_import_animations"]
                .as_bool()
                .unwrap_or(d.blend_import_animations),
        }
    }
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "version": 1, "interface_version": INTERFACE_VERSION,
            "language": self.language.as_str(), "recent_projects": self.recent_projects,
            "theme": self.theme.as_str(), "accent": self.accent.as_str(),
            "left_width":self.left_width,"right_width":self.right_width,
            "creation_panel_position":self.creation_panel_position,
            "tools_expanded": self.tools_expanded,
            "animation_visible": self.animation_visible,
            "animation_layers_open": self.animation_layers_open,
            "collapsed_sections": self.collapsed_sections,
            "left_panel": self.left_panel, "right_panel": self.right_panel,
            "viewport_shading": self.viewport_shading,
            "show_backfaces": self.show_backfaces,
            "grid": self.grid, "skeleton": self.skeleton,
            "material_grid":self.material_grid,"material_thumbnail":self.material_thumbnail as u32,"material_favorites":self.material_favorites,"material_category":self.material_category,
            "adaptive_preview":self.adaptive_preview, "performance_overlay":self.performance_overlay,
            "blender_path": self.blender_path.as_ref().map(|p| p.to_string_lossy()),
            "blend_import_animations": self.blend_import_animations,
        })
    }
    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .map(|v| Self::from_json(&v))
            .unwrap_or_default()
    }
    pub fn remember_project(&mut self, path: &Path) {
        let path=if path.is_absolute() {path.to_path_buf()} else {
            std::env::current_dir().map(|base|base.join(path)).unwrap_or_else(|_|path.to_path_buf())
        };
        self.recent_projects.retain(|p|*p!=path);
        self.recent_projects.insert(0,path);
        self.recent_projects.truncate(6);
    }
    pub fn save(&self) -> Result<(), String> {
        let path = Self::path().ok_or("Не удалось определить папку настроек")?;
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let data = self.to_json();
        let tmp = path.with_extension(format!("{}.json.tmp", std::process::id()));
        std::fs::write(
            &tmp,
            serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }
}

#[derive(Clone, Debug)]
pub struct SceneRow {
    pub id: u64,
    pub depth: usize,
    pub has_children: bool,
    pub matches: bool,
}

/// Filtering retains ancestors for context and temporarily ignores collapsed branches.
/// Iterative traversal also handles deep imported rigs without recursion.
#[cfg(test)]
pub fn scene_rows(
    project: &Project,
    query: &str,
    joints_only: bool,
    collapsed: &BTreeSet<u64>,
) -> Vec<SceneRow> {
    scene_rows_filtered(project, query, joints_only, collapsed, 0)
}
pub fn scene_rows_filtered(
    project: &Project,
    query: &str,
    joints_only: bool,
    collapsed: &BTreeSet<u64>,
    hidden: u8,
) -> Vec<SceneRow> {
    let query = query.trim().to_lowercase();
    let filtered = !query.is_empty() || joints_only || hidden != 0;
    let by_id: BTreeMap<_, _> = project.nodes.iter().map(|n| (n.id, n)).collect();
    let mut children: BTreeMap<Option<u64>, Vec<u64>> = BTreeMap::new();
    let mut matches = BTreeSet::new();
    let mut included = BTreeSet::new();
    for n in &project.nodes {
        let parent = n.parent.filter(|p| by_id.contains_key(p));
        children.entry(parent).or_default().push(n.id);
        let name_matches = n.name.to_lowercase().contains(&query);
        let camera_matches = n.camera.is_some() && "камера camera".contains(&query);
        let kind = if n.camera.is_some() {
            8
        } else if n.is_joint {
            2
        } else {
            1
        };
        if hidden & kind == 0 && (!joints_only || n.is_joint) && (name_matches || camera_matches) {
            matches.insert(n.id);
            let mut cursor = Some(n.id);
            while let Some(id) = cursor {
                if !included.insert(id) {
                    break;
                }
                cursor = by_id.get(&id).and_then(|n| n.parent);
            }
        }
    }
    let order: BTreeMap<_, _> = project
        .outliner
        .order
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            if let poseforge_core::SceneItem::Node(id) = item {
                Some((*id, i))
            } else {
                None
            }
        })
        .collect();
    for siblings in children.values_mut() {
        siblings.sort_by_key(|id| order.get(id).copied().unwrap_or(usize::MAX));
    }
    let mut stack: Vec<_> = children
        .get(&None)
        .into_iter()
        .flatten()
        .rev()
        .map(|id| (*id, 0))
        .collect();
    let mut visited = BTreeSet::new();
    let mut rows = Vec::new();
    while let Some((id, depth)) = stack.pop() {
        if !visited.insert(id) || (filtered && !included.contains(&id)) {
            continue;
        }
        let descendants = children.get(&Some(id));
        rows.push(SceneRow {
            id,
            depth,
            has_children: descendants.is_some_and(|c| !c.is_empty()),
            matches: matches.contains(&id),
        });
        if filtered || !collapsed.contains(&id) {
            if let Some(c) = descendants {
                stack.extend(c.iter().rev().map(|id| (*id, depth + 1)));
            }
        }
    }
    rows
}

pub fn shortened(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    if max_chars == 0 {
        return String::new();
    }
    text.chars()
        .take(max_chars - 1)
        .chain(std::iter::once('…'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RecoveryTestDirectory(PathBuf);
    impl RecoveryTestDirectory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("poseforge-recovery-test-{}", new_recovery_id()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for RecoveryTestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn recovery_directory_for(windows: bool, values: &[(&str, &Path)]) -> Result<PathBuf, String> {
        recovery_directory_with(windows, |key| {
            values
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, path)| path.as_os_str().to_os_string())
        })
    }

    #[test]
    fn recovery_uses_user_state_directories_without_a_cwd_fallback() {
        let root = std::env::temp_dir().join("poseforge-user-state-test");
        let local = root.join("local-app-data");
        let state = root.join("xdg-state");
        let home = root.join("home");
        assert_eq!(
            recovery_directory_for(true, &[("LOCALAPPDATA", &local), ("HOME", &home)]).unwrap(),
            local.join("PoseForge/recovery")
        );
        assert_eq!(
            recovery_directory_for(false, &[("XDG_STATE_HOME", &state), ("HOME", &home)]).unwrap(),
            state.join("PoseForge/recovery")
        );
        assert_eq!(
            recovery_directory_for(false, &[("HOME", &home)]).unwrap(),
            home.join(".local/state/PoseForge/recovery")
        );
        assert_eq!(
            recovery_directory_for(false, &[("XDG_STATE_HOME", Path::new("")), ("HOME", &home)])
                .unwrap(),
            home.join(".local/state/PoseForge/recovery")
        );
        assert!(recovery_directory_for(true, &[]).is_err());
        assert!(recovery_directory_for(false, &[]).is_err());
    }

    #[test]
    fn recovery_override_must_be_absolute_and_cannot_traverse_parents() {
        let root = std::env::temp_dir().join("poseforge-user-state-test");
        let custom = root.join("custom");
        assert_eq!(
            recovery_directory_for(
                true,
                &[("POSEFORGE_RECOVERY_DIR", &custom), ("LOCALAPPDATA", &root)]
            )
            .unwrap(),
            custom
        );
        for invalid in [
            PathBuf::new(),
            PathBuf::from("recovery"),
            root.join("../outside"),
        ] {
            assert!(recovery_directory_for(
                false,
                &[("POSEFORGE_RECOVERY_DIR", &invalid), ("HOME", &root)]
            )
            .is_err());
        }
        assert!(recovery_directory_for(true, &[("LOCALAPPDATA", Path::new("local"))]).is_err());
        assert!(recovery_directory_for(
            false,
            &[("XDG_STATE_HOME", Path::new("state")), ("HOME", &root)]
        )
        .is_err());
    }

    #[test]
    fn recovery_names_are_stable_per_document_and_scoped_to_the_recovery_folder() {
        let case = RecoveryTestDirectory::new();
        let store = case.0.join("state/PoseForge/recovery");
        let first = case.0.join("source-one/scene.poseforge");
        let second = case.0.join("source-two/scene.poseforge");
        let a = prepare_recovery_path(&store, Some(&first), "session-one").unwrap();
        let again = prepare_recovery_path(&store, Some(&first), "session-two").unwrap();
        let b = prepare_recovery_path(&store, Some(&second), "session-one").unwrap();
        assert_eq!(a, again);
        assert_ne!(a, b);
        assert_eq!(a.parent(), Some(store.as_path()));
        assert_eq!(b.parent(), Some(store.as_path()));
        assert!(!a.starts_with(case.0.join("source-one")));
        assert!(!a.exists());
        assert!(store.is_dir());

        let alias = case.0.join("source-one/nested/../scene.poseforge");
        assert_eq!(
            prepare_recovery_path(&store, Some(&alias), "unused").unwrap(),
            a
        );
        let relative =
            prepare_recovery_path(&store, Some(Path::new("../scene.poseforge")), "unused").unwrap();
        assert_eq!(relative.parent(), Some(store.as_path()));
    }

    #[test]
    fn untitled_documents_have_distinct_stable_recovery_slots() {
        let case = RecoveryTestDirectory::new();
        let first = new_recovery_id();
        let second = new_recovery_id();
        assert_ne!(first, second);
        let a = prepare_recovery_path(&case.0, None, &first).unwrap();
        assert_eq!(prepare_recovery_path(&case.0, None, &first).unwrap(), a);
        assert_ne!(prepare_recovery_path(&case.0, None, &second).unwrap(), a);
        assert!(prepare_recovery_path(&case.0, None, "").is_err());
        assert_eq!(
            recovery_file_path(&case.0, None, "hello")
                .file_name()
                .unwrap(),
            "untitled-a430d84680aabd0b.autosave.poseforge"
        );
    }

    #[test]
    fn recovery_write_does_not_modify_the_saved_document_or_its_folder() {
        let case = RecoveryTestDirectory::new();
        let source = case.0.join("watched-source/scene.poseforge");
        let mut project = Project::demo();
        poseforge_io::save_project(&project, &source).unwrap();
        let original = std::fs::read(&source).unwrap();
        project.title = "Несохранённые изменения".into();
        let recovery = prepare_recovery_path(
            &case.0.join("local-data/PoseForge/recovery"),
            Some(&source),
            "unused",
        )
        .unwrap();
        poseforge_io::save_project(&project, &recovery).unwrap();
        assert_eq!(std::fs::read(&source).unwrap(), original);
        assert_eq!(
            std::fs::read_dir(source.parent().unwrap()).unwrap().count(),
            1
        );
        assert_eq!(poseforge_io::load_project(recovery).unwrap(), project);
    }

    #[test]
    fn recovery_directory_creation_failure_is_reported_without_a_fallback() {
        let case = RecoveryTestDirectory::new();
        let blocked = case.0.join("not-a-directory");
        std::fs::write(&blocked, b"preserve this file").unwrap();
        let result = prepare_recovery_path(&blocked.join("recovery"), None, "session");
        assert!(result.is_err());
        assert_eq!(std::fs::read(&blocked).unwrap(), b"preserve this file");
        assert_eq!(std::fs::read_dir(&case.0).unwrap().count(), 1);
    }

    #[test]
    fn search_reveals_collapsed_ancestors_and_preserves_project() {
        let p = Project::demo();
        let child = p
            .nodes
            .iter()
            .find(|n| n.is_joint && n.parent.is_some())
            .unwrap();
        let collapsed = p.nodes.iter().map(|n| n.id).collect();
        let rows = scene_rows(&p, &child.name, true, &collapsed);
        assert!(rows.iter().any(|r| r.id == child.id && r.matches));
        assert!(rows.iter().any(|r| Some(r.id) == child.parent));
        assert!(scene_rows(&p, "no-such-bone", false, &collapsed).is_empty());
    }
    #[test]
    fn collapsing_root_hides_descendants() {
        let p = Project::demo();
        let root = p
            .nodes
            .iter()
            .find(|n| n.parent.is_none() && p.nodes.iter().any(|c| c.parent == Some(n.id)))
            .unwrap();
        let rows = scene_rows(&p, "", false, &BTreeSet::from([root.id]));
        assert!(rows.iter().any(|r| r.id == root.id));
        assert!(!rows
            .iter()
            .any(|r| p.node(r.id).unwrap().parent == Some(root.id)));
    }
    #[test]
    fn invalid_preferences_fall_back_and_unicode_truncation_is_safe() {
        let p = Preferences::from_json(&serde_json::json!({"theme":"unknown", "grid":"bad"}));
        assert_eq!(p.theme, Theme::Dark);
        assert!(p.grid && p.skeleton);
        assert!(p.left_panel && !p.right_panel);
        assert!(!p.blend_import_animations);
        assert_eq!(shortened("Кисть.Л", 5), "Кист…");
    }
    #[test]
    fn legacy_workspace_migrates_panels_without_losing_other_preferences() {
        let old = serde_json::json!({
            "version": 1, "theme": "light", "left_panel": true,
            "right_panel": true, "grid": false, "skeleton": false,
            "blender_path": "C:/Tools/Blender/blender.exe"
        });
        let migrated = Preferences::from_json(&old);
        assert!(migrated.left_panel && !migrated.right_panel);
        assert_eq!(migrated.theme, Theme::Light);
        assert!(!migrated.grid && !migrated.skeleton);
        assert_eq!(
            migrated.blender_path,
            Some(PathBuf::from("C:/Tools/Blender/blender.exe"))
        );
        assert!(!migrated.blend_import_animations);
        assert_eq!(migrated.to_json()["interface_version"], INTERFACE_VERSION);
    }
    #[test]
    fn saved_new_layout_preserves_every_panel_choice() {
        for left_panel in [false, true] {
            for right_panel in [false, true] {
                let preferences = Preferences {
                    left_panel,
                    right_panel,
                    ..Preferences::default()
                };
                let restored = Preferences::from_json(&preferences.to_json());
                assert_eq!(restored.left_panel, left_panel);
                assert_eq!(restored.right_panel, right_panel);
            }
        }
    }
    #[test]
    fn blend_animation_preference_defaults_off_and_roundtrips_both_choices() {
        assert!(!Preferences::default().blend_import_animations);
        for value in [
            serde_json::json!({}),
            serde_json::json!({"blend_import_animations": "true"}),
        ] {
            assert!(!Preferences::from_json(&value).blend_import_animations);
        }
        for enabled in [false, true] {
            let preferences = Preferences {
                theme: Theme::Light,
                blender_path: Some(PathBuf::from("C:/Программы/Blender/blender.exe")),
                blend_import_animations: enabled,
                ..Preferences::default()
            };
            let bytes = serde_json::to_vec(&preferences.to_json()).unwrap();
            let restored = Preferences::from_json(&serde_json::from_slice(&bytes).unwrap());
            assert_eq!(restored.blend_import_animations, enabled);
            assert_eq!(restored.theme, preferences.theme);
            assert_eq!(restored.blender_path, preferences.blender_path);
        }
    }
    #[test]
    fn accent_and_theme_roundtrip_without_affecting_old_settings() {
        for theme in [Theme::Dark, Theme::Light] {
            for accent in Accent::ALL {
                let prefs = Preferences {
                    theme,
                    accent,
                    ..Preferences::default()
                };
                let restored = Preferences::from_json(&prefs.to_json());
                assert_eq!(restored.theme, theme);
                assert_eq!(restored.accent, accent);
                assert_eq!(restored.palette().slider, theme.palette().slider);
            }
        }
        for value in [
            serde_json::json!({}),
            serde_json::json!({"accent":"unknown"}),
            serde_json::json!({"accent": 3}),
        ] {
            assert_eq!(Preferences::from_json(&value).accent, Accent::Blue);
        }
    }
}

#[test]
fn collapsed_sections_roundtrip_and_old_preferences_keep_sections_open() {
    let mut preferences = Preferences::default();
    preferences.collapsed_sections.extend([100, 101, 305, 1001]);
    let saved = preferences.to_json();
    let restored = Preferences::from_json(&saved);
    assert_eq!(restored.collapsed_sections, preferences.collapsed_sections);
    assert!(Preferences::from_json(&serde_json::json!({"version":1}))
        .collapsed_sections
        .is_empty());
    let malformed =
        Preferences::from_json(&serde_json::json!({"collapsed_sections":[-1,"x",70000,101]}));
    assert_eq!(malformed.collapsed_sections, BTreeSet::from([101]));
}

#[cfg(test)]
mod recent_tests {
    use super::*;
    #[test]
    fn recent_projects_are_bounded_deduplicated_and_language_persists() {
        let mut p=Preferences::default();p.language=crate::localization::Language::English;
        for i in 0..10 {p.remember_project(Path::new(&format!("/projects/Лиса {i}.artforge")));}
        p.remember_project(Path::new("/projects/Лиса 5.artforge"));
        assert_eq!(p.recent_projects.len(),6);assert_eq!(p.recent_projects[0],PathBuf::from("/projects/Лиса 5.artforge"));
        let loaded=Preferences::from_json(&p.to_json());assert_eq!(loaded.recent_projects,p.recent_projects);assert_eq!(loaded.language,p.language);
        assert!(Preferences::from_json(&serde_json::json!({})).recent_projects.is_empty());
    }
}
