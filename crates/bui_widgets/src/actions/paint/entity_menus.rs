use super::*;

pub(super) fn paint_buttons_and_menus(paint: &mut PaintEntityContext<'_>) -> bool {
    let world = &mut *paint.world;
    let entity = paint.entity;
    let layout = paint.layout;
    let rect = paint.rect;
    let clip = paint.clip;
    let z = paint.z;
    let theme = paint.theme;
    let base_theme = paint.base_theme;
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
    if let Some(button) = world.get::<IconButton>(entity).cloned() {
        let enabled = enabled && button.enabled;
        let icon_role = world.get::<UiRole>(entity).map(|role| role.0.as_str());
        let header_play = icon_role == Some("header-play-icon");
        let properties_rail =
            header_play || icon_role.is_some_and(|role| role.starts_with("flat-icon-"));
        let side_tab = icon_role.is_some_and(|role| role.starts_with("side-tab"));
        let side_tab_active = side_tab && icon_role.is_some_and(|role| role.ends_with("-active"));
        let side_tab_hovered = side_tab
            && world
                .get_resource::<UiFocus>()
                .and_then(|focus| focus.state(layout.window))
                .is_some_and(|state| state.hovered == Some(entity));
        let tab_inner: f32 = if base_theme.kind == bui_style::ThemeKind::Light {
            179.0 / 255.0
        } else {
            29.0 / 255.0
        };
        let tab_hover = (tab_inner + 15.0 / 255.0).min(1.0);
        let icon_color = match icon_role {
            Some("flat-icon-orange" | "side-tab-orange" | "side-tab-orange-active") => {
                Rgba(0.91, 0.52, 0.23, 1.0)
            }
            Some("flat-icon-blue" | "side-tab-blue" | "side-tab-blue-active") => base_theme.accent,
            Some("flat-icon-red" | "side-tab-red" | "side-tab-red-active") => base_theme.error,
            _ => theme.text,
        };
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(if side_tab {
                    Rect {
                        x: rect.x + 1.0,
                        width: (rect.width - 1.0).max(0.0),
                        height: (rect.height - 1.0).max(0.0),
                        ..rect
                    }
                } else {
                    rect
                }),
                radius: if properties_rail || !side_tab_active {
                    1.0
                } else {
                    theme.corner_radius
                },
                color: if enabled {
                    if side_tab_active {
                        base_theme.background
                    } else if side_tab_hovered {
                        Rgba(tab_hover, tab_hover, tab_hover, 1.0)
                    } else if side_tab {
                        Rgba(tab_inner, tab_inner, tab_inner, 1.0)
                    } else if header_play {
                        base_theme.background.mix(Rgba(0.0, 0.0, 0.0, 1.0), 0.5)
                    } else if properties_rail {
                        theme.background
                    } else {
                        theme.surface
                    }
                } else {
                    theme.color(ThemeColorToken::ButtonDisabledSurface)
                },
            },
        ));
        if header_play {
            for row in 0_i32..9 {
                let width = ((4 - (row - 4).abs()) * 2 + 2) as f32;
                let y = rect.y + 5.0 + row as f32;
                camera_items.push(item(
                    clip,
                    z.saturating_add(1),
                    Primitive::Line {
                        from: [rect.x + 8.0, y],
                        to: [rect.x + 8.0 + width, y],
                        width: 1.0,
                        color: theme.text,
                    },
                ));
            }
            return true;
        }
        if side_tab {
            paint_properties_tab_icon(
                camera_items,
                clip,
                z,
                rect,
                &button.accessible_name,
                icon_color,
            );
            return true;
        }
        if world.get::<IconButtonAsset>(entity).is_none() {
            let mut icon_theme = theme.clone();
            if side_tab {
                icon_theme.font_size *= 1.4;
            }
            let text_width = measured_text_width(world, entity, &button.icon, &icon_theme);
            let text_height = icon_theme.font_size * icon_theme.line_height;
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
                        icon_color
                    } else {
                        theme.color(ThemeColorToken::DisabledText)
                    },
                    &icon_theme,
                    false,
                ),
            ));
        }
        return true;
    }
    if let Some(menu) = world.get::<MenuControl>(entity) {
        let properties_menu = world
            .get::<UiRole>(entity)
            .is_some_and(|role| matches!(role.0.as_str(), "flat-menu" | "properties-flat-menu"));
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
        if properties_menu && menu.label == "⌕" {
            let center = [
                rect.x + rect.width * 0.5 - 1.0,
                rect.y + rect.height * 0.5 - 1.0,
            ];
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                Primitive::Ellipse {
                    rect: ClipRect {
                        x: center[0] - 4.5,
                        y: center[1] - 4.5,
                        width: 9.0,
                        height: 9.0,
                    },
                    color: theme.text,
                },
            ));
            camera_items.push(item(
                clip,
                z.saturating_add(2),
                Primitive::Ellipse {
                    rect: ClipRect {
                        x: center[0] - 3.2,
                        y: center[1] - 3.2,
                        width: 6.4,
                        height: 6.4,
                    },
                    color: background,
                },
            ));
            camera_items.push(item(
                clip,
                z.saturating_add(3),
                Primitive::Line {
                    from: [center[0] + 2.8, center[1] + 2.8],
                    to: [center[0] + 6.4, center[1] + 6.4],
                    width: 1.5,
                    color: theme.text,
                },
            ));
            return true;
        }
        let flat_label = properties_menu.then(|| menu.label.clone());
        let normal_label = (!properties_menu).then(|| {
            format!(
                "{} {}",
                menu.label,
                if menu.session.open { "▴" } else { "▾" }
            )
        });
        let flat_text_width = flat_label
            .as_ref()
            .map(|label| measured_text_width(world, entity, label, theme));
        camera_items.push(item(
            clip,
            z.saturating_add(1),
            if properties_menu {
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [
                            rect.x + (rect.width - flat_text_width.unwrap_or(0.0)).max(0.0) * 0.5,
                            rect.y + 1.0,
                        ],
                        text: flat_label.unwrap_or_default(),
                    },
                    theme.text,
                    theme,
                    false,
                )
            } else {
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [
                            rect.x + theme.spacing.menu.padding_x,
                            rect.y + theme.spacing.menu.padding_y,
                        ],
                        text: normal_label.unwrap_or_default(),
                    },
                    theme.text,
                    theme,
                    false,
                )
            },
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

