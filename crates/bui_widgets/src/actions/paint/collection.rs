use super::*;

pub(in crate::actions) fn paint_collection_item(
    control: &CollectionItemControl,
    rect: Rect,
    clip: ClipRect,
    z: i32,
    enabled: bool,
    theme: &Theme,
    output: &mut Vec<DrawItem>,
) {
    let enabled = enabled && control.enabled;
    let background = if !enabled {
        theme.color(ThemeColorToken::CollectionDisabledSurface)
    } else if control.selected {
        theme.color(ThemeColorToken::CollectionSelectedSurface)
    } else {
        theme.surface
    };
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: if control.kind == CollectionItemKind::PreviewGrid {
                theme.corner_radius.max(2.0)
            } else {
                theme.corner_radius.min(3.0)
            },
            color: background,
        },
    ));

    let foreground = if enabled {
        theme.text
    } else {
        theme.color(ThemeColorToken::DisabledText)
    };
    if control.kind == CollectionItemKind::PreviewGrid {
        let spacing = theme.spacing.collection;
        let padding = spacing.preview_padding;
        let preview_rect = Rect {
            x: rect.x + padding,
            y: rect.y + padding,
            width: (rect.width - padding * 2.0).max(0.0),
            height: (rect.height - spacing.preview_caption_height - padding * 2.0).max(0.0),
        };
        if let Some(asset_id) = control.asset_id {
            output.push(item(
                clip,
                z.saturating_add(1),
                Primitive::Image {
                    rect: ClipRect::from(preview_rect),
                    asset_id,
                },
            ));
        } else {
            output.push(item(
                clip,
                z.saturating_add(1),
                Primitive::RoundedRect {
                    rect: ClipRect::from(preview_rect),
                    radius: theme.corner_radius.min(4.0),
                    color: theme.color(ThemeColorToken::PreviewPlaceholder),
                },
            ));
            let swatch = Rect {
                x: preview_rect.x + spacing.padding,
                y: preview_rect.y + spacing.padding,
                width: (preview_rect.width - spacing.padding * 2.0).max(0.0),
                height: (preview_rect.height - spacing.padding * 2.0).max(0.0),
            };
            if swatch.width > 8.0 && swatch.height > 8.0 {
                output.push(item(
                    clip,
                    z.saturating_add(2),
                    Primitive::RoundedRect {
                        rect: ClipRect::from(swatch),
                        radius: theme.corner_radius.min(4.0),
                        color: Rgba(control.tint[0], control.tint[1], control.tint[2], 1.0),
                    },
                ));
            }
        }
        if control.selected {
            output.push(item(
                clip,
                z.saturating_add(3),
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect.inset(1.0, 1.0, 1.0, 1.0)),
                    radius: theme.corner_radius.max(2.0),
                    color: theme.color(ThemeColorToken::CollectionSelectionOverlay),
                },
            ));
        }
        output.push(item(
            clip,
            z.saturating_add(4),
            styled_text_primitive(
                clip,
                [
                    rect.x + theme.spacing.collection.padding,
                    rect.y + rect.height - spacing.preview_caption_height
                        + theme.spacing.list.padding_y,
                ],
                control.label.clone(),
                foreground,
                theme,
                false,
            ),
        ));
        return;
    }

    let label_x = rect.x
        + theme.spacing.list.padding_x
        + if control.kind == CollectionItemKind::Tree {
            control.depth as f32 * theme.spacing.list.indent
        } else {
            0.0
        };
    let text_x = if control.kind == CollectionItemKind::Tree {
        let disclosure = if control.has_children {
            if control.expanded { "▾" } else { "▸" }
        } else {
            "·"
        };
        output.push(item(
            clip,
            z.saturating_add(1),
            styled_text_primitive(
                clip,
                [label_x, rect.y + theme.spacing.list.padding_y],
                disclosure.to_owned(),
                if control.has_children {
                    foreground
                } else {
                    theme.text_muted
                },
                theme,
                false,
            ),
        ));
        label_x + theme.spacing.list.indent
    } else {
        label_x
    };
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive(
            clip,
            [text_x, rect.y + theme.spacing.list.padding_y],
            control.label.clone(),
            foreground,
            theme,
            false,
        ),
    ));
}
