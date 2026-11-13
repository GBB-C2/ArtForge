"""PoseForge's private Blender-to-GLB bridge; run only through Blender --python.

Blender 3.6+ RNA is inspected to avoid forwarding options removed/added between
versions. The default profile keeps the model, textures and deforming skeleton
without source clips. Opt-in animation bakes the current scene range, not the
editable Blender control rig. No .blend save operation is used.

API references:
https://docs.blender.org/api/4.5/bpy.ops.export_scene.html
https://docs.blender.org/api/current/bpy.props.html#bpy.props.EnumProperty
https://github.com/KhronosGroup/glTF-Blender-IO/blob/main/addons/io_scene_gltf2/__init__.py
https://docs.blender.org/api/current/bpy.ops.wm.html
https://docs.blender.org/manual/en/latest/advanced/command_line/arguments.html
"""
import json
from pathlib import Path
import sys
from time import perf_counter


def export_options(properties, output, include_animations=False):
    """Build a conservative exporter profile using properties from its RNA."""
    wanted = {
        "filepath": str(output), "check_existing": False, "export_format": "GLB",
        "export_materials": "EXPORT", "export_image_format": "AUTO",
        "export_texcoords": True, "export_normals": True,
        "export_skins": True, "export_all_influences": True,
        "export_def_bones": True, "export_leaf_bone": False,
        "export_morph": True, "export_morph_normal": True,
        "export_apply": False, "export_animations": include_animations,
        "export_morph_animation": include_animations,
        "export_force_sampling": include_animations,
        "export_bake_animation": include_animations,
        "export_rest_position_armature": True,
        "export_hierarchy_flatten_bones": False, "export_hierarchy_flatten_objs": False,
        "export_armature_object_remove": False,
        "use_active_scene": True, "use_selection": False,
        "use_visible": False, "use_renderable": False,
        "export_yup": True, "export_cameras": True, "export_lights": True,
        "export_extras": False, "will_save_settings": False,
        "export_draco_mesh_compression_enable": False,
        "export_meshopt_compression_enable": False, "export_use_gltfpack": False,
        "export_use_ktx_compression": False, "export_image_add_compressed_images": False,
        "export_pointer_animation": False, "export_gpu_instances": False,
    }
    required = {
        "filepath", "export_format", "export_skins", "export_morph", "export_animations",
        "export_materials", "export_image_format", "export_texcoords", "export_normals",
        "export_all_influences", "export_def_bones", "export_apply", "export_rest_position_armature",
        "use_active_scene", "use_selection", "use_visible", "use_renderable",
    }
    if include_animations:
        wanted.update({
            "export_animation_mode": "SCENE", "export_frame_range": True,
            "export_frame_step": 1, "export_anim_scene_split_object": False,
            "export_negative_frame": "SLIDE", "export_anim_slide_to_zero": True,
            "export_reset_pose_bones": True,
            "export_sampling_interpolation_fallback": "LINEAR",
        })
        required.update({
            "export_morph_animation", "export_animation_mode", "export_force_sampling",
            "export_bake_animation", "export_frame_range", "export_frame_step",
            "export_anim_scene_split_object",
        })
    missing = sorted(required.difference(properties))
    if missing:
        raise RuntimeError("glTF exporter lacks required properties: " + ", ".join(missing))
    result = {}
    for name, value in wanted.items():
        prop = properties.get(name)
        if prop is None:
            continue
        if getattr(prop, "type", None) == "ENUM":
            identifiers = {item.identifier for item in prop.enum_items if item.identifier}
            # Callback-backed enums (including recent export_format) can expose
            # no items through operator RNA before an operator instance exists.
            # An empty list means "not known here", not "unsupported": pass the
            # explicit value and let Blender validate it when calling the exporter.
            # Never switch to another format/animation mode to hide a rejection.
            if identifiers and value not in identifiers:
                if name in required:
                    raise RuntimeError("glTF exporter does not support " + name + "=" + str(value))
                continue
        result[name] = value
    return result


def stage(name):
    """Small flushed protocol consumed by the parent process; not a percentage."""
    print("POSEFORGE_STAGE:" + name, flush=True)


def auto_rig_pro_warnings(scene, include_animations):
    """Inspect without executing the add-on or altering authored bones/weights.

    ARP export conventions: https://www.lucky3d.fr/auto-rig-pro/doc/ge_export_doc.html
    Bendy Bones / additive limits: https://www.lucky3d.fr/auto-rig-pro/doc/auto_rig.html
    """
    warnings = []
    for obj in scene.objects:
        if obj.type != "ARMATURE":
            continue
        bones = obj.data.bones
        names = {bone.name.rsplit(":", 1)[-1].lower() for bone in bones}
        if not {"root.x", "spine_01.x"}.issubset(names):
            continue
        if "c_traj" not in names and not any(name.startswith(("arm_stretch.", "thigh_stretch.")) for name in names):
            continue
        warnings.append(f"Auto-Rig Pro · {obj.name}: импортируется деформирующий скелет с весами. " +
                        ("Движение активной сцены запекается по кадрам; IK/FK-контроллеры и Python-драйверы не переносятся как инструменты ArtForge."
                         if include_animations else
                         "Чтобы перенести движение, включите импорт анимации либо экспортируйте GLB через Auto-Rig Pro → Export."))
        if any(getattr(bone, "use_deform", False) and getattr(bone, "bbone_segments", 1) > 1 for bone in bones):
            warnings.append(f"Auto-Rig Pro · {obj.name}: найдены Bendy Bones. Для совпадения деформаций подготовьте экспорт с режимом Secondary Controllers → Twist в Blender; сегменты B-Bone не становятся отдельными суставами glTF автоматически.")
        if any(getattr(mod, "object", None) == obj and getattr(mod, "use_deform_preserve_volume", False)
               for mesh in scene.objects if mesh.type == "MESH" for mod in mesh.modifiers if mod.type == "ARMATURE"):
            warnings.append(f"Auto-Rig Pro · {obj.name}: включён Preserve Volume. ArtForge использует линейный skinning, поэтому объём на сгибах может отличаться; исходная настройка Blender не изменена.")
    return warnings


