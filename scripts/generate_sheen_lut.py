"""Bake the Charlie/Neubelt directional albedo used for material layering.

Standard-library-only, deterministic cosine hemisphere quadrature. The checked-in
64x64 RGBA8 asset stores raw E/8 in R/G and normalized E in B/A (16 bits each).
Reciprocal normalization bounds the approximate Neubelt visibility at grazing
angles: BRDF / max(1, E(view), E(light)). No build-time
generator or runtime integration is required. Regenerate from the repository root.
"""
from pathlib import Path
import math

SIZE = 64
SAMPLES = 4096

def radical_inverse(value):
    result, weight = 0.0, 0.5
    while value:
        result += (value & 1) * weight
        value >>= 1
        weight *= 0.5
    return result

def bake():
    lights = []
    for sample in range(SAMPLES):
        radius = math.sqrt((sample + 0.5) / SAMPLES)
        phi = math.tau * radical_inverse(sample)
        lights.append((radius * math.cos(phi), radius * math.sin(phi), math.sqrt(1 - radius * radius)))
    # Cache angle terms once; roughness only changes the distribution exponent.
    angles = []
    for x in range(SIZE):
        nv = max(x / (SIZE - 1), 0.0001)
        vx = math.sqrt(1 - nv * nv)
        row = []
        for lx, ly, nl in lights:
            hz = nl + nv
            norm = (lx + vx) ** 2 + ly * ly + hz * hz
            sin2h = max(1 - hz * hz / norm, 0.0)
            visibility = 1 / (4 * max(nv + nl - nv * nl, 0.000001))
            row.append((sin2h, visibility))
        angles.append(row)
    data = bytearray()
    values = []
    for y in range(SIZE):
        roughness = max(y / (SIZE - 1), 0.07)
        inv_alpha = 1 / max(roughness * roughness, 0.0049)
        factor = (2 + inv_alpha) / 2
        raw = []
        for row in angles:
            # Cosine-weighted PDF = N.L / PI: integral is PI * average(BRDF).
            value = factor * sum(s ** (inv_alpha * 0.5) * v for s, v in row) / SAMPLES
            assert math.isfinite(value) and 0 <= value < 8
            raw.append(value)
        light_energy = []
        for _, _, nl in lights:
            coordinate = nl * (SIZE - 1)
            index = min(int(coordinate), SIZE - 2)
            light_energy.append(raw[index] + (raw[index + 1] - raw[index]) * (coordinate - index))
        for x, row in enumerate(angles):
            energy = factor * sum(s ** (inv_alpha * 0.5) * v / max(1, raw[x], e)
                for (s, v), e in zip(row, light_energy)) / SAMPLES
            assert math.isfinite(energy) and 0 <= energy <= 1.000001
            values.append(energy)
            encoded = round(raw[x] / 8 * 65535)
            normalized = round(min(energy, 1) * 65535)
            data.extend((encoded >> 8, encoded & 255, normalized >> 8, normalized & 255))
    target = Path(__file__).resolve().parents[1] / 'crates/render/src/sheen_albedo.rgba8'
    target.write_bytes(data)
    print(f'{target.name}: {len(data)} bytes, albedo range {min(values):.6f}..{max(values):.6f}')

if __name__ == '__main__':
    bake()
