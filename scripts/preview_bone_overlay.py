"""Rasterize actual BoneOverlay QA primitives without a GPU.

This previews the editor's geometry/color output, not a native window capture.
First run the Rust reference_style test with ARTFORGE_QA_BONE_DIR set, then:
    python scripts/preview_bone_overlay.py <json-directory> <png-directory>
Requires NumPy and Pillow, as do the other CPU preview scripts.
"""
import argparse
import json
import math
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont


def linear(rgb):
    rgb = np.asarray(rgb, dtype=np.float32)
    return np.where(rgb <= 0.04045, rgb / 12.92, ((rgb + 0.055) / 1.055) ** 2.4)


def render(data, scale=3):
    width, height = int(data['width']), int(data['height'])
    canvas = np.broadcast_to(linear(data['background'][:3]),
                             (height * scale, width * scale, 3)).copy()
    for primitive in data['rectangles']:
        x, y, w, h = primitive['rect']
        left, top = max(0, math.floor(x * scale)), max(0, math.floor(y * scale))
        right = min(width * scale, math.ceil((x + w) * scale))
        bottom = min(height * scale, math.ceil((y + h) * scale))
        if right <= left or bottom <= top:
            continue
        triangle, segment = primitive.get('triangle'), primitive.get('segment')
        if triangle:
            points = [(p[0], p[1]) for p in triangle]
        elif segment:
            ax, ay, bx, by, stroke = segment
            length = math.hypot(bx - ax, by - ay)
            if length < 1e-8:
                continue
            nx, ny = -(by - ay) * stroke / (2 * length), (bx - ax) * stroke / (2 * length)
            points = [(ax + nx, ay + ny), (bx + nx, by + ny),
                      (bx - nx, by - ny), (ax - nx, ay - ny)]
        else:
            points = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)]
        mask = Image.new('L', (right - left, bottom - top))
        ImageDraw.Draw(mask).polygon([(px * scale - left, py * scale - top)
                                     for px, py in points], fill=255)
        alpha = np.asarray(mask, dtype=np.float32)[..., None] / 255 * primitive['color'][3]
        patch = canvas[top:bottom, left:right]
        patch[:] = patch * (1 - alpha) + linear(primitive['color'][:3]) * alpha
    srgb = np.where(canvas <= 0.0031308, canvas * 12.92,
                    1.055 * np.maximum(canvas, 0) ** (1 / 2.4) - 0.055)
    image = Image.fromarray((np.clip(srgb, 0, 1) * 255 + 0.5).astype('uint8'))
    # Labels come from the real UI export. Font rasterization is a CPU substitute
    # for the application's GPU glyph atlas, not proof of native text rendering.
    painter = ImageDraw.Draw(image)
    fonts = {}
    for text in data.get('texts', []):
        size = max(1, round(text['size'] * scale))
        if size not in fonts:
            fonts[size] = ImageFont.truetype('DejaVuSans.ttf', size)
        color = tuple(round(max(0, min(1, c)) * 255) for c in text['color'][:3])
        painter.text((text['x'] * scale, text['y'] * scale), text['text'],
                     font=fonts[size], fill=color, anchor='lt')
    return image.resize((width, height), Image.Resampling.LANCZOS)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    files = sorted(args.source.glob('bones-*.json'))
    if not files:
        parser.error('No bones-*.json files; run the Rust QA export first.')
    for path in files:
        target = args.output / (path.stem + '.png')
        render(json.loads(path.read_text(encoding='utf-8'))).save(target)
        print(target)


if __name__ == '__main__':
    main()
