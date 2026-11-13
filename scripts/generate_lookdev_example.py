"""Build the editable 0.12 studio from the shipped UV sphere scene; no downloads."""
from pathlib import Path
import json


def main():
    root = Path(__file__).resolve().parents[1]
    project = json.loads((root / "examples/material-fibers.artforge").read_text())
    project["version"] = 8
    project["title"] = "Студия 0.12 · материалы, температура и контактный свет"
    settings = project["render_settings"]
    settings.update(ambient_occlusion=0.7, ambient_occlusion_radius=0.35,
                    ambient_intensity=0.9, output_width=1200, output_height=800,
                    background_color=[0.055, 0.065, 0.08], background_mode="Solid")
    materials = project["materials"]
    for material in materials:
        material.update(uv_scale=[1.0, 1.0], uv_offset=[0.0, 0.0], uv_rotation=0.0)
    materials[0].update(base_color=[0.27, 0.3, 0.34, 1.0], roughness=0.8)
    materials[1].update(name="Золото · направленный блик", base_color=[0.78, 0.48, 0.17, 1.0])
    materials[2].update(name="Полосы · масштаб и поворот UV", metallic=0.0, roughness=0.28,
                        coat_weight=0.65, anisotropy=0.0, base_color=[0.9, 0.95, 1.0, 1.0],
                        base_color_texture=9800, uv_scale=[4.0, 2.0], uv_rotation=25.0)
    materials[3].update(name="Стекло · матовое", metallic=0.0, roughness=0.22, transmission=1.0,
                        ior=1.5, thickness=0.8, attenuation_distance=2.0,
                        base_color=[0.75, 0.9, 0.95, 1.0], double_sided=False)
    materials[6].update(name="Керамика · та же карта, другие UV", base_color_texture=9800,
                        base_color=[0.95, 0.85, 0.65, 1.0], roughness=0.3, coat_weight=0.8,
                        uv_scale=[1.0, 6.0], uv_offset=[0.2, 0.0], uv_rotation=-25.0)
    rgba = []
    for y in range(64):
        for x in range(64):
            color = [235, 226, 203] if x < 40 else [34, 92, 113]
            rgba.extend([*color, 255])
    project["textures"].append(dict(id=9800, name="Две полосы · повторяемая карта", width=64, height=64, rgba8=rgba))
    for node in project["nodes"]:
        if 1011 <= node["id"] <= 1016:
            node["name"] = materials[node["id"] - 1010]["name"]
            mesh = next(mesh for mesh in project["meshes"] if mesh["id"] == node["mesh"])
            mesh["name"] = node["name"]
    for index, light in enumerate(project["lights"]):
        light.update(use_temperature=True, temperature_kelvin=[3400.0, 9000.0, 6500.0][index],
                     color=[1.0, 1.0, 1.0], area_rotation=[-25.0, 35.0, 0.0][index])
    project["lights"][0].update(name="Ключевой · 3400 K · наклонный софтбокс", kind="Area", area_size=[3.0, 0.8], intensity=14.0)
    project["lights"][1].update(name="Контровой · 9000 K", area_size=[1.0, 3.0], intensity=8.0)
    project["lights"][2]["name"] = "Заполнение · 6500 K"
    path = root / "examples/lookdev-studio.artforge"
    path.write_text(json.dumps(project, ensure_ascii=False, separators=(",", ":")) + "\n")
    print(path)


if __name__ == "__main__":
    main()
