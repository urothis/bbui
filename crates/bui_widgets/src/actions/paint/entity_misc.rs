use super::*;

pub(super) fn paint_misc_controls(paint: &mut PaintEntityContext<'_>) -> bool {
    let world = &mut *paint.world;
    let entity = paint.entity;
    let layout = paint.layout;
    let rect = paint.rect;
    let clip = paint.clip;
    let z = paint.z;
    let theme = paint.theme;
    let camera_items = &mut *paint.output;

    if let Some(option) = world.get::<DropdownOptionControl>(entity) {
        let color = if !option.enabled {
            theme.color(ThemeColorToken::MenuDisabledSurface)
        } else if option.selected || option.highlighted {
            theme.color(ThemeColorToken::MenuSelectedSurface)
        } else {
            theme.surface
        };
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: 2.0,
                color,
            },
        ));
        camera_items.push(item(
            clip,
            z.saturating_add(1),
            styled_text_primitive_for_entity(
                world,
                entity,
                clip,
                TextPlacement {
                    origin: [
                        rect.x + theme.spacing.menu.padding_x,
                        rect.y + theme.spacing.menu.padding_y,
                    ],
                    text: option.label.clone(),
                },
                if option.enabled {
                    theme.text
                } else {
                    theme.color(ThemeColorToken::MenuDisabledText)
                },
                theme,
                false,
            ),
        ));
        return true;
    }
    if let Some(dropdown) = world.get::<Dropdown>(entity) {
        let dropdown_role = world.get::<UiRole>(entity).map(|role| role.0.as_str());
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: theme.corner_radius,
                color: if dropdown_role == Some("compact-dropdown") {
                    theme.background.mix(theme.surface, 0.35)
                } else {
                    theme.surface
                },
            },
        ));
        let label = world
            .get::<Label>(entity)
            .map_or_else(String::new, |label| label.text.clone());
        let choice = dropdown
            .selected
            .and_then(|index| dropdown.options.get(index))
            .cloned()
            .unwrap_or_default();
        if dropdown.open
            && let Some(popup_state) = world.get::<DropdownPopupState>(entity)
            && !popup_state.popup.bounds.is_empty()
        {
            let popup = popup_state.popup.bounds;
            let popup_clip = ClipRect::from(popup_state.viewport);
            let popup_z = z.saturating_add(80);
            let shadow_clip =
                popup_viewport_clip(world, layout.window, layout.clip, popup_state.viewport);
            push_popup_shadow(
                popup,
                theme.corner_radius,
                theme.surface,
                shadow_clip,
                popup_z.saturating_sub(1),
                theme,
                camera_items,
            );
            camera_items.push(item(
                popup_clip,
                popup_z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(popup),
                    radius: theme.corner_radius,
                    color: theme.surface,
                },
            ));
            let right = popup.x + popup.width;
            let bottom = popup.y + popup.height;
            for (from, to) in [
                ([popup.x, popup.y], [right, popup.y]),
                ([right, popup.y], [right, bottom]),
                ([right, bottom], [popup.x, bottom]),
                ([popup.x, bottom], [popup.x, popup.y]),
            ] {
                camera_items.push(item(
                    popup_clip,
                    popup_z.saturating_add(3),
                    Primitive::Line {
                        from,
                        to,
                        width: 1.0,
                        color: theme.color(ThemeColorToken::MenuBorder),
                    },
                ));
            }
        }
        camera_items.push(item(
            clip,
            z.saturating_add(1),
            styled_text_primitive_for_entity(
                world,
                entity,
                clip,
                TextPlacement {
                    origin: [
                        rect.x + theme.spacing.control.padding_x,
                        rect.y + theme.spacing.control.padding_y,
                    ],
                    text: if dropdown_role == Some("icon-dropdown") {
                        "▣ ▾".into()
                    } else if dropdown_role == Some("compact-dropdown") {
                        format!("▣  {choice}  ▾")
                    } else if label.is_empty() {
                        choice
                    } else {
                        format!("{label}: {choice} ▾")
                    },
                },
                if dropdown_role == Some("icon-dropdown") {
                    Rgba(0.91, 0.52, 0.23, 1.0)
                } else {
                    theme.text
                },
                theme,
                false,
            ),
        ));
        return true;
    }
    if let Some(panel) = world.get::<Panel>(entity) {
        let object_panel = world
            .get::<UiRole>(entity)
            .is_some_and(|role| role.0 == "object-panel");
        let sub_panel = world
            .get::<UiRole>(entity)
            .is_some_and(|role| role.0 == "sub-panel");
        let header_height = rect.height.min(theme.spacing.panel.header_height);
        let panel_rect = Rect {
            height: header_height + (rect.height - header_height).max(0.0) * panel.state.progress,
            ..rect
        };
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(panel_rect),
                radius: theme.corner_radius,
                color: if sub_panel {
                    theme.surface.mix(theme.background, 0.30)
                } else {
                    theme.surface
                },
            },
        ));
        let arrow = if panel.state.target_expanded {
            "▾"
        } else {
            "▸"
        };
        camera_items.push(item(
            clip,
            z.saturating_add(1),
            styled_text_primitive_for_entity(
                world,
                entity,
                clip,
                TextPlacement {
                    origin: [
                        rect.x + theme.spacing.panel.content_padding,
                        rect.y + theme.spacing.panel.content_padding,
                    ],
                    text: if object_panel {
                        arrow.into()
                    } else {
                        format!("{arrow} {}", panel.title)
                    },
                },
                theme.text,
                theme,
                false,
            ),
        ));
        if object_panel {
            camera_items.push(item(
                clip,
                z.saturating_add(2),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [rect.x + 39.0, rect.y + theme.spacing.panel.content_padding],
                        text: panel.title.clone(),
                    },
                    theme.text,
                    theme,
                    false,
                ),
            ));
            camera_items.push(item(
                clip,
                z.saturating_add(2),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [rect.x + 20.0, rect.y + theme.spacing.panel.content_padding],
                        text: "▣".into(),
                    },
                    Rgba(0.91, 0.52, 0.23, 1.0),
                    theme,
                    false,
                ),
            ));
        }
        return true;
    }
    if let Some(progress) = world.get::<Progress>(entity) {
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: theme.corner_radius,
                color: theme.surface,
            },
        ));
        if progress.value > 0.0 {
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                Primitive::RoundedRect {
                    rect: ClipRect {
                        width: rect.width * progress.value,
                        ..ClipRect::from(rect)
                    },
                    radius: theme.corner_radius,
                    color: theme.accent,
                },
            ));
        }
    }
    false
}
