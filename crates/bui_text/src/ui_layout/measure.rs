use super::*;

/// Shape text without a resolved rectangle and update `UiIntrinsicSize` before
/// the layout pass. Explicit pixel widths are used for wrapped text areas;
/// auto/fill widths use the unbounded preferred text width.
pub fn measure_text_intrinsics(world: &mut World) {
    let batch = take_text_work(world, UiDirtyDomain::TextMeasure);
    if batch.as_ref().is_some_and(UiDirtyBatch::is_empty) {
        return;
    }
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let subset = batch
        .as_ref()
        .filter(|batch| !batch.all)
        .map(|batch| &batch.entities);
    let entities = text_entities(world, false, subset);
    if entities.is_empty() {
        settle_text_cursor(world);
        return;
    }

    world.resource_scope(|world, mut pipeline: Mut<TextPipeline>| {
        world.resource_scope(|world, fonts: Mut<Assets<Font>>| {
            world.resource_scope(|world, mut font_cx: Mut<FontCx>| {
                world.resource_scope(|world, mut layout_cx: Mut<LayoutCx>| {
                    for target in entities {
                        let Some(mut text_layout) =
                            world.get::<UiTextLayout>(target.entity).cloned()
                        else {
                            continue;
                        };
                        let visible_text = text_layout.visible_text(&target.buffer);
                        let display_text = visible_text.text;
                        let max_width = intrinsic_width(&text_layout, target.node);
                        let computed_style = world.get::<UiComputedStyle>(target.entity).cloned();
                        let (viewport, scale_factor) =
                            logical_viewport_metrics(world, target.entity, None);
                        let font = resolve_text_font(
                            world,
                            target.entity,
                            text_layout.font(&theme, computed_style.as_ref()),
                        );
                        let key = text_layout.cache_key(
                            &display_text,
                            max_width,
                            ShapeContext {
                                font,
                                line_height: text_layout
                                    .line_height(&theme, computed_style.as_ref()),
                                scale_factor,
                                viewport_size: viewport,
                            },
                        );
                        if text_layout.intrinsic_key.as_ref() == Some(&key)
                            && text_layout.intrinsic_bounds.is_some()
                        {
                            continue;
                        }
                        let shaped = TextLayout::shape_with_bevy_config(
                            &display_text,
                            max_width,
                            UiTextLayout::config(&key),
                            &mut pipeline,
                            &fonts,
                            &mut font_cx,
                            &mut layout_cx,
                        );
                        match shaped {
                            Ok(shaped) => {
                                let bounds = shaped.bounds();
                                let intrinsic = intrinsic_size(
                                    bounds,
                                    &text_layout,
                                    target.node,
                                    key.scale_factor,
                                );
                                text_layout.intrinsic_key = Some(key);
                                text_layout.intrinsic_bounds = Some(bounds);
                                text_layout.last_error = None;
                                world
                                    .entity_mut(target.entity)
                                    .insert((text_layout, UiIntrinsicSize(intrinsic)));
                            }
                            Err(error) => {
                                text_layout.intrinsic_key = Some(key);
                                text_layout.intrinsic_bounds = None;
                                text_layout.last_error = Some(format!("{error:?}"));
                                world.entity_mut(target.entity).insert(text_layout);
                            }
                        }
                    }
                });
            });
        });
    });
    settle_text_cursor(world);
}
