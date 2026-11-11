# Visual gallery

These are captures from runnable Bevy examples. Open the [contact sheet](example_gallery.png) to scan the set, then use the full-resolution links below for review. They are visual references; GPU readback tests use tolerant pixel and geometry assertions.

## Complete compositions

| Example | Capture |
| --- | --- |
| Properties editor | [Object page](examples/properties_editor.png) · [Modifiers](examples/properties_editor_modifiers.png) · [Material](examples/properties_editor_material.png) · [Outliner](examples/properties_editor_outliner.png) · [Preferences](examples/properties_editor_preferences.png) · [Geometry Nodes](examples/properties_editor_nodes.png) · [wide graph](examples/properties_editor_nodes_wide.png) |
| World-space properties editor | [FPS view](examples/fps_world_ui.png) · [Shooting Range panel](examples/fps_world_ui_range.png) |
| Browser/WebGPU | [Properties editor](examples/properties_editor_web.png) · [Geometry Nodes](examples/properties_editor_web_nodes.png) |
| Full showcase | [Showcase](examples/showcase.png) · [layout gallery](examples/layout_gallery.png) |
| Live themes | [Theme gallery](examples/theme_gallery.png) |

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

The animation capture script samples the real examples at 60 Hz. Short UI transitions use a motion scale of 4; the theme gallery uses its own 1.1-second transition in each direction.

## Regenerate captures

On Linux, build the examples and use an X11 display at least 1600×1000 with 24-bit color:

```sh
cargo build -p bbui --examples --locked
DISPLAY=:99 python3 scripts/capture_examples.py
python3 scripts/make_example_gallery.py
cargo build --release -p bbui --examples --locked
DISPLAY=:99 python3 scripts/capture_animations.py --profile release
```

`capture_examples.py` requires `xwininfo` and `xwd`; input captures also require `libXtst`. Animated WebP capture requires Pillow with WebP support. The scripts live in [`scripts/`](../../scripts/) and accept individual example names for a targeted recapture.
