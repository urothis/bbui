#!/usr/bin/env python3
"""Capture real Bevy UI transitions as animated WebP files on an X11 display.

Build with `cargo build --release -p bbui --examples --locked`, then run with
`DISPLAY=:99 python3 scripts/capture_animations.py --profile release`.
Requires Pillow, xwininfo, xwd, libX11, and
libXtst. The output contains actual rendered example frames.
"""

from __future__ import annotations

import argparse
import ctypes
import os
from pathlib import Path
import struct
import subprocess
import sys
import threading
import time

from PIL import Image, ImageChops

sys.dont_write_bytecode = True
from capture_examples import ROOT, raise_window, windows


def xwd_image(data: bytes) -> Image.Image:
    """Decode the common 32-bit XWD format without a per-pixel Python loop."""
    header = struct.unpack(">25I", data[:100])
    size, version, fmt, depth, width, height = header[:6]
    byte_order, bits, stride = header[7], header[11], header[12]
    red, green, blue, colors = header[14], header[15], header[16], header[19]
    if (version, fmt, depth, bits, byte_order, red, green, blue) != (
        7, 2, 24, 32, 0, 0xFF0000, 0x00FF00, 0x0000FF
    ):
        raise ValueError("unsupported XWD pixel format; use a 24-bit X11 screen")
    pixels = data[size + colors * 12:]
    return Image.frombytes("RGB", (width, height), pixels, "raw", "BGRX", stride, 1)


def frame(window_id: str) -> Image.Image:
    data = subprocess.run(
        ["xwd", "-id", window_id], check=True, capture_output=True
    ).stdout
    return xwd_image(data)


def input_event(window_id: str, *, x: int = 0, y: int = 0, key: int = 0) -> None:
    """Focus a window and send a short click or keypress via XTest."""
    x11 = ctypes.CDLL("libX11.so.6")
    xtst = ctypes.CDLL("libXtst.so.6")
    x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XSetInputFocus.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
    x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
    x11.XKeysymToKeycode.restype = ctypes.c_uint
    x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
    x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
    xtst.XTestFakeMotionEvent.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_ulong]
    xtst.XTestFakeButtonEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
    xtst.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
    display = x11.XOpenDisplay(None)
    if not display:
        raise RuntimeError("cannot open DISPLAY")
    try:
        x11.XSetInputFocus(display, int(window_id, 16), 1, 0)
        if key:
            code = x11.XKeysymToKeycode(display, key)
            xtst.XTestFakeKeyEvent(display, code, 1, 0)
            x11.XSync(display, 0)
            time.sleep(0.03)
            xtst.XTestFakeKeyEvent(display, code, 0, 0)
        else:
            xtst.XTestFakeMotionEvent(display, -1, x, y, 0)
            x11.XSync(display, 0)
            time.sleep(0.03)
            xtst.XTestFakeButtonEvent(display, 1, 1, 0)
            x11.XSync(display, 0)
            time.sleep(0.03)
            xtst.XTestFakeButtonEvent(display, 1, 0, 0)
        x11.XSync(display, 0)
    finally:
        x11.XCloseDisplay(display)


def theme_cycle(window_id: str) -> None:
    """Capture both eased directions so the WebP loop has no hard color jump."""
    input_event(window_id, key=0xFFC3)
    time.sleep(1.45)
    input_event(window_id, key=0xFFC3)


def record(window_id: str, action, crop: tuple[int, int, int, int], seconds: float) -> list[tuple[float, Image.Image]]:
    images = []
    start = time.monotonic()
    fired = False
    while time.monotonic() - start < seconds:
        elapsed = time.monotonic() - start
        if elapsed >= 0.12 and not fired:
            threading.Thread(target=action, daemon=True).start()
            fired = True
        image = frame(window_id).crop(crop)
        images.append((time.monotonic() - start, image))
        # XWD capture is the limiting factor; keep the sampled cadence stable.
        target = start + len(images) / 60
        if target > time.monotonic():
            time.sleep(target - time.monotonic())
    return images


def distinct_frames(images: list[tuple[float, Image.Image]]) -> list[tuple[float, Image.Image]]:
    unique = []
    for timestamp, current in images:
        if max(channel[1] for channel in current.getextrema()) <= 10:
            continue
        if not unique or ImageChops.difference(unique[-1][1], current).getbbox():
            unique.append((timestamp, current))
    return unique


