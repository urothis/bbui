use super::*;

pub(in crate::actions) fn paint_tooltip_overlay(
    world: &World,
    owner: Entity,
    tooltip: &Tooltip,
    window: Entity,
    theme: &Theme,
    output: &mut Vec<DrawItem>,
) {
    let Some(hover) = world
        .get_resource::<WidgetTooltips>()
        .and_then(|tooltips| tooltips.0.get(&window))
        .filter(|hover| hover.owner == owner)
    else {
        return;
    };
    let Some(anchor) = world.get::<UiLayoutResult>(hover.trigger).copied() else {
        return;
    };
    let viewport = world
        .iter_entities()
        .find_map(|entity| {
            entity
                .get::<UiLayoutRoot>()
                .filter(|root| root.window == window)
                .map(|root| root.viewport)
        })
        .unwrap_or(Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        });
    let spacing = theme.spacing.tooltip;
    let padding_x = spacing.padding_x.max(4.0);
    let padding_y = spacing.padding_y.max(3.0);
    let max_content_width = spacing
        .maximum_width
        .min((viewport.width - padding_x * 2.0).max(1.0));
    let estimated_text_width = tooltip.text.chars().count() as f32 * theme.font_size * 0.56;
    let content_width = estimated_text_width.clamp(1.0, max_content_width);
    let line_height = theme.font_size * theme.line_height;
    let line_count = (estimated_text_width / max_content_width).ceil().max(1.0);
    let height = line_height * line_count + padding_y * 2.0;
    let width = content_width + padding_x * 2.0;
    let margin = spacing
        .viewport_margin
        .min(viewport.width * 0.5)
        .min(viewport.height * 0.5);
    let below = anchor.rect.y + anchor.rect.height + spacing.anchor_gap;
    let y = if below + height <= viewport.y + viewport.height - margin {
        below
    } else {
        anchor.rect.y - height - spacing.anchor_gap
    }
    .clamp(
        viewport.y + margin,
        (viewport.y + viewport.height - height - margin).max(viewport.y + margin),
    );
    let rect = Rect {
        x: anchor.rect.x.clamp(
            viewport.x + margin,
            (viewport.x + viewport.width - width - margin).max(viewport.x + margin),
        ),
        y,
        width,
        height: height.min(viewport.height - margin * 2.0),
    };
    let clip = ClipRect::from(viewport);
    let z = anchor
        .z
        .max(
            world
                .get::<UiLayoutResult>(owner)
                .map_or(0, |layout| layout.z),
        )
        .saturating_add(200);
    let shadow_clip = popup_viewport_clip(
        world,
        window,
        world
            .get::<UiLayoutResult>(owner)
            .and_then(|layout| layout.clip),
        viewport,
    );
    push_popup_shadow(
        rect,
        theme.corner_radius,
        theme.surface,
        shadow_clip,
        z.saturating_sub(1),
        theme,
        output,
    );
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: theme.corner_radius,
            color: theme.surface,
        },
    ));
    push_rect_outline(rect, theme.border, clip, z.saturating_add(1), output);
    let content_clip = ClipRect::from(Rect {
        x: rect.x + padding_x,
        y: rect.y + padding_y,
        width: (rect.width - padding_x * 2.0).max(1.0),
        height: (rect.height - padding_y * 2.0).max(1.0),
    });
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive_for_entity(
            world,
            owner,
            content_clip,
            TextPlacement {
                origin: [rect.x + padding_x, rect.y + padding_y],
                text: tooltip.text.clone(),
            },
            theme.text,
            theme,
            true,
        ),
    ));
}

pub(in crate::actions) struct TextEditorPaint<'a> {
    pub(in crate::actions) clip: ClipRect,
    pub(in crate::actions) z: i32,
    pub(in crate::actions) widget_rect: Rect,
    pub(in crate::actions) fallback_text: String,
    pub(in crate::actions) theme: &'a Theme,
}

