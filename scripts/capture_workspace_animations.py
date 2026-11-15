#!/usr/bin/env python3
"""Capture real offscreen workspace interactions and encode 60 Hz WebP clips.

Run `python3 scripts/capture_workspace_animations.py` from the repository root.
Requires Pillow and a software Vulkan adapter; no X11 display is needed.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import tempfile

from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parent.parent
FPS = 60
FRAMES = 120


def encoded_durations(path: Path) -> list[int]:
    """Read frame timestamps from the animated WebP container."""
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


def cursor(image: Image.Image, x: float, y: float) -> None:
    """Draw the scripted pointer over an unmodified Bevy frame."""
    px, py = round(x), round(y)
    arrow = [(0, 0), (0, 17), (4, 13), (7, 20), (10, 19), (7, 12), (13, 11)]
    ImageDraw.Draw(image).polygon(
        [(px + dx, py + dy) for dx, dy in arrow],
        fill="white",
        outline="#171717",
        width=1,
    )


def encode(frames_dir: Path, output: Path) -> None:
    paths = sorted(frames_dir.glob("[0-9][0-9][0-9].png"))
    if len(paths) != FRAMES:
        raise RuntimeError(f"{frames_dir}: expected {FRAMES} frames, got {len(paths)}")
    positions = [
        tuple(map(float, line.split(",")))
        for line in (frames_dir / "cursor.csv").read_text().splitlines()
    ]
    if len(positions) != FRAMES:
        raise RuntimeError(f"{frames_dir}: expected {FRAMES} cursor positions")
    images = []
    for path, (x, y) in zip(paths, positions, strict=True):
        with Image.open(path) as raw:
            image = raw.convert("RGB")
        cursor(image, x, y)
        images.append(image)
    durations = [
        round((index + 1) * 1000 / FPS) - round(index * 1000 / FPS)
        for index in range(FRAMES)
    ]
    output.parent.mkdir(parents=True, exist_ok=True)
    images[0].save(
        output,
        format="WEBP",
        save_all=True,
        append_images=images[1:],
        duration=durations,
        loop=0,
        quality=82,
        method=5,
    )
    stored = encoded_durations(output)
    if sum(stored) != 2_000 or not any(duration in (16, 17) for duration in stored):
        raise RuntimeError(f"{output}: encoded WebP lost its 60 Hz frame timing")
    print(
        f"{output}: {FRAMES} samples at {FPS} Hz, {len(stored)} stored frames "
        f"after combining holds, {sum(stored)} ms, {output.stat().st_size // 1024} KiB"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=Path, default=ROOT / "docs/visuals/animations"
    )
    parser.add_argument("--profile", choices=("debug", "release"), default="release")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="bbui-workspace-animation-") as temporary:
        frames_dir = Path(temporary)
        command = ["cargo", "test"]
        if args.profile == "release":
            command.append("--release")
        command += [
            "-p", "bbui", "workspace::gpu::capture_workspace_dark_and_light",
            "--lib", "--locked", "--", "--ignored",
        ]
        subprocess.run(
            command,
            cwd=ROOT,
            env={
                **os.environ,
                "BBUI_WORKSPACE_ANIMATION_FRAMES_DIR": str(frames_dir),
                "WGPU_BACKEND": os.environ.get("WGPU_BACKEND", "vulkan"),
            },
            check=True,
        )
        for name in ("dock", "resize"):
            encode(frames_dir / name, args.output / f"workspace_{name}.webp")


if __name__ == "__main__":
    main()
