# Feature map

This page maps the implemented feature families to their public examples. It is a navigation aid for library users; start with [getting started](getting-started.md) and [widgets](widgets.md) for integration instructions.

| Area | Available behavior | Example and capture |
| --- | --- | --- |
| Layout and styling | Rows, columns, grids, flex, scroll, clipping, semantic Dark/Light themes | [`layout_gallery`](visuals/examples/layout_gallery.png), [`showcase`](visuals/examples/showcase.png) |
| Controls | Buttons, checkboxes, radios, dropdowns, sliders, numeric and text fields, tooltips | [`controls`](visuals/examples/controls.png), [`sliders`](visuals/examples/sliders.png) |
| Numeric editing | Cursor-hidden drag, Shift precision, Ctrl snap/step, multi-field drag, expressions, undo | [`numeric_group`](visuals/examples/numeric_group.png), [`numeric_expressions`](visuals/examples/numeric_expressions.png) |
| Menus and panels | Menu bar, context/popover/pie menus, fuzzy search, collapse, reorder | [`menus`](visuals/examples/menus.png), [`panel_reorder`](visuals/examples/panel_reorder.png) |
| Color and curves | HSV editor, editable ramps and curves, keyboard and pointer editing | [`hsv_color`](visuals/examples/hsv_color.png), [`color_ramp`](visuals/examples/color_ramp.png), [`curve_editor`](visuals/examples/curve_editor.png) |
| Data and editors | Virtualized lists, trees, previews, tables, canvas, node graph | [`collections`](visuals/examples/collections.png), [`data_table`](visuals/examples/data_table.png), [`node_graph`](visuals/examples/node_graph.png) |
| Files and media | Navigation, bookmarks, selection, operations and errors; image, SVG, video | [`file_browser`](visuals/examples/file_browser.png), [`showcase`](visuals/examples/showcase.png) |
| Motion | Typed keyframe tracks, curves, playback controls, reduced motion | [Animation clips](visuals/README.md#animation-clips) |
| 3D and browser | UI on a textured mesh with crosshair input and projected accessibility; WebGPU properties editor | [`fps_world_ui`](visuals/examples/fps_world_ui.png), [browser](visuals/examples/properties_editor_web.png) |

The file-browser example uses a disposable temp directory, confines navigation to it, and confirms deletion. Library callers can use `FileBrowser::confine_to` for a restricted subtree and own their confirmation policy. Browser navigation/view edits use UI history; external file mutations are reported separately and are not reverted by Ctrl+Z.

The supported desktop targets are Linux, Windows, and macOS. The properties editor has a WebGPU browser build. See [performance](performance.md) for measurement scope and [accessibility](accessibility.md) for semantics. This project is an original Rust implementation inspired by BUI's behavior; it does not embed BUI's GPL runtime or ship a CPU rasterizer.
