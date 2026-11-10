# BUI for Bevy

This repository is an in-progress, original Rust implementation of Blender-style UI behavior for Bevy. The accepted feature contract is [the feature inventory](docs/feature-inventory.md), and the cross-crate change propagation rules are in the [UI invalidation contract](docs/invalidation.md). The workspace now includes a Bevy input bridge, shared layout and hit testing, a GPU UI renderer, a widget tree, and an interactive showcase. The complete feature inventory remains a work in progress; unchecked items require their full behavior, tests, and examples.

## Workspace

- `bui`: public Bevy plugin and prelude.
- `bui_core`: UI hierarchy, input routing, focus, and history.
- `bui_layout`: measurement, layout, clipping, and hit-test bounds.
- `bui_style`: typed Dark/Light theme tokens and style resolution.
- `bui_text`: numeric expression parsing and text editing.
- `bui_render`: Bevy UI GPU backend, draw commands, and world surfaces.
- `bui_media`: raster, SVG, and video decoding support.
- `bui_widgets`: controls, editors, interactions, and declarative trees.
- `bui_macros`: `bui!` Rust UI declaration macro.

Implementation is organized by responsibility within each crate. For example,
`bui_widgets/src/actions/` contains event handling, widget interactions, view synchronization,
and `paint/` renderers; `bui_render/src/` separates draw data from incremental and full GPU
synchronization; `bui_media/src/` keeps each decoder in its own module. Large unit-test suites
live beside their parent modules in `tests/` folders. Add behavior to the owning module and
keep its tests with that module.

## Development

Use Rust 1.99 or newer. Dependencies are pinned through `Cargo.lock`, including Bevy 0.20.1.

Every crate inherits the workspace lint policy in `Cargo.toml`. It forbids unsafe Rust and
denies `unwrap`, `expect`, `panic`, `unreachable`, `todo`, `unimplemented`, and direct process
exit calls. Handle fallible work with `Result`, `Option`, or an explicit logged fallback.
Clippy permits assertion-style `unwrap`, `expect`, and `panic` inside tests; test helpers
must be marked `#[cfg(test)]`. The all-targets Clippy command below checks the policy.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked
cargo test --workspace --all-targets --locked
cargo test -p bui --test gpu_readback --locked # offscreen WGPU pixel check
cargo test -p bui --test icon_asset_gpu_readback --locked # asset lifetime and tint pixels
cargo run -p bui --example showcase
cargo run -p bui --example controls
cargo run -p bui --example sliders
cargo run -p bui --example layout_gallery
cargo run -p bui --example numeric_group
cargo run -p bui --example numeric_expressions
cargo run -p bui --example accessibility_live
cargo run -p bui --example icon_assets
cargo run -p bui --example panel_reorder
cargo run -p bui_style --example theme_tokens
cargo run -p bui_text --example font_asset_recovery
cargo run -p bui --example file_browser
cargo run -p bui --example color_ramp
cargo run -p bui --example hsv_color
cargo run -p bui --example curve_editor
cargo run -p bui --example collections
cargo run -p bui --example data_table
cargo run -p bui --example canvas
cargo run -p bui --example node_graph
cargo run -p bui --example text_area
cargo run -p bui --example text_history
cargo run -p bui_layout --example scrolling
cargo run -p bui_layout --example responsive
cargo run -p bui_render --example world_surface
cargo run -p bui_media --example media_playback
cargo run -p bui_render --example render_sync_bench --release --locked
cargo run -p bui_widgets --example canvas_stress --release --locked

