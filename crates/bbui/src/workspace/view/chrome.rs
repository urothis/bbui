use super::*;

fn style(background: ThemeColorToken, border: Option<ThemeColorToken>) -> UiInlineStyle {
    let mut style = UiInlineStyle::default();
    style.set_color_token(UiStyleProperty::Background, background);
    if let Some(border) = border {
        style.set_color_token(UiStyleProperty::BorderColor, border);
        style.set(UiStyleProperty::BorderWidth, UiStyleValue::Scalar(1.0));
    }
    style
}

fn clipped_style(background: ThemeColorToken) -> UiInlineStyle {
    let mut style = style(background, None);
    style.set(UiStyleProperty::ClipContents, UiStyleValue::Bool(true));
    style
}

pub(super) fn chrome(node: &DockNode, workspace: &Workspace) -> WidgetNode {
    let appearance = &workspace.appearance;
    match node {
        DockNode::Tabs(group) => {
            let tabs = group.panes.iter().map(|pane| {
                let title = workspace.registrations.get(pane).map_or_else(
                    || format!("Missing: {}", pane.0),
                    |registration| registration.title.clone(),
                );
                let selected = group.active.as_ref() == Some(pane);
                let tab_width = (title.chars().count() as f32 * 8.0 + 24.0).clamp(
                    appearance.tab_min_width.max(24.0),
                    appearance
                        .tab_max_width
                        .max(appearance.tab_min_width.max(24.0)),
                );
                let close_width = appearance
                    .close_width
                    .clamp(16.0, (tab_width - 8.0).max(16.0));
                let tab = WidgetNode::button(title)
                    .with_key(format!("ws:tab:{}:{}", group.id, pane.0))
                    .with_tab_selected(selected)
                    .with_role("workspace-tab")
                    .with_width(UiLength::Px(tab_width - close_width))
                    .with_height(UiLength::Px(appearance.tab_height.max(18.0)))
                    .with_style(style(
                        if selected {
                            appearance.active_tab_background
                        } else {
                            appearance.tab_background
                        },
                        None,
                    ));
                let close = WidgetNode::button("×")
                    .with_key(format!("ws:close:{}", pane.0))
                    .with_role("workspace-tab-close")
                    .with_width(UiLength::Px(close_width))
                    .with_height(UiLength::Px(appearance.tab_height.max(18.0)))
                    .with_accessible_name(format!("Close {}", pane.0))
                    .with_style(style(
                        if selected {
                            appearance.active_tab_background
                        } else {
                            appearance.tab_background
                        },
                        None,
                    ));
                WidgetNode::row([tab, close])
                    .with_key(format!("ws:tab-pair:{}", pane.0))
                    .with_gap(0.0)
            });
            let mut children = Vec::new();
            if group.options.show_single_tab || group.panes.len() > 1 {
                children.push(
                    WidgetNode::row(tabs)
                        .with_key(format!("ws:tabs:{}", group.id))
                        .with_tab_list()
                        .with_gap(0.0)
                        .with_width(UiLength::Fill(1.0))
                        .with_height(UiLength::Px(appearance.tab_height.max(18.0)))
                        .with_role("workspace-tab-bar")
                        .with_style(style(appearance.tab_background, None)),
                );
            } else if let Some(pane) = group.panes.first() {
                children.push(
                    WidgetNode::button("⋮")
                        .with_key(format!("ws:tab:{}:{}", group.id, pane.0))
                        .with_accessible_name(format!("Move {}", pane.0))
                        .with_role("workspace-pane-handle")
                        .with_width(UiLength::Px(24.0))
                        .with_height(UiLength::Px(18.0)),
                );
            }
            children.push(
                WidgetNode::column([])
                    .with_key(format!("ws:content:{}", group.id))
                    .with_role("workspace-pane-content")
                    .with_width(UiLength::Fill(1.0))
                    .with_height(UiLength::Fill(1.0))
                    .with_style(clipped_style(appearance.group_background)),
            );
            WidgetNode::column(children)
                .with_key(format!("ws:group:{}", group.id))
                .with_role("workspace-tab-group")
                .with_gap(0.0)
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Fill(1.0))
                .with_style(clipped_style(appearance.group_background))
        }
        DockNode::Split {
            id,
            axis,
            children,
            shares,
        } => {
            let mut parts = Vec::new();
            for (index, child) in children.iter().enumerate() {
                let mut child = chrome(child, workspace);
                child = match axis {
                    SplitAxis::Horizontal => child.with_width(UiLength::Fill(shares[index])),
                    SplitAxis::Vertical => child.with_height(UiLength::Fill(shares[index])),
                };
                parts.push(child);
                if index + 1 < children.len() {
                    let splitter = WidgetNode::button("")
                        .with_key(format!("ws:splitter:{id}:{index}"))
                        .with_role("workspace-splitter")
                        .with_accessible_name("Resize workspace split")
                        .with_style(style(appearance.border, None));
                    parts.push(match axis {
                        SplitAxis::Horizontal => splitter
                            .with_width(UiLength::Px(appearance.splitter_thickness.max(4.0)))
                            .with_height(UiLength::Fill(1.0)),
                        SplitAxis::Vertical => splitter
                            .with_height(UiLength::Px(appearance.splitter_thickness.max(4.0)))
                            .with_width(UiLength::Fill(1.0)),
                    });
                }
            }
            let container = match axis {
                SplitAxis::Horizontal => WidgetNode::row(parts),
                SplitAxis::Vertical => WidgetNode::column(parts),
            };
            container
                .with_key(format!("ws:split:{id}"))
                .with_gap(0.0)
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Fill(1.0))
        }
        DockNode::Grid {
            id,
            columns,
            children,
        } => WidgetNode::grid(
            *columns,
            children.iter().map(|child| {
                chrome(child, workspace)
                    .with_width(UiLength::Fill(1.0))
                    .with_height(UiLength::Fill(1.0))
            }),
        )
        .with_key(format!("ws:grid:{id}"))
        .with_gap(0.0)
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0)),
    }
}
