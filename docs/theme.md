# Themes and styling

`Theme` supplies Dark and Light palettes, semantic colors, spacing, and motion tokens. `Theme::default()` is Dark. Change the `Theme` resource at runtime to switch palettes without recreating controls; the [theme animation](visuals/animations/theme_switch.webp) shows the result.

Use `ThemeColorToken` for roles such as background, surface, text, border, accent, menu, table, editor, focus, and error. This keeps a widget readable in both palettes. `Theme.spacing` groups measurements by control family: panels, menus, property rows, tables, graph nodes, color editors, and more. `Theme.effects.popup_shadow` sets the common popup shadow.

`UiInlineStyle` applies a one-off override. `UiStyleSheet` plus `UiRole` and `UiClasses` define reusable rules. `set_ui_style_state` changes a state such as hover or selected without rebuilding the tree. `UiAutoAnimations` can transition style properties using a `ThemeMotionToken`; `UiReducedMotion(true)` snaps those transitions to their target value. See [keyframe motion](motion.md) for explicit clips.

The bundled Inter font is the default UI face (`assets/fonts/Inter[opsz,wght].ttf`); Noto Sans Symbols 2 (`assets/fonts/NotoSansSymbols2-Regular.ttf`) supplies Unicode symbols missing from Inter. Both are embedded, so BBUI does not require system fonts or Bevy's sample font. Attach `UiFontAsset` to a control or ancestor to request another Bevy font; `UiFontAssetState` exposes loading, ready, and failure states. The default remains usable while an override is unavailable. See [`icon_assets`](visuals/examples/icon_assets.png) and [`properties_editor`](visuals/examples/properties_editor.png) for built-in assets and style roles.
