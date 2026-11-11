use super::*;

pub fn resolve_style(
    theme: &Theme,
    sheet: &UiStyleSheet,
    role: Option<&UiRole>,
    classes: Option<&UiClasses>,
    state: UiStyleState,
    parent: Option<&UiComputedStyle>,
    inline: &UiInlineStyle,
) -> UiComputedStyle {
    let mut result = UiComputedStyle::default();
    for property in [
        UiStyleProperty::Foreground,
        UiStyleProperty::MutedForeground,
        UiStyleProperty::SelectionColor,
        UiStyleProperty::IconColor,
        UiStyleProperty::MutedIconColor,
        UiStyleProperty::FontSize,
        UiStyleProperty::LineHeight,
    ] {
        if let Some(value) = parent.and_then(|style| style.get(property)) {
            result.properties.insert(property, value);
        }
    }
    for property in [
        UiStyleProperty::Background,
        UiStyleProperty::Foreground,
        UiStyleProperty::MutedForeground,
        UiStyleProperty::BorderColor,
        UiStyleProperty::AccentColor,
        UiStyleProperty::SelectionColor,
        UiStyleProperty::FocusColor,
        UiStyleProperty::DisabledColor,
        UiStyleProperty::ErrorColor,
        UiStyleProperty::IconColor,
        UiStyleProperty::MutedIconColor,
        UiStyleProperty::DisabledIconColor,
        UiStyleProperty::BorderWidth,
        UiStyleProperty::CornerRadius,
        UiStyleProperty::Opacity,
        UiStyleProperty::FontSize,
        UiStyleProperty::LineHeight,
    ] {
        if let Some(value) = theme.value(property)
            && (!matches!(
                property,
                UiStyleProperty::Foreground
                    | UiStyleProperty::MutedForeground
                    | UiStyleProperty::SelectionColor
                    | UiStyleProperty::IconColor
                    | UiStyleProperty::MutedIconColor
                    | UiStyleProperty::FontSize
                    | UiStyleProperty::LineHeight
            ) || !result.properties.contains_key(&property))
        {
            result.properties.insert(property, value);
        }
    }
    if let Some(role) = role {
        result.properties.extend(
            role_style_values(theme, &role.0)
                .into_iter()
                .map(|(key, value)| (key, resolve_theme_color_token(theme, value))),
        );
    }
    if state.contains(UiStyleState::DISABLED) {
        result.properties.insert(
            UiStyleProperty::Background,
            UiStyleValue::Color(theme.disabled_surface),
        );
        result.properties.insert(
            UiStyleProperty::Foreground,
            UiStyleValue::Color(theme.disabled),
        );
        result.properties.insert(
            UiStyleProperty::BorderColor,
            UiStyleValue::Color(theme.disabled_border),
        );
        result.properties.insert(
            UiStyleProperty::Opacity,
            UiStyleValue::Scalar(nonnegative(theme.disabled_opacity).min(1.0)),
        );
        result.properties.insert(
            UiStyleProperty::DisabledIconColor,
            UiStyleValue::Color(theme.disabled),
        );
    } else {
        let selected = state.contains(UiStyleState::SELECTED);
        let background = if state.contains(UiStyleState::PRESSED) {
            Some(if selected {
                theme.selection.mix(theme.surface_pressed, 0.3)
            } else {
                theme.surface_pressed
            })
        } else if state.contains(UiStyleState::HOVERED) {
            Some(if selected {
                theme.selection.mix(theme.surface_hover, 0.28)
            } else {
                theme.surface_hover
            })
        } else if selected {
            Some(theme.selection)
        } else {
            None
        };
        if let Some(background) = background {
            result
                .properties
                .insert(UiStyleProperty::Background, UiStyleValue::Color(background));
        }
        if state.contains(UiStyleState::FOCUSED) || state.contains(UiStyleState::ACTIVE) {
            result.properties.insert(
                UiStyleProperty::BorderColor,
                UiStyleValue::Color(theme.focus),
            );
        }
        if state.contains(UiStyleState::CHECKED) {
            result.properties.insert(
                UiStyleProperty::AccentColor,
                UiStyleValue::Color(theme.accent),
            );
        }
        if state.contains(UiStyleState::ERROR) {
            result.properties.insert(
                UiStyleProperty::BorderColor,
                UiStyleValue::Color(theme.error),
            );
        }
    }
    let mut matches: Vec<(u16, i16, usize, &UiStyleRule)> = sheet
        .rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| rule.selector.matches(role, classes, state))
        .map(|(index, rule)| (rule.selector.specificity(), rule.priority, index, rule))
        .collect();
    matches.sort_by_key(|(specificity, priority, index, _)| (*specificity, *priority, *index));
    for (_, _, _, rule) in matches {
        result.properties.extend(
            rule.declaration
                .0
                .iter()
                .map(|(key, value)| (*key, resolve_theme_color_token(theme, value.clone()))),
        );
    }
    result.properties.extend(
        inline
            .0
            .iter()
            .map(|(key, value)| (*key, resolve_theme_color_token(theme, value.clone()))),
    );
    result
}

pub(super) fn resolve_theme_color_token(theme: &Theme, value: UiStyleValue) -> UiStyleValue {
    match value {
        UiStyleValue::ColorToken(token) => UiStyleValue::Color(theme.color(token)),
        UiStyleValue::BoxShadow(shadow) => {
            UiStyleValue::BoxShadow(shadow.resolve_theme_color(theme))
        }
        value => value,
    }
}

