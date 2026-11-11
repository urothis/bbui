use super::*;

pub(super) fn paint_misc_controls(paint: &mut PaintEntityContext<'_>) -> bool {
    let world = &mut *paint.world;
    let entity = paint.entity;
    let layout = paint.layout;
    let rect = paint.rect;
    let clip = paint.clip;
    let z = paint.z;
    let theme = paint.theme;
    let base_theme = paint.base_theme;
    let camera_items = &mut *paint.output;

    if world
        .get::<UiRole>(entity)
        .is_some_and(|role| matches!(role.0.as_str(), EditorRole::MENU_BAR | EditorRole::TAB_RAIL))
    {
        let rail_color = base_theme.color(ThemeColorToken::EditorChromeSurface);
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: 0.0,
                color: rail_color,
            },
        ));
        return true;
    }

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
                radius: theme.corner_radius,
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
        let compact = dropdown_role == Some("compact-dropdown");
        let icon_only = dropdown_role == Some("icon-dropdown");
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: theme.corner_radius,
                color: if compact || icon_only {
                    base_theme.background.mix(base_theme.surface, 0.35)
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
        if compact || icon_only {
            push_rounded_outline(
                Rect {
                    x: rect.x + 0.5,
                    y: rect.y + 0.5,
                    width: (rect.width - 1.0).max(0.0),
                    height: (rect.height - 1.0).max(0.0),
                },
                theme.corner_radius,
                base_theme.surface.mix(base_theme.text, 0.25),
                clip,
                z.saturating_add(1),
                camera_items,
            );
        }
        if dropdown.open
            && let Some(popup_state) = world.get::<DropdownPopupState>(entity)
            && !popup_state.popup.bounds.is_empty()
        {
            let popup = popup_state.popup.bounds;
            let popup_clip = ClipRect::from(popup_state.viewport);
            let popup_z = z.saturating_add(80);
            let shadow_clip =
                popup_viewport_clip(world, layout.window, layout.clip, popup_state.viewport);
            push_popup_surface(
                popup,
                popup_clip,
                shadow_clip,
                popup_z,
                theme,
                base_theme,
                camera_items,
            );
        }
        if compact || icon_only {
            let text_y = rect.y + (rect.height - theme.font_size * theme.line_height) * 0.5;
            for (x, text, color) in [
                (
                    rect.x + 5.0,
                    "▣".to_owned(),
                    if icon_only {
                        Rgba(0.91, 0.52, 0.23, 1.0)
                    } else {
                        theme.text
                    },
                ),
                (
                    rect.x + 21.0,
                    if compact { choice } else { String::new() },
                    theme.text,
                ),
                (rect.x + rect.width - 14.0, "⌄".to_owned(), theme.text),
            ] {
                if !text.is_empty() {
                    camera_items.push(item(
                        clip,
                        z.saturating_add(2),
                        styled_text_primitive_for_entity(
                            world,
                            entity,
                            clip,
                            TextPlacement {
                                origin: [x, text_y],
                                text,
                            },
                            color,
                            theme,
                            false,
                        ),
                    ));
                }
            }
        } else {
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
                        text: if label.is_empty() {
                            choice
                        } else {
                            format!("{label}: {choice} ▾")
                        },
                    },
                    theme.text,
                    theme,
                    false,
                ),
            ));
        }
        return true;
    }
    if let Some(panel) = world.get::<Panel>(entity) {
        let panel_hovered = world
            .get_resource::<UiFocus>()
            .and_then(|focus| focus.state(layout.window))
            .is_some_and(|state| state.hovered == Some(entity));
        let object_panel = world
            .get::<UiRole>(entity)
            .is_some_and(|role| role.0 == EditorRole::PRIMARY_PANEL);
        let sub_panel = world
            .get::<UiRole>(entity)
            .is_some_and(|role| role.0 == EditorRole::SUB_PANEL);
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
                    base_theme.color(ThemeColorToken::EditorSubpanelSurface)
                } else {
                    if panel_hovered {
                        base_theme.surface
                    } else {
                        theme.surface
                    }
                },
            },
        ));
        if sub_panel {
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                Primitive::RoundedRect {
                    rect: ClipRect::from(Rect {
                        height: header_height,
                        ..rect
                    }),
                    radius: 0.0,
                    color: base_theme.surface,
                },
            ));
        }
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
                        rect.x
                            + if sub_panel {
                                20.0
                            } else {
                                theme.spacing.panel.content_padding
                            },
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
                    base_theme.color(ThemeColorToken::EditorObjectAccent),
                    theme,
                    false,
                ),
            ));
        }
        let reorderable = !sub_panel
            && world
                .get::<UiParent>(entity)
                .and_then(|parent| world.get::<PanelGroup>(parent.0))
                .is_some_and(|group| group.reorderable);
        if reorderable {
            let dot_color = theme.text_muted.mix(theme.surface, 0.45);
            for row in 0..3 {
                for column in 0..4 {
                    camera_items.push(item(
                        clip,
                        z.saturating_add(2),
                        Primitive::Ellipse {
                            rect: ClipRect {
                                x: rect.x + rect.width - 20.0 + column as f32 * 3.0,
                                y: rect.y + header_height * 0.5 - 3.5 + row as f32 * 3.0,
                                width: 1.5,
                                height: 1.5,
                            },
                            color: dot_color,
                        },
                    ));
                }
            }
        }
        return true;
    }
    if let Some(progress) = world.get::<Progress>(entity) {
        let properties_progress = world
            .get::<UiRole>(entity)
            .is_some_and(|role| role.0 == EditorRole::PROGRESS);
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: theme.corner_radius,
                color: if properties_progress {
                    base_theme.color(ThemeColorToken::EditorProgressTrack)
                } else {
                    theme.surface
                },
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
