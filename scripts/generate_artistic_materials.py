#!/usr/bin/env python3
"""Create the editable, texture-free artistic material gallery (no renderer required).

This generator creates scene data, not a preview or evidence of GPU execution.
Run from any directory: python scripts/generate_artistic_materials.py
"""
from copy import deepcopy
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PRESETS = [['PlasticineSoft', 'Пластилин · мягкий', 'Plasticine', [0.48, 0.065, 0.032, 1], [0, 0.52, 1.46, 0.08, 0.4], {'bump': 0.22, 'tool_marks': 0.18, 'subsurface': 0.2}], ['PlasticineSculpted', 'Пластилин · следы лепки', 'Plasticine', [0.035, 0.22, 0.36, 1], [0, 0.6, 1.46, 0.06, 0.45], {'bump': 0.65, 'tool_marks': 0.85, 'variation': 0.22, 'grain': 0.35}], ['ClayWet', 'Глина · влажная', 'Clay', [0.25, 0.12, 0.065, 1], [0, 0.34, 1.48, 0.35, 0.24], {'bump': 0.35, 'grain': 0.3, 'tool_marks': 0.6}], ['ClayDry', 'Глина · сухая', 'Clay', [0.52, 0.36, 0.23, 1], [0, 0.94, 1.45, 0, 0.1], {'bump': 0.7, 'grain': 0.85, 'cracks': 0.5}], ['Terracotta', 'Терракота', 'Clay', [0.46, 0.115, 0.04, 1], [0, 0.87, 1.5, 0, 0.1], {'bump': 0.5, 'grain': 0.8, 'cracks': 0.22, 'tool_marks': 0.15}], ['WatercolorLight', 'Акварель · лёгкая', 'Watercolor', [0.035, 0.23, 0.48, 1], [0, 0.96, 1.33, 0, 0.1], {'pigment': 0.52, 'wash': 0.7, 'grain': 0.25}], ['WatercolorGranulated', 'Акварель · грануляция', 'Watercolor', [0.24, 0.055, 0.35, 1], [0, 0.98, 1.33, 0, 0.1], {'pigment': 0.78, 'grain': 0.95, 'paper': 0.85, 'dry_brush': 0.25, 'edge_darkening': 0.7}], ['Gouache', 'Гуашь · матовая', 'Gouache', [0.5, 0.28, 0.035, 1], [0, 0.96, 1.4, 0, 0.1], {}], ['Wax', 'Воск · мягкий', 'Wax', [0.76, 0.51, 0.23, 1], [0, 0.4, 1.44, 0.1, 0.32], {}], ['CeramicGlazed', 'Керамика · глазурь', 'Ceramic', [0.035, 0.32, 0.22, 1], [0, 0.28, 1.5, 0.9, 0.1], {}], ['Paper', 'Бумага · волокна', 'Paper', [0.8, 0.74, 0.62, 1], [0, 0.98, 1.4, 0, 0.1], {}], ['Cardboard', 'Картон · крафт', 'Paper', [0.32, 0.19, 0.09, 1], [0, 1, 1.4, 0, 0.1], {'scale': 4, 'bump': 0.35, 'grain': 0.9, 'variation': 0.3, 'subsurface': 0}], ['Pastel', 'Пастель · сухая', 'Pastel', [0.44, 0.09, 0.16, 1], [0, 1, 1.35, 0, 0.1], {}]]


def settings(style, overrides):
    value = dict(enabled=True, style=style, scale=6.0, bump=0.3, grain=0.2,
        variation=0.12, tool_marks=0.3, cracks=0.0, subsurface=0.15,
        pigment=0.7, paper=0.35, edge_darkening=0.25, wash=0.3,
        dry_brush=0.0, seed=0)
    changes = {
        "Plasticine": {},
        "Clay": dict(bump=.55, grain=.65, variation=.2, tool_marks=.45, subsurface=.03),
        "Watercolor": dict(scale=3., bump=.035, grain=.55, variation=.35,
            tool_marks=0., subsurface=0., pigment=.6, paper=.65,
            edge_darkening=.55, wash=.6, dry_brush=.12),
        "Gouache": dict(scale=4., bump=.1, grain=.45, variation=.25,
            tool_marks=.5, subsurface=0., pigment=.95, paper=.35, wash=.12, dry_brush=.2),
        "Wax": dict(scale=3., bump=.12, grain=.06, variation=.08,
            tool_marks=.08, subsurface=.7),
        "Ceramic": dict(scale=5., bump=.14, grain=.15, variation=.16,
            tool_marks=.15, cracks=.28, subsurface=0.),
        "Paper": dict(scale=5., bump=.12, grain=.7, variation=.2,
            tool_marks=0., subsurface=.04, paper=.8),
        "Pastel": dict(scale=4., bump=.1, grain=.85, variation=.32,
            tool_marks=.35, subsurface=0., pigment=.8, paper=.75, wash=.15, dry_brush=.55),
    }
    value.update(changes[style])
    value.update(overrides)
    return value


