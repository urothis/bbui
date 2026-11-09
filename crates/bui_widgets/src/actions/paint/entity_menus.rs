use super::*;

pub(super) fn paint_buttons_and_menus(paint: &mut PaintEntityContext<'_>) -> bool {
    let world = &mut *paint.world;
    let entity = paint.entity;
    let layout = paint.layout;
    let rect = paint.rect;
    let clip = paint.clip;
    let z = paint.z;
    let theme = paint.theme;
    let enabled = paint.enabled;
    let camera_items = &mut *paint.output;

    if let Some(button) = world.get::<Button>(entity) {
        let background = if !enabled || !button.enabled {
            theme.color(ThemeColorToken::ButtonDisabledSurface)
        } else if button.status == crate::WidgetStatus::Hovered {
            theme.color(ThemeColorToken::ButtonHoverSurface)
        } else {
            theme.surface
        };
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: theme.corner_radius,
                color: background,
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
                        rect.x + theme.spacing.control.padding_x,
                        rect.y + theme.spacing.control.padding_y,
                    ],
                    text: button.label.clone(),
                },
                if !enabled || !button.enabled {
                    theme.color(ThemeColorToken::DisabledText)
                } else {
                    theme.text
                },
                theme,
                false,
            ),
        ));
        return true;
    }
    if let Some(button) = world.get::<IconButton>(entity) {
        let enabled = enabled && button.enabled;
        let properties_rail = world
            .get::<UiRole>(entity)
            .is_some_and(|role| role.0 == "flat-icon-button");
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: if properties_rail {
                    1.0
                } else {
                    theme.corner_radius
                },
                color: if enabled {
                    if properties_rail {
                        theme.background
                    } else {
                        theme.surface
                    }
                } else {
                    theme.color(ThemeColorToken::ButtonDisabledSurface)
                },
            },
        ));
        if world.get::<IconButtonAsset>(entity).is_none() {
            let text_width = button.icon.chars().count() as f32 * theme.font_size * 0.56;
            let text_height = theme.font_size * theme.line_height;
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [
                            rect.x + (rect.width - text_width).max(0.0) * 0.5,
                            rect.y + (rect.height - text_height).max(0.0) * 0.5,
                        ],
                        text: button.icon.clone(),
                    },
                    if enabled {
                        theme.text
                    } else {
                        theme.color(ThemeColorToken::DisabledText)
                    },
                    theme,
                    false,
                ),
            ));
        }
        return true;
    }
    if let Some(menu) = world.get::<MenuControl>(entity) {
        let properties_menu = world
            .get::<UiRole>(entity)
            .is_some_and(|role| role.0 == "flat-menu");
        let background = if menu.session.open {
            theme.color(ThemeColorToken::MenuOpenSurface)
        } else if properties_menu {
            theme.background
        } else {
            theme.surface
        };
        let animation = world
            .get_resource::<WidgetMenuAnimations>()
            .and_then(|animations| animations.0.get(&entity))
            .copied();
        let progress = menu_animation_progress(world, entity, theme)
            .unwrap_or(if menu.session.open { 1.0 } else { 0.0 });
        let visual_popup = animation.map_or(menu.popup.bounds, |animation| {
            interpolate_rect(animation.anchor, animation.popup, progress)
        });
        if menu.session.open || progress > f32::EPSILON {
            let popup = visual_popup;
            let popup_clip_rect = layout
                .clip
                .and_then(|parent| popup.intersection(parent))
                .unwrap_or(popup);
            let popup_clip = ClipRect::from(popup_clip_rect);
            let popup_z = z.saturating_add(90);
            let shadow_clip = popup_viewport_clip(world, layout.window, layout.clip, popup);
            if menu.display == MenuDisplay::Pie {
                push_popup_shadow(
                    popup,
                    popup.width.min(popup.height) * 0.5,
                    Rgba(0.0, 0.0, 0.0, 0.0),
                    shadow_clip,
                    popup_z.saturating_sub(1),
                    theme,
                    camera_items,
                );
                camera_items.push(item(
                    popup_clip,
                    popup_z,
                    Primitive::Ellipse {
                        rect: ClipRect::from(popup),
                        color: theme.surface,
                    },
                ));
                let pie_parent_path = if menu.session.path.len() > 1 {
                    menu.session.path[..menu.session.path.len() - 1].to_vec()
                } else {
                    Vec::new()
                };
                let wedge_count = menu
                    .tree
                    .entries_at(&pie_parent_path)
                    .unwrap_or_default()
                    .iter()
                    .filter(|entry| entry.enabled && !entry.separator)
                    .count();
                let center = [popup.x + popup.width * 0.5, popup.y + popup.height * 0.5];
                let outer_radius = popup.width.min(popup.height) * 0.5;
                for wedge in 0..wedge_count {
                    let angle = std::f32::consts::TAU * wedge as f32 / wedge_count as f32
                        - std::f32::consts::FRAC_PI_2;
                    camera_items.push(item(
                        popup_clip,
                        popup_z.saturating_add(1),
                        Primitive::Line {
                            from: center,
                            to: [
                                center[0] + angle.cos() * outer_radius,
                                center[1] + angle.sin() * outer_radius,
                            ],
                            width: 1.0,
                            color: with_opacity(
                                theme.color(ThemeColorToken::MenuBorder),
                                progress * 0.8,
                            ),
                        },
                    ));
                }
                let inset = theme
                    .spacing
                    .menu
                    .pie_inner_inset
                    .min(popup.width.min(popup.height) * 0.25);
                camera_items.push(item(
                    popup_clip,
                    popup_z.saturating_add(2),
                    Primitive::Ellipse {
                        rect: ClipRect::from(Rect {
                            x: popup.x + inset,
                            y: popup.y + inset,
                            width: (popup.width - inset * 2.0).max(0.0),
                            height: (popup.height - inset * 2.0).max(0.0),
                        }),
                        color: theme.background,
                    },
                ));
                let center_label = menu
                    .session
                    .path
                    .split_last()
                    .and_then(|(index, parent)| {
                        menu.tree
                            .entries_at(parent)
                            .and_then(|entries| entries.get(*index))
                    })
                    .map_or_else(|| menu.label.clone(), |entry| entry.label.clone())
                    .to_uppercase();
                let text_width = center_label.chars().count() as f32 * theme.font_size * 0.56;
                let text_height = theme.font_size * theme.line_height;
                camera_items.push(item(
                    popup_clip,
                    popup_z.saturating_add(3),
                    styled_text_primitive_for_entity(
                        world,
                        entity,
                        popup_clip,
                        TextPlacement {
                            origin: [center[0] - text_width * 0.5, center[1] - text_height * 0.5],
                            text: center_label,
                        },
                        with_opacity(theme.text, progress),
                        theme,
                        false,
                    ),
                ));
            } else {
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
                let border = theme.color(ThemeColorToken::MenuBorder);
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
                        popup_z.saturating_add(1),
                        Primitive::Line {
                            from,
                            to,
                            width: 1.0,
                            color: border,
                        },
                    ));
                }
            }
        }
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: theme.corner_radius,
                color: background,
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
                    text: if properties_menu {
                        menu.label.clone()
                    } else {
                        format!(
                            "{} {}",
                            menu.label,
                            if menu.session.open { "▴" } else { "▾" }
                        )
                    },
                },
                theme.text,
                theme,
                false,
            ),
        ));
        return true;
    }
    if let Some(menu_item) = world.get::<MenuItemControl>(entity) {
        let Some(menu) = world.get::<MenuControl>(menu_item.owner) else {
            return true;
        };
        let opacity = menu_animation_progress(world, menu_item.owner, theme).unwrap_or(1.0);
        let clip = if let Some(animation) = world
            .get_resource::<WidgetMenuAnimations>()
            .and_then(|animations| animations.0.get(&menu_item.owner))
        {
            let popup = interpolate_rect(animation.anchor, animation.popup, opacity);
            let Some(animated_clip) = intersect_clip_rect(clip, popup) else {
                return true;
            };
            animated_clip
        } else {
            clip
        };
        let mut selected_path = menu_item.parent_path.clone();
        selected_path.push(menu_item.index);
        let selected = menu.session.path == selected_path;
        let color = if !enabled || !menu_item.enabled {
            theme.color(ThemeColorToken::MenuDisabledSurface)
        } else if selected {
            theme.color(ThemeColorToken::MenuSelectedSurface)
        } else {
            theme.surface
        };
        if !menu_item.separator {
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect),
                    radius: theme.corner_radius,
                    color: with_opacity(color, opacity),
                },
            ));
            let suffix = if menu_item.has_submenu {
                "  ›".to_owned()
            } else {
                menu_item
                    .shortcut
                    .as_ref()
                    .map_or_else(String::new, |shortcut| format!("  {shortcut}"))
            };
            let text = format!("{}{suffix}", menu_item.label);
            let text_width = text.chars().count() as f32 * theme.font_size * 0.56;
            let text_height = theme.font_size * theme.line_height;
            let text_origin = if menu.display == MenuDisplay::Pie {
                [
                    rect.x + (rect.width - text_width).max(0.0) * 0.5,
                    rect.y + (rect.height - text_height).max(0.0) * 0.5,
                ]
            } else {
                [
                    rect.x + theme.spacing.menu.padding_x,
                    rect.y + theme.spacing.menu.padding_y,
                ]
            };
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: text_origin,
                        text,
                    },
                    with_opacity(
                        if !enabled || !menu_item.enabled {
                            theme.color(ThemeColorToken::MenuDisabledText)
                        } else {
                            theme.text
                        },
                        opacity,
                    ),
                    theme,
                    false,
                ),
            ));
        }
        return true;
    }
    if let Some(file_item) = world.get::<FileBrowserItem>(entity) {
        let file_view = world
            .get::<FileBrowser>(file_item.owner)
            .map_or(FileView::List, |browser| browser.view);
        let background = if file_item.selected {
            theme.color(ThemeColorToken::FileBrowserSelectedSurface)
        } else {
            theme.surface
        };
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: theme.corner_radius,
                color: background,
            },
        ));
        let glyph = file_preview_glyph(file_item.preview);
        let text = if file_view == FileView::List {
            let size = if file_item.directory {
                String::new()
            } else {
                format!("  {}", format_file_size(file_item.size))
            };
            format!("{glyph}  {}{size}", file_item.name)
        } else {
            format!("{glyph}  {}", file_item.name)
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
                        rect.x + theme.spacing.file_browser.padding_x,
                        rect.y + (rect.height - theme.font_size * theme.line_height).max(0.0) * 0.5,
                    ],
                    text,
                },
                theme.text,
                theme,
                false,
            ),
        ));
        return true;
    }
    if let Some(browser) = world.get::<FileBrowser>(entity) {
        let header_height = theme.spacing.file_browser.header_height.min(rect.height);
        let header_row_height = header_height * 0.5;
        let header_text_offset =
            (header_row_height - theme.font_size * theme.line_height).max(0.0) * 0.5;
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: theme.corner_radius,
                color: theme.background,
            },
        ));
        let breadcrumbs = browser
            .breadcrumbs()
            .into_iter()
            .map(|crumb| crumb.name)
            .collect::<Vec<_>>()
            .join("  ›  ");
        camera_items.push(item(
            clip,
            z.saturating_add(1),
            styled_text_primitive_for_entity(
                world,
                entity,
                clip,
                TextPlacement {
                    origin: [
                        rect.x + theme.spacing.file_browser.padding_x,
                        rect.y + header_text_offset,
                    ],
                    text: breadcrumbs,
                },
                theme.text,
                theme,
                false,
            ),
        ));
        let view = if browser.view == FileView::List {
            "List"
        } else {
            "Grid"
        };
        let sort = match browser.sort() {
            FileSort::Name => "Name",
            FileSort::Kind => "Kind",
            FileSort::Size => "Size",
            FileSort::Modified => "Modified",
        };
        let bookmark_names = browser
            .bookmarks
            .iter()
            .filter_map(|path| path.file_name().map(|name| name.to_string_lossy()))
            .map(|name| name.into_owned())
            .collect::<Vec<_>>()
            .join("  ");
        camera_items.push(item(
            clip,
            z.saturating_add(1),
            styled_text_primitive_for_entity(
                world,
                entity,
                clip,
                TextPlacement {
                    origin: [
                        rect.x + theme.spacing.file_browser.padding_x,
                        rect.y + header_row_height + header_text_offset,
                    ],
                    text: format!(
                        "{view}  •  Sort: {sort} {}  •  Filter: {}  •  ★ {bookmark_names}",
                        if browser.ascending() { "↑" } else { "↓" },
                        if browser.filter().is_empty() {
                            "type to search"
                        } else {
                            browser.filter()
                        }
                    ),
                },
                theme.text,
                theme,
                false,
            ),
        ));
        return true;
    }
    false
}
