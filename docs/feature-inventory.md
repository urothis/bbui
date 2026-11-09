# Feature Inventory and Complete TODO

This is the scope for a Rust UI framework running inside Bevy. Check an item only when its behavior works end to end, has automated coverage, and appears in a runnable example. The intended result is Blender-style widgets and interactions, not a port of BUI's C runtime. Rendering uses Bevy's GPU pipeline; there is no software UI rasterizer, separate window host, C ABI, or Python/JavaScript/Qt/ImGui adapter.

Current implementation details are in [Architecture](architecture.md), and measured renderer CPU baselines are in [Renderer CPU Baseline](performance.md). The checkboxes below remain the completion gate for each whole feature family.

## Foundation: work every feature depends on

- [x] Establish a Cargo workspace with `bui` (public plugin and prelude), `bui_core`, `bui_layout`, `bui_style`, `bui_text`, `bui_render`, `bui_media`, `bui_widgets`, and `bui_macros`; keep examples, tests, benchmark harnesses, and assets with their owning crates, and shared documentation in `docs/`.
- [x] Define stable entity/component ownership for UI hierarchy, widget values, enabled/visible state, focus, selection, overlays, and per-window state. Specify system ordering and the data passed to Bevy's render world.
- [x] Implement measurement and layout for rows, columns, aligned controls, property splits, boxes, panels, flex/grid, inline text, scrolling, clipping, overlays, and DPI changes. Layout, paint, and hit testing must agree on bounds and stacking order.
- [x] Implement pointer and keyboard dispatch, capture, hover, drag thresholds, double-clicks, focus traversal, cursors, clipboard, timers, and cancellation. Define behavior when a widget is removed or disabled during an interaction.
- [x] Implement a transaction-based undo/redo service covering every UI value edit. Define transaction boundaries for typing, scrubbing, multi-field changes, checkbox painting, colors, curves, ramps, lists, trees, canvas edits, and node edits; restore focus and selection where relevant.
- [x] Implement Bevy-native GPU drawing for widget shapes, text, icons, images, curves, links, clipping, shadows, and overlays. Include batching, atlas lifetime, resize handling, color correctness, and damage-aware updates without an independent CPU raster path.
- [x] Implement text measurement, shaping, font fallback, selection, caret navigation, clipboard, multiline editing, and IME composition. Use the same text metrics for layout, hit testing, and drawing.
- [x] Define keyboard and screen-reader semantics for controls and editors, including names, values, enabled state, focus order, and change announcements. Verify pointer, keyboard, and accessibility operation for each widget.

## Numeric fields and sliders

- [x] Support bounded and unbounded integer and float fields, sliders, defaults, units where configured, validation, and consistent value-change events.
- [x] Drag horizontally to scrub with a hidden, continuous cursor. Lock the cursor to avoid screen-edge limits and restore its original position on commit or cancel. Preserve the drag across high-DPI and multi-monitor scale changes.
- [x] Apply Shift precision and Ctrl snapping during a drag, including modifier changes mid-drag. Escape restores the value from drag start; pointer release commits one undo transaction.
- [x] Support Ctrl+Wheel stepping, Ctrl+C/Ctrl+V of the displayed value, Backspace reset to the configured default, and keyboard-only value adjustment. Reject invalid pasted values without corrupting state.
- [x] Implement Blender-compatible numeric expression evaluation **in Rust**: constants, arithmetic, precedence, parentheses, supported functions, variables such as `x`, and conditional expressions. Cover `2*pi`, `min(4, 3)`, `round(1.7)`, and `x if c else y`; report parse, domain, overflow, and division errors without committing a bad value.
- [x] Match Blender-style float display precision: readable significant digits, small values that do not collapse to zero, stable display while editing, and round-trip behavior within the field's configured precision.
- [x] Allow vertical selection of compatible numeric fields followed by one horizontal drag that changes them together. Define mixed values, per-field bounds/snapping, cancellation, and a single undo/redo transaction.

## Toggles, panels, and shared controls

- [x] Provide buttons, checkboxes, radio groups, dropdowns, text fields, text areas, progress indicators, labels, icon controls, tooltips, and disabled/read-only states with coherent pointer and keyboard behavior.
- [x] Drag across checkboxes to set every traversed checkbox to the first new state. Visit each checkbox once per drag; support cancellation and one undo/redo transaction.
- [x] Build aligned property rows, nested panels, and collapsible panels with Blender-style header behavior and animation. Support Ctrl+click exclusive expansion, keyboard toggling, reordering where enabled, and persisted expansion state.
- [x] Make Tab and Shift+Tab traverse editable controls in visible order, skip disabled/hidden controls, and handle focus entering/leaving popups and composite editors.
- [x] Make Ctrl+Z/Ctrl+Shift+Z work on every UI edit. Inside a text field, undo typed text within that field before moving to earlier application edits; coalesce a burst of typing into a meaningful step.

