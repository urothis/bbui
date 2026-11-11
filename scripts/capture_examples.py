#!/usr/bin/env python3
"""Capture every graphical BBUI example on an X11 display.

Build first with `cargo build -p bbui --examples --locked`, then run this script
with DISPLAY set to an Xorg or Xvfb display. Requires xwininfo and xwd. The GPU
stress benchmark is intentionally excluded because it has no visible window.
"""

from __future__ import annotations

import argparse
import ctypes
import os
from pathlib import Path
import re
import struct
import subprocess
import time
import zlib

ROOT = Path(__file__).resolve().parent.parent
EXAMPLES = ROOT / "crates" / "bbui" / "examples"
BINARIES = ROOT / "target" / "debug" / "examples"
WINDOW_RE = re.compile(
    r'^\s*(0x[0-9a-f]+)\s+"([^"]+)": \("([^"]+)" "[^"]+"\)',
    re.MULTILINE,
)
EXCLUDE = {"render_gpu_stress_bench"}


def windows() -> dict[str, tuple[str, str]]:
    result = subprocess.run(
        ["xwininfo", "-root", "-tree"],
        check=True,
        capture_output=True,
        text=True,
    )
    return {wid: (title, window_class) for wid, title, window_class in WINDOW_RE.findall(result.stdout)}


def channel(pixel: int, mask: int) -> int:
    shift = (mask & -mask).bit_length() - 1
    maximum = mask >> shift
    return ((pixel & mask) >> shift) * 255 // maximum


def png_chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(
        ">I", zlib.crc32(kind + data)
    )


def xwd_to_png(xwd: bytes) -> tuple[bytes, bool]:
    header = struct.unpack(">25I", xwd[:100])
    header_size, version, pixel_format, depth, width, height = header[:6]
    byte_order, bits_per_pixel, bytes_per_line = header[7], header[11], header[12]
    red_mask, green_mask, blue_mask, colors = header[14], header[15], header[16], header[19]
    if version != 7 or pixel_format != 2 or depth not in (24, 32):
        raise ValueError("unsupported XWD image format")
    if bits_per_pixel not in (24, 32) or byte_order not in (0, 1):
        raise ValueError("unsupported XWD pixel packing")
    bytes_per_pixel = bits_per_pixel // 8
    pixels = memoryview(xwd)[header_size + colors * 12 :]
    rows = bytearray()
    painted = False
    order = "little" if byte_order == 0 else "big"
    for y in range(height):
        rows.append(0)
        offset = y * bytes_per_line
        for x in range(width):
            start = offset + x * bytes_per_pixel
            pixel = int.from_bytes(pixels[start : start + bytes_per_pixel], order)
            rgb = (
                channel(pixel, red_mask),
                channel(pixel, green_mask),
                channel(pixel, blue_mask),
            )
            painted |= max(rgb) > 10
            rows.extend(rgb)
    png = (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
        + png_chunk(b"IDAT", zlib.compress(bytes(rows), level=9))
        + png_chunk(b"IEND", b"")
    )
    return png, painted


def raise_window(window_id: str) -> None:
    x11 = ctypes.CDLL("libX11.so.6")
    x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XRaiseWindow.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
    x11.XFlush.argtypes = [ctypes.c_void_p]
    x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
    display = x11.XOpenDisplay(None)
    if not display:
        raise RuntimeError("cannot open DISPLAY to raise the example window")
    try:
        x11.XRaiseWindow(display, int(window_id, 16))
        x11.XFlush(display)
    finally:
        x11.XCloseDisplay(display)


def click_window(window_id: str, x: int, y: int) -> None:
    """Use the shared X11 input helper for an interaction capture."""
    subprocess.run(
        ["python3", str(ROOT / "scripts" / "x11_input.py"), str(x), str(y),
         "--window", window_id],
        check=True,
    )


def resize_window(window_id: str, width: int, height: int) -> None:
    x11 = ctypes.CDLL("libX11.so.6")
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XResizeWindow.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_uint, ctypes.c_uint]
    x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
    x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
    display = x11.XOpenDisplay(None)
    if not display:
        raise RuntimeError("cannot open DISPLAY to resize the example window")
    try:
        x11.XResizeWindow(display, int(window_id, 16), width, height)
        x11.XSync(display, 0)
    finally:
        x11.XCloseDisplay(display)


