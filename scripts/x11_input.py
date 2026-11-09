#!/usr/bin/env python3
"""Send one X11 pointer click or press-drag-release on the current DISPLAY.

Examples: `python3 scripts/x11_input.py 100 80` or
`python3 scripts/x11_input.py 100 80 --drag 170 120`.
Requires libX11 and libXtst; useful for checking example interactions under Xvfb.
"""

import argparse
import ctypes
import time


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("x", type=int)
    parser.add_argument("y", type=int)
    parser.add_argument("--button", type=int, default=1)
    parser.add_argument("--drag", nargs=2, type=int, metavar=("X", "Y"))
    parser.add_argument("--move-only", action="store_true", help="Move the pointer without clicking")
    parser.add_argument("--window", help="X11 window ID to raise and focus before input")
    args = parser.parse_args()

    x11 = ctypes.CDLL("libX11.so.6")
    xtst = ctypes.CDLL("libXtst.so.6")
    x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
    x11.XFlush.argtypes = [ctypes.c_void_p]
    x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
    x11.XRaiseWindow.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
    x11.XSetInputFocus.argtypes = [
        ctypes.c_void_p,
        ctypes.c_ulong,
        ctypes.c_int,
        ctypes.c_ulong,
    ]
    xtst.XTestFakeMotionEvent.argtypes = [
        ctypes.c_void_p,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_ulong,
    ]
    xtst.XTestFakeButtonEvent.argtypes = [
        ctypes.c_void_p,
        ctypes.c_uint,
        ctypes.c_int,
        ctypes.c_ulong,
    ]
    xtst.XTestFakeMotionEvent.restype = ctypes.c_int
    xtst.XTestFakeButtonEvent.restype = ctypes.c_int
    display = x11.XOpenDisplay(None)
    if not display:
        raise SystemExit("Cannot open DISPLAY")
    try:
        if args.window:
            window = int(args.window, 0)
            x11.XRaiseWindow(display, window)
            x11.XSetInputFocus(display, window, 1, 0)
            x11.XSync(display, 0)
        if not xtst.XTestFakeMotionEvent(display, -1, args.x, args.y, 0):
            raise RuntimeError("XTest pointer motion failed")
        x11.XSync(display, 0)
        time.sleep(0.08)
        if args.move_only:
            return
        if not xtst.XTestFakeButtonEvent(display, args.button, 1, 0):
            raise RuntimeError("XTest button down failed")
        x11.XSync(display, 0)
        time.sleep(0.12)
        if args.drag:
            if not xtst.XTestFakeMotionEvent(display, -1, args.drag[0], args.drag[1], 0):
                raise RuntimeError("XTest drag motion failed")
            x11.XSync(display, 0)
            time.sleep(0.12)
        if not xtst.XTestFakeButtonEvent(display, args.button, 0, 0):
            raise RuntimeError("XTest button up failed")
        x11.XSync(display, 0)
    finally:
        x11.XCloseDisplay(display)


if __name__ == "__main__":
    main()