pub(super) fn role_style_values(
    theme: &Theme,
    role: &str,
) -> BTreeMap<UiStyleProperty, UiStyleValue> {
    use UiStyleProperty::*;
    let mut values = BTreeMap::new();
    let uniform = |x, y| UiEdges {
        left: x,
        top: y,
        right: x,
        bottom: y,
    };
    match role {
        "menu-popup" | "dropdown-popup" | "context-menu-popup" | "tooltip-popup"
        | "pie-menu-popup" => {
            values.insert(
                BoxShadow,
                UiStyleValue::BoxShadow(theme.effects.popup_shadow.clone()),
            );
        }
        "layout-row" | "layout-column" | "layout-grid" | "layout-flex" | "layout-inline"
        | "scroll" => {
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.control.gap));
        }
        "menu-bar" => {
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.menu.section_gap));
        }
        "flat-menu" | "flat-icon-button" | "flat-icon-orange" | "flat-icon-blue"
        | "flat-icon-red" | "header-play-icon" => {
            values.insert(Background, UiStyleValue::Color(theme.background));
            values.insert(CornerRadius, UiStyleValue::Scalar(0.0));
        }
        EditorRole::FLAT_MENU => {
            values.insert(
                Background,
                UiStyleValue::Color(theme.color(ThemeColorToken::EditorChromeSurface)),
            );
            values.insert(CornerRadius, UiStyleValue::Scalar(0.0));
        }
        "box" => {
            values.insert(Background, UiStyleValue::Color(theme.surface));
            values.insert(BorderColor, UiStyleValue::Color(theme.border));
            values.insert(BorderWidth, UiStyleValue::Scalar(1.0));
            values.insert(CornerRadius, UiStyleValue::Scalar(theme.corner_radius));
            values.insert(
                Padding,
                UiStyleValue::Edges(UiEdges::all(theme.spacing.box_container.padding)),
            );
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.box_container.gap));
        }
        "property-row" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.property_row.row_height)),
            );
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.property_row.gap));
        }
        "property-label" => {
            values.insert(
                Width,
                UiStyleValue::Length(UiLength::Px(theme.spacing.property_row.label_width)),
            );
        }
        "button" | "checkbox" | "radio-group" | "dropdown" | "text-field" | "numeric-field" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.control_height)),
            );
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.control.padding_x,
                    theme.spacing.control.padding_y,
                )),
            );
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.control.gap));
        }
        "slider" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.control.slider_height)),
            );
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.control.padding_x,
                    theme.spacing.control.padding_y,
                )),
            );
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.control.gap));
        }
        "text-area" => {
            values.insert(MinHeight, UiStyleValue::Scalar(theme.control_height));
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.control.padding_x,
                    theme.spacing.control.padding_y,
                )),
            );
        }
        "panel" => {
            values.insert(
                Padding,
                UiStyleValue::Edges(UiEdges::all(theme.spacing.panel.content_padding)),
            );
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.panel.section_gap));
        }
        "panel-header" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.panel.header_height)),
            );
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.control.padding_x,
                    theme.spacing.control.padding_y,
                )),
            );
        }
        "menu" | "popup" => {
            values.insert(
                Padding,
                UiStyleValue::Edges(UiEdges::all(theme.spacing.menu.popup_padding)),
            );
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.menu.section_gap));
        }
        "menu-trigger" | "popover-trigger" | "context-menu-trigger" | "pie-menu-trigger" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.control_height)),
            );
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.menu.padding_x,
                    theme.spacing.menu.padding_y,
                )),
            );
        }
        "menu-item" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.menu.item_height)),
            );
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.menu.padding_x,
                    theme.spacing.menu.padding_y,
                )),
            );
        }
        "list" => {
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.list.section_gap));
        }
        "list-row" | "tree-row" | "collection-list-row" | "collection-tree-row" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.list.row_height)),
            );
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.list.padding_x,
                    theme.spacing.list.padding_y,
                )),
            );
        }
        "preview-grid-item" | "collection-preview-item" => {
            values.insert(
                Width,
                UiStyleValue::Length(UiLength::Px(theme.spacing.collection.preview_cell_width)),
            );
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.collection.preview_cell_height)),
            );
        }
        "file-browser" => {
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(theme.spacing.file_browser.padding_x, 0.0)),
            );
        }
        "file-browser-header" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.file_browser.header_height)),
            );
        }
        "file-browser-list-row" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.file_browser.list_row_height)),
            );
        }
        "file-browser-grid-item" => {
            values.insert(
                Width,
                UiStyleValue::Length(UiLength::Px(theme.spacing.file_browser.grid_cell_width)),
            );
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.file_browser.grid_cell_height)),
            );
        }
        "collection" => {
            values.insert(
                Padding,
                UiStyleValue::Edges(UiEdges::all(theme.spacing.collection.padding)),
            );
        }
        "collection-header" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.collection.header_height)),
            );
        }
        "table-header" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.table.header_height)),
            );
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.table.cell_padding_x,
                    theme.spacing.table.cell_padding_y,
                )),
            );
        }
        "table-row" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.table.row_height)),
            );
        }
        "table-cell" => {
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.table.cell_padding_x,
                    theme.spacing.table.cell_padding_y,
                )),
            );
        }
        "editor" | "color-editor" | "curve-editor" | "ramp-editor" | "node-graph" | "canvas" => {
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.editor.padding_x,
                    theme.spacing.editor.padding_y,
                )),
            );
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.editor.toolbar_gap));
        }
        "editor-gutter" => {
            values.insert(
                Width,
                UiStyleValue::Length(UiLength::Px(theme.spacing.editor.gutter_width)),
            );
        }
        _ => {}
    }
    values
}
