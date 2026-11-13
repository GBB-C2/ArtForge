"""Contract tests for the bridge without importing or pretending to run Blender."""
import importlib.util
import io
import json
import math
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "export_blend.py"
spec = importlib.util.spec_from_file_location("poseforge_export_blend", SCRIPT)
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)


def property_info(name, values=None):
    return SimpleNamespace(identifier=name, type="ENUM" if values is not None else "BOOLEAN",
                           enum_items=[SimpleNamespace(identifier=value) for value in (values or [])])


def properties():
    return {name: property_info(name) for name in [
        "filepath", "export_format", "export_skins", "export_morph", "export_animations",
        "export_animation_mode", "export_force_sampling", "use_active_scene", "export_apply",
        "export_all_influences", "export_def_bones", "use_selection", "export_frame_range",
        "export_frame_step", "export_bake_animation", "export_rest_position_armature",
        "export_morph_animation", "export_anim_scene_split_object", "use_visible", "use_renderable", "export_lights",
        "export_texcoords", "export_normals",
    ]} | {
        "export_format": property_info("export_format", ["GLB", "GLTF_SEPARATE"]),
        "export_animation_mode": property_info("export_animation_mode", ["ACTIONS", "SCENE"]),
        "export_materials": property_info("export_materials", ["EXPORT", "NONE"]),
        "export_image_format": property_info("export_image_format", ["AUTO", "JPEG", "NONE"]),
    }


class ExportOperator:
    def __init__(self, values, calls, *, result=None, error=None,
                 payload=b"fake-glb-for-adapter-test"):
        self.values = values
        self.calls = calls
        self.result = {"FINISHED"} if result is None else result
        self.error = error
        self.payload = payload

    def get_rna_type(self):
        return SimpleNamespace(properties=list(self.values.values()))

    def __call__(self, **kwargs):
        self.calls.append(("export", kwargs))
        if self.payload is not None:
            Path(kwargs["filepath"]).write_bytes(self.payload)
        if self.error is not None:
            raise self.error
        return self.result


def blender_stub(exporter, calls, *, version=(4, 5, 0), version_string="mock-4.5"):
    """Expose only the bridge's bpy contract; no Blender evaluation is simulated."""
    def open_file(**kwargs):
        calls.append(("open", kwargs))
        return {"FINISHED"}

    return SimpleNamespace(
        app=SimpleNamespace(version=version, version_string=version_string, autoexec_fail=True),
        context=SimpleNamespace(
            preferences=SimpleNamespace(filepaths=SimpleNamespace(use_scripts_auto_execute=True)),
            scene=SimpleNamespace(render=SimpleNamespace(fps=30, fps_base=1.001),
                                  frame_start=1, frame_end=24,
                                  objects=[SimpleNamespace(type="MESH", modifiers=[SimpleNamespace(type="SUBSURF")])])),
        ops=SimpleNamespace(wm=SimpleNamespace(open_mainfile=open_file),
                            export_scene=SimpleNamespace(gltf=exporter)),
    )


