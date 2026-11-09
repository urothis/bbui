use super::*;

pub(in crate::actions) fn styled_text_primitive(
    clip: ClipRect,
    origin: [f32; 2],
    text: String,
    color: Rgba,
    theme: &Theme,
    wrap: bool,
) -> Primitive {
    Primitive::TextWrapped {
        origin,
        text,
        color,
        max_width: (clip.width - (origin[0] - clip.x)).max(1.0),
        wrap,
        style: Some(TextRenderStyle {
            font: Some(ui_default_text_font(theme.font_size)),
            line_height: Some(LineHeight::RelativeToFont(theme.line_height)),
            letter_spacing: bevy_text::LetterSpacing::default(),
            justify: bevy_text::Justify::default(),
            line_break: if wrap {
                LineBreak::WordBoundary
            } else {
                LineBreak::NoWrap
            },
        }),
    }
}

pub(in crate::actions) struct TextPlacement {
    pub(in crate::actions) origin: [f32; 2],
    pub(in crate::actions) text: String,
}

pub(in crate::actions) fn styled_text_primitive_for_entity(
    world: &World,
    entity: Entity,
    clip: ClipRect,
    placement: TextPlacement,
    color: Rgba,
    theme: &Theme,
    wrap: bool,
) -> Primitive {
    let mut primitive =
        styled_text_primitive(clip, placement.origin, placement.text, color, theme, wrap);
    if let Primitive::TextWrapped {
        style: Some(style), ..
    } = &mut primitive
        && let Some(font) = style.font.take()
    {
        style.font = Some(resolve_text_font(world, entity, font));
    }
    primitive
}

pub(in crate::actions) fn is_visible(world: &World, entity: Entity) -> bool {
    let mut current = Some(entity);
    let mut remaining_entities = world.entities().len();
    while let Some(at) = current {
        if remaining_entities == 0 || world.get::<bui_core::UiNode>(at).is_none() {
            return false;
        }
        remaining_entities -= 1;
        if world.get::<UiVisible>(at).is_some_and(|visible| !visible.0) {
            return false;
        }
        current = world.get::<UiParent>(at).map(|parent| parent.0);
    }
    true
}
