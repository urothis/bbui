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
[selected Transform pie](examples/menus_transform_pie.png), the properties editor
[Render menu](examples/properties_editor_render_menu.png) and
[Mode dropdown](examples/properties_editor_mode_dropdown.png), and all five secondary properties
pages: [Modifiers](examples/properties_editor_modifiers.png),
[Material](examples/properties_editor_material.png),
[Outliner](examples/properties_editor_outliner.png),
[Preferences](examples/properties_editor_preferences.png), and
[Geometry Nodes](examples/properties_editor_nodes.png).

The [properties editor source map](properties-editor-reference.md) traces this composition to
BUI's component demo and native panel layout code.

## Regenerate

On Linux, start an X11 display with at least a 1600×1000 24-bit screen, then run:

```bash
cargo build -p bui --examples --locked
DISPLAY=:99 python3 scripts/capture_examples.py
for page in '80 modifiers' '109 material' '138 outliner' '167 preferences' '196 nodes'; do
  read -r y name <<< "$page"
  DISPLAY=:99 python3 scripts/capture_examples.py --click "13,$y" --output "/tmp/bui-$name" properties_editor
  cp "/tmp/bui-$name/properties_editor.png" "docs/visuals/examples/properties_editor_$name.png"
done
DISPLAY=:99 python3 scripts/capture_examples.py --click 280,140 --output /tmp/bui-object-actions menus
cp /tmp/bui-object-actions/menus.png docs/visuals/examples/menus_object_actions.png
DISPLAY=:99 python3 scripts/capture_examples.py --click 112,12 --output /tmp/bui-render-menu properties_editor
cp /tmp/bui-render-menu/properties_editor.png docs/visuals/examples/properties_editor_render_menu.png
DISPLAY=:99 python3 scripts/capture_examples.py --click 350,291 --output /tmp/bui-mode-dropdown properties_editor
cp /tmp/bui-mode-dropdown/properties_editor.png docs/visuals/examples/properties_editor_mode_dropdown.png
python3 scripts/make_example_gallery.py
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