# Statistical CPU benchmarks and HTML report
cargo bench -p bui --bench performance --locked
```

The project intentionally contains no C port, software rasterizer, or other-language bindings. See the feature inventory for the intended full implementation and its completion rule.
See [Keyboard and Pointer Shortcuts](docs/shortcuts.md) for the tested interaction map.
See [Accessibility](docs/accessibility.md) for AccessKit roles, focus, live announcements, and screen-reader actions.

The `showcase` example mounts the control, menu, panel, color, collection, table, canvas,
node, browser, font, icon, and animated-media families in one scrollable 1280×760 window.
The right strip plays the bundled synthetic GIF, VP8 WebM, and H.264 MP4 fixtures.
Press F6 to switch Dark/Light themes and F7 to toggle reduced motion.

## Desktop targets

The workspace targets Linux (X11 and Wayland), Windows, and macOS through Bevy 0.20.1. `.github/workflows/ci.yml` is configured to run formatting, strict Clippy, rustdoc, and all-target tests on those three desktop runners; a Linux Mesa Vulkan job runs offscreen GPU pixel checks. Local GPU tests use an image render target and do not need a visible desktop session. The properties editor also has a WebAssembly browser build. Android and the other examples are outside the current browser support contract.

## Properties editor in a browser

Install the `wasm32-unknown-unknown` Rust target, `wasm-bindgen-cli` version `0.2.130`, and `llvm-strip` from your LLVM tools package, then build and serve the example:

```sh
rustup target add wasm32-unknown-unknown
cargo install --force wasm-bindgen-cli --version 0.2.130 --locked
scripts/build_properties_editor_web.sh
python3 -m http.server 8000 --directory target/web
```

Open `http://localhost:8000/properties_editor/` in a WebGL2-capable browser. The page resizes the Bevy canvas to the browser viewport and scrolls when the viewport is smaller than the editor's 520×690 baseline. The object, material, outliner, preferences, and graph tabs share the desktop example's controls. Serve the files over HTTP so the browser can load the generated JavaScript and WASM bundle. See the [browser screenshot](docs/visuals/examples/properties_editor_web.png) and [geometry nodes screenshot](docs/visuals/examples/properties_editor_web_nodes.png).

## Text editing

Text fields and areas navigate and delete whole Unicode grapheme clusters, so combining marks and joined emoji remain intact. Use Ctrl (Command on macOS) with Arrow to move by word, Backspace/Delete to delete a word, and Home/End for document edges. Shift with navigation keys extends selection. Ctrl+Z/Ctrl+Shift+Z edit the focused text buffer before falling back to application history; typing pauses longer than 750 ms start a new local undo step. IME composition stays separate until committed.

Leaving a text editor commits the session into shared undo history. Global undo from another control restores the prior text and directed selection; redo restores the committed text. Enter also commits a single-line field. Run `cargo run -p bui --example text_history` to exercise both editors and the history handoff.

Attach `UiFontAsset::new("fonts/interface.ttf")` to a text entity or an ancestor to load a Bevy font for both shaping and drawing. `UiFontAssetState` reports `Hidden`, `Loading`, `Ready`, or `Failed(reason)`; the default font remains usable while a requested font is unavailable. Change the path or call `UiFontAsset::retry()` to request a failed file again. Hidden or clipped owners release BUI's strong handle. Run `cargo run -p bui_text --example font_asset_recovery` for a missing-file, fallback, and retry demonstration.

## Controls example

Run `cargo run -p bui --example controls` to see buttons, icon buttons, checkboxes, a radio group, dropdown, text fields and area, progress indicator, labels, and delayed tooltip in one tree. Radio groups select with pointer clicks or Arrow keys and Space/Enter; each change emits `WidgetEvent::ValueChanged`. Drag across checkboxes to paint their first new state; Escape cancels the brush and Ctrl+Z undoes a completed brush in one step. Disabled buttons do not activate, read-only text remains selectable and navigable without accepting edits, and icon buttons expose their accessible name while painting the supplied glyph.

Dropdowns open toward the available viewport space and scroll long option lists; only rows inside the popup viewport are visible and hit-testable. Space or Enter opens the popup; Arrow, Page, Home, and End move the highlight; Enter chooses it and Escape cancels. Each radio and dropdown option has its own accessible name, position, and selection state.

Image-backed icon buttons use `WidgetNode::icon_button_asset("icons/save.svg", "Save")` or `icon_button_asset(...)` in `bui!`. Attach `IconButtonAsset` to configure a source, and read `IconButtonAssetState` for `Hidden`, `Loading`, `Ready`, or `Failed(reason)`. Call `IconButtonAsset::retry()` after replacing a failed source. The icon uses the current theme tint; its strong Bevy handle and GPU registration are released when the widget leaves the visible layout. Run `cargo run -p bui --example icon_assets` to see a loaded SVG, missing-asset error, and glyph-only button together.