def sphere(mesh_id, material_id, name):
    segments, rings = 32, 24
    vertices, indices = [], []
    for y in range(rings+1):
        theta = math.pi*y/rings
        for x in range(segments+1):
            phi = math.tau*x/segments
            normal = [math.sin(theta)*math.cos(phi), math.cos(theta), math.sin(theta)*math.sin(phi)]
            vertices.append(dict(position=[v*.9 for v in normal], normal=normal,
                uv=[x/segments, y/rings], joints=[0]*4, weights=[0]*4))
    for y in range(rings):
        for x in range(segments):
            a = y*(segments+1)+x
            b = a+segments+1
            if y > 0:
                indices.extend([a, a+1, b])
            if y < rings-1:
                indices.extend([a+1, b+1, b])
    return dict(id=mesh_id, name=name, vertices=vertices, indices=indices,
        material=material_id, morph_targets=[])


def build():
    scene = json.loads((ROOT/'examples/area-studio.artforge').read_text(encoding='utf-8'))
    scene.update(version=43, title="Художественные материалы · 13 образцов",
        textures=[], clips=[], poses=[], rig_profiles=[], active_camera=None)
    floor_node = deepcopy(scene['nodes'][0])
    floor_mesh = deepcopy(scene['meshes'][0])
    floor_material = deepcopy(scene['materials'][0])
    floor_material.update(base_color=[.23,.24,.25,1.], roughness=.85)
    scene.update(nodes=[floor_node], meshes=[floor_mesh], materials=[floor_material])
    for index, (_variant, label, style, color, pbr, overrides) in enumerate(PRESETS):
        mid, mesh_id, node_id = 201+index, 11+index, 1011+index
        material = deepcopy(floor_material)
        metallic, roughness, ior, coat, coat_roughness = pbr
        category = 'Лепка' if index < 5 else ('Живопись' if index in (5,6,7,12) else 'Ремесло')
        material.update(id=mid, name=label, base_color=color, metallic=metallic,
            roughness=roughness, ior=ior, coat_weight=coat, coat_roughness=coat_roughness,
            artistic=settings(style,overrides), category=category,
            tags=['художественный','процедурный'])
        node = deepcopy(floor_node)
        row, column = divmod(index,4)
        x = (column-1.5)*2.6 if row<3 else 0.
        node.update(id=node_id, name=label, mesh=mesh_id)
        node['transform']['translation'] = [x,.9,(row-1.5)*2.6]
        scene['materials'].append(material)
        scene['meshes'].append(sphere(mesh_id,mid,label))
        scene['nodes'].append(node)
    scene['camera'].update(eye=[9.,11.,15.], target=[0.,.6,0.], fov_y_radians=.67)
    for light, direction, intensity in zip(scene['lights'],
            [[.35,-.8,-.48],[-.6,-.5,.6],[0.,-.6,-.8]], [2.3,1.1,.35]):
        length = math.sqrt(sum(v*v for v in direction))
        light.update(kind='Directional', direction=[v/length for v in direction], intensity=intensity)
    scene['render_settings'].update(ambient_intensity=.5, output_width=1440, output_height=1080,
        sky_color=[.4,.43,.48], ground_color=[.16,.14,.12], background_color=[.045,.055,.07])
    return scene


if __name__ == '__main__':
    path = ROOT/'examples/artistic-materials.artforge'
    path.write_text(json.dumps(build(), ensure_ascii=False, separators=(',',':'))+'\n', encoding='utf-8')
    print(path)
