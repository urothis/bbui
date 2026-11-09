use bevy_app::App;
use bevy_asset::Assets;
use bevy_ecs::system::{Res, ResMut, SystemState};
use bevy_text::{Font, FontCx, FontSource, LayoutCx, TextFont, TextPipeline};
use bui_text::{
    TextLayout, TextPlugin, UI_DEFAULT_FONT_FAMILY, UI_SYMBOL_FONT_FAMILY, ui_default_text_font,
};

fn has_bundled_faces(source: &FontSource) -> bool {
    matches!(source, FontSource::List(faces) if matches!(faces.as_slice(),
        [FontSource::Family(primary), FontSource::Family(symbols)]
        if primary == UI_DEFAULT_FONT_FAMILY && symbols == UI_SYMBOL_FONT_FAMILY))
}

#[test]
fn bundled_default_font_is_available_to_shaping_before_first_update() {
    let mut app = App::new();
    app.add_plugins(TextPlugin);

    let font = ui_default_text_font(18.0);
    assert!(has_bundled_faces(&font.font));
    assert!(
        app.world_mut()
            .resource_mut::<FontCx>()
            .collection
            .family_id(UI_DEFAULT_FONT_FAMILY)
            .is_some()
    );
    assert!(
        app.world_mut()
            .resource_mut::<FontCx>()
            .collection
            .family_id(UI_SYMBOL_FONT_FAMILY)
            .is_some()
    );

    let mut state = SystemState::<(
        ResMut<TextPipeline>,
        Res<Assets<Font>>,
        ResMut<FontCx>,
        ResMut<LayoutCx>,
    )>::new(app.world_mut());
    let (mut pipeline, fonts, mut font_cx, mut layout_cx) = state
        .get_mut(app.world_mut())
        .expect("TextPlugin initializes Bevy's text shaping resources");
    let width = |text: &str,
                 pipeline: &mut TextPipeline,
                 fonts: &Assets<Font>,
                 font_cx: &mut FontCx,
                 layout_cx: &mut LayoutCx| {
        TextLayout::shape_with_bevy(text, 100.0, &font, pipeline, fonts, font_cx, layout_cx)
            .expect("the bundled font family should shape")
            .bounds()
            .width
    };
    let wide_advance = width("W", &mut pipeline, &fonts, &mut font_cx, &mut layout_cx);
    let narrow_advance = width("i", &mut pipeline, &fonts, &mut font_cx, &mut layout_cx);
    let arrows_advance = width("▾ ▸ ▴", &mut pipeline, &fonts, &mut font_cx, &mut layout_cx);
    state.apply(app.world_mut());

    assert!(wide_advance > narrow_advance * 1.5);
    assert!(arrows_advance > 0.0);
}

#[test]
fn default_font_preserves_the_requested_size() {
    let default = ui_default_text_font(21.0);
    assert_eq!(default.font_size, TextFont::from_font_size(21.0).font_size);
    assert!(has_bundled_faces(&default.font));
}
