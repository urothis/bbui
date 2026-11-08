# Themes and motion

`bui_style::Theme` is the runtime source for the built-in Dark and Light palettes,
logical spacing, control metrics, and motion durations. `Theme::default()` is
Dark. Switching `Theme.kind` through `Theme::switch` updates an existing UI;
entities and widget state do not need to be recreated.

## Semantic colors

Use `Theme::color(ThemeColorToken)` instead of copying a palette color into a
widget. `ThemeColorToken::ALL` is the complete list of built-in tokens, and the
style tests check that every token resolves to a finite, theme-specific color.
Tokens are grouped by use:

- Base surface and state colors: `Background`, `Surface`, `HoverSurface`,
  `PressedSurface`, `SelectedSurface`, `DisabledSurface`, `Text`, `MutedText`,
  `DisabledText`, `Accent`, `Border`, `DisabledBorder`, `Focus`, and `Error`.
- Controls and menus: `Icon`, `MutedIcon`, `DisabledIcon`, the `Button*` tokens,
  and the `Menu*` tokens.
- Collections and tables: `FileBrowserSelectedSurface`, `DataTable*`, and
  `Collection*` tokens.
- Editors: `TextSelectionOverlay`, `EditorActiveFieldSurface`, `EditorCaret`,
  `EditorImeUnderline`, `Curve*`, `NodeGraph*`, and `CanvasSelectionOverlay`.
- Elevated surfaces: `PopupShadow`, resolved separately for Dark and Light.
- Preview and disabled controls: `PreviewPlaceholder` and `ControlDisabled*`.

The base palette values are sRGB channels in the 0–1 range. Dark uses background
`(0.10, 0.10, 0.10)`, surface `(0.20, 0.20, 0.20)`, text `(0.88, 0.88, 0.88)`,
accent `(0.32, 0.55, 0.78)`, and border `(0.06, 0.06, 0.06)`. Light uses
background `(0.86, 0.86, 0.86)`, surface `(0.94, 0.94, 0.94)`, text
`(0.08, 0.08, 0.08)`, accent `(0.18, 0.38, 0.68)`, and border
`(0.67, 0.67, 0.67)`. Derived semantic tokens blend these base values; socket
type and preview placeholder tokens have explicit palette values.

## Spacing and motion values

`Theme.spacing` contains logical-pixel values shared by both built-in palettes,
so switching colors preserves layout rhythm. Typed groups cover `control`,
`box_container`, `property_row`, `panel`, `menu`, `list`, `table`, `editor`,
`curve`, `hsv`, `color_ramp`, `tooltip`, `node_graph`, `file_browser`, and
`collection`. For example, control padding is 8×4 px with an 8 px gap, menu
items are 24 px tall, list rows are 22 px tall, table rows are 24 px tall, and
node graph headers are 28 px tall. HSV wheel/field placement, ColorRamp tracks
and channel rows, Curve status labels, tooltip bounds, and pie-menu insets also
come from these groups. Pie-menu popup diameter, dead-zone/outer radii, item-ring
radii, item width, and inner-disc inset are configurable too. Each group can be
customized through the active `Theme` resource. The NodeGraph header hint width
is `ThemeNodeGraphSpacing.header_hint_width`.

`Theme.effects.popup_shadow` supplies the default shadow for the semantic roles
`menu-popup`, `dropdown-popup`, `context-menu-popup`, `tooltip-popup`, and
`pie-menu-popup`. The style plugin maps it to Bevy UI's native `BoxShadow`
component. A style declaration can override it with any number of ordered
layers:

```rust
let mut shadow = bui_style::UiBoxShadow::themed(
    bui_style::ThemeColorToken::PopupShadow,
    0.0,
    4.0,
    0.0,
    12.0,
);
shadow.push(bui_style::UiShadow::new(
    bui_style::Rgba(0.0, 0.0, 0.0, 0.2),
    0.0,
    1.0,
    0.0,
    2.0,
));
let mut inline = bui_style::UiInlineStyle::default();
inline.set(
    bui_style::UiStyleProperty::BoxShadow,
    bui_style::UiStyleValue::BoxShadow(shadow),
);
```

`Theme.motion` defaults are:

| Token | Default duration |
| --- | ---: |
| `Fast` | 80 ms |
| `Standard` | 160 ms |
| `Slow` | 280 ms |
| `PanelExpand` | 160 ms |
| `MenuOpen` | 120 ms |

`Theme.motion_scale` scales those durations. `UiReducedMotion(true)` snaps style,
panel, and menu-popup animations to their destination while keeping the same input
behavior. `MenuControl` popups, including pie menus, animate their visible surface
between trigger and popup bounds using `MenuOpen`; rows fade and clip during opening.
Hit boxes still move to the final popup bounds immediately, and return to the trigger
as soon as the menu closes. The popup surface then contracts to the trigger. Standalone
`Dropdown` popups and tooltips still appear immediately.

## Cascade and runtime changes

The cascade resolves theme defaults, inherited text properties, role defaults,
state selectors, stylesheet rules, and inline values in that order. Layout
properties are applied before layout computation. Transitions are configured
per property and interpolate from the currently displayed value, so reversing
or interrupting an animation remains continuous.

```rust
let theme = app.world().resource::<bui_style::Theme>().clone();
let mut transitions = bui_style::UiTransitions::default();
transitions.set_motion(
    bui_style::UiStyleProperty::Background,
    &theme,
    bui_style::ThemeMotionToken::Standard,
);

// Attach `transitions` to a styled UiNode. Later:
app.world_mut()
    .resource_mut::<bui_style::Theme>()
    .switch(bui_style::ThemeKind::Light);
app.world_mut().resource_mut::<bui_style::UiReducedMotion>().0 = true;
```

The runnable `theme_tokens` example demonstrates state transitions, a semantic
curve-editor color, a role-driven native popup shadow, Dark/Light switching, and
the reduced-motion override. The integrated showcase switches themes with F6
and toggles reduced motion with F7.
