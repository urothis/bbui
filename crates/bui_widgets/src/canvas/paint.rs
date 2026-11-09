use super::*;

/// Draws a canvas from the same transform and clip used by hit testing.
pub fn paint_canvas(
    canvas: &Canvas,
    rect: UiRect,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    images: Option<&CanvasImageAssets>,
    output: &mut Vec<DrawItem>,
) {
    paint_canvas_objects(
        canvas,
        0..canvas.objects.len(),
        rect,
        clip,
        z,
        theme,
        images,
        output,
    );
}

/// Paint only the supplied object positions after building a spatial query for the viewport.
/// The Canvas widget uses per-object child fragments; this entry point is useful for direct
/// canvas rendering and the stress harness.
#[expect(
    clippy::too_many_arguments,
    reason = "Direct Canvas painting keeps the model, sparse positions, viewport, theme, assets, and output explicit"
)]
pub fn paint_canvas_indexed(
    canvas: &Canvas,
    object_indices: &[usize],
    rect: UiRect,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    images: Option<&CanvasImageAssets>,
    output: &mut Vec<DrawItem>,
) {
    paint_canvas_objects(
        canvas,
        object_indices.iter().copied(),
        rect,
        clip,
        z,
        theme,
        images,
        output,
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "The shared Canvas loop carries the model, object positions, viewport, theme, assets, and output"
)]
pub(super) fn paint_canvas_objects(
    canvas: &Canvas,
    object_indices: impl IntoIterator<Item = usize>,
    rect: UiRect,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    images: Option<&CanvasImageAssets>,
    output: &mut Vec<DrawItem>,
) {
    let viewport = UiRect::new(clip.x, clip.y, clip.width, clip.height).intersection(rect);
    paint_canvas_background(rect, clip, z, theme, output);
    for object_index in object_indices {
        let Some(object) = canvas.objects.get(object_index) else {
            continue;
        };
        if !object.visible {
            continue;
        }
        let bounds = object.bounds();
        let position = canvas.world_to_screen([bounds.x, bounds.y]);
        // Include the selection stroke and line width at the viewport edge.
        let screen_bounds = UiRect::new(
            rect.x + position[0] - 8.0,
            rect.y + position[1] - 8.0,
            (bounds.width * canvas.zoom).max(1.0) + 16.0,
            (bounds.height * canvas.zoom).max(1.0) + 16.0,
        );
        if !screen_bounds.is_finite()
            || !viewport.is_some_and(|viewport| viewport.intersection(screen_bounds).is_some())
        {
            continue;
        }
        let to_screen = |point: [f32; 2]| {
            let local = canvas.world_to_screen(point);
            [rect.x + local[0], rect.y + local[1]]
        };
        let origin = to_screen(object.origin);
        let color = Rgba(
            object.color[0],
            object.color[1],
            object.color[2],
            object.color[3],
        );
        let primitive = match &object.shape {
            CanvasShape::Rectangle { size } => Some(Primitive::RoundedRect {
                rect: ClipRect {
                    x: origin[0],
                    y: origin[1],
                    width: size[0] * canvas.zoom,
                    height: size[1] * canvas.zoom,
                },
                radius: 0.0,
                color,
            }),
            CanvasShape::Ellipse { size } => Some(Primitive::Ellipse {
                rect: ClipRect {
                    x: origin[0],
                    y: origin[1],
                    width: size[0] * canvas.zoom,
                    height: size[1] * canvas.zoom,
                },
                color,
            }),
            CanvasShape::Line { end } => Some(Primitive::Line {
                from: origin,
                to: to_screen(*end),
                width: (2.0 * canvas.zoom).clamp(1.0, 16.0),
                color,
            }),
            CanvasShape::Image { size, asset } => images
                .filter(|images| images.state(asset) == Some(&CanvasImageState::Ready))
                .and_then(|images| images.id(asset))
                .map(|asset_id| Primitive::Icon {
                    rect: ClipRect {
                        x: origin[0],
                        y: origin[1],
                        width: size[0] * canvas.zoom,
                        height: size[1] * canvas.zoom,
                    },
                    asset_id,
                    tint: color,
                }),
        };
        if let Some(primitive) = primitive {
            output.push(DrawItem {
                clip,
                z: z.saturating_add(1),
                primitive,
            });
        }
        if let CanvasShape::Image { asset, .. } = &object.shape
            && !matches!(
                images.and_then(|images| images.state(asset)),
                Some(CanvasImageState::Ready)
            )
        {
            let message = match images.and_then(|images| images.state(asset)) {
                Some(CanvasImageState::Failed(reason)) => format!("Image error: {reason}"),
                _ => format!("Loading image: {asset}"),
            };
            output.push(DrawItem {
                clip,
                z: z.saturating_add(1),
                primitive: Primitive::Text {
                    origin: [origin[0] + 4.0, origin[1] + 4.0],
                    text: message,
                    color: theme.text_muted,
                },
            });
        }
        if canvas.selected.contains(&object.id) {
            let bounds = object.bounds();
            let a = to_screen([bounds.x, bounds.y]);
            let b = to_screen([bounds.right(), bounds.bottom()]);
            let x0 = a[0].min(b[0]);
            let y0 = a[1].min(b[1]);
            let x1 = a[0].max(b[0]);
            let y1 = a[1].max(b[1]);
            if x1 > x0 && y1 > y0 {
                output.push(DrawItem {
                    clip,
                    z: z.saturating_add(2),
                    primitive: Primitive::RoundedRect {
                        rect: ClipRect {
                            x: x0,
                            y: y0,
                            width: x1 - x0,
                            height: y1 - y0,
                        },
                        radius: 0.0,
                        color: theme.color(ThemeColorToken::CanvasSelectionOverlay),
                    },
                });
            }
            for (from, to) in [
                ([x0, y0], [x1, y0]),
                ([x1, y0], [x1, y1]),
                ([x1, y1], [x0, y1]),
                ([x0, y1], [x0, y0]),
            ] {
                output.push(DrawItem {
                    clip,
                    z: z.saturating_add(2),
                    primitive: Primitive::Line {
                        from,
                        to,
                        width: 1.5,
                        color: theme.selection,
                    },
                });
            }
        }
    }
}

