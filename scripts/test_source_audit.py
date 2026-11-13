"""Regression tests for the source-only auditor, without a Rust toolchain."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import zipfile


AUDITOR = Path(__file__).with_name("source_audit.py")


class SourceAuditTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir=AUDITOR.parent)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.write("Cargo.toml", '[workspace]\nmembers = ["crates/app"]\n[workspace.package]\nversion = "0.3.0"\n')
        self.write("Cargo.lock", 'version = 4\n[[package]]\nname = "test-app"\nversion = "0.3.0"\n')
        self.write("crates/app/Cargo.toml", '[package]\nname = "test-app"\nversion.workspace = true\n')
        self.write("crates/app/src/main.rs", 'mod ui;\nconst RESOURCE: &str = include_str!("../asset.txt");\nfn main() {}\n')
        self.write("crates/app/src/ui.rs", 'pub enum Action {\n    Cancel,\n}\n')
        self.write("crates/app/asset.txt", "Bundled resource\n")
        self.write("examples/minimal.poseforge", '{}\n')

    def write(self, name, content):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    def audit(self, archive=None):
        command = [sys.executable, str(AUDITOR), str(self.root)]
        if archive is not None:
            command.append(str(archive))
        completed = subprocess.run(command, capture_output=True, text=True, check=False)
        self.assertIn(completed.returncode, (0, 1), completed.stderr)
        report = json.loads(completed.stdout)
        self.assertEqual(completed.returncode, bool(report["errors"]))
        return report

    def test_inherited_workspace_version_and_resource_are_checked(self):
        report = self.audit()
        self.assertEqual(report["errors"], [])
        self.assertEqual(report["workspace_packages"], [{"name": "test-app", "version": "0.3.0"}])
        self.assertEqual(report["compile_time_resources_checked"], ["crates/app/asset.txt"])
        self.assertEqual(report["source_modules_checked"], 1)

    def test_serde_usage_requires_manifest_and_lock_dependencies(self):
        self.write("crates/app/src/main.rs", "fn hash<T: serde::Serialize>() {}\nfn main() {}\n")
        self.assertTrue(any("direct serde dependency" in error for error in self.audit()["errors"]))
        self.write("crates/app/Cargo.toml", '[package]\nname = "test-app"\nversion.workspace = true\n[dependencies]\nserde = "1"\n')
        self.assertTrue(any("Cargo.lock" in error for error in self.audit()["errors"]))
        self.write("Cargo.lock", 'version = 4\n[[package]]\nname = "test-app"\nversion = "0.3.0"\ndependencies = ["serde"]\n')
        self.assertEqual(self.audit()["errors"], [])

    def test_empty_controls_parent_glob_does_not_supply_vec2_or_gizmo(self):
        body = "use super::*;\nfn overlay() { let p = Vec2::ZERO; gizmo::axis_color(); }\n"
        self.write("crates/app/src/empty_controls.rs", body)
        errors = self.audit()["errors"]
        self.assertTrue(any("glam::Vec2" in error for error in errors))
        self.assertTrue(any("crate::gizmo" in error for error in errors))
        self.write("crates/app/src/empty_controls.rs", "use glam::Vec2;\nuse crate::gizmo;\n" + body)
        self.assertEqual(self.audit()["errors"], [])

    def test_stale_lock_version_is_rejected(self):
        self.write("Cargo.lock", 'version = 4\n[[package]]\nname = "test-app"\nversion = "0.2.0"\n')
        self.assertTrue(any("does not match" in error for error in self.audit()["errors"]))

    def test_reserved_wgsl_name_is_reported_with_shader_and_line(self):
        self.write('crates/render/src/fx.wgsl', '// FX\nfn fx_coordinate() {\n let meta = vec4(1.);\n}\n')
        report = self.audit()
        self.assertEqual(report['wgsl_files_scanned'], 1)
        self.assertTrue(any("fx.wgsl:3: reserved WGSL word 'meta'" in e for e in report['errors']))

    def test_wgsl_comments_are_ignored_and_longer_identifiers_are_valid(self):
        self.write('crates/render/src/fx.wgsl', '''// meta target shared
/* outer meta /* nested module */ target */
struct Data { guide_meta: vec4<f32>, };
fn fx_coordinate() { let guide_info = vec4(1.); let metadata = guide_info; }
''')
        self.assertEqual(self.audit()['errors'], [])

    def test_wgsl_reserved_parameters_and_members_are_rejected(self):
        for name in ['meta', 'target', 'shared']:
            with self.subTest(name=name):
                self.write('crates/render/src/fx.wgsl', f'fn example({name}: f32) {{}}\n')
                self.assertTrue(any(f"reserved WGSL word {name!r}" in e for e in self.audit()['errors']))
                self.write('crates/render/src/fx.wgsl', f'struct Data {{ {name}: f32, }}\n')
                self.assertTrue(any(f"reserved WGSL word {name!r}" in e for e in self.audit()['errors']))

    def test_missing_literal_resource_is_rejected(self):
        (self.root / "crates/app/asset.txt").unlink()
        self.assertTrue(any("missing compile-time resource" in error for error in self.audit()["errors"]))

    def test_missing_external_module_is_rejected(self):
        self.write("crates/app/src/main.rs", "mod missing;\nfn main() {}\n")
        self.assertTrue(any("source module 'missing' is missing" in error for error in self.audit()["errors"]))

    def test_literal_offsets_and_error_line_after_comments(self):
        self.write("crates/app/src/main.rs", '''fn main() {
    let text = r###"unmatched } ] and "##"###;
    let byte = b'}'; let character = '\\u{29}';
    /* outer [ /* nested } */ ] */
}
''')
        self.assertEqual(self.audit()["errors"], [])
        self.write("crates/app/src/main.rs", '// heading\nfn main() {\n    ]\n}\n')
        self.assertTrue(any(':3: unexpected ]' in error for error in self.audit()["errors"]))

    def test_bare_ui_path_requires_the_module_not_only_its_types(self):
        self.write("crates/app/src/controls.rs", "use crate::ui::{Action, Ui};\nfn draw() { ui::MediaAction::Slot(0); }\n")
        errors = self.audit()["errors"]
        self.assertTrue(any("controls.rs:2: ui:: is used without importing" in error for error in errors))

    def test_direct_nested_alias_and_parent_imports_bind_ui(self):
        for declaration in ["use crate::ui;", "use crate::ui::{self, Action};",
                            "use crate::{ui::{self, Action}};", "use crate::{ui, icons::Icon};",
                            "use crate::ui as ui;", "use super::*;"]:
            with self.subTest(declaration=declaration):
                self.write("crates/app/src/controls.rs", declaration + "\nfn draw() { ui::contains(); }\n")
                self.assertEqual(self.audit()["errors"], [])

    def test_helper_icon_requires_its_sibling_module_import(self):
        body = "fn draw() { empty_controls::field_icon(); }\n"
        self.write("crates/app/src/scene_tree.rs", "use super::{App, SceneRow};\n" + body)
        self.assertTrue(any("empty_controls:: is used without importing" in error
                            for error in self.audit()["errors"]))
        for declaration in ["use super::{empty_controls, App, SceneRow};",
                            "use crate::app::empty_controls;", "use super::*;"]:
            with self.subTest(declaration=declaration):
                self.write("crates/app/src/scene_tree.rs", declaration + "\n" + body)
                self.assertEqual(self.audit()["errors"], [])
        self.write("crates/app/src/scene_tree.rs", "fn draw() { super::empty_controls::field_icon(); }\n")
        self.assertEqual(self.audit()["errors"], [])

    def test_specialized_dropdown_tooltip_must_precede_fallback(self):
        template = '''pub enum Action {
    DropdownOpen(DropdownKind),
}
pub enum DropdownKind { EmptyCreate }
impl Action {
    pub fn hint(&self) -> Option<&'static str> {
        match self {
ARMS
        }
    }
}
'''
        generic = 'Self::DropdownOpen(_) => Some("generic"),'
        specific = 'Self::DropdownOpen(DropdownKind::EmptyCreate) => Some("shapes"),'
        self.write("crates/app/src/ui.rs", template.replace("ARMS", generic + "\n" + specific))
        self.assertTrue(any("unreachable dropdown tooltip" in error for error in self.audit()["errors"]))
        self.write("crates/app/src/ui.rs", template.replace("ARMS", specific + "\n" + generic +
                   '\n// Self::DropdownOpen(DropdownKind::EmptyCreate) => Some("comment"),'))
        self.assertEqual(self.audit()["errors"], [])

    def test_qualified_paths_comments_and_literals_do_not_require_an_import(self):
        self.write("crates/app/src/controls.rs", '''use crate::{ui::Action};
fn draw() { crate::ui::contains(); }
// ui::Comment
/* outer ui::Comment /* nested ui::Comment */ */
const TEXT: &str = "ui::Text";
const RAW: &str = r##"ui::Raw"##;
''')
        self.assertEqual(self.audit()["errors"], [])

    def test_source_archive_must_ship_resources_and_use_safe_paths(self):
        archive = self.root / "package.zip"
        with zipfile.ZipFile(archive, "w") as bundle:
            bundle.write(self.root / "Cargo.toml", "PoseForge/Cargo.toml")
            bundle.writestr("../outside.txt", "invalid archive path")
        errors = self.audit(archive)["errors"]
        self.assertTrue(any("omits compile-time resource" in error for error in errors))
        self.assertTrue(any("unsafe archive name" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
