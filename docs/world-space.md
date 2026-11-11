# World-space UI

A world panel uses a Bevy `Camera2d` rendering BBUI to an `Image`, a textured 3D mesh, and `UiWorldSurface` on that mesh. The surface records the offscreen camera, plane size, and image resolution. The framework maps pointer rays back to UI pixels and projects AccessKit bounds into the physical window. The [FPS example](../crates/bbui/examples/fps_world_ui/scene.rs) is the complete setup; see its [screenshot](visuals/examples/fps_world_ui.png).

```rust
UiWorldSurface {
    target_camera: ui_camera,
    texture_asset_id: 1,
    world_size: Vec2::new(2.55, 3.385),
    resolution: UVec2::new(520, 690),
}
```

Create an `Image::new_target_texture`, target a `Camera2d` at that image, set `RenderTargetCamera` to its entity, and use the same image as a `StandardMaterial` texture on an XY plane. Attach `UiWorldSurface` to that plane; its local origin is the center. Add `UiCrosshairPointer` to the physical window camera for center-screen ray input. The nearest visible mesh blocks pointer hits. A started drag remains on its original UI plane beyond the edges; deactivating the camera or removing the surface cancels stale pointer state.

Screen-reader roles, names, values, and actions remain available on the same logical controls. Their bounds follow the projected mesh and window DPI; nodes outside the view become hidden. For the FPS demo, WASD moves, the mouse aims, F1 releases or recaptures the cursor, and Escape cancels a UI edit or closes a popup. The shooting-range panel is below the main Object page; its [capture](visuals/examples/fps_world_ui_range.png) shows the extra controls.
