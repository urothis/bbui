# Visual gallery

These are captures from runnable Bevy examples and BBUI's offscreen GPU tests. Open the [contact sheet](example_gallery.png) to scan the set, then use the full-resolution links below for review. They are visual references; GPU readback tests use tolerant pixel and geometry assertions.

## Complete compositions

| Example | Capture |
| --- | --- |
| Properties editor | [Object page](examples/properties_editor.png) · [Modifiers](examples/properties_editor_modifiers.png) · [Material](examples/properties_editor_material.png) · [Outliner](examples/properties_editor_outliner.png) · [Preferences](examples/properties_editor_preferences.png) · [Geometry Nodes](examples/properties_editor_nodes.png) · [wide graph](examples/properties_editor_nodes_wide.png) |
| World-space properties editor | [FPS view](examples/fps_world_ui.png) · [Shooting Range panel](examples/fps_world_ui_range.png) |
| Browser/WebGPU | [Properties editor](examples/properties_editor_web.png) · [Geometry Nodes](examples/properties_editor_web_nodes.png) |
| Full showcase | [Showcase](examples/showcase.png) · [layout gallery](examples/layout_gallery.png) |
| Live themes | [Theme gallery](examples/theme_gallery.png) |
| Docking workspace | [Dark](examples/workspace-dark.png) · [Light](examples/workspace-light.png) · [edge drop preview](examples/workspace-drop.png) |
| Editor workspace composition | [Dark](examples/editor-workspace-dark.png) · [Light](examples/editor-workspace-light.png) |
| Workspace shell | [Dark](examples/workspace-shell-dark.png) · [Light](examples/workspace-shell-light.png) |
| Visual effects | [Gradient border and edge glow](examples/visual-effects-dark.png) · [rotated border](examples/visual-effects-rotated.png) · [eight variants](examples/visual-effect-variants.png) |
| Progress bars | [Six-style gallery with corner options](examples/progress-gallery.png) |

The [properties editor source map](properties-editor-reference.md) records the composition and visual reference. The desktop and FPS examples use the same shared widget tree.

## Control and editor examples

| Family | Captures |
| --- | --- |
| Menus and controls | [Menus](examples/menus.png) · [Object Actions open](examples/menus_object_actions.png) · [Transform pie](examples/menus_transform_pie.png) · [Controls](examples/controls.png) · [Sliders](examples/sliders.png) · [panel reorder](examples/panel_reorder.png) |
| Numeric and text | [Numeric group](examples/numeric_group.png) · [expressions](examples/numeric_expressions.png) · [text area](examples/text_area.png) · [text history](examples/text_history.png) |
| Color and graphs | [Color ramp](examples/color_ramp.png) · [HSV](examples/hsv_color.png) · [curve](examples/curve_editor.png) · [node graph](examples/node_graph.png) · [canvas](examples/canvas.png) |
| Data and files | [Collections](examples/collections.png) · [data table](examples/data_table.png) · [file browser](examples/file_browser.png) |
| Other | [Accessibility live](examples/accessibility_live.png) · [icon assets](examples/icon_assets.png) · [render basics](examples/render_basics.png) |

Open menu and choice states are captured separately: [Render menu](examples/properties_editor_render_menu.png) and [Mode dropdown](examples/properties_editor_mode_dropdown.png). The file-browser capture shows the confined, disposable demo directory.

## Animation clips

| Clip | What it shows |
| --- | --- |
| [Keyframe curves](animations/keyframe_curves.webp) | Linear, Ease In, and Ease Out tracks |
| [Panel expansion](animations/panel_expand.webp) · [Transform collapse](animations/properties_transform.webp) · [Visibility collapse](animations/properties_visibility.webp) | Layout and paint moving together |
| [Menu opening](animations/menu_open.webp) · [properties Render menu](animations/properties_render_menu.webp) | Popup animation |
| [Theme switch](animations/theme_switch.webp) | Smooth Dark/Light transition in the theme gallery |
| [Workspace docking](animations/workspace_dock.webp) · [splitter resize](animations/workspace_resize.webp) | Tab drag and continuous split resizing in the dockable workspace |
| [Gradient border](animations/gradient-border.webp) · [edge glow](animations/edge-glow.webp) · [combined](animations/border-edge-glow.webp) · [full composition](animations/visual-effects.webp) | Rotating border and pulsing glow, captured at 60 fps |
| [Eight effect variants](animations/visual-effect-variants.webp) | Solid and multicolor borders and glows, static and animated |
| [Progress gallery](animations/progress-gallery.webp) · [gradient](animations/progress-gradient.webp) · [segmented](animations/progress-segmented.webp) · [glow](animations/progress-glow.webp) · [indeterminate](animations/progress-indeterminate.webp) | Filled-region effects and determinate/indeterminate motion at 60 fps |

The animation capture scripts sample the real examples at 60 Hz. Short UI transitions use a motion scale of 4; the theme gallery uses its own 1.1-second transition in each direction. Workspace clips capture scripted pointer input through the offscreen Bevy renderer at the same 60 Hz cadence.

## Regenerate captures

On Linux, build the examples and use an X11 display at least 1600×1000 with 24-bit color:

```sh
cargo build -p bbui --examples --locked
DISPLAY=:99 python3 scripts/capture_examples.py
python3 scripts/make_example_gallery.py
cargo build --release -p bbui --examples --locked
DISPLAY=:99 python3 scripts/capture_animations.py --profile release
python3 scripts/capture_workspace_animations.py --profile release
python3 scripts/capture_visual_effect_animations.py
python3 scripts/capture_visual_effect_variants.py
```

`capture_examples.py` requires `xwininfo` and `xwd`; input captures also require `libXtst`. Animated WebP capture requires Pillow with WebP support. The scripts live in [`scripts/`](../../scripts/) and accept individual example names for a targeted recapture.

`capture_workspace_animations.py` uses offscreen GPU readback and does not need an X11 display. It writes `workspace_dock.webp` and `workspace_resize.webp` from 120 samples per clip over two seconds.

The workspace captures come from `cargo test -p bbui workspace::gpu::capture_workspace_dark_and_light --lib -- --ignored`; copy the resulting `target/workspace-preview-*.png` files into `docs/visuals/examples/` after visual review.

The editor workspace captures come from `cargo test -p bbui --example editor_workspace --locked -- --ignored`; the capture test writes both themes directly to `docs/visuals/examples/`.

The workspace shell captures come from `cargo test -p bbui --example workspace_shell --locked -- --ignored`; the capture test writes both themes directly to `docs/visuals/examples/`.

`capture_visual_effect_animations.py` uses offscreen GPU readback to write the visual effects stills and four 120-frame WebP clips. Each clip is two seconds at 60 fps. Pass `--frames-dir` to encode an existing set of capture frames.

`capture_visual_effect_variants.py` writes the eight-case overview and one WebP per variant. Animated variants sample 120 frames over two seconds; static variants are single-frame WebPs. Set `BBUI_VISUAL_VARIANTS=1` when running the `visual_effects` example to inspect the gallery live.

`capture_progress_animations.py` writes the progress gallery screenshot and five WebP clips from offscreen GPU readback. Each clip has 120 frames over two seconds at 60 fps. Pass `--frames-dir` to encode an existing capture.