## Menus, popups, and search

- [x] Implement menu bars, dropdown/context menus, submenus, popovers, tooltips, and pie menus with placement that respects the viewport and DPI.
- [x] Support press-drag-release selection as well as click-to-open/click-to-pick. Handle hover submenu changes, outside click, Escape, keyboard arrows, Enter, and focus restoration.
- [x] Implement pie-menu directional selection, dead zone, keyboard alternative, and cancellation.
- [x] Implement fuzzy search across nested menu items: word-initial matching (`ad cu` finds `Add Cube`), typo tolerance, deterministic ranking, and recent-choice ranking. Store recency per menu context and define how it is cleared.

## Color, data views, and editors

- [x] Implement an HSV color wheel, value/alpha controls, numeric channel entry, hex entry, color-space conversion, preview, and one undo transaction per gesture.
- [x] Implement editable color ramps: add/remove stops, drag/reorder, interpolation modes, stop colors, keyboard editing, and serialization.
- [x] Implement editable curves: points, handles, interpolation, pan/zoom, selection, keyboard editing, serialization, and undo/redo.
- [x] Implement lists, trees, and preview grids with shared scrolling, selection, multi-selection, keyboard navigation, inline rename, and visible-item virtualization. Tree expansion and drag reordering must preserve identity and selection.
- [x] Implement a data table with column sizing, sorting, selection, keyboard navigation, scrolling, and virtualized rows/cells. Large data sets must not create one Bevy entity per cell.
- [x] Implement multiline text areas with wrapping, selection, navigation, clipboard, scrolling, IME, and text-local undo/redo.
- [x] Implement canvases with pan/zoom, clipping, hit testing, draw primitives, image placement, and editable object selection.
- [x] Implement a node editor with nodes, typed sockets, curved links, pan/zoom, selection, move, link creation/reconnection/removal, box selection, cut links, collapse, keyboard commands, and undo/redo.
- [x] Implement a Blender-style file browser with path navigation, breadcrumbs, bookmarks, search/filter, sorting, list/grid previews, multi-selection, keyboard navigation, file operations, and explicit error handling for filesystem failures.

## Themes, assets, and motion

- [x] Ship Blender-style Dark and Light theme definitions covering every widget state, editor, menu, icon, spacing value, and animation. Switch themes at run time without rebuilding the UI; invalidate only affected visuals.
- [x] Define typed Rust styling plus a coherent cascade for inherited values, state selectors, component overrides, transitions, and effects. Keep widget appearance and layout values in theme/style data rather than scattered constants.
- [x] Support fonts, icons, raster images, SVG, animated images, and video as Bevy assets with clear loading/error states, cache limits, cancellation of off-screen work, and correct GPU resource lifetime. Animation clocks pause when content is not visible.
- [x] Respect reduced-motion settings while preserving usable panel, menu, and editor behavior.

## Verification and completion gate

- [x] Add unit/property tests for layout bounds, expression evaluation, float formatting, search ranking, style resolution, and undo transactions.
- [x] Add interaction tests for pointer capture, modifier changes, cancellation, focus traversal, popup routing, clipboard, IME, and every widget's keyboard path.
- [x] Add GPU visual regression scenes for Dark and Light themes, DPI scales, clipping, text, menus, overlays, and each editor; compare outputs with defined tolerances.
- [x] Add stress benchmarks for thousands of controls, long lists/tables, many nodes/links, image grids, animation, and idle frames. Record CPU time, GPU time, allocations, and memory so regressions are visible.
- [x] Provide a runnable example for every feature family and a single showcase exercising them together. Document public APIs, shortcuts, theme values, error behavior, and supported platforms.
- [x] Run formatting, linting, tests, visual checks, and examples in CI on supported desktop targets. A checkbox is complete only when its implementation, tests, documentation, and example land together; incomplete behavior remains unchecked.

## Source and licensing boundary

BUI is a behavior and architecture reference. Its Blender-derived code is GPL-licensed. Any direct reuse requires an explicit licensing decision and preserved notices; this inventory assumes original Rust implementations that match the specified behavior.
