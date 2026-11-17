#!/usr/bin/env python3
"""Capture the eight solid/gradient, static/animated effect combinations.

Run from the repository root. Requires Pillow and a software Vulkan adapter;
no X11 display is needed. Pass --frames-dir to encode an existing capture.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import tempfile

from PIL import Image, ImageChops

from capture_visual_effect_animations import ROOT, encoded_durations


FRAMES = 120
FPS = 60
SIZE = (1200, 720)
KINDS = ("solid-static", "solid-animated", "gradient-static", "gradient-animated")


def frames_changed(frames: list[Image.Image]) -> int:
    return sum(
        ImageChops.difference(previous, current).getbbox() is not None
        for previous, current in zip(frames, frames[1:])
    )


def encode(frames_dir: Path, output_dir: Path) -> None:
    paths = sorted(frames_dir.glob("[0-9][0-9][0-9].png"))
    if len(paths) != FRAMES:
        raise RuntimeError(f"{frames_dir}: expected {FRAMES} frames, got {len(paths)}")
    images = []
    for path in paths:
        with Image.open(path) as raw:
            if raw.size != SIZE:
                raise RuntimeError(f"{path}: expected {SIZE}, got {raw.size}")
            images.append(raw.convert("RGB"))
    durations = [(17, 17, 16)[index % 3] for index in range(FRAMES)]
    output_dir.mkdir(parents=True, exist_ok=True)
    clips = {"visual-effect-variants.webp": (0, 0, 1200, 720, True)}
    for family, top in (("border", 118), ("glow", 388)):
        for index, kind in enumerate(KINDS):
            left = 9 + index * 290
            clips[f"{family}-{kind}.webp"] = (
                left,
                top,
                left + 282,
                top + 250,
                "animated" in kind,
            )
    for name, (left, top, right, bottom, animated) in clips.items():
        frames = [image.crop((left, top, right, bottom)) for image in images]
        changes = frames_changed(frames)
        if animated and changes < 100:
            raise RuntimeError(f"{name}: only {changes} rendered frame transitions change")
        if not animated and changes != 0:
            raise RuntimeError(f"{name}: static preview changes in {changes} frames")
        output = output_dir / name
        if animated:
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
            if len(stored) < 100 or sum(stored) != 2_000 or any(duration < 16 for duration in stored):
                raise RuntimeError(f"{output}: incorrect 60 fps timing")
            print(f"{output}: {len(stored)} stored frames / 120 samples, 2.000 s")
        else:
            frames[0].save(output, format="WEBP", lossless=True, method=5)
            print(f"{output}: static WebP")


def capture(frames_dir: Path, profile: str) -> None:
    command = ["cargo", "test"]
    if profile == "release":
        command.append("--release")
    command += [
        "-p", "bbui", "--example", "visual_effects", "capture_visual_effect_variants",
        "--locked", "--", "--ignored", "--nocapture",
    ]
    subprocess.run(
        command,
        cwd=ROOT,
        env={
            **os.environ,
            "BBUI_CAPTURE_VARIANTS_DIR": str(frames_dir),
            "WGPU_BACKEND": os.environ.get("WGPU_BACKEND", "vulkan"),
        },
        check=True,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--frames-dir", type=Path)
    parser.add_argument(
        "--output", type=Path, default=ROOT / "docs/visuals/animations"
    )
    parser.add_argument("--profile", choices=("debug", "release"), default="debug")
    args = parser.parse_args()
    if args.frames_dir is not None:
        encode(args.frames_dir, args.output)
    else:
        with tempfile.TemporaryDirectory(prefix="bbui-visual-effect-variants-") as temporary:
            frames_dir = Path(temporary)
            capture(frames_dir, args.profile)
            encode(frames_dir, args.output)


if __name__ == "__main__":
    main()
