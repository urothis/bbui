use super::*;

/// Keep legacy display models synchronized with the shared byte-indexed text buffer.
pub fn sync_text_controls(world: &mut World) {
    let batch = bbui_text::take_text_sync(world);
    if batch
        .as_ref()
        .is_some_and(bbui_core::UiDirtyBatch::is_empty)
    {
        return;
    }
    let mut query = world.query::<(
        Entity,
        &TextBuffer,
        Option<&Label>,
        Option<&TextField>,
        Option<&TextArea>,
    )>();
    let targets: Vec<_> = if let Some(batch) = batch.as_ref().filter(|batch| !batch.all) {
        batch.entities.iter().copied().collect()
    } else {
        query.iter(world).map(|(entity, ..)| entity).collect()
    };
    let buffers: Vec<_> = targets
        .into_iter()
        .filter_map(|entity| {
            let Ok((_, buffer, label, field, area)) = query.get(world, entity) else {
                return None;
            };
            let label = label.map_or_else(String::new, |label| label.text.clone());
            let placeholder = field
                .map(|field| field.placeholder.clone())
                .or_else(|| area.map(|area| area.field.placeholder.clone()))
                .unwrap_or_default();
            let wrap = area.is_some_and(|area| area.wrap);
            let scroll_line = area.map_or(0, |area| area.scroll_line);
            Some((
                entity,
                buffer.as_str().to_owned(),
                buffer.caret(),
                buffer.selection(),
                world.get::<UiReadOnly>(entity).is_some(),
                label,
                placeholder,
                wrap,
                scroll_line,
            ))
        })
        .collect();
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let mut changed_text_layouts = Vec::new();
    for (entity, text, caret, selection, read_only, label, placeholder, wrap, scroll_line) in
        buffers
    {
        let entity_theme = styled_theme(world, entity, &theme);
        let caret_chars = text[..caret.min(text.len())].chars().count();
        let selection_chars = (selection.start != selection.end).then(|| {
            text[..selection.start.min(text.len())].chars().count()
                ..text[..selection.end.min(text.len())].chars().count()
        });
        let mut display_scroll_line = scroll_line;
        if let Some(layout) = world.get::<UiTextLayout>(entity)
            && layout.multiline
        {
            let line_height = text_line_height_pixels(layout, &entity_theme);
            display_scroll_line = (layout.scroll_offset[1].max(0.0) / line_height).round() as usize;
        }
        let field_changed = world.get::<TextField>(entity).is_some_and(|field| {
            field.text != text
                || field.caret != caret_chars
                || field.selection != selection_chars
                || field.read_only != read_only
        });
        if field_changed && let Some(mut field) = world.get_mut::<TextField>(entity) {
            field.text.clone_from(&text);
            field.caret = caret_chars;
            field.selection = selection_chars.clone();
            field.read_only = read_only;
        }
        let area_changed = world.get::<TextArea>(entity).is_some_and(|area| {
            area.field.text != text
                || area.field.caret != caret_chars
                || area.field.selection != selection_chars
                || area.field.read_only != read_only
                || area.scroll_line != display_scroll_line
        });
        if area_changed && let Some(mut area) = world.get_mut::<TextArea>(entity) {
            area.field.text.clone_from(&text);
            area.field.caret = caret_chars;
            area.field.selection = selection_chars;
            area.field.read_only = read_only;
            area.scroll_line = display_scroll_line;
        }
        let prefix = if label.is_empty() {
            String::new()
        } else {
            format!("{label}: ")
        };
        let layout_changed = world.get::<UiTextLayout>(entity).is_some_and(|layout| {
            layout.prefix != prefix
                || layout.placeholder != placeholder
                || (layout.multiline && layout.wrap != wrap)
        });
        if layout_changed && let Some(mut layout) = world.get_mut::<UiTextLayout>(entity) {
            layout.prefix = prefix;
            layout.placeholder = placeholder;
            if layout.multiline {
                layout.wrap = wrap;
            }
        }
        if field_changed || area_changed || layout_changed {
            changed_text_layouts.push(entity);
        }
    }
    if let Some(mut dirty) = world.get_resource_mut::<UiDirty>() {
        for entity in changed_text_layouts {
            dirty.mark(UiDirtyDomain::TextLayout, entity);
        }
    }
}

pub(super) fn scroll_text_area(
    world: &mut World,
    target: Entity,
    delta_y: f32,
    unit: bbui_core::UiWheelUnit,
) {
    if delta_y == 0.0 {
        return;
    }
    let base_theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let theme = styled_theme(world, target, &base_theme);
    let line_height = world
        .get::<UiTextLayout>(target)
        .map_or(theme.font_size * theme.line_height, |layout| {
            text_line_height_pixels(layout, &theme)
        });
    let steps = match unit {
        bbui_core::UiWheelUnit::Lines => delta_y.abs().ceil(),
        bbui_core::UiWheelUnit::Pixels => (delta_y.abs() / line_height).ceil().max(1.0),
    } as usize;
    let scroll_line = {
        let Some(mut area) = world.get_mut::<TextArea>(target) else {
            return;
        };
        area.scroll_line = if delta_y > 0.0 {
            area.scroll_line.saturating_sub(steps)
        } else {
            area.scroll_line.saturating_add(steps)
        };
        area.scroll_line
    };
    if let Some(mut layout) = world.get_mut::<UiTextLayout>(target)
        && layout.multiline
    {
        layout.scroll_offset[1] = line_height * scroll_line as f32;
    }
}

pub(super) fn text_line_height_pixels(layout: &UiTextLayout, theme: &Theme) -> f32 {
    let font_size = layout
        .font
        .as_ref()
        .map_or(theme.font_size, |font| match font.font_size {
            FontSize::Px(size) => size,
            FontSize::Rem(size) => size * theme.font_size,
            _ => theme.font_size,
        });
    match layout
        .line_height
        .unwrap_or(LineHeight::RelativeToFont(theme.line_height))
    {
        LineHeight::Px(pixels) => pixels.max(1.0),
        LineHeight::RelativeToFont(relative) => (font_size * relative).max(1.0),
    }
}