def run(bpy, source, output, report_path, include_animations=False):
    started = perf_counter()
    if tuple(bpy.app.version) < (3, 6, 0):
        raise RuntimeError("PoseForge requires Blender 3.6 or newer")
    source, output, report_path = Path(source).resolve(), Path(output).resolve(), Path(report_path).resolve()
    if not source.is_file() or source.suffix.lower() != ".blend":
        raise RuntimeError("Input must be an existing .blend file")
    if source == output or source == report_path or output.suffix.lower() != ".glb":
        raise RuntimeError("The bridge cannot overwrite the source file")
    if output.exists() or report_path.exists():
        raise RuntimeError("Temporary output paths must be unused")
    bpy.context.preferences.filepaths.use_scripts_auto_execute = False
    stage("OPEN")
    open_started = perf_counter()
    result = bpy.ops.wm.open_mainfile(filepath=str(source), load_ui=False, use_scripts=False)
    if "FINISHED" not in result:
        raise RuntimeError("Blender could not open the source file")
    open_finished = perf_counter()
    stage("PREPARE")
    # Factory startup avoids user add-ons; enable only Blender's bundled exporter.
    try:
        properties = bpy.ops.export_scene.gltf.get_rna_type().properties
    except (RuntimeError, AttributeError):
        bpy.ops.preferences.addon_enable(module="io_scene_gltf2")
        properties = bpy.ops.export_scene.gltf.get_rna_type().properties
    properties = {prop.identifier: prop for prop in properties}
    options = export_options(properties, output, include_animations)
    scene = bpy.context.scene
    source_fps = scene.render.fps / scene.render.fps_base
    warnings = [
        ("Переносится активная сцена Blender, включая скрытые объекты, и её диапазон кадров. Неиспользуемые Actions и другие сцены не импортируются."
         if include_animations else
         "Переносится активная сцена Blender, включая скрытые объекты: модель, поддерживаемые материалы/текстуры, деформирующий скелет, веса и shape keys. Исходные анимационные клипы не импортируются."),
        ("Автовыполнение Python из .blend отключено. Риги с пользовательскими Python-драйверами могут требовать ручного запекания в Blender."
         if include_animations else
         "Автовыполнение Python из .blend отключено. Вычисляемая геометрия и исходная поза с Python-драйверами могут требовать подготовки в Blender."),
        "Материалы переносятся в пределах glTF/PoseForge. Процедурные узлы требуют запекания в изображения; area-свет, мир и эффекты Blender не переносятся. Point/spot/sun импортируются без анимации.",
    ]
    warnings.extend(auto_rig_pro_warnings(scene, include_animations))
    if any(any(mod.type != "ARMATURE" for mod in obj.modifiers) for obj in scene.objects if obj.type == "MESH"):
        warnings.append("Модификаторы мешей не применяются принудительно, чтобы сохранить shape keys; проверьте геометрию после импорта.")
    if bpy.app.autoexec_fail:
        warnings.append("Blender заблокировал скрипты/драйверы исходного файла; " +
                        ("часть позы или движения могла не вычислиться." if include_animations else
                         "часть вычисляемой геометрии или исходной позы могла не вычислиться."))
    prepare_finished = perf_counter()
    stage("EXPORT_ANIMATION" if include_animations else "EXPORT_MODEL")
    result = bpy.ops.export_scene.gltf(**options)
    export_finished = perf_counter()
    if "FINISHED" not in result or not output.is_file() or output.stat().st_size == 0:
        raise RuntimeError("Blender glTF export did not produce a complete GLB")
    report = {"blender_version": bpy.app.version_string, "fps": source_fps,
              "frame_start": scene.frame_start, "frame_end": scene.frame_end,
              "import_profile": "ANIMATION" if include_animations else "MODEL",
              "animations_exported": include_animations,
              "animation_mode": "SCENE" if include_animations else "NONE",
              "timings_seconds": {"open": open_finished - open_started,
                                  "prepare": prepare_finished - open_finished,
                                  "export": export_finished - prepare_finished,
                                  "total": perf_counter() - started},
              "warnings": warnings}
    with report_path.open("x", encoding="utf-8") as handle:
        json.dump(report, handle, ensure_ascii=False)
    stage("DONE")


def main():
    if "--" not in sys.argv:
        raise RuntimeError("Expected -- input.blend output.glb report.json [--animations]")
    args = sys.argv[sys.argv.index("--") + 1:]
    include_animations = len(args) == 4 and args[-1] == "--animations"
    if len(args) != 3 and not include_animations:
        raise RuntimeError("Expected three bridge paths and optional final --animations")
    import bpy
    run(bpy, *args[:3], include_animations=include_animations)


if __name__ == "__main__":
    main()
