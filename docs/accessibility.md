# Accessibility

BBUI publishes AccessKit nodes from the same logical controls used by pointer and keyboard input. Nodes expose names, roles, values, enabled/hidden state, focus, and actions. Screen-reader actions use the widget action path, so they receive the same validation and undo behavior. `UiFocusable`, visible tree order, and `UiTabIndex` control keyboard traversal.

| Control family | AccessKit behavior |
| --- | --- |
| Buttons, checkboxes, tabs, panels | Activation, toggle/selection, expansion, and named focus targets |
| Numbers, sliders, colors, ramps, curves | Current values or selected points; supported increment, decrement, and set actions |
| Text fields and areas | Text input, selection, read-only state, and replacement actions |
| Menus, radios, dropdowns | Expanded state and named, selectable choices |
| Collections, tables, file browser | Visible virtual rows with names, position, selection, and actions |
| Canvas and graph | Named objects, nodes, sockets, and links; socket activation can create a validated link |

Attach `UiAccessibleName` when the painted label is insufficient. Add `UiLiveRegion::polite()` or `::assertive()` to changing status labels; `.atomic()` announces the full value. Numeric expression errors and file-browser operation status use assertive announcements.

Window UI bounds use layout coordinates and window DPI. A `UiWorldSurface` projects its controls through the physical view camera into screen coordinates; moving the mesh or camera updates those bounds, and a surface outside the view is hidden. Removing the surface hides its AccessKit controls until it returns. Semantic access remains available for the world panel; see [world-space UI](world-space.md).

Run `cargo run -p bbui --example accessibility_live` and `cargo test -p bbui --test plugin_integration --locked` to inspect live regions and world-panel bounds. The [accessibility capture](visuals/examples/accessibility_live.png) shows the example.
