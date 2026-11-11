# Keyboard and pointer shortcuts

The focused control receives keyboard input. Disabled and hidden controls are skipped. On macOS, text and undo shortcuts also accept Command.

| Control | Input | Effect |
| --- | --- | --- |
| All controls | Tab / Shift+Tab | Move through visible focus targets |
| All editors | Ctrl+Z / Ctrl+Shift+Z | Undo or redo; focused text uses local history first |
| Buttons, toggles, panels | Enter / Space | Activate, toggle, or expand |
| Numeric field | Horizontal drag | Scrub with hidden continuous cursor; release commits |
| Numeric field | Shift / Ctrl while dragging | Precision / step snapping |
| Numeric field | Ctrl+Wheel, Ctrl+C/V, Backspace | Step, copy/paste full value, reset |
| Numeric field | Double-click and type | Evaluate an expression such as `2*pi` or `x if c else y` |
| Numeric group | Drag vertically, then horizontally | Select compatible fields and scrub together |
| Slider | Arrows / Home / End | Step or jump to bounds |
| Dropdown | Enter, arrows, Page, Home/End, Escape | Open, navigate, choose, or cancel |
| Menu | Arrows, Enter, typed search, Escape | Navigate, activate, fuzzy-search, or close |
| Reorderable panel | Header drag / Ctrl+Arrow | Reorder and commit one undo step |
| Canvas | Middle-drag, wheel, arrows, Delete | Pan, zoom, nudge, remove selection |
| Node editor | Socket drag, Alt-drag, middle-drag, Ctrl+Wheel | Link, cut links, pan, zoom |
| File browser | Arrows, Page, Home/End, Ctrl+A, Enter, Backspace | Navigate and select visible entries |
| FPS world UI example | WASD, mouse, F1, Escape | Move, aim, toggle cursor capture, cancel UI edit |

Escape also cancels a numeric scrub, text edit, panel reorder, or active canvas/graph gesture. Text fields support word navigation/deletion, selection, clipboard, IME composition, and local undo. The [visual gallery](visuals/README.md) shows menus, controls, and editors in use.
