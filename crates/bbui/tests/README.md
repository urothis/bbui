# BBUI verification checks

`plugin_integration.rs` runs the public `BbuiPlugin` with Bevy's `AssetPlugin` and the other
headless plugins it needs. It checks declarative widget creation, shared layout and hit boxes,
widget paint output, Bevy UI render entities, desktop input messages, text/IME handling, clipboard
requests, and the world-surface keyboard and cursor-capture routes.

Run it with:

```sh
cargo test -p bbui --test plugin_integration
```

`gpu_readback.rs` is a Linux pixel smoke test. It disables Winit and PBR, renders clipped BBUI
primitives to a Bevy `Image` target, and reads pixels back through Bevy's GPU readback plugin. CI
installs Mesa's software Vulkan driver, so it runs as part of the ordinary Linux test suite. The
test is gated off on Windows and macOS because this CI matrix does not provision a fallback
adapter there.

Run it directly with:

```sh
cargo test -p bbui --test gpu_readback -- --nocapture
```

The pixel check verifies Dark and Light theme colors, clipped glyph output, image tint, the GPU HSV
wheel, interleaved z-order across overlapping clip groups, and the clip boundary at DPI scales 1
and 2. It samples stable regions rather than comparing a golden image, and the DPI check does not
emulate a physical display scale factor. It also prints CPU wall time from scheduling each
readback through delivery of its callback; this includes Bevy update/queue delay and is not a GPU
timestamp. The interactive `crates/bbui/examples/showcase.rs` remains the visual scene for desktop
inspection. The offscreen check runs without a desktop display.

`widget_gpu_readback.rs` renders a real `WidgetTree` with an opened menu popup, HSV editor, curve
editor, color ramp, list, tree, and preview grid. It checks the popup row color and label, verifies
that the HSV painter contributes one wheel primitive, and samples the actual GPU-rendered hue ring
and disk. For the curve editor it checks Constant, Linear, and Smooth interpolation content,
samples the rendered path and selected point, and checks both selected smooth handles. For the color
ramp it samples interpolated gradient colors and verifies selected and unselected stop markers. It
also checks selected list/tree rows, an unselected preview swatch, and the selected preview tile's
accent overlay. All scene checks run for Dark and Light themes at DPI scales 1 and 2 through
llvmpipe in Linux CI.

Run it directly with:

```sh
cargo test -p bbui --test widget_gpu_readback -- --nocapture
```

`icon_asset_gpu_readback.rs` verifies an SVG icon from Bevy's `AssetServer` through the
GPU UI pipeline. It samples Dark/Light tint at DPI scales 1 and 2, observes a missing
asset failure and explicit retry after the file appears, and checks that hiding the
button releases its renderer registration and strong image handle.

```sh
cargo test -p bbui --test icon_asset_gpu_readback -- --nocapture
```
