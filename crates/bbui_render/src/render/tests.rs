use super::*;
use bbui_text::UI_DEFAULT_FONT_FAMILY;
use bevy::ecs::prelude::With;

fn rect_item(z: i32, x: f32) -> DrawItem {
    DrawItem {
        clip: ClipRect {
            x: 0.0,
            y: 0.0,
            width: 120.0,
            height: 80.0,
        },
        z,
        primitive: Primitive::RoundedRect {
            rect: ClipRect {
                x,
                y: 4.0,
                width: 30.0,
                height: 18.0,
            },
            radius: 4.0,
            color: Rgba(0.2, 0.4, 0.8, 1.0),
        },
    }
}

fn widget_fragment(
    entity: Entity,
    order: u64,
    target_camera: Option<Entity>,
    items: Vec<DrawItem>,
) -> WidgetRenderFragment {
    WidgetRenderFragment {
        entity,
        order,
        target_camera,
        items,
    }
}

fn cached_single_node(cache: &RenderCache, key: RenderKey) -> Entity {
    match &cache.items[&key].nodes {
        RenderedNodes::Single(entity) => *entity,
        RenderedNodes::Multiple(entities) => *entities
            .first()
            .expect("fragment item should render a Bevy node"),
        RenderedNodes::Empty => panic!("fragment item should render a Bevy node"),
    }
}

#[path = "tests/cameras_and_text.rs"]
mod cameras_and_text;
#[path = "tests/draw_order_and_node_lifetime.rs"]
mod draw_order_and_node_lifetime;
#[path = "tests/incremental_rendering.rs"]
mod incremental_rendering;
#[path = "tests/world_surface.rs"]
mod world_surface;
