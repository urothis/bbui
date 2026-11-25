# Window chrome

`DesktopChrome` builds a BBUI title and command row. The host forwards a primary press to `handle_chrome_press` to start native window movement at the press edge. `chrome_action` translates control activations into close, minimize, and maximize requests. The host decides how to handle close requests, including unsaved work. On macOS, the row reserves space for native traffic lights.

`view_with` accepts `DesktopChromeAppearance` for titlebar height, padding, spacing, labels, control width, native button inset, control visibility, and themed styles for the surface, title, menu, and window controls. `view_with_slots` adds host content before or after the drag region and at the horizontal center of the titlebar. Controls retain stable keys for routing. Set `close_style` separately when the close action needs a distinct treatment.

Pass a BBUI tree as `DesktopChromeSlots.center`. It occupies a `center_width`-pixel region centered across the whole titlebar. The widget can be a text label, a progress bar, or a composition such as `WidgetNode::row([WidgetNode::label("Exporting"), WidgetNode::progress(0.65).with_width(UiLength::Px(78.0)), WidgetNode::label("65%")])`. Primary presses on noninteractive descendants start native window dragging; focusable controls keep their own input. The host styles and updates the widget and sets `center_width` to fit its content. For centered text, give a label the `centered-label` role and place it in a row with `UiAlign::Center` so it is centered on both axes.

The default minimize, maximize, and close marks are drawn as centered geometry inside equal-sized controls. Their size follows `control_style` / `close_style` font size. Replacing a control label in the appearance uses centered text instead, while its accessible name and action key remain stable.

`MobileChrome` lays out four host-owned widget regions through `MobileChromeSlots`: `toolbar`, `content`, optional `sheet`, and `navigation`. The shell supplies themed surfaces and dimensions; hosts compose every label, icon, menu, button, selected state, and sheet header inside those regions. Set `chrome:mobile:commands` on a command menu, `chrome:mobile:sheet-close` on a dismiss control, and `chrome:mobile:nav:<id>` on a navigation control when using `chrome_action`. The host owns the active navigation ID and decides when to show or dismiss the sheet.

`MobileChromeAppearance` controls toolbar and navigation height, sheet padding and spacing, and the three surface styles. Pass it to `view_with`. Use `compact_chrome(window.resolution.width())` to switch at 600 logical pixels or `compact_chrome_at(width, breakpoint)` for a host-selected breakpoint. The example composes a two-line toolbar, a material sheet, and navigation buttons with explicit selected state; none of those contents are built into the shell.

| Mode | Dark theme | Light theme |
| --- | --- | --- |
| Desktop editor | ![Desktop chrome around the editor in dark theme](visuals/examples/chrome-desktop-dark.png) | ![Desktop chrome around the editor in light theme](visuals/examples/chrome-desktop-light.png) |
| Centered progress | ![A progress indicator in the draggable chrome center in dark theme](visuals/examples/chrome-desktop-progress-dark.png) | ![A progress indicator in the draggable chrome center in light theme](visuals/examples/chrome-desktop-progress-light.png) |
| Compact editor | ![Compact chrome and material sheet in dark theme](visuals/examples/chrome-mobile-dark.png) | ![Compact chrome and material sheet in light theme](visuals/examples/chrome-mobile-light.png) |
| Desktop control hover | [Preview](visuals/examples/chrome-desktop-hover-dark.png) | [Preview](visuals/examples/chrome-desktop-hover-light.png) |
| Compact navigation hover | [Preview](visuals/examples/chrome-mobile-hover-dark.png) | [Preview](visuals/examples/chrome-mobile-hover-light.png) |
| Window controls, enlarged | ![Centered controls in dark theme](visuals/examples/chrome-controls-dark.png) | ![Centered controls in light theme](visuals/examples/chrome-controls-light.png) |

Run `cargo run -p bbui --example chrome_shell` for the desktop composition. The compact composition uses the same example at a 390-pixel window width. BBUI maps a primary touch to pointer events and ignores additional fingers until the active touch ends or is cancelled.

The library is checked for `aarch64-apple-ios` and `aarch64-linux-android` in CI. Embedding requires a host app to create a Bevy window and mount the chrome tree; BBUI does not package mobile app bundles. Device behavior still depends on the host's window integration.

Run `cargo bench -p bbui --bench performance --locked -- chrome` to measure reconciliation while opening and closing a mobile navigation sheet.