class BlenderContractTests(unittest.TestCase):
    def test_auto_rig_pro_reports_real_deformation_limits_without_mutating_the_rig(self):
        bones = [SimpleNamespace(name=n, use_deform=True, bbone_segments=1)
                 for n in ["root.x", "spine_01.x", "arm_stretch.l", "c_traj"]]
        bones[2].bbone_segments = 4
        rig = SimpleNamespace(type="ARMATURE", name="FoxRig", data=SimpleNamespace(bones=bones))
        modifier = SimpleNamespace(type="ARMATURE", object=rig, use_deform_preserve_volume=True)
        scene = SimpleNamespace(objects=[rig, SimpleNamespace(type="MESH", modifiers=[modifier])])
        warnings = bridge.auto_rig_pro_warnings(scene, True)
        self.assertEqual(len(warnings), 3)
        self.assertIn("по кадрам", warnings[0])
        self.assertIn("Bendy Bones", warnings[1])
        self.assertIn("Preserve Volume", warnings[2])
        self.assertEqual(bones[2].bbone_segments, 4)
        self.assertTrue(modifier.use_deform_preserve_volume)
        bones[0].name = "OtherRig"
        self.assertEqual(bridge.auto_rig_pro_warnings(scene, True), [])

    def setUp(self):
        self.output = io.StringIO()
        stdout = patch("sys.stdout", self.output)
        stdout.start()
        self.addCleanup(stdout.stop)

    def test_options_do_not_forward_unknown_version_specific_properties(self):
        props = properties()
        result = bridge.export_options(props, Path("model.glb"), include_animations=True)
        self.assertEqual(set(result).difference(props), set())
        self.assertEqual(result["export_animation_mode"], "SCENE")
        self.assertFalse(result["export_apply"])
        self.assertTrue(result["export_morph"])
        self.assertTrue(result["export_all_influences"])
        self.assertTrue(result["export_def_bones"])
        self.assertTrue(result["export_lights"])
        self.assertNotIn("export_use_ktx_compression", result)

    def test_missing_required_export_features_fail_instead_of_changing_semantics(self):
        for name in ["filepath", "export_format", "export_skins", "export_morph",
                     "export_animation_mode", "export_force_sampling"]:
            with self.subTest(property=name):
                props = properties()
                del props[name]
                with self.assertRaisesRegex(RuntimeError, name):
                    bridge.export_options(props, Path("model.glb"), include_animations=True)

    def test_scene_mode_and_glb_format_must_be_supported_enum_values(self):
        for name, values in [("export_animation_mode", ["ACTIONS"]), ("export_format", ["GLTF_SEPARATE"])]:
            with self.subTest(property=name):
                props = properties()
                props[name] = property_info(name, values)
                with self.assertRaisesRegex(RuntimeError, name):
                    bridge.export_options(props, Path("model.glb"), include_animations=True)

    def test_empty_dynamic_enums_keep_required_values_for_operator_validation(self):
        for values in [[], [""]]:  # A separator/header is not an allowed value.
            for dynamic in [("export_format",), ("export_animation_mode",),
                            ("export_format", "export_animation_mode")]:
                with self.subTest(dynamic=dynamic, enum_items=values):
                    props = properties()
                    for name in dynamic:
                        props[name] = property_info(name, values)
                    result = bridge.export_options(props, Path("model.glb"), include_animations=True)
                    self.assertEqual(result["export_format"], "GLB")
                    self.assertEqual(result["export_animation_mode"], "SCENE")
                    self.assertTrue(result["export_skins"])
                    self.assertTrue(result["export_morph"])
                    self.assertTrue(result["export_force_sampling"])
                    self.assertFalse(result["export_apply"])

    def test_empty_optional_enum_is_not_treated_as_a_known_static_mismatch(self):
        props = properties()
        props["export_sampling_interpolation_fallback"] = property_info("export_sampling_interpolation_fallback", [])
        result = bridge.export_options(props, Path("model.glb"), include_animations=True)
        self.assertEqual(result["export_sampling_interpolation_fallback"], "LINEAR")

        props["export_sampling_interpolation_fallback"] = property_info("export_sampling_interpolation_fallback", ["STEP"])
        result = bridge.export_options(props, Path("model.glb"), include_animations=True)
        self.assertNotIn("export_sampling_interpolation_fallback", result)
        self.assertEqual(result["export_format"], "GLB")

    def test_string_metadata_keeps_requested_format_and_animation_mode(self):
        props = properties()
        for name in ["export_format", "export_animation_mode"]:
            # Some API descriptions expose a string without a static enum list.
            props[name] = SimpleNamespace(identifier=name, type="STRING")
        result = bridge.export_options(props, Path("model.glb"), include_animations=True)
        self.assertEqual(result["export_format"], "GLB")
        self.assertEqual(result["export_animation_mode"], "SCENE")

    def test_open_disables_source_scripts_and_keeps_original_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            source, output, report = base / "персонаж.blend", base / "out.glb", base / "report.json"
            original = b"original test document; never a Blender-generated fixture"
            source.write_bytes(original)
            calls = []
            bpy = blender_stub(ExportOperator(properties(), calls), calls)
            bridge.run(bpy, source, output, report, include_animations=True)
            self.assertEqual(calls[0][0], "open")
            self.assertIs(calls[0][1]["use_scripts"], False)
            self.assertIs(calls[0][1]["load_ui"], False)
            self.assertFalse(bpy.context.preferences.filepaths.use_scripts_auto_execute)
            self.assertEqual(source.read_bytes(), original)
            data = json.loads(report.read_text(encoding="utf-8"))
            self.assertAlmostEqual(data["fps"], 30 / 1.001)
            self.assertEqual(data["animation_mode"], "SCENE")
            self.assertTrue(any("заблокировал" in warning for warning in data["warnings"]))
            self.assertTrue(any("Модификаторы" in warning for warning in data["warnings"]))

    def test_run_passes_dynamic_values_to_exporter_once_and_writes_report_on_success(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            source = base / "персонаж с пробелами.blend"
            output, report = base / "out.glb", base / "report.json"
            original = b"unchanged mock .blend; not a Blender fixture"
            source.write_bytes(original)
            props = properties()
            props["export_format"] = property_info("export_format", [])
            props["export_animation_mode"] = property_info("export_animation_mode", [])
            calls = []
            bpy = blender_stub(ExportOperator(props, calls), calls,
                               version=(5, 3, 0), version_string="mock-5.3-alpha")

            bridge.run(bpy, source, output, report, include_animations=True)

            self.assertEqual([kind for kind, _ in calls], ["open", "export"])
            self.assertEqual(calls[1][1]["export_format"], "GLB")
            self.assertEqual(calls[1][1]["export_animation_mode"], "SCENE")
            self.assertEqual(Path(calls[1][1]["filepath"]), output.resolve())
            self.assertFalse(calls[1][1]["export_apply"])
            self.assertTrue(calls[1][1]["export_skins"])
            self.assertTrue(calls[1][1]["export_morph"])
            self.assertEqual(source.read_bytes(), original)
            self.assertGreater(output.stat().st_size, 0)
            data = json.loads(report.read_text(encoding="utf-8"))
            self.assertEqual(data["animation_mode"], "SCENE")
            self.assertEqual(data["import_profile"], "ANIMATION")
            self.assertIs(data["animations_exported"], True)
            self.assertEqual(data["blender_version"], "mock-5.3-alpha")
            self.assertAlmostEqual(data["fps"], 30 / 1.001)
            self.assertEqual(self.output.getvalue().splitlines(), [
                "POSEFORGE_STAGE:OPEN", "POSEFORGE_STAGE:PREPARE",
                "POSEFORGE_STAGE:EXPORT_ANIMATION", "POSEFORGE_STAGE:DONE",
            ])

    def test_run_rejects_static_mismatch_and_missing_required_property_before_export(self):
        for problem in ["unsupported_format", "missing_format"]:
            with self.subTest(problem=problem), tempfile.TemporaryDirectory() as directory:
                base = Path(directory)
                source, output, report = base / "source.blend", base / "out.glb", base / "report.json"
                source.write_bytes(b"original")
                props = properties()
                if problem == "unsupported_format":
                    props["export_format"] = property_info("export_format", ["GLTF_SEPARATE"])
                else:
                    del props["export_format"]
                calls = []
                bpy = blender_stub(ExportOperator(props, calls), calls)

                with self.assertRaisesRegex(RuntimeError, "export_format"):
                    bridge.run(bpy, source, output, report, include_animations=True)

                self.assertEqual([kind for kind, _ in calls], ["open"])
                self.assertFalse(output.exists())
                self.assertFalse(report.exists())
                self.assertEqual(source.read_bytes(), b"original")

    def test_operator_rejection_of_dynamic_value_is_not_retried_or_reported_as_success(self):
        for error_type in [TypeError, RuntimeError]:
            with self.subTest(error=error_type.__name__), tempfile.TemporaryDirectory() as directory:
                base = Path(directory)
                source, output, report = base / "source.blend", base / "out.glb", base / "report.json"
                source.write_bytes(b"original")
                props = properties()
                props["export_format"] = property_info("export_format", [])
                calls = []
                exporter = ExportOperator(props, calls, error=error_type("mock exporter rejected GLB"),
                                          payload=b"partial output must not imply success")
                bpy = blender_stub(exporter, calls)

                with self.assertRaisesRegex((TypeError, RuntimeError), "rejected GLB"):
                    bridge.run(bpy, source, output, report, include_animations=True)

                self.assertEqual([kind for kind, _ in calls], ["open", "export"])
                self.assertEqual(calls[1][1]["export_format"], "GLB")
                self.assertEqual(calls[1][1]["export_animation_mode"], "SCENE")
                self.assertFalse(report.exists())
                self.assertEqual(source.read_bytes(), b"original")

    def test_cancelled_export_never_writes_success_report_even_with_nonempty_output(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            source, output, report = base / "source.blend", base / "out.glb", base / "report.json"
            source.write_bytes(b"original")
            props = properties()
            props["export_format"] = property_info("export_format", [])
            calls = []
            bpy = blender_stub(ExportOperator(props, calls, result={"CANCELLED"}), calls)

            with self.assertRaisesRegex(RuntimeError, "complete GLB"):
                bridge.run(bpy, source, output, report, include_animations=True)

            self.assertEqual([kind for kind, _ in calls], ["open", "export"])
            self.assertGreater(output.stat().st_size, 0)
            self.assertFalse(report.exists())
            self.assertEqual(source.read_bytes(), b"original")

    def test_finished_export_without_nonempty_glb_is_not_reported_as_success(self):
        for payload in [None, b""]:
            with self.subTest(payload=payload), tempfile.TemporaryDirectory() as directory:
                base = Path(directory)
                source, output, report = base / "source.blend", base / "out.glb", base / "report.json"
                source.write_bytes(b"original")
                calls = []
                bpy = blender_stub(ExportOperator(properties(), calls, payload=payload), calls)

                with self.assertRaisesRegex(RuntimeError, "complete GLB"):
                    bridge.run(bpy, source, output, report, include_animations=True)

                self.assertEqual([kind for kind, _ in calls], ["open", "export"])
                self.assertFalse(report.exists())
                self.assertEqual(source.read_bytes(), b"original")

    def test_old_blender_version_is_rejected_before_touching_files(self):
        bpy = SimpleNamespace(app=SimpleNamespace(version=(3, 5, 9)))
        with self.assertRaisesRegex(RuntimeError, "3.6"):
            bridge.run(bpy, "missing.blend", "out.glb", "report.json")

    def test_existing_outputs_are_never_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            source, output = base / "source.blend", base / "out.glb"
            source.write_bytes(b"source")
            output.write_bytes(b"keep")
            bpy = SimpleNamespace(app=SimpleNamespace(version=(3, 6, 0)))
            with self.assertRaisesRegex(RuntimeError, "unused"):
                bridge.run(bpy, source, output, base / "report.json")
            self.assertEqual(source.read_bytes(), b"source")
            self.assertEqual(output.read_bytes(), b"keep")

    def test_default_model_profile_keeps_textures_skin_and_shape_geometry_without_clips(self):
        result = bridge.export_options(properties(), Path("model.glb"))
        self.assertEqual(result["export_format"], "GLB")
        self.assertEqual(result["export_materials"], "EXPORT")
        self.assertEqual(result["export_image_format"], "AUTO")
        for name in ["export_texcoords", "export_normals", "export_skins", "export_all_influences",
                     "export_def_bones", "export_morph", "export_rest_position_armature"]:
            self.assertIs(result[name], True, name)
        for name in ["export_animations", "export_morph_animation", "export_force_sampling",
                     "export_bake_animation", "export_apply"]:
            self.assertIs(result[name], False, name)
        for name in ["export_animation_mode", "export_frame_range", "export_frame_step",
                     "export_anim_scene_split_object", "export_negative_frame", "export_anim_slide_to_zero"]:
            self.assertNotIn(name, result)

    def test_model_profile_accepts_exporter_without_animation_only_capabilities(self):
        props = properties()
        animation_only = ["export_morph_animation", "export_animation_mode", "export_force_sampling",
                          "export_bake_animation", "export_frame_range", "export_frame_step",
                          "export_anim_scene_split_object"]
        for name in animation_only:
            del props[name]
        result = bridge.export_options(props, Path("model.glb"))
        self.assertFalse(result["export_animations"])
        self.assertTrue(result["export_skins"])
        self.assertEqual(result["export_materials"], "EXPORT")
        self.assertFalse(set(animation_only).intersection(result))
        with self.assertRaisesRegex(RuntimeError, "required properties"):
            bridge.export_options(props, Path("model.glb"), include_animations=True)

    def test_model_requires_explicit_animation_disable_and_texture_skin_capabilities(self):
        for name in ["export_animations", "export_materials", "export_image_format", "export_texcoords",
                     "export_normals", "export_skins", "export_rest_position_armature"]:
            with self.subTest(property=name):
                props = properties()
                del props[name]
                with self.assertRaisesRegex(RuntimeError, name):
                    bridge.export_options(props, Path("model.glb"))
        for name in ["export_materials", "export_image_format"]:
            with self.subTest(unsupported=name):
                props = properties()
                props[name] = property_info(name, ["NONE"])
                with self.assertRaisesRegex(RuntimeError, name):
                    bridge.export_options(props, Path("model.glb"))

    def test_default_run_reports_model_profile_ordered_stages_and_phase_durations(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            source, output, report = base / "модель.blend", base / "out.glb", base / "report.json"
            source.write_bytes(b"unchanged contract fixture; not a real Blender file")
            calls = []
            props = properties()
            props["export_format"] = property_info("export_format", [])
            bpy = blender_stub(ExportOperator(props, calls), calls)
            bpy.context.scene.frame_current = 17
            with patch.object(bridge, "perf_counter", side_effect=[0.0, 0.1, 0.5, 0.7, 1.5, 1.6]):
                bridge.run(bpy, source, output, report)
            self.assertEqual([kind for kind, _ in calls], ["open", "export"])
            self.assertIs(calls[1][1]["export_animations"], False)
            self.assertIs(calls[1][1]["export_skins"], True)
            self.assertIs(calls[1][1]["export_texcoords"], True)
            self.assertEqual(calls[1][1]["export_image_format"], "AUTO")
            self.assertNotIn("export_animation_mode", calls[1][1])
            self.assertEqual(bpy.context.scene.frame_current, 17)
            self.assertEqual(source.read_bytes(), b"unchanged contract fixture; not a real Blender file")
            data = json.loads(report.read_text(encoding="utf-8"))
            self.assertEqual(data["import_profile"], "MODEL")
            self.assertIs(data["animations_exported"], False)
            self.assertEqual(data["animation_mode"], "NONE")
            self.assertAlmostEqual(data["fps"], 30 / 1.001)
            self.assertEqual((data["frame_start"], data["frame_end"]), (1, 24))
            self.assertTrue(any("клипы не импортируются" in warning for warning in data["warnings"]))
            for name, expected in {"open": 0.4, "prepare": 0.2, "export": 0.8, "total": 1.6}.items():
                value = data["timings_seconds"][name]
                self.assertTrue(math.isfinite(value) and value >= 0)
                self.assertAlmostEqual(value, expected)
            self.assertEqual(self.output.getvalue().splitlines(), [
                "POSEFORGE_STAGE:OPEN", "POSEFORGE_STAGE:PREPARE",
                "POSEFORGE_STAGE:EXPORT_MODEL", "POSEFORGE_STAGE:DONE",
            ])

    def test_failed_default_export_has_no_done_stage_or_success_report(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            source, output, report = base / "source.blend", base / "out.glb", base / "report.json"
            source.write_bytes(b"original")
            calls = []
            bpy = blender_stub(ExportOperator(properties(), calls, result={"CANCELLED"}), calls)
            with self.assertRaisesRegex(RuntimeError, "complete GLB"):
                bridge.run(bpy, source, output, report)
            self.assertFalse(report.exists())
            self.assertEqual(self.output.getvalue().splitlines(), [
                "POSEFORGE_STAGE:OPEN", "POSEFORGE_STAGE:PREPARE", "POSEFORGE_STAGE:EXPORT_MODEL",
            ])
            self.assertEqual(source.read_bytes(), b"original")

    def test_main_defaults_to_model_and_accepts_explicit_final_animation_flag(self):
        bpy = object()
        paths = ["input.blend", "out.glb", "report.json"]
        for include_animations in [False, True]:
            arguments = paths + (["--animations"] if include_animations else [])
            with self.subTest(animations=include_animations), patch.dict("sys.modules", {"bpy": bpy}), \
                    patch.object(bridge.sys, "argv", ["blender", "--", *arguments]), \
                    patch.object(bridge, "run") as run:
                bridge.main()
                run.assert_called_once_with(bpy, *paths, include_animations=include_animations)

    def test_main_rejects_missing_paths_and_unknown_or_duplicate_extras(self):
        paths = ["input.blend", "out.glb", "report.json"]
        invalid = [["blender"], ["blender", "--"], ["blender", "--", *paths[:2]],
                   ["blender", "--", *paths, "--unknown"],
                   ["blender", "--", *paths, "--animations", "--animations"],
                   ["blender", "--", "--animations", *paths]]
        for arguments in invalid:
            with self.subTest(arguments=arguments), patch.object(bridge.sys, "argv", arguments), \
                    patch.object(bridge, "run") as run:
                with self.assertRaisesRegex(RuntimeError, "Expected"):
                    bridge.main()
                run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