pub(crate) fn paint_canvas_background(
    rect: UiRect,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    output: &mut Vec<DrawItem>,
) {
    output.push(DrawItem {
        clip,
        z,
        primitive: Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: theme.corner_radius,
            color: theme.background,
        },
    });
}

#[expect(
    clippy::too_many_arguments,
    reason = "One stable Canvas fragment needs its model, object, viewport, z, theme, assets, and output"
)]
pub(crate) fn paint_canvas_object_fragment(
    canvas: &Canvas,
    object: &CanvasObject,
    rect: UiRect,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    images: Option<&CanvasImageAssets>,
    output: &mut Vec<DrawItem>,
) {
    if !object.visible {
        return;
    }
    let viewport = UiRect::new(clip.x, clip.y, clip.width, clip.height).intersection(rect);
    let bounds = object.bounds();
    let position = canvas.world_to_screen([bounds.x, bounds.y]);
    let screen_bounds = UiRect::new(
        rect.x + position[0] - 8.0,
        rect.y + position[1] - 8.0,
        (bounds.width * canvas.zoom).max(1.0) + 16.0,
        (bounds.height * canvas.zoom).max(1.0) + 16.0,
    );
    if !screen_bounds.is_finite()
        || !viewport.is_some_and(|viewport| viewport.intersection(screen_bounds).is_some())
    {
        return;
    }
    let to_screen = |point: [f32; 2]| {
        let local = canvas.world_to_screen(point);
        [rect.x + local[0], rect.y + local[1]]
    };
    let origin = to_screen(object.origin);
    let color = Rgba(
        object.color[0],
        object.color[1],
        object.color[2],
        object.color[3],
    );
    let primitive = match &object.shape {
        CanvasShape::Rectangle { size } => Some(Primitive::RoundedRect {
            rect: ClipRect {
                x: origin[0],
                y: origin[1],
                width: size[0] * canvas.zoom,
                height: size[1] * canvas.zoom,
            },
            radius: 0.0,
            color,
        }),
        CanvasShape::Ellipse { size } => Some(Primitive::Ellipse {
            rect: ClipRect {
                x: origin[0],
                y: origin[1],
                width: size[0] * canvas.zoom,
                height: size[1] * canvas.zoom,
            },
            color,
        }),
        CanvasShape::Line { end } => Some(Primitive::Line {
            from: origin,
            to: to_screen(*end),
            width: (2.0 * canvas.zoom).clamp(1.0, 16.0),
            color,
        }),
        CanvasShape::Image { size, asset } => images
            .filter(|images| images.state(asset) == Some(&CanvasImageState::Ready))
            .and_then(|images| images.id(asset))
            .map(|asset_id| Primitive::Icon {
                rect: ClipRect {
                    x: origin[0],
                    y: origin[1],
                    width: size[0] * canvas.zoom,
                    height: size[1] * canvas.zoom,
                },
                asset_id,
                tint: color,
            }),
    };
    if let Some(primitive) = primitive {
        output.push(DrawItem {
            clip,
            z: z.saturating_add(1),
            primitive,
        });
    }
    if let CanvasShape::Image { asset, .. } = &object.shape
        && !matches!(
            images.and_then(|images| images.state(asset)),
            Some(CanvasImageState::Ready)
        )
    {
        let message = match images.and_then(|images| images.state(asset)) {
            Some(CanvasImageState::Failed(reason)) => format!("Image error: {reason}"),
            _ => format!("Loading image: {asset}"),
        };
        output.push(DrawItem {
            clip,
            z: z.saturating_add(1),
            primitive: Primitive::Text {
                origin: [origin[0] + 4.0, origin[1] + 4.0],
                text: message,
                color: theme.text_muted,
            },
        });
    }
    if canvas.selected.contains(&object.id) {
        let a = to_screen([bounds.x, bounds.y]);
        let b = to_screen([bounds.right(), bounds.bottom()]);
        let x0 = a[0].min(b[0]);
        let y0 = a[1].min(b[1]);
        let x1 = a[0].max(b[0]);
        let y1 = a[1].max(b[1]);
        if x1 > x0 && y1 > y0 {
            output.push(DrawItem {
                clip,
                z: z.saturating_add(2),
                primitive: Primitive::RoundedRect {
                    rect: ClipRect {
                        x: x0,
                        y: y0,
                        width: x1 - x0,
                        height: y1 - y0,
                    },
                    radius: 0.0,
                    color: theme.color(ThemeColorToken::CanvasSelectionOverlay),
                },
            });
        }
        for (from, to) in [
            ([x0, y0], [x1, y0]),
            ([x1, y0], [x1, y1]),
            ([x1, y1], [x0, y1]),
            ([x0, y1], [x0, y0]),
        ] {
            output.push(DrawItem {
                clip,
                z: z.saturating_add(2),
                primitive: Primitive::Line {
                    from,
                    to,
                    width: 1.5,
                    color: theme.selection,
                },
            });
        }
    }
}
