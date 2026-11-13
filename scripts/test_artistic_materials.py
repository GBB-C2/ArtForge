"""Static source/data regression checks. These do NOT compile Rust or WGSL."""
import json
import math
from pathlib import Path
import re
import unittest

from generate_artistic_materials import build, PRESETS

ROOT = Path(__file__).resolve().parents[1]


def source(path):
    return (ROOT/path).read_text(encoding='utf-8')


def uncomment(text):
    return re.sub(r'//[^\n]*|/\*.*?\*/', '', text, flags=re.S)


def split_fields(text):
    """Split comma-separated type/field declarations outside generic brackets."""
    depth, start, result = 0, 0, []
    for index, char in enumerate(text):
        depth += (char == '<') - (char == '>')
        if char == ',' and depth == 0:
            if text[start:index].strip():
                result.append(text[start:index].strip())
            start = index+1
    if text[start:].strip():
        result.append(text[start:].strip())
    return result


def wgsl_layout(name, definitions):
    """Small independent layout calculation for scalar/vector/matrix/array structs."""
    name = name.strip()
    if name in ('f32', 'u32', 'i32'):
        return 4, 4, {}
    vector = re.fullmatch(r'vec([234])<(f32|u32|i32)>', name)
    if vector:
        n = int(vector[1])
        return (8 if n == 2 else 16), n*4, {}
    matrix = re.fullmatch(r'mat([234])x([234])<f32>', name)
    if matrix:
        columns, rows = map(int, matrix.groups())
        align, size, _ = wgsl_layout(f'vec{rows}<f32>', definitions)
        return align, math.ceil(size/align)*align*columns, {}
    if name.startswith('array<'):
        inner, count = split_fields(name[6:-1])
        align, size, _ = wgsl_layout(inner, definitions)
        return align, math.ceil(size/align)*align*int(count), {}
    fields, alignment, size, offsets = definitions[name], 1, 0, {}
    for field in split_fields(fields):
        label, typ = field.split(':', 1)
        align, length, _ = wgsl_layout(typ, definitions)
        alignment = max(alignment, align)
        size = math.ceil(size/align)*align
        offsets[label.strip()] = size
        size += length
    return alignment, math.ceil(size/alignment)*alignment, offsets


class ArtisticMaterialsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.scene = json.loads(source('examples/artistic-materials.artforge'))
        cls.art = uncomment(source('crates/render/src/artistic.wgsl'))

    def test_generator_exactly_matches_saved_scene(self):
        self.assertEqual(build(), self.scene)
        self.assertEqual(self.scene['version'], 43)

    def test_gallery_links_ids_and_material_ranges(self):
        p = self.scene
        materials = {m['id']:m for m in p['materials']}
        meshes = {m['id']:m for m in p['meshes']}
        self.assertEqual(len(materials), 14)
        self.assertEqual(len(meshes), 14)
        self.assertEqual(len({n['id'] for n in p['nodes']}), 14)
        self.assertFalse(p['textures'])
        styles = []
        for material in materials.values():
            if 'artistic' not in material:
                continue
            a = material['artistic']
            self.assertTrue(a['enabled'])
            styles.append(a['style'])
            self.assertTrue(.1 <= a['scale'] <= 64.)
            self.assertIsInstance(a['seed'], int)
            self.assertTrue(0 <= a['seed'] <= 65535)
            for key in ['bump','grain','variation','tool_marks','cracks','subsurface',
                        'pigment','paper','edge_darkening','wash','dry_brush']:
                self.assertTrue(0 <= a[key] <= 1, key)
            self.assertEqual(material['alpha_mode'], 'Opaque')
        self.assertEqual(len(styles), 13)
        self.assertEqual(len(set(styles)), 8)
        for node in p['nodes']:
            self.assertIn(node['mesh'], meshes)
        for mesh in meshes.values():
            self.assertIn(mesh['material'], materials)

    def test_gallery_geometry_is_finite_outward_and_nondegenerate(self):
        for mesh in self.scene['meshes']:
            vertices, indices = mesh['vertices'], mesh['indices']
            self.assertEqual(len(indices)%3, 0)
            for vertex in vertices:
                self.assertTrue(all(math.isfinite(v) for key in ('position','normal','uv') for v in vertex[key]))
                self.assertAlmostEqual(sum(v*v for v in vertex['normal']), 1., places=6)
            for index in range(0,len(indices),3):
                tri = [vertices[i] for i in indices[index:index+3]]
                p, q, r = [v['position'] for v in tri]
                a, b = [q[i]-p[i] for i in range(3)], [r[i]-p[i] for i in range(3)]
                cross = [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]
                normal = [sum(v['normal'][i] for v in tri) for i in range(3)]
                self.assertGreater(sum(cross[i]*normal[i] for i in range(3)), 1e-10, mesh['name'])

    def test_uniform_layouts_and_rust_order_agree(self):
        code='\n'.join(source('crates/render/src/'+name+'.wgsl') for name in ['artistic','npr','graph','scene','world'])
        definitions=dict(re.findall(r'struct\s+(\w+)\s*\{(.*?)\}',uncomment(code),re.S))
        align,size,fields=wgsl_layout('Artistic',definitions)
        self.assertEqual((align,size),(16,64))
        self.assertEqual(fields,dict(params=0,detail=16,paint=32,finish=48))
        _,size,fields=wgsl_layout('Draw',definitions)
        self.assertEqual(size,3680)
        self.assertEqual(fields['artistic'],3616)
        _,ray_size,ray_fields=wgsl_layout('RayMaterial',definitions)
        self.assertEqual(ray_fields['layers']-ray_fields['artistic'],64)
        self.assertEqual(ray_size,2528)
        rust=source('crates/render/src/artistic.rs')
        body=re.search(r'struct GpuArtistic\s*\{(.*?)\}',rust,re.S)[1]
        self.assertEqual(re.findall(r'(\w+):\s*\[f32; 4\]',body),['params','detail','paint','finish'])
        world=source('crates/render/src/world.rs')
        self.assertRegex(world,r'npr:.*\n\s*artistic:.*\n\s*layers:')

    def test_shared_shaders_are_wired_and_disabled_skips_noise(self):
        shader_sources=source('crates/render/src/shaders.rs')
        self.assertEqual(shader_sources.count('include_str!("artistic.wgsl")'),2)
        sample=self.art.split('fn art_sample(',1)[1].split('fn art_base(',1)[0]
        self.assertLess(sample.index('if a.params.x < 0.5 { return out; }'),sample.index('art_noise('))
        self.assertEqual(sample.count('art_noise('),2)
        self.assertNotRegex(self.art,r'\b(for|loop|while)\b|textureSample|\btime\b|dpdx|dpdy|fwidth')
        gpu=source('crates/render/src/artistic.rs')
        self.assertIn('if !s.enabled { return Self::zeroed(); }',gpu)

    def test_primary_derivatives_precede_discard_and_alpha_is_unmodified(self):
        fragment=source('crates/render/src/scene.wgsl').split('@fragment fn fs_main',1)[1]
        self.assertLess(fragment.index('let height_dx = dpdx(art.height)'),fragment.index('discard;'))
        self.assertIn('out.rest = v.position;',source('crates/render/src/scene.wgsl'))
        self.assertIn('-> vec3<f32>',self.art.split('fn art_finish(',1)[1])
        self.assertNotIn('discard',self.art)

    def test_presets_keep_legacy_order_and_are_unique(self):
        core=source('crates/core/src/material.rs')
        all_body=re.search(r'pub const ALL: \[Self; 26\] = \[(.*?)\];',core,re.S)[1]
        names=re.findall(r'Self::(\w+)',all_body)
        self.assertEqual(names[:13],['Matte','Plastic','Metal','Ceramic','Rubber','Glass','Water','BrushedMetal','Satin','Velvet','Toon','Anime','Ink'])
        self.assertEqual(names[13:],[p[0] for p in PRESETS])
        self.assertEqual(len(set(names)),26)
        for _,label,*_ in PRESETS:
            self.assertIn(label,core)

    def test_paint_controls_keep_routes_and_removed_sections_stay_absent(self):
        ui=source('crates/app/src/artistic_controls.rs')
        self.assertEqual(sorted(set(map(int,re.findall(r'"\s*,\s*(3[7-9]|4[0-8])\)',ui)))),list(range(44,49)))
        self.assertIn('SurfaceMaterialNumber(49)',ui)
        for command in ['SurfaceArtisticToggle','SurfaceArtisticNextStyle','SurfaceArtisticReset']:
            self.assertNotIn(command,ui)
        surface=source('crates/app/src/surfaces.rs')
        self.assertEqual(surface.count('37..=49'),3)
        self.assertIn('field == 49 && value.fract() != 0.',surface)
        self.assertIn('field @ 37..=49',source('crates/app/src/number_defaults.rs'))
        sections=source('crates/app/src/property_sections.rs')
        self.assertIn('title=="Краска и бумага" {182}',sections)
        for title in ['Художественная поверхность','Микрорельеф и лепка']:
            self.assertNotIn(title,sections)

    def test_paint_slider_ranges_match_fractional_and_seed_fields(self):
        # Source-level regression guard for E0308; not a substitute for cargo check.
        core = uncomment(source('crates/core/src/artistic.rs'))
        ui = uncomment(source('crates/app/src/ui.rs'))
        controls = uncomment(source('crates/app/src/artistic_controls.rs'))
        self.assertRegex(core, r'fn bounds\(field: usize\) -> Option<\(f32, f32\)>')
        self.assertRegex(ui, r'fn new\(min: f64, max: f64, step: f64\)')
        self.assertIn('SliderRange::new(0.,1.,0.01)',controls)
        self.assertIn('SliderRange::new(0.,65535.,1.)',controls)

    def test_schema_defaults_migration_and_validation_remain_opt_in(self):
        core=source('crates/core/src/document.rs')
        version = int(re.search(r'SCHEMA_VERSION: u32 = (\d+)', core).group(1))
        self.assertGreaterEqual(version, 43)
        self.assertRegex(core,r'#\[serde\(default\)\]\s*pub artistic:')
        self.assertIn('!m.artistic.valid()',core)
        self.assertIn(f'1..={version - 1} =>',source('crates/io/src/project.rs'))
        self.assertIn('enabled: false',source('crates/core/src/artistic.rs'))

    def test_versions_and_fx_hotfix_are_preserved(self):
        self.assertIn('version = "0.48.3"',source('Cargo.toml'))
        self.assertEqual(source('Cargo.lock').count('version = "0.48.3"'),5)
        self.assertIn('0.48.3',source('START_WINDOWS.cmd'))
        self.assertRegex(source('crates/simulation/src/gas.rs'),r'let reach\s*:\s*f32\s*=')


if __name__ == '__main__':
    unittest.main()