def wheel_window(steps: int) -> None:
    """Scroll over a locked crosshair without warping its pointer or camera."""
    x11 = ctypes.CDLL("libX11.so.6")
    xtst = ctypes.CDLL("libXtst.so.6")
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
    x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
    xtst.XTestFakeButtonEvent.argtypes = [
        ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong
    ]
    display = x11.XOpenDisplay(None)
    if not display:
        raise RuntimeError("cannot open DISPLAY to scroll the example window")
    try:
        for _ in range(steps):
            if not xtst.XTestFakeButtonEvent(display, 5, 1, 0):
                raise RuntimeError("XTest wheel press failed")
            if not xtst.XTestFakeButtonEvent(display, 5, 0, 0):
                raise RuntimeError("XTest wheel release failed")
            x11.XSync(display, 0)
            time.sleep(0.06)
    finally:
        x11.XCloseDisplay(display)


def capture(
    name: str,
    output: Path,
    settle: float,
    click: tuple[int, int] | None = None,
    wheel_down: int = 0,
    resize: tuple[int, int] | None = None,
) -> None:
    binary = BINARIES / name
    if not binary.is_file():
        raise FileNotFoundError(f"{binary} is missing; run cargo build -p bbui --examples --locked")
    before = set(windows())
    log_dir = Path("/tmp/bbui-example-capture-logs")
    log_dir.mkdir(exist_ok=True)
    with (log_dir / f"{name}.log").open("wb") as log:
        process = subprocess.Popen(
            [str(binary)],
            cwd=ROOT,
            env={
                **os.environ,
                "BEVY_ASSET_ROOT": str(ROOT),
                "WGPU_BACKEND": os.environ.get("WGPU_BACKEND", "vulkan"),
            },
            stdout=log,
            stderr=subprocess.STDOUT,
        )
        try:
            deadline = time.monotonic() + 20.0
            window = None
            while time.monotonic() < deadline:
                found = windows()
                fresh = [
                    (wid, title)
                    for wid, (title, window_class) in found.items()
                    if wid not in before and window_class == name
                ]
                if fresh:
                    window = fresh[0]
                    break
                if process.poll() is not None:
                    raise RuntimeError(f"{name} exited before opening a window")
                time.sleep(0.1)
            if window is None:
                raise TimeoutError(f"{name} did not open a window")
            if resize is not None:
                resize_window(window[0], *resize)
            time.sleep(settle)
            raise_window(window[0])
            if click is not None:
                click_window(window[0], *click)
            if wheel_down:
                wheel_window(wheel_down)
            time.sleep(0.6 if resize is not None else 0.2)
            for _ in range(10):
                result = subprocess.run(
                    ["xwd", "-id", window[0]], check=True, capture_output=True
                )
                png, painted = xwd_to_png(result.stdout)
                if painted:
                    output.write_bytes(png)
                    break
                time.sleep(1.0)
            else:
                raise RuntimeError(f"{name} remained black after rendering")
            print(f"{name}: {window[1]} -> {output}", flush=True)
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "docs" / "visuals" / "examples")
    parser.add_argument("--settle", type=float, default=1.5)
    parser.add_argument("--click", metavar="X,Y", help="Click at window-local coordinates before capture")
    parser.add_argument("--resize", metavar="WIDTHxHEIGHT", help="Resize the example window before capture")
    parser.add_argument(
        "--wheel-down", type=int, default=0, metavar="N",
        help="Scroll N steps before capture without moving the pointer",
    )
    parser.add_argument("examples", nargs="*")
    args = parser.parse_args()
    click = None
    if args.click:
        try:
            click = tuple(int(part) for part in args.click.split(","))
        except ValueError:
            parser.error("--click must be X,Y integer coordinates")
        if len(click) != 2:
            parser.error("--click must be X,Y integer coordinates")
    if "DISPLAY" not in os.environ:
        parser.error("DISPLAY must point to an X11 display")
    if args.wheel_down < 0:
        parser.error("--wheel-down must be nonnegative")
    resize = None
    if args.resize:
        try:
            resize = tuple(int(part) for part in args.resize.lower().split("x"))
        except ValueError:
            parser.error("--resize must be WIDTHxHEIGHT")
        if len(resize) != 2 or min(resize) <= 0:
            parser.error("--resize must contain positive WIDTH and HEIGHT")
    names = args.examples or sorted(path.stem for path in EXAMPLES.glob("*.rs") if path.stem not in EXCLUDE)
    args.output.mkdir(parents=True, exist_ok=True)
    failures = []
    for name in names:
        try:
            capture(name, args.output / f"{name}.png", args.settle, click, args.wheel_down, resize)
        except (FileNotFoundError, RuntimeError, TimeoutError, subprocess.CalledProcessError, ValueError) as error:
            failures.append(f"{name}: {error}")
            print(f"FAILED {failures[-1]}", flush=True)
    if failures:
        raise SystemExit("\n".join(failures))


if __name__ == "__main__":
    main()
