use super::*;

pub(super) fn paint_text_controls(paint: &mut PaintEntityContext<'_>) -> bool {
    let world = &mut *paint.world;
    let entity = paint.entity;
    let layout = paint.layout;
    let rect = paint.rect;
    let clip = paint.clip;
    let z = paint.z;
    let theme = paint.theme;
    let enabled = paint.enabled;
    let camera_items = &mut *paint.output;

    if let Some(label) = world.get::<Label>(entity)
        && (world.get::<ListModel<String>>(entity).is_some()
            || world.get::<TreeModel<String>>(entity).is_some()
            || world.get::<PreviewGrid>(entity).is_some())
    {
        let count = world
            .get::<ListModel<String>>(entity)
            .map(|model| model.items.len())
            .or_else(|| {
                world
                    .get::<TreeModel<String>>(entity)
                    .map(|model| model.nodes.len())
            })
            .or_else(|| {
                world
                    .get::<PreviewGrid>(entity)
                    .map(|model| model.items.len())
            })
            .unwrap_or(0);
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: theme.corner_radius,
                color: theme.background,
            },
        ));
        camera_items.push(item(
            clip,
            z.saturating_add(1),
            Primitive::RoundedRect {
                rect: ClipRect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: theme.spacing.collection.header_height.min(rect.height),
                },
                radius: theme.corner_radius,
                color: theme.surface,
            },
        ));
        camera_items.push(item(
            clip,
            z.saturating_add(2),
            styled_text_primitive_for_entity(
                world,
                entity,
                clip,
                TextPlacement {
                    origin: [
                        rect.x + theme.spacing.collection.padding,
                        rect.y + theme.spacing.editor.padding_y,
                    ],
                    text: format!("{}  ·  {count}", label.text),
                },
                theme.text,
                theme,
                false,
            ),
        ));
        camera_items.push(item(
            clip,
            z.saturating_add(2),
            Primitive::Line {
                from: [rect.x, rect.y + theme.spacing.collection.header_height],
                to: [
                    rect.x + rect.width,
                    rect.y + theme.spacing.collection.header_height,
                ],
                width: 1.0,
                color: theme.border,
            },
        ));
        return true;
    }
    if let Some(label) = world.get::<Label>(entity) {
        let composite = world.get::<NumericField>(entity).is_some()
            || world.get::<Slider>(entity).is_some()
            || world.get::<TextField>(entity).is_some()
            || world.get::<TextArea>(entity).is_some()
            || world.get::<Dropdown>(entity).is_some();
        if !composite {
            camera_items.push(item(
                clip,
                z,
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [rect.x, rect.y],
                        text: label.text.clone(),
                    },
                    theme.text,
                    theme,
                    false,
                ),
            ));
            return true;
        }
    }
    if let Some(slider) = world.get::<Slider>(entity) {
        let label = world
            .get::<Label>(entity)
            .map_or_else(String::new, |label| label.text.clone());
        let focused = world
            .get_resource::<UiFocus>()
            .and_then(|focus| focus.state(layout.window))
            .is_some_and(|state| state.focused == Some(entity));
        let dragging = world
            .get_resource::<WidgetInteractionState>()
            .and_then(|state| state.slider_drags.get(&layout.window))
            .is_some_and(|drag| drag.target == entity);
        paint_slider(
            SliderPaint {
                slider,
                label: &label,
                rect,
                clip,
                z,
                enabled: enabled && slider.enabled,
                focused,
                dragging,
                filled: world
                    .get::<UiRole>(entity)
                    .is_some_and(|role| role.0 == "filled-slider"),
                theme,
            },
            camera_items,
        );
        return true;
    }
    if let Some(field) = world.get::<NumericField>(entity) {
        let scrub_selected = world
            .get_resource::<WidgetInteractionState>()
            .is_some_and(|state| {
                state
                    .numeric_scrubs
                    .values()
                    .any(|scrub| scrub.targets.contains(&entity))
            });
        let axis_suffix = world
            .get::<UiRole>(entity)
            .and_then(|role| match role.0.as_str() {
                "vector-axis-meters" => Some(" m"),
                "vector-axis-degrees" => Some("°"),
                _ => None,
            });
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: if axis_suffix.is_some() {
                    1.0
                } else {
                    theme.corner_radius
                },
                color: if scrub_selected {
                    theme.color(ThemeColorToken::EditorActiveFieldSurface)
                } else if axis_suffix.is_some() {
                    theme.surface.mix(theme.text, 0.12)
                } else {
                    theme.surface
                },
            },
        ));
        let label = world
            .get::<Label>(entity)
            .map_or_else(String::new, |label| label.text.clone());
        if let Some(suffix) = axis_suffix {
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [rect.x + 14.0, rect.y + theme.spacing.control.padding_y],
                        text: label,
                    },
                    theme.text_muted,
                    theme,
                    false,
                ),
            ));
            let value = format!("{}{}", field.display(), suffix);
            let estimated_width = value.chars().count() as f32 * theme.font_size * 0.53;
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [
                            rect.x + (rect.width - estimated_width - 12.0).max(42.0),
                            rect.y + theme.spacing.control.padding_y,
                        ],
                        text: value,
                    },
                    theme.text,
                    theme,
                    false,
                ),
            ));
            return true;
        }
        let content = if label.is_empty() {
            field.display()
        } else {
            format!("{label}: {}", field.display())
        };
        let content = if let Some(error) = &field.expression_error {
            format!("{content}  ({error})")
        } else {
            content
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
                        rect.x + theme.spacing.control.padding_x,
                        rect.y + theme.spacing.control.padding_y,
                    ],
                    text: content,
                },
                if field.expression_error.is_some() {
                    theme.error
                } else {
                    theme.text
                },
                theme,
                false,
            ),
        ));
        if field.expression_error.is_some() {
            let width = theme.spacing.control.error_outline_width;
            if width.is_finite() && width > 0.0 {
                for (from, to) in [
                    ([rect.x, rect.y], [rect.x + rect.width, rect.y]),
                    (
                        [rect.x + rect.width, rect.y],
                        [rect.x + rect.width, rect.y + rect.height],
                    ),
                    (
                        [rect.x + rect.width, rect.y + rect.height],
                        [rect.x, rect.y + rect.height],
                    ),
                    ([rect.x, rect.y + rect.height], [rect.x, rect.y]),
                ] {
                    camera_items.push(item(
                        clip,
                        z.saturating_add(2),
                        Primitive::Line {
                            from,
                            to,
                            width,
                            color: theme.error,
                        },
                    ));
                }
            }
        }
        return true;
    }
    if let Some(field) = world.get::<TextField>(entity) {
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: theme.corner_radius,
                color: theme.surface,
            },
        ));
        let live_text = world
            .get::<TextBuffer>(entity)
            .map_or(field.text.as_str(), TextBuffer::as_str);
        let content = if live_text.is_empty() {
            field.placeholder.clone()
        } else {
            live_text.to_owned()
        };
        let label = world
            .get::<Label>(entity)
            .map_or_else(String::new, |label| label.text.clone());
        let fallback = if label.is_empty() {
            content
        } else {
            format!("{label}: {content}")
        };
        paint_text_editor(
            world,
            entity,
            TextEditorPaint {
                clip,
                z: z.saturating_add(1),
                widget_rect: rect,
                fallback_text: fallback,
                theme,
            },
            camera_items,
        );
        return true;
    }
    if let Some(area) = world.get::<TextArea>(entity) {
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: theme.corner_radius,
                color: theme.surface,
            },
        ));
        let live_text = world
            .get::<TextBuffer>(entity)
            .map_or(area.field.text.as_str(), TextBuffer::as_str);
        let label = world
            .get::<Label>(entity)
            .map_or_else(String::new, |label| label.text.clone());
        let fallback = if label.is_empty() {
            live_text.to_owned()
        } else {
            format!("{label}: {live_text}")
        };
        paint_text_editor(
            world,
            entity,
            TextEditorPaint {
                clip,
                z: z.saturating_add(1),
                widget_rect: rect,
                fallback_text: fallback,
                theme,
            },
            camera_items,
        );
        return true;
    }
    false
}