## Reorderable panel example

Run `cargo run -p bui --example panel_reorder`. Put panels under a `PanelGroup::with_panels([...]).with_reordering("stable-group-key")` and give each panel a stable `UiKey` or `Panel.persistence_key`. Drag panel headers or focus a panel and press Ctrl+ArrowUp/Down; both paths update the sibling layout and emit one `PanelOrderCommitted` history event. Escape or pointer cancellation restores the order from the start of the drag. `PanelOrderStore` restores the keyed order when the panel tree is rebuilt. Undo rejects an order snapshot whose stable key set no longer matches the live group and leaves that history step available.

## Sliders

Run `cargo run -p bui --example sliders` to see floating point and integer range sliders. Declare a reusable model with `Slider::new(value, min, max, step)` and `slider("Opacity", model)`, or write `slider_range("Opacity", 0.72, 0.0, 1.0, 0.01)` and `integer_slider_range("Grid size", 16, 1, 64, 1)` in `bui!`. Pointer press and drag move the handle; release emits one `NumericCommitted` event and one undo step, while Escape restores the value from drag start. Arrow keys step, Home/End jump to the bounds, and Ctrl+Wheel steps the focused slider. Disabled and read-only sliders ignore pointer, keyboard, wheel, and action updates. Slider models share numeric field bounds, defaults, units, validation, and value restoration.

## Numeric group example

Run `cargo run -p bui --example numeric_group`. Press and drag vertically across compatible numeric fields to select them, then move horizontally to scrub the group. Shift applies precision movement, Control snaps each field to its own step, and Escape restores every value from the start of the gesture. A completed drag is one undoable transaction.

Numeric fields also support Arrow Up/Down adjustment, Ctrl+Wheel stepping, Ctrl+C/Ctrl+V value copy and paste, and Backspace reset to the configured default. Invalid pasted expressions keep the previous value until corrected or cancelled.

## Numeric expressions

Run `cargo run -p bui --example numeric_expressions`. Double-click a number and type an expression such as `2*pi`, `min(4, 3)`, or `x if c else y`; Enter commits and Escape cancels. Attach `NumericExpressionVariables` to a field entity to supply application variables like `c` and `y`. The field's current value always supplies `x`. Parse, domain, overflow, and division errors keep the value and input intact, paint an error outline, and emit `WidgetEvent::NumericExpressionError` with the reason.
The error reason also becomes an assertive screen-reader announcement until the input is corrected or cancelled.

The example's tiny field shows a nonzero value in scientific notation. `NumericField::precision` controls significant display digits; copied float values retain their full precision and exclude the display unit.

## File browser example

The file browser example opens the current directory. Use **Path** and **Go** to open a directory directly, or choose an ancestor or saved bookmark from **Ancestors and bookmarks**. **Bookmark** saves the current directory and **Remove bookmark** removes it. Type in **Filter** to narrow entries, select rows with Ctrl or Shift, and use the toolbar for sorting, list/grid view, and file operations. Arrow keys move the cursor, Page Up/Down move a viewport, Enter opens the selection, Ctrl+A selects all visible entries, and Home/End jump to the first/last visible entry. Backspace goes up when the filter is empty; Escape clears the filter. **Name** supplies a new folder or rename target; **Destination** supplies an existing directory for copy or move. Filesystem errors appear in the status line.

Library callers read browser state with `entries()`, `filter()`, `sort()`, and `ascending()`. Change the filter and sort with `set_filter` and `set_sort`, and update the directory snapshot with navigation methods or `refresh`; these methods keep the indexed visible view consistent.

Browser navigation, bookmarks, selection, filter, sort, and view changes have undo/redo snapshots. Restoring a missing directory reports an error and keeps the undo step available. File creation, deletion, copy, move, and rename are external filesystem changes and emit explicit irreversible-operation events.

