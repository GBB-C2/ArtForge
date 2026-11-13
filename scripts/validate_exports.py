#!/usr/bin/env python3
"""End-to-end CLI export checks. Python stdlib only; a graphics adapter is required."""
import argparse
import hashlib
import json
import pathlib
import struct
import subprocess
import zlib


def png_rgba(path):
    data = path.read_bytes()
    assert data.startswith(b"\x89PNG\r\n\x1a\n"), f"Not PNG: {path}"
    chunks, offset, header = [], 8, None
    while offset < len(data):
        length = struct.unpack(">I", data[offset:offset + 4])[0]
        kind = data[offset + 4:offset + 8]
        payload = data[offset + 8:offset + 8 + length]
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", payload)
        elif kind == b"IDAT":
            chunks.append(payload)
        offset += length + 12
    assert header is not None
    width, height, depth, color, compression, filtering, interlaced = header
    assert (depth, color, compression, filtering, interlaced) == (8, 6, 0, 0, 0), header
    raw = zlib.decompress(b"".join(chunks))
    stride = width * 4
    pixels, previous = bytearray(), bytearray(stride)
    for y in range(height):
        start = y * (stride + 1)
        mode = raw[start]
        row = bytearray(raw[start + 1:start + 1 + stride])
        for x in range(stride):
            left = row[x - 4] if x >= 4 else 0
            above = previous[x]
            upper_left = previous[x - 4] if x >= 4 else 0
            if mode == 1:
                predictor = left
            elif mode == 2:
                predictor = above
            elif mode == 3:
                predictor = (left + above) // 2
            elif mode == 4:
                p = left + above - upper_left
                distances = (abs(p - left), abs(p - above), abs(p - upper_left))
                predictor = (left, above, upper_left)[distances.index(min(distances))]
            else:
                assert mode == 0
                predictor = 0
            row[x] = (row[x] + predictor) & 255
        pixels.extend(row)
        previous = row
    return width, height, bytes(pixels)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=pathlib.Path, default=pathlib.Path("target/debug/poseforge"))
    parser.add_argument("--output", type=pathlib.Path, default=pathlib.Path("output/cli-validation"))
    args = parser.parse_args()
    binary = args.binary.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    log, checks = [], []

    def run(*arguments, success=True):
        completed = subprocess.run([str(binary), *(str(a) for a in arguments)], capture_output=True, text=True)
        log.append({"args": list(map(str, arguments)), "code": completed.returncode,
                    "stdout": completed.stdout, "stderr": completed.stderr})
        if success and completed.returncode != 0:
            raise AssertionError(completed.stderr)
        if not success and completed.returncode == 0:
            raise AssertionError("Command should have rejected inconsistent input")
        return completed

    try:
        run("--self-check", output / "self-check")
        checks.append("self_check_save_reopen_evaluation_pbf")
        run("--demo", output / "demo.poseforge")
        run("--validate", output / "demo.poseforge")
        run("--validate", "crates/io/tests/fixtures/skinned_triangle.glb")
        checks.append("saved_project_and_glb_validate")
        run("--render", "demo", output / "single.png", 640, 360, 0.5)
        run("--sequence", "demo", output / "sequence", 12, 12, 640, 360)
        single = png_rgba(output / "single.png")
        frame = output / "sequence/frame_000012.png"
        assert single[:2] == (640, 360)
        assert single == png_rgba(frame), "Single PNG and matching sequence frame differ"
        checks.append("single_equals_sequence_same_time")
        original_time = frame.stat().st_mtime_ns
        run("--sequence", "demo", output / "sequence", 12, 12, 640, 360, "--resume")
        assert frame.stat().st_mtime_ns == original_time, "Valid frame was re-rendered"
        checks.append("resume_preserves_valid_frame")
        frame.write_bytes(b"intentional interrupted/corrupted output")
        run("--sequence", "demo", output / "sequence", 12, 12, 640, 360, "--resume")
        assert png_rgba(frame) == single
        checks.append("resume_repairs_corrupt_frame")
        digest_before = hashlib.sha256(frame.read_bytes()).hexdigest()
        run("--sequence", "demo", output / "sequence", 12, 12, 800, 360, "--resume", success=False)
        assert hashlib.sha256(frame.read_bytes()).hexdigest() == digest_before
        checks.append("resume_rejects_changed_settings_without_touching_frames")
        run("--render", "demo", output / "transparent.png", 640, 360, 0.5, "--transparent")
        alpha = png_rgba(output / "transparent.png")[2][3::4]
        assert min(alpha) == 0 and max(alpha) == 255
        checks.append("rgba_preserves_transparent_background_and_opaque_geometry")
        cancelled = output / "cancelled"
        cancelled.mkdir()
        (cancelled / "CANCEL").touch()
        run("--sequence", "demo", cancelled, 0, 1, 320, 180)
        journal = json.loads((cancelled / "render-manifest.json").read_text())
        assert journal["status"] == "cancelled" and not journal["completed"]
        (cancelled / "CANCEL").unlink()
        run("--sequence", "demo", cancelled, 0, 1, 320, 180, "--resume")
        journal = json.loads((cancelled / "render-manifest.json").read_text())
        assert journal["status"] == "complete" and len(journal["completed"]) == 2
        checks.append("cancel_then_resume_finishes_remaining_frames")
        for second in (0, 1):
            run("--render", "crates/io/tests/fixtures/skinned_triangle.glb",
                output / f"import-{second}.png", 320, 320, second, "--transparent")
        assert png_rgba(output / "import-0.png") != png_rgba(output / "import-1.png")
        checks.append("imported_reordered_skin_and_morph_affect_rendered_frame")
    finally:
        report = {"checks_passed": checks, "commands": log}
        (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"passed": len(checks), "checks": checks, "report": str(output / "report.json")}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
