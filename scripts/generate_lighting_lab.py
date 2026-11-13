"""Generate the self-contained 0.13 lighting lab and an original Radiance panorama."""
from pathlib import Path
import json
import math


def node(identifier, kind, position, value, inputs=None):
    return dict(id=identifier, kind=kind, position=position, value=value,
                inputs=inputs or [None, None, None])


def panorama(width=256, height=128):
    pixels = []
    for y in range(height):
        v = (y + 0.5) / height
        for x in range(width):
            u = (x + 0.5) / width
            base = [0.045, 0.055, 0.075] if v > 0.5 else [0.12, 0.15, 0.20]
            for cx, cy, sx, sy, color in [
                (0.18, 0.28, 0.09, 0.10, [10., 7.8, 5.2]),
                (0.72, 0.32, 0.035, 0.17, [4., 6.5, 10.]),
                (0.48, 0.12, 0.18, 0.04, [3., 3., 3.]),
            ]:
                dx = min(abs(u-cx), 1.-abs(u-cx)) / sx
                dy = abs(v-cy) / sy
                amount = math.exp(-(dx**8+dy**8))
                base = [a + b * amount for a, b in zip(base, color)]
            pixels.append([round(c, 5) for c in base])
    return dict(name="ArtForge · Studio softboxes.hdr", width=width, height=height, rgb=pixels)


def write_hdr(path, image):
    """Radiance scanline RGBE, literal RLE packets, preserving HDR values."""
    width, height = image['width'], image['height']
    output = bytearray(f'#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y {height} +X {width}\n'.encode())
    for y in range(height):
        row = []
        for color in image['rgb'][y*width:(y+1)*width]:
            high = max(color)
            if high < 1e-32:
                row.append([0, 0, 0, 0])
            else:
                mantissa, exponent = math.frexp(high)
                scale = mantissa * 256. / high
                row.append([*(min(255, int(c*scale)) for c in color), exponent+128])
        output.extend([2, 2, width >> 8, width & 255])
        for channel in range(4):
            for start in range(0, width, 128):
                values = [pixel[channel] for pixel in row[start:start+128]]
                output.append(len(values))
                output.extend(values)
    path.write_bytes(output)


def main():
    root = Path(__file__).resolve().parents[1]
    project = json.loads((root/'examples/lookdev-studio.artforge').read_text())
    project.update(version=9, title='Студия 0.13 · HDRI, отражения, тени и узлы', environment_map=panorama())
    project['render_settings'].update(hdri_enabled=True, hdri_background=False, hdri_strength=0.7,
        hdri_rotation=25., ray_reflections=True, reflection_samples=16, ray_shadows=True,
        area_shadow_samples=64, ambient_intensity=0.7, ambient_occlusion=0.5, exposure=-0.3)
    project['materials'][0].update(name='Отражающее основание', metallic=0.85, roughness=0.14,
        base_color=[0.35, 0.39, 0.45, 1.])
    for m in project['materials']:
        m['graph'] = None
    material = project['materials'][2]
    material.update(name='Узлы · бирюзовая керамика', base_color_texture=None,
                    uv_scale=[1.,1.], uv_offset=[0.,0.], uv_rotation=0., coat_weight=0.35,
                    graph=dict(enabled=True, nodes=[
                        node(1,'Color',[10.,15.],[0.015,0.31,0.32,1.]),
                        node(2,'Color',[10.,125.],[0.82,0.72,0.54,1.]),
                        node(3,'Checker',[225.,15.],[8.,0.,0.,0.],[1,2,None]),
                        node(4,'Value',[225.,160.],[0.23,0.,0.,0.]),
                    ], outputs=[3,None,4,None], output_position=[470.,15.]))
    project['nodes'][2]['name']=material['name']
    project['lights'][0].update(intensity=10., area_size=[2.5, 1.6])
    project['lights'][1].update(intensity=5.)
    path=root/'examples/lighting-lab.artforge'
    path.write_text(json.dumps(project,ensure_ascii=False,separators=(',',':'))+'\n')
    write_hdr(root/'examples/studio-softboxes.hdr', project['environment_map'])
    print(path)


if __name__ == '__main__':
    main()