fn paint_properties_tab_icon(
    output: &mut Vec<DrawItem>,
    clip: ClipRect,
    z: i32,
    rect: Rect,
    name: &str,
    color: Rgba,
) {
    let x = rect.x + (rect.width - 16.0) * 0.5;
    let y = rect.y + (rect.height - 16.0) * 0.5;
    let z = z.saturating_add(1);
    let mut line = |a: [f32; 2], b: [f32; 2], width: f32| {
        output.push(item(
            clip,
            z,
            Primitive::Line {
                from: [x + a[0], y + a[1]],
                to: [x + b[0], y + b[1]],
                width,
                color,
            },
        ));
    };
    match name {
        "Object" => {
            for (a, b) in [
                ([2.0, 4.0], [5.0, 4.0]),
                ([2.0, 4.0], [2.0, 7.0]),
                ([11.0, 4.0], [14.0, 4.0]),
                ([14.0, 4.0], [14.0, 7.0]),
                ([2.0, 10.0], [2.0, 13.0]),
                ([2.0, 13.0], [5.0, 13.0]),
                ([14.0, 10.0], [14.0, 13.0]),
                ([11.0, 13.0], [14.0, 13.0]),
            ] {
                line(a, b, 1.2);
            }
        }
        "Modifiers" => {
            line([4.0, 13.0], [11.0, 6.0], 3.0);
            line([10.0, 5.0], [10.0, 2.0], 1.4);
            line([10.0, 5.0], [13.0, 6.0], 1.4);
            line([13.0, 6.0], [14.0, 3.0], 1.4);
        }
        "Outliner" => {
            line([3.0, 3.0], [3.0, 13.0], 1.1);
            for row in [3.0, 8.0, 13.0] {
                line([3.0, row], [7.0, row], 1.1);
                line([9.0, row], [14.0, row], 2.2);
            }
        }
        "Preferences" => {
            for offset in 0..8 {
                let angle = offset as f32 * std::f32::consts::TAU / 8.0;
                let direction = [angle.cos(), angle.sin()];
                line(
                    [8.0 + direction[0] * 5.0, 8.0 + direction[1] * 5.0],
                    [8.0 + direction[0] * 7.0, 8.0 + direction[1] * 7.0],
                    1.8,
                );
            }
            output.push(item(
                clip,
                z,
                Primitive::Ellipse {
                    rect: ClipRect {
                        x: x + 5.0,
                        y: y + 5.0,
                        width: 6.0,
                        height: 6.0,
                    },
                    color,
                },
            ));
        }
        "Geometry Nodes" => {
            line([4.0, 4.0], [11.0, 8.0], 1.3);
            line([4.0, 12.0], [11.0, 8.0], 1.3);
            for point in [[4.0, 4.0], [4.0, 12.0], [12.0, 8.0]] {
                output.push(item(
                    clip,
                    z,
                    Primitive::Ellipse {
                        rect: ClipRect {
                            x: x + point[0] - 2.0,
                            y: y + point[1] - 2.0,
                            width: 4.0,
                            height: 4.0,
                        },
                        color,
                    },
                ));
            }
        }
        _ => {
            line([8.0, 2.0], [8.0, 14.0], 1.2);
            output.push(item(
                clip,
                z,
                Primitive::Ellipse {
                    rect: ClipRect {
                        x: x + 2.0,
                        y: y + 2.0,
                        width: 12.0,
                        height: 12.0,
                    },
                    color,
                },
            ));
        }
    }
}
