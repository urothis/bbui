use super::*;

/// Shape text against final BBUI bounds after layout and before widget paint.
pub fn update_text_layouts(world: &mut World) {
    let batch = take_text_work(world, UiDirtyDomain::TextLayout);
    if batch.as_ref().is_some_and(UiDirtyBatch::is_empty) {
        return;
    }
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let subset = batch
        .as_ref()
        .filter(|batch| !batch.all)
        .map(|batch| &batch.entities);
    let entities = text_entities(world, true, subset);
    if entities.is_empty() {
        settle_text_cursor(world);
        return;
    }

    world.resource_scope(|world, mut pipeline: Mut<TextPipeline>| {
        world.resource_scope(|world, fonts: Mut<Assets<Font>>| {
            world.resource_scope(|world, mut font_cx: Mut<FontCx>| {
                world.resource_scope(|world, mut layout_cx: Mut<LayoutCx>| {
                    for target in entities {
                        let Some(result) = target.result else {
                            continue;
                        };
                        let Some(mut text_layout) =
                            world.get::<UiTextLayout>(target.entity).cloned()
                        else {
                            continue;
                        };
                        let visible_text = text_layout.visible_text(&target.buffer);
                        let display_text = visible_text.text;
                        let prefix_bytes = text_layout.prefix.len();
                        let value_len = target.buffer.len_bytes();
                        let padding = target.node.map_or([0.0; 2], |node| {
                            [
                                node.padding.left + node.padding.right,
                                node.padding.top + node.padding.bottom,
                            ]
                        });
                        let content_width =
                            (result.rect.width - padding[0] - text_layout.inset[0].max(0.0) * 2.0)
                                .max(0.0);
                        let content_height =
                            (result.rect.height - padding[1] - text_layout.inset[1].max(0.0) * 2.0)
                                .max(0.0);
                        let content_viewport = [content_width, content_height];
                        let computed_style = world.get::<UiComputedStyle>(target.entity).cloned();
                        let (viewport, scale_factor) =
                            logical_viewport_metrics(world, target.entity, Some(result));
                        let font = resolve_text_font(
                            world,
                            target.entity,
                            text_layout.font(&theme, computed_style.as_ref()),
                        );
                        let key = text_layout.cache_key(
                            &display_text,
                            content_width,
                            ShapeContext {
                                font,
                                line_height: text_layout
                                    .line_height(&theme, computed_style.as_ref()),
                                scale_factor,
                                viewport_size: viewport,
                            },
                        );
                        let focused = world
                            .get_resource::<UiFocus>()
                            .and_then(|focus| focus.state(result.window))
                            .is_some_and(|state| state.focused == Some(target.entity));
                        let geometry_changed = text_layout.display_text != display_text
                            || text_layout.prefix_bytes != prefix_bytes
                            || text_layout.value_len != value_len
                            || text_layout.replacement_range != visible_text.replacement_range
                            || text_layout.preedit_range != visible_text.preedit_range
                            || text_layout.preedit_caret != visible_text.preedit_caret
                            || text_layout.buffer_selection != Some(target.buffer.selection())
                            || text_layout.scroll_viewport != Some(content_viewport)
                            || text_layout.last_focused != Some(focused);
                        text_layout.display_text.clone_from(&display_text);
                        text_layout.prefix_bytes = prefix_bytes;
                        text_layout.value_len = value_len;
                        text_layout.replacement_range = visible_text.replacement_range;
                        text_layout.preedit_range = visible_text.preedit_range;
                        text_layout.preedit_caret = visible_text.preedit_caret;
                        text_layout.buffer_selection = Some(target.buffer.selection());
                        text_layout.scroll_viewport = Some(content_viewport);
                        text_layout.last_focused = Some(focused);
                        let shape_is_current = text_layout.shape_key.as_ref() == Some(&key)
                            && text_layout.text_layout.is_some();
                        if !shape_is_current {
                            text_layout.reset_visual_navigation();
                            let shaped = TextLayout::shape_with_bevy_config(
                                &display_text,
                                content_width,
                                UiTextLayout::config(&key),
                                &mut pipeline,
                                &fonts,
                                &mut font_cx,
                                &mut layout_cx,
                            );
                            match shaped {
                                Ok(shaped) => {
                                    text_layout.text_layout = Some(shaped);
                                    text_layout.shape_key = Some(key);
                                    text_layout.last_error = None;
                                }
                                Err(error) => {
                                    text_layout.text_layout = None;
                                    text_layout.shape_key = Some(key);
                                    text_layout.text_origin =
                                        Some(text_origin(&text_layout, result, target.node));
                                    text_layout.last_error = Some(format!("{error:?}"));
                                    world.entity_mut(target.entity).insert(text_layout);
                                    continue;
                                }
                            }
                        }

                        let scroll_changed = if focused && (geometry_changed || !shape_is_current) {
                            ensure_caret_visible(&mut text_layout, &target.buffer, content_viewport)
                        } else {
                            clamp_scroll_offset(&mut text_layout, &target.buffer, content_viewport)
                        };
                        let origin = text_origin(&text_layout, result, target.node);
                        let origin_changed = text_layout.text_origin != Some(origin);
                        text_layout.text_origin = Some(origin);
                        if geometry_changed || scroll_changed || origin_changed || !shape_is_current
                        {
                            world.entity_mut(target.entity).insert(text_layout);
                        }
                    }
                });
            });
        });
    });
    settle_text_cursor(world);
}

/// Refresh only text entities recorded as dirty by widget synchronization.
pub fn update_text_layouts_if_dirty(world: &mut World) {
    world.init_resource::<UiDirty>();
    world.init_resource::<TextWorkCache>();
    world.resource_mut::<TextWorkCache>().prepared_layout = true;
    update_text_layouts(world);
}

pub(super) fn take_text_work(world: &mut World, domain: UiDirtyDomain) -> Option<UiDirtyBatch> {
    let prepared =
        world
            .get_resource_mut::<TextWorkCache>()
            .is_some_and(|mut cache| match domain {
                UiDirtyDomain::TextMeasure => std::mem::take(&mut cache.prepared_measure),
                UiDirtyDomain::TextLayout => std::mem::take(&mut cache.prepared_layout),
                _ => false,
            });
    prepared.then(|| world.resource_mut::<UiDirty>().take(domain))
}
