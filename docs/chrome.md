# Window chrome

`DesktopChrome` builds a BBUI title and command row. The host forwards a primary press to `handle_chrome_press` to start native window movement at the press edge. `chrome_action` translates control activations into close, minimize, and maximize requests. The host decides how to handle close requests, including unsaved work. On macOS, the row reserves space for native traffic lights.

`view_with` accepts `DesktopChromeAppearance` for titlebar height, padding, spacing, labels, control width, native button inset, control visibility, and themed styles for the surface, title, menu, and window controls. `view_with_slots` adds host content before or after the drag region and at the horizontal center of the titlebar. Controls retain stable keys for routing. Set `close_style` separately when the close action needs a distinct treatment.

Pass a BBUI tree as `DesktopChromeSlots.center`. It occupies a `center_width`-pixel region centered across the whole titlebar. The widget can be a text label, a progress bar, or a composition such as `WidgetNode::row([WidgetNode::label("Exporting"), WidgetNode::progress(0.65).with_width(UiLength::Px(78.0)), WidgetNode::label("65%")])`. Primary presses on noninteractive descendants start native window dragging; focusable controls keep their own input. The host styles and updates the widget and sets `center_width` to fit its content. For centered text that fits a narrow window, give a label the `centered-ellipsis-label` role and place it in a row with `UiAlign::Center`. The drag title also truncates within its available width.

The default minimize, maximize, and close marks are drawn as centered geometry inside equal-sized controls. Their size follows `control_style` / `close_style` font size. Replacing a control label in the appearance uses centered text instead, while its accessible name and action key remain stable.

`MobileChrome` lays out four host-owned widget regions through `MobileChromeSlots`: `toolbar`, `content`, optional `sheet`, and `navigation`. The shell supplies themed surfaces and dimensions; hosts compose every label, icon, menu, button, selected state, and sheet header inside those regions. Set `chrome:mobile:commands` on a command menu, `chrome:mobile:sheet-close` on a dismiss control, and `chrome:mobile:nav:<id>` on a navigation control when using `chrome_action`. The host owns the active navigation ID and decides when to show or dismiss the sheet. The overlay backdrop has the `chrome:mobile:sheet-backdrop` action key and also returns `DismissSheet` when activated.

`MobileChromeAppearance` controls toolbar and navigation height, sheet padding and spacing, surface styles, and `safe_area`. Supply the platform's logical-pixel insets: the top is reserved in the toolbar, the sides are reserved in toolbar, content, sheet, and navigation, and the bottom is reserved in navigation or an open overlay sheet. Invalid and negative insets become zero. The default `MobileSheetPresentation::Overlay` paints a full-screen dimming backdrop and raises the sheet above page content; `Inline` keeps the sheet in the page column. `scrim_style` and `sheet_style` control their paint, including opacity and shadow. Pass the appearance to `view_with`. Use `compact_chrome(window.resolution.width())` to switch at 600 logical pixels or `compact_chrome_at(width, breakpoint)` for a host-selected breakpoint.

Use the `ellipsis-label` or `centered-ellipsis-label` role for bounded labels. Chrome navigation buttons with the `chrome-mobile-nav-item` role and menus with the `chrome-mobile-menu` role also shorten long text to fit. The underlying widget keeps the full label for accessibility and action routing. The example composes a two-line toolbar, a material sheet, and navigation buttons with explicit selected state; none of those contents are built into the shell.

| Mode | Dark theme | Light theme |
| --- | --- | --- |
| Desktop editor | ![Desktop chrome around the editor in dark theme](visuals/examples/chrome-desktop-dark.png) | ![Desktop chrome around the editor in light theme](visuals/examples/chrome-desktop-light.png) |
| Centered progress | ![A progress indicator in the draggable chrome center in dark theme](visuals/examples/chrome-desktop-progress-dark.png) | ![A progress indicator in the draggable chrome center in light theme](visuals/examples/chrome-desktop-progress-light.png) |
| Compact editor | ![Compact chrome and material sheet in dark theme](visuals/examples/chrome-mobile-dark.png) | ![Compact chrome and material sheet in light theme](visuals/examples/chrome-mobile-light.png) |
| Compact, sheet closed | [Preview](visuals/examples/chrome-mobile-closed-dark.png) | [Preview](visuals/examples/chrome-mobile-closed-light.png) |
| Compact, safe insets | [Preview](visuals/examples/chrome-mobile-insets-dark.png) | [Preview](visuals/examples/chrome-mobile-insets-light.png) |
| Overlay sheet, safe insets | [Preview](visuals/examples/chrome-mobile-insets-open-dark.png) | [Preview](visuals/examples/chrome-mobile-insets-open-light.png) |
| Compact, long labels | [Preview](visuals/examples/chrome-mobile-long-dark.png) | [Preview](visuals/examples/chrome-mobile-long-light.png) |
| Desktop control hover | [Preview](visuals/examples/chrome-desktop-hover-dark.png) | [Preview](visuals/examples/chrome-desktop-hover-light.png) |
| Compact navigation hover | [Preview](visuals/examples/chrome-mobile-hover-dark.png) | [Preview](visuals/examples/chrome-mobile-hover-light.png) |
| Window controls, enlarged | ![Centered controls in dark theme](visuals/examples/chrome-controls-dark.png) | ![Centered controls in light theme](visuals/examples/chrome-controls-light.png) |

Run `cargo run -p bbui --example chrome_shell` for the desktop composition. The compact composition uses the same example at a 390-pixel window width. BBUI maps a primary touch to pointer events and ignores additional fingers until the active touch ends or is cancelled.

The library is checked for `aarch64-apple-ios` and `aarch64-linux-android` in CI. Embedding requires a host app to create a Bevy window and mount the chrome tree; BBUI does not package mobile app bundles. Device behavior still depends on the host's window integration.

Run `cargo bench -p bbui --bench performance --locked -- chrome` to measure reconciliation while opening and closing a mobile navigation sheet.
