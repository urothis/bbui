# Architecture

`bbui` is the application-facing Bevy plugin and prelude. Install `BbuiPlugin`, mount a `WidgetTree` below a `UiLayoutRoot`, and read `WidgetEventQueue` after `UiSystems::Widgets`. The [getting-started guide](getting-started.md) has a complete host setup.

| Crate | Responsibility |
| --- | --- |
| `bbui_core` | Logical hierarchy, focus, input routing, history, and dirty journal |
| `bbui_layout` | Measurement, scrolling, clipping, and hit boxes |
| `bbui_style` | Dark/Light themes, semantic tokens, and style cascade |
| `bbui_text` | Shaping, text editing, IME, and numeric expressions |
| `bbui_motion` | Typed keyframe clips and bindings |
| `bbui_widgets` | Controls, editors, declarative trees, and actions |
| `bbui_render` | GPU draw data, Bevy UI entities, image ownership, and world surfaces |
| `bbui_media` | Raster, SVG, animated image, and video decoding |
| `bbui_macros` | `bbui!` declaration macro |

## Frame flow

`UiSystems` runs style, text measurement, layout, input, the post-input layout/style passes, text, widgets, and render in that order. Widget values and application events settle before paint and AccessKit synchronization. A single `UiLayoutResult` drives clipping, hit testing, paint placement, and accessible bounds. For a 3D panel, the accessible bounds are projected from that result through its `UiWorldSurface` and physical view camera.

The [invalidation contract](invalidation.md) explains how changes reach only affected consumers. `WidgetTree::reconcile` retains unchanged keyed entities; direct component edits enter the same change journal. Internal menu choices and visible collection rows are virtual child entities owned by their control.

## Ownership boundaries

Widgets own values and transient editing state. The host owns the application model, physical window and cameras, 3D meshes, and external filesystem decisions. BBUI uses Bevy's GPU UI renderer and asset handles; it does not maintain a second software rasterizer. The [world-space guide](world-space.md) shows the camera/image/mesh contract, and [performance](performance.md) lists the measured workloads.