pub(in crate::actions) fn paint_text_editor(
    world: &World,
    target: Entity,
    paint: TextEditorPaint<'_>,
    output: &mut Vec<DrawItem>,
) {
    let TextEditorPaint {
        clip,
        z,
        widget_rect,
        fallback_text,
        theme,
    } = paint;
    let Some(text_layout) = world.get::<UiTextLayout>(target) else {
        let origin = [
            widget_rect.x + theme.spacing.editor.padding_x,
            widget_rect.y + theme.spacing.editor.padding_y,
        ];
        let wrap = world.get::<TextArea>(target).is_some_and(|area| area.wrap);
        output.push(item(
            clip,
            z,
            styled_text_primitive_for_entity(
                world,
                target,
                clip,
                TextPlacement {
                    origin,
                    text: fallback_text,
                },
                theme.text,
                theme,
                wrap,
            ),
        ));
        return;
    };
    let text = text_layout
        .display_text()
        .map_or(fallback_text, str::to_owned);
    let origin = text_layout.text_origin().unwrap_or([
        widget_rect.x + theme.spacing.editor.padding_x,
        widget_rect.y + theme.spacing.editor.padding_y,
    ]);
    let horizontal_padding = world
        .get::<UiLayoutNode>(target)
        .map_or(0.0, |node| node.padding.left + node.padding.right);
    let max_width =
        (widget_rect.width - horizontal_padding - text_layout.inset[0].max(0.0) * 2.0).max(0.0);
    if let Some(buffer) = world.get::<TextBuffer>(target) {
        let selection_color = theme.color(ThemeColorToken::TextSelectionOverlay);
        for selection in text_layout.selection_rects(buffer) {
            let selection = Rect {
                x: selection.x,
                y: selection.y,
                width: selection.width,
                height: selection.height,
            };
            if let Some(selection) = selection.intersection(widget_rect) {
                output.push(item(
                    clip,
                    z,
                    Primitive::RoundedRect {
                        rect: ClipRect::from(selection),
                        radius: 1.0,
                        color: selection_color,
                    },
                ));
            }
        }
    }
    output.push(item(
        clip,
        z.saturating_add(1),
        Primitive::TextWrapped {
            origin,
            text,
            color: theme.text,
            max_width,
            wrap: text_layout.multiline && text_layout.wrap,
            style: Some(TextRenderStyle {
                font: Some(resolve_text_font(
                    world,
                    target,
                    text_layout
                        .font
                        .clone()
                        .unwrap_or_else(|| ui_default_text_font(theme.font_size)),
                )),
                line_height: text_layout
                    .line_height
                    .or(Some(LineHeight::RelativeToFont(theme.line_height))),
                letter_spacing: text_layout.letter_spacing,
                justify: text_layout.justify,
                line_break: text_layout.line_break.unwrap_or(
                    if text_layout.multiline && text_layout.wrap {
                        LineBreak::WordBoundary
                    } else {
                        LineBreak::NoWrap
                    },
                ),
            }),
        },
    ));
    for preedit in text_layout.preedit_rects() {
        let y = preedit.y + preedit.height.max(1.0) - 1.0;
        output.push(item(
            clip,
            z.saturating_add(2),
            Primitive::Line {
                from: [preedit.x, y],
                to: [preedit.x + preedit.width, y],
                width: 1.0,
                color: theme.color(ThemeColorToken::EditorImeUnderline),
            },
        ));
    }
    let focused = world
        .get_resource::<UiFocus>()
        .and_then(|focus| focus.state(world.get::<UiLayoutResult>(target)?.window))
        .is_some_and(|state| state.focused == Some(target));
    if focused
        && let Some(buffer) = world.get::<TextBuffer>(target)
        && let Some(caret) = text_layout.caret_rect(buffer)
    {
        output.push(item(
            clip,
            z.saturating_add(3),
            Primitive::Line {
                from: [caret.x, caret.y],
                to: [caret.x, caret.y + caret.height],
                width: 1.0,
                color: theme.color(ThemeColorToken::EditorCaret),
            },
        ));
    }
}

pub(in crate::actions) fn file_preview_glyph(preview: crate::PreviewKind) -> &'static str {
    match preview {
        crate::PreviewKind::Directory => "▸",
        crate::PreviewKind::Image => "▧",
        crate::PreviewKind::Svg => "◇",
        crate::PreviewKind::AnimatedImage => "◉",
        crate::PreviewKind::Video => "▶",
        crate::PreviewKind::Text => "≡",
        crate::PreviewKind::Other => "·",
    }
}

pub(in crate::actions) fn format_file_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
