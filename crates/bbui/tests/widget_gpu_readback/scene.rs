use super::*;

struct Scene {
    app: App,
    target: bevy::prelude::Handle<Image>,
    root: Entity,
    table_root: Entity,
    table_viewport: Rect,
    dropdown: Entity,
    dropdown_viewport: Rect,
    dropdown_viewport_root: Entity,
    list_widget: Entity,
    tree_widget: Entity,
    preview_grid_widget: Entity,
    curve: Entity,
    ramp_widget: Entity,
    menu: Entity,
    editor: Entity,
}

#[path = "scene/checks.rs"]
mod checks;
#[path = "scene/setup.rs"]
mod setup;

#[test]
fn widget_menu_editors_and_collections_render_in_dark_light_and_dpi_variants() {
    checks::verify_scene(setup::build_scene());
}
