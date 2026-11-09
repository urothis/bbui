# Example visual review gallery

These PNGs are captures of the actual Bevy example windows on an X11 virtual display. They are
review images, not pixel-perfect test baselines. The renderer's GPU readback tests use tolerant
pixel and geometry assertions. The [contact sheet](example_gallery.png) shows every graphical
example together; open an individual image below to inspect it at full resolution.

| Examples | Full-resolution captures |
| --- | --- |
| Main compositions | [Properties editor](examples/properties_editor.png) · [Showcase](examples/showcase.png) · [Layout gallery](examples/layout_gallery.png) |
| Menus and controls | [Menus](examples/menus.png) · [Controls](examples/controls.png) · [Sliders](examples/sliders.png) · [Numeric group](examples/numeric_group.png) · [Numeric expressions](examples/numeric_expressions.png) · [Panel reorder](examples/panel_reorder.png) |
| Editors | [Color ramp](examples/color_ramp.png) · [HSV color](examples/hsv_color.png) · [Curve editor](examples/curve_editor.png) · [Node graph](examples/node_graph.png) · [Canvas](examples/canvas.png) |
| Collections and data | [Data table](examples/data_table.png) · [Collections](examples/collections.png) · [File browser](examples/file_browser.png) |
| Text and other | [Text area](examples/text_area.png) · [Text history](examples/text_history.png) · [Accessibility live](examples/accessibility_live.png) · [Icon assets](examples/icon_assets.png) · [Render basics](examples/render_basics.png) |

Interaction captures show [Object Actions open](examples/menus_object_actions.png), the
[selected Transform pie](examples/menus_transform_pie.png), and the
[Visibility tab in the properties editor](examples/properties_editor_visibility.png).

## Regenerate

On Linux, start an X11 display with at least a 1600×1000 24-bit screen, then run:

```sh
cargo build -p bui --examples --locked
DISPLAY=:99 python3 scripts/capture_examples.py
```

The capture script requires `xwininfo` and `xwd`; it checks that every named example opens and
renders, and writes PNGs to `docs/visuals/examples/`. The GPU stress benchmark has no visible
window and is excluded. `scripts/x11_input.py` can raise/focus a window and send pointer actions
when reviewing interactive states.

The four-node NodeGraph GPU readback scene has additional reviewed
[Dark](node_graph_example_dark.png) and [Light](node_graph_example_light.png) captures at
1280×820. Regenerate those with:

```sh
BUI_NODE_GRAPH_CAPTURE_DIR="$PWD/docs/visuals" cargo test -p bui --test node_graph_gpu_readback --locked
```
