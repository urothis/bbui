# Commands and palette

`CommandRegistry` holds application command IDs, labels, descriptions, categories, availability, and optional shortcuts. Registration rejects duplicate IDs and conflicting shortcuts. The host decides what an activated command does.

`CommandPalette` builds a BBUI widget tree from the registry. Its default view includes a themed surface, search field, descriptions, categories, shortcut hints, selection, and an empty state. The surface uses BBUI's `UiBorderEffect` and `UiEdgeGlow`; colors resolve from the current theme. Mount `view` inside an application overlay or shell, pass widget events to `handle_widget_event`, and reconcile the tree when the query changes. Forward unconsumed arrow, Enter, and Escape keys to `handle_key`. Both activation methods return a command ID. `CommandRegistry::menu_entries` supplies the same labels, availability, and shortcut hints to a BBUI menu.

Use `view_with` and `CommandPaletteAppearance` to change copy, width, padding, spacing, row height, every text and control style, and the surface border and glow. Set `surface_effects` to `None` for a plain surface. `visible_limit` bounds both the rendered results and keyboard navigation. For an entirely different composition, build a widget tree from `CommandRegistry::ranked` and assign `CommandPalette::QUERY_KEY` and `CommandPalette::action_key(id)` to the corresponding controls; event handling remains reusable.

```rust
let mut appearance = CommandPaletteAppearance {
    title: "Find an action".into(),
    width: UiLength::Px(680.0),
    ..Default::default()
};
appearance.selected_style.set_color_token(
    UiStyleProperty::Background,
    ThemeColorToken::SelectedSurface,
);
let palette_node = palette.view_with(&registry, &appearance);
```

| Dark | Light |
| --- | --- |
| ![Command palette in dark theme](visuals/examples/command-palette-dark.png) | ![Command palette in light theme](visuals/examples/command-palette-light.png) |

Run `cargo run -p bbui --example command_palette` to use the palette over BBUI's docked material editor. The model has no platform menu ownership or user shortcut override storage.

For ranking and lookup measurements, run `cargo bench -p bbui --bench performance --locked -- commands`. The benchmark searches 32, 256, and 1,024 commands and measures shortcut resolution separately.
