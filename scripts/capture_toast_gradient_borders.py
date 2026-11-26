#!/usr/bin/env python3
"""Capture the four severity border palettes at a deterministic 60 fps."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import tempfile

from PIL import Image, ImageChops


ROOT = Path(__file__).resolve().parent.parent
FRAMES = 120
SIZE = (1100, 760)
CROP = (728, 226, 1068, 728)
OUTPUT = ROOT / "docs/visuals/animations/toast-gradient-borders.webp"


def encoded_durations(path: Path) -> list[int]:
    data = path.read_bytes()
    if data[:4] != b"RIFF" or data[8:12] != b"WEBP":
        raise RuntimeError(f"{path}: invalid WebP container")
    durations = []
    offset = 12
    while offset + 8 <= len(data):
        kind = data[offset : offset + 4]
        size = int.from_bytes(data[offset + 4 : offset + 8], "little")
        if kind == b"ANMF":
            durations.append(int.from_bytes(data[offset + 20 : offset + 23], "little"))
        offset += 8 + size + (size & 1)
    return durations


def encode(frames_dir: Path, output: Path) -> None:
    paths = sorted(frames_dir.glob("[0-9][0-9][0-9].png"))
    if len(paths) != FRAMES:
        raise RuntimeError(f"{frames_dir}: expected {FRAMES} frames, got {len(paths)}")
    frames = []
    for path in paths:
        with Image.open(path) as source:
            if source.size != SIZE:
                raise RuntimeError(f"{path}: expected {SIZE}, got {source.size}")
            frames.append(source.convert("RGB").crop(CROP))
    changes = sum(
        ImageChops.difference(previous, current).getbbox() is not None
        for previous, current in zip(frames, frames[1:], strict=False)
    )
    if changes < 110:
        raise RuntimeError(f"gradient borders: only {changes} of 119 frames change")
    durations = [(17, 17, 16)[index % 3] for index in range(FRAMES)]
    output.parent.mkdir(parents=True, exist_ok=True)
    frames[0].save(
        output,
        format="WEBP",
        save_all=True,
        append_images=frames[1:],
        duration=durations,
        loop=0,
        lossless=True,
        method=5,
    )
    stored = encoded_durations(output)
    if len(stored) != FRAMES or sum(stored) != 2_000 or set(stored) != {16, 17}:
        raise RuntimeError(f"{output}: incorrect 60 fps timing or frame count")
    print(f"{output}: {FRAMES} frames, 2.000 s, {output.stat().st_size // 1024} KiB")


def capture(frames_dir: Path) -> None:
    subprocess.run(
        [
            "cargo", "test", "-p", "bbui", "--example", "toast_gradient_borders",
            "capture_toast_gradient_borders", "--locked", "--", "--ignored", "--nocapture",
        ],
        cwd=ROOT,
        env={
            **os.environ,
            "BBUI_NOTICE_GRADIENT_FRAMES_DIR": str(frames_dir),
            "WGPU_BACKEND": os.environ.get("WGPU_BACKEND", "vulkan"),
        },
        check=True,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--frames-dir", type=Path)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    args = parser.parse_args()
    if args.frames_dir is not None:
        encode(args.frames_dir, args.output)
    else:
        with tempfile.TemporaryDirectory(prefix="bbui-toast-gradients-") as temporary:
            frames_dir = Path(temporary)
            capture(frames_dir)
            encode(frames_dir, args.output)


if __name__ == "__main__":
    main()