## Menus and popups example

Run `cargo run -p bui --example menus`. The `bui!` macro supports `menu_bar`, `menu`, `context_menu`, `popover`, `pie_menu`, and `tooltip` declarations; menu entries can contain nested submenus and shortcuts. Click to open and pick, or press and drag through a menu before releasing on an entry. Arrow keys move through entries, Enter activates the selection, and Escape closes the popup and restores focus. Type while a menu is open to search nested entries by word initials or approximate spelling. Recent choices rank within that menu instance; queue `WidgetActionKind::ClearMenuHistory` to clear its history. Right-click the context-menu trigger to place its popup at the pointer, and move radially in the pie menu to choose a slice.

## Properties editor example

Run `cargo run -p bui --example properties_editor` for a compact object-properties panel inspired by Blender. Drag the Location and Rotation fields, adjust Scale, switch modes, toggle visibility options, and collapse sections. The left rail switches between Object, Modifiers, Material, Outliner, Preferences, and Geometry Nodes pages. The menu bar and progress footer complete the reference-sized composition.

Reviewed screenshots of every graphical example, including open menu states, are in [docs/visuals](docs/visuals/README.md).

## Canvas example

Run `cargo run -p bui --example canvas` from the workspace root. Left-click selects a shape; Ctrl/Shift-click adds to the selection. Drag selected shapes, middle-drag to pan, and use the wheel to zoom around the cursor. Arrow keys nudge selected shapes (Shift: 10 units; Ctrl: 0.1), Page Up/Down zoom, and Home resets the view. Escape cancels a drag, Delete removes selected objects, and Ctrl+Z/Ctrl+Shift+Z undo and redo edits. The scene draws a rectangle, ellipse, line, and SVG-backed image from `assets/canvas/checker.svg`.

## Node editor example

Run `cargo run -p bui --example node_graph`. Drag node headers to move them and compatible sockets to create or reconnect links. Drag across empty space for box selection, Alt-drag across links to cut them, and middle-drag to pan. Ctrl+Wheel zooms; Ctrl+A selects nodes; C collapses the selection; N adds a node; arrow keys move selected nodes; Delete removes the selection. Ctrl+Z/Ctrl+Shift+Z undo and redo graph edits. The model rejects incompatible socket links and serializes to validated JSON.

Reviewed Dark/Light offscreen captures of the four-node example are in [docs/visuals](docs/visuals/README.md).

For screen-reader link creation, activate one named socket and then a compatible socket of the opposite direction; either order works. Activating the same socket twice cancels the pending pair. The generated link uses the graph's normal validation and undo path.

## Themes and layout

`Theme` supplies typed Dark/Light color and spacing tokens for controls, panels, menus, lists, tables, and editors. `UiInlineStyle` and `UiStyleSheet` resolve widget state through the same cascade; `set_ui_style_state` updates a state without rebuilding the tree. `UiLayoutNode::flex` and `UiLayoutNode::responsive_grid` provide wrapping and width-based columns, while `UiScroll` handles clipped wheel and keyboard scrolling.

`Theme.motion` supplies typed fast, standard, slow, panel, and menu durations. Use `transitions.set_motion(property, &theme, ThemeMotionToken::Standard)` for theme-aware style transitions; `UiReducedMotion(true)` snaps them to their target values.
See [Theme values and styling](docs/theme.md) for the Dark/Light palettes, spacing groups, motion durations, and cascade order.

`WidgetNode` and `bui!` also expose `overlay`, `grid`, `responsive_grid`, `flex_row`, `flex_column`, `inline`, `boxed`, `property_row`, and `scroll` containers. Use `with_width`, `with_height`, `with_gap`, `with_style`, or `with_layout` for a particular node; `property_row` and `boxed` take their spacing from `Theme.spacing.property_row` and `Theme.spacing.box_container`. Run `cargo run -p bui --example layout_gallery` and resize the window to inspect these layouts.
Rows, columns, grids, flex, and inline flows take their gap from `Theme.spacing.control.gap`, including after a live theme change.
