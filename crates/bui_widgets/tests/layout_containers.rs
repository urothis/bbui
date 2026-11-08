use bevy_app::App;
use bui_core::{CorePlugin, UiChildren};
use bui_layout::{LayoutPlugin, Rect, UiLayoutResult, UiLayoutRoot, UiLength};
use bui_render::{DisplayList, Primitive};
use bui_style::{StylePlugin, Theme, ThemeKind};
use bui_text::TextPlugin;
use bui_widgets::{WidgetNode, WidgetPlugin, WidgetTree};

#[test]
fn declarative_property_grid_overlay_and_scroll_share_live_layout_geometry() {
    let mut app = App::new();
    app.add_plugins((
        CorePlugin,
        StylePlugin,
        LayoutPlugin,
        TextPlugin,
        WidgetPlugin,
    ));
    let window = app.world_mut().spawn_empty().id();
    let tree = WidgetTree::new(
        WidgetNode::column([
            WidgetNode::property_row("Scale", WidgetNode::number("Value", 1.0, 0.0, 10.0, 0.1)),
            WidgetNode::grid(
                2,
                [WidgetNode::button("First"), WidgetNode::button("Second")],
            ),
            WidgetNode::overlay([
                WidgetNode::label("Background"),
                WidgetNode::label("Foreground"),
            ]),
            WidgetNode::scroll((0..20).map(|index| WidgetNode::label(format!("Row {index}"))))
                .with_height(UiLength::Px(40.0)),
            WidgetNode::row([WidgetNode::button("Custom spacing")]).with_gap(20.0),
        ])
        .with_width(UiLength::Px(500.0))
        .with_height(UiLength::Px(300.0)),
    );
    let root = tree.spawn(app.world_mut(), None).unwrap();
    app.world_mut().entity_mut(root).insert(UiLayoutRoot::new(
        window,
        Rect::new(0.0, 0.0, 500.0, 300.0),
        1.0,
    ));
    app.update();

    let children = app.world().get::<UiChildren>(root).unwrap().0.clone();
    let property_children = app
        .world()
        .get::<UiChildren>(children[0])
        .unwrap()
        .0
        .clone();
    let label = app
        .world()
        .get::<UiLayoutResult>(property_children[0])
        .unwrap()
        .rect;
    let field = app
        .world()
        .get::<UiLayoutResult>(property_children[1])
        .unwrap()
        .rect;
    assert_eq!(label.width, 112.0);
    assert!(field.x >= label.x + label.width);
    let grid_children = app
        .world()
        .get::<UiChildren>(children[1])
        .unwrap()
        .0
        .clone();
    let first = app
        .world()
        .get::<UiLayoutResult>(grid_children[0])
        .unwrap()
        .rect;
    let second = app
        .world()
        .get::<UiLayoutResult>(grid_children[1])
        .unwrap()
        .rect;
    assert!(second.x >= first.x + first.width);
    let overlay_children = app
        .world()
        .get::<UiChildren>(children[2])
        .unwrap()
        .0
        .clone();
    let back = app
        .world()
        .get::<UiLayoutResult>(overlay_children[0])
        .unwrap()
        .rect;
    let front = app
        .world()
        .get::<UiLayoutResult>(overlay_children[1])
        .unwrap()
        .rect;
    assert_eq!([back.x, back.y], [front.x, front.y]);
    let scroll_children = app
        .world()
        .get::<UiChildren>(children[3])
        .unwrap()
        .0
        .clone();
    assert!(
        app.world()
            .get::<UiLayoutResult>(scroll_children[0])
            .unwrap()
            .clip
            .is_some()
    );
    assert_eq!(
        app.world()
            .get::<bui_layout::UiLayoutNode>(children[4])
            .unwrap()
            .gap,
        20.0
    );

    {
        let mut theme = app.world_mut().resource_mut::<Theme>();
        theme.spacing.property_row.label_width = 160.0;
        theme.spacing.control.gap = 16.0;
    }
    app.update();
    let resized = app
        .world()
        .get::<UiLayoutResult>(property_children[0])
        .unwrap()
        .rect;
    assert_eq!(resized.width, 160.0);
    let first_after = app
        .world()
        .get::<UiLayoutResult>(grid_children[0])
        .unwrap()
        .rect;
    let second_after = app
        .world()
        .get::<UiLayoutResult>(grid_children[1])
        .unwrap()
        .rect;
    assert_eq!(
        app.world()
            .get::<bui_layout::UiLayoutNode>(children[1])
            .unwrap()
            .gap,
        16.0
    );
    assert_eq!(
        app.world()
            .get::<bui_layout::UiLayoutNode>(children[4])
            .unwrap()
            .gap,
        20.0
    );
    assert!(second_after.x > second.x);
    assert!(first_after.width <= first.width);
}

#[test]
fn boxed_container_paints_the_current_theme_surface_and_border() {
    let mut app = App::new();
    app.add_plugins((
        CorePlugin,
        StylePlugin,
        LayoutPlugin,
        TextPlugin,
        WidgetPlugin,
    ));
    let window = app.world_mut().spawn_empty().id();
    let root = WidgetTree::new(
        WidgetNode::boxed([WidgetNode::label("Content")])
            .with_width(UiLength::Px(240.0))
            .with_height(UiLength::Px(100.0)),
    )
    .spawn(app.world_mut(), None)
    .unwrap();
    app.world_mut().entity_mut(root).insert(UiLayoutRoot::new(
        window,
        Rect::new(0.0, 0.0, 240.0, 100.0),
        1.0,
    ));
    for kind in [ThemeKind::Dark, ThemeKind::Light] {
        *app.world_mut().resource_mut::<Theme>() = Theme::new(kind);
        app.update();
        let surface = app.world().resource::<Theme>().surface;
        let border = app.world().resource::<Theme>().border;
        let items = app.world().resource::<DisplayList>();
        assert!(items.widget_items().iter().any(|item| matches!(
            &item.primitive,
            Primitive::RoundedRect { color, .. } if *color == surface
        )));
        assert_eq!(
            items
                .widget_items()
                .iter()
                .filter(|item| matches!(
                    &item.primitive,
                    Primitive::Line { color, width, .. } if *color == border && *width == 1.0
                ))
                .count(),
            4
        );
    }
}