def capture(name: str, output: Path, action, crop, seconds: float, profile: str) -> None:
    before = set(windows())
    log_dir = Path("/tmp/bbui-animation-capture-logs")
    log_dir.mkdir(exist_ok=True)
    with (log_dir / f"{name}.log").open("wb") as log:
        process = subprocess.Popen(
            [str(ROOT / "target" / profile / "examples" / name)], cwd=ROOT,
            stdout=log, stderr=subprocess.STDOUT,
            env={**os.environ, "WGPU_BACKEND": os.environ.get("WGPU_BACKEND", "vulkan"),
                 "BBUI_EXAMPLE_MOTION_SCALE": "4"},
        )
        try:
            deadline = time.monotonic() + 25
            window_id = None
            while time.monotonic() < deadline:
                for wid, (_, window_class) in windows().items():
                    if wid not in before and window_class == name:
                        window_id = wid
                        break
                if window_id:
                    break
                if process.poll() is not None:
                    raise RuntimeError(f"{name} exited before opening a window")
                time.sleep(0.1)
            if window_id is None:
                raise TimeoutError(f"{name} did not open a window")
            time.sleep(1.2)
            raise_window(window_id)
            while time.monotonic() < deadline:
                if max(channel[1] for channel in frame(window_id).getextrema()) > 10:
                    break
                time.sleep(0.1)
            else:
                raise RuntimeError(f"{name} remained black after rendering")
            time.sleep(0.2)
            sampled = record(window_id, lambda: action(window_id), crop, seconds)
            unique = distinct_frames(sampled)
            print(f"{name}: sampled {len(sampled)} frames, {len(unique)} distinct", flush=True)
            if len(unique) < 3:
                raise RuntimeError(f"{name} yielded only {len(unique)} distinct frames")
            images = [image for _, image in unique]
            durations = [
                max(11, round((unique[index + 1][0] - timestamp) * 1000))
                for index, (timestamp, _) in enumerate(unique[:-1])
            ]
            durations.append(max(100, round((seconds - unique[-1][0]) * 1000)))
            output.parent.mkdir(parents=True, exist_ok=True)
            images[0].save(
                output, format="WEBP", save_all=True, append_images=images[1:],
                duration=durations,
                loop=0, quality=82, method=5,
            )
            print(f"{output}: {len(images)} frames, {sum(durations)} ms, {output.stat().st_size // 1024} KiB", flush=True)
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "docs/visuals/animations")
    parser.add_argument("--example", action="append", help="Capture only this example (repeatable)")
    parser.add_argument("--profile", choices=("debug", "release"), default="release")
    args = parser.parse_args()
    if not os.environ.get("DISPLAY"):
        parser.error("DISPLAY must point to an X11 display")
    cases = [
        ("panel_reorder", "panel_expand.webp", lambda wid: input_event(wid, x=80, y=120), (0, 0, 880, 250), 1.1),
        ("menus", "menu_open.webp", lambda wid: input_event(wid, x=280, y=140), (20, 80, 570, 310), 0.9),
        ("keyframe_gallery", "keyframe_curves.webp", lambda wid: input_event(wid, key=0xFFC2), (0, 0, 760, 380), 2.1),
        ("theme_gallery", "theme_switch.webp", theme_cycle, (0, 0, 720, 430), 3.2),
        ("properties_editor", "properties_transform.webp", lambda wid: input_event(wid, x=80, y=74), (0, 0, 520, 690), 1.1),
        ("properties_editor", "properties_visibility.webp", lambda wid: input_event(wid, x=80, y=325), (0, 0, 520, 690), 1.1),
        ("properties_editor", "properties_render_menu.webp", lambda wid: input_event(wid, x=120, y=14), (0, 0, 520, 690), 0.9),
    ]
    if args.example:
        unknown = set(args.example) - {name for name, *_ in cases}
        if unknown:
            parser.error(f"unknown example(s): {', '.join(sorted(unknown))}")
        cases = [case for case in cases if case[0] in args.example]
    for name, filename, action, crop, seconds in cases:
        capture(name, args.output / filename, action, crop, seconds, args.profile)


if __name__ == "__main__":
    main()
