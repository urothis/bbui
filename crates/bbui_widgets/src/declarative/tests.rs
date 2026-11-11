use super::*;
use crate::IconButtonAssetStatus;
use bbui_core::{UiChildren, UiFocusable, UiParent};
use bbui_layout::UiIntrinsicSize;
use bbui_style::Theme;
use bbui_text::TextPlugin;
use bevy_app::{App, Update};
use bevy_ecs::change_detection::{DetectChanges, Ref};
use bevy_ecs::prelude::World;
use bevy_ecs::prelude::{Query, ResMut, Resource};
use bevy_ecs::schedule::IntoScheduleConfigs;

#[derive(Resource, Default)]
struct IntrinsicLabelBufferObservation {
    changed: Option<bool>,
}

fn observe_intrinsic_label_buffers(
    buffers: Query<Ref<TextBuffer>, With<IntrinsicLabelText>>,
    mut observation: ResMut<IntrinsicLabelBufferObservation>,
) {
    observation.changed = Some(buffers.iter().any(|buffer| buffer.is_changed()));
}

#[test]
fn explicit_roles_override_control_defaults_when_mounted() {
    let mut world = World::new();
    let root = WidgetTree::new(WidgetNode::row([
        WidgetNode::icon_button("◇", "Keep").with_role("flat-icon-button"),
        WidgetNode::slider_range("Scale", 1.0, 0.0, 4.0, 0.1).with_role("filled-slider"),
        WidgetNode::menu("File", []).with_role("flat-menu"),
    ]))
    .spawn(&mut world, None)
    .unwrap();
    let children = &world.get::<UiChildren>(root).unwrap().0;
    for (entity, expected) in
        children
            .iter()
            .zip(["flat-icon-button", "filled-slider", "flat-menu"])
    {
        assert_eq!(world.get::<UiRole>(*entity).unwrap().0, expected);
    }
}

#[test]
fn reconcile_reorders_keyed_controls_without_losing_live_edits() {
    let mut world = World::new();
    let original = WidgetTree::new(WidgetNode::column([
        WidgetNode::text_field("Name", "initial").with_key("name"),
        WidgetNode::button("Save").with_key("save"),
    ]));
    let root = original.spawn(&mut world, None).unwrap();
    let before = world.get::<UiChildren>(root).unwrap().0.clone();
    world.get_mut::<TextField>(before[0]).unwrap().text = "edited".to_owned();
    let window = world.spawn_empty().id();
    world.insert_resource(bbui_core::UiFocus::default());
    world
        .resource_mut::<bbui_core::UiFocus>()
        .state_mut(window)
        .focused = Some(before[0]);
    let reordered = WidgetTree::new(WidgetNode::column([
        WidgetNode::button("Save").with_key("save"),
        WidgetNode::text_field("Name", "initial").with_key("name"),
        WidgetNode::label("Ready").with_key("status"),
    ]));
    assert_eq!(reordered.reconcile(&mut world, root).unwrap(), root);
    let after = world.get::<UiChildren>(root).unwrap().0.clone();
    assert_eq!(after[0], before[1]);
    assert_eq!(after[1], before[0]);
    assert_eq!(world.get::<TextField>(before[0]).unwrap().text, "edited");
    assert_eq!(
        world
            .resource::<bbui_core::UiFocus>()
            .state(window)
            .and_then(|state| state.focused),
        Some(before[0])
    );
    assert_eq!(world.get::<Label>(after[2]).unwrap().text, "Ready");
    reordered.reconcile(&mut world, root).unwrap();
    assert_eq!(world.get::<UiChildren>(root).unwrap().0, after);
}

#[test]
fn reconcile_removes_old_children_and_rejects_duplicate_keys_before_editing() {
    let mut world = World::new();
    let root = WidgetTree::new(WidgetNode::column([
        WidgetNode::button("One").with_key("one"),
        WidgetNode::button("Two").with_key("two"),
    ]))
    .spawn(&mut world, None)
    .unwrap();
    let old = world.get::<UiChildren>(root).unwrap().0.clone();
    let invalid = WidgetTree::new(WidgetNode::column([
        WidgetNode::button("A").with_key("duplicate"),
        WidgetNode::button("B").with_key("duplicate"),
    ]));
    assert!(matches!(
        invalid.reconcile(&mut world, root),
        Err(WidgetTreeError::DuplicateKey(_))
    ));
    assert_eq!(world.get::<UiChildren>(root).unwrap().0, old);
    WidgetTree::new(WidgetNode::column([
        WidgetNode::button("Two").with_key("two")
    ]))
    .reconcile(&mut world, root)
    .unwrap();
    assert!(world.get_entity(old[0]).is_err());
    assert_eq!(world.get::<UiChildren>(root).unwrap().0, vec![old[1]]);
    let incompatible = WidgetTree::new(WidgetNode::label("Different root kind"));
    assert!(matches!(
        incompatible.reconcile(&mut world, root),
        Err(WidgetTreeError::RootTypeChanged)
    ));
    assert!(world.get::<UiChildren>(root).is_some());
}

#[test]
fn reconcile_keeps_unchanged_file_browser_without_reopening_disk() {
    let directory =
        std::env::temp_dir().join(format!("bbui-reconcile-browser-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let tree = WidgetTree::new(WidgetNode::file_browser(&directory).with_key("browser"));
    let mut world = World::new();
    let root = tree.spawn(&mut world, None).unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(tree.reconcile(&mut world, root).unwrap(), root);
}

#[test]
fn reconcile_updates_container_layout_when_kind_changes() {
    let mut world = World::new();
    let root = WidgetTree::new(WidgetNode::row([WidgetNode::label("A")]))
        .spawn(&mut world, None)
        .unwrap();
    WidgetTree::new(WidgetNode::column([WidgetNode::label("A")]))
        .reconcile(&mut world, root)
        .unwrap();
    assert_eq!(
        world.get::<UiLayoutNode>(root).unwrap().mode,
        UiLayoutMode::Column
    );
}

#[test]
fn intrinsic_label_sync_skips_idle_buffers_and_reconciles_changed_or_new_labels() {
    let mut app = App::new();
    app.add_plugins((TextPlugin, crate::WidgetPlugin));
    let entity = WidgetTree::new(WidgetNode::label("Stable label"))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.init_resource::<IntrinsicLabelBufferObservation>();
    app.add_systems(
        Update,
        observe_intrinsic_label_buffers.after(sync_intrinsic_label_text),
    );

    app.update();
    app.update();
    assert_eq!(
        app.world()
            .resource::<IntrinsicLabelBufferObservation>()
            .changed,
        Some(false),
        "an idle intrinsic label must not rewrite its measurement buffer"
    );

    *app.world_mut().get_mut::<TextBuffer>(entity).unwrap() = TextBuffer::new("stale");
    app.update();
    assert_eq!(
        app.world().get::<TextBuffer>(entity).unwrap().as_str(),
        "Stable label",
        "a direct buffer edit must be reconciled with the public label"
    );
    assert_eq!(
        app.world()
            .resource::<IntrinsicLabelBufferObservation>()
            .changed,
        Some(true)
    );

    let newly_mounted = WidgetTree::new(WidgetNode::label("Mounted later"))
        .spawn(app.world_mut(), Some(entity))
        .unwrap();
    *app.world_mut()
        .get_mut::<TextBuffer>(newly_mounted)
        .unwrap() = TextBuffer::new("stale mount");
    app.update();
    assert_eq!(
        app.world()
            .get::<TextBuffer>(newly_mounted)
            .unwrap()
            .as_str(),
        "Mounted later",
        "a newly mounted intrinsic label must be synchronized even when its parent changes"
    );
}

#[test]
fn label_intrinsic_width_uses_shared_font_and_tracks_theme_size() {
    let text = "NodeGraph · typed sockets — Ωmega";
    let mut app = App::new();
    app.add_plugins((TextPlugin, crate::WidgetPlugin));
    let entity = WidgetTree::new(WidgetNode::label(text))
        .spawn(app.world_mut(), None)
        .unwrap();
    assert!(app.world().get::<UiFocusable>(entity).is_none());
    app.update();

    let first_width = app
        .world()
        .get::<UiIntrinsicSize>(entity)
        .expect("TextPlugin measures declarative labels")
        .0[0];
    assert!(
        first_width > 160.0,
        "the full Unicode title must not be clipped to the old 160px default: {first_width}"
    );

    app.world_mut().get_mut::<Label>(entity).unwrap().text =
        "A runtime replacement label with a longer value — Ωmega".to_owned();
    app.update();
    let replaced_width = app.world().get::<UiIntrinsicSize>(entity).unwrap().0[0];
    assert!(replaced_width > first_width);

    app.world_mut().resource_mut::<Theme>().font_size = 24.0;
    app.update();
    let larger_width = app.world().get::<UiIntrinsicSize>(entity).unwrap().0[0];
    assert!(
        larger_width > replaced_width * 1.4,
        "intrinsic label width must track the renderer's theme font size: {replaced_width} -> {larger_width}"
    );
}

#[test]
fn tree_spawns_typed_bevy_controls_and_hierarchy() {
    let mut world = World::new();
    let parent = world.spawn(UiNode).id();
    let tree = WidgetTree::new(WidgetNode::column([
        WidgetNode::label("Settings").with_key("heading"),
        WidgetNode::checkbox("Visible", true).with_key("visible"),
        WidgetNode::panel(
            "Advanced",
            false,
            [WidgetNode::number("Scale", 1.0, 0.0, 10.0, 0.1)],
        ),
    ]));
    let root = tree.spawn(&mut world, Some(parent)).unwrap();
    let children = &world.get::<UiChildren>(root).unwrap().0;
    assert_eq!(children.len(), 3);
    assert!(world.get::<Label>(children[0]).is_some());
    assert!(world.get::<Checkbox>(children[1]).unwrap().is_checked());
    assert_eq!(world.get::<UiParent>(root).unwrap().0, parent);
    let panel_children = &world.get::<UiChildren>(children[2]).unwrap().0;
    assert!(!world.get::<UiVisible>(panel_children[0]).unwrap().0);
}

#[test]
fn tooltip_declaration_wraps_a_live_child_control() {
    let mut world = World::new();
    let host = WidgetTree::new(WidgetNode::tooltip(
        "Apply the pending edits",
        WidgetNode::button("Apply").with_key("apply"),
    ))
    .spawn(&mut world, None)
    .unwrap();
    let child = world.get::<UiChildren>(host).unwrap().0[0];
    assert_eq!(
        world.get::<TooltipControl>(host).unwrap().text,
        "Apply the pending edits"
    );
    assert!(world.get::<Button>(child).is_some());
    assert_eq!(world.get::<UiKey>(child).unwrap().0, "apply");
    assert_eq!(world.get::<UiRole>(host).unwrap().0, "tooltip-host");
    assert_eq!(world.get::<TooltipControl>(host).unwrap().delay_millis, 400);
}

#[test]
fn radio_and_icon_declarations_spawn_focusable_accessible_controls() {
    let mut group = RadioGroup::new(["Object", "Edit", "Sculpt"], Some(1));
    group.options[2].enabled = false;
    let tree = WidgetTree::new(WidgetNode::row([
        WidgetNode::radio_group("Mode", group.clone()).with_key("mode"),
        WidgetNode::icon_button("+", "Add object").with_key("add-object"),
    ]));
    let mut world = World::new();
    let root = tree.spawn(&mut world, None).unwrap();
    let [radio, icon] = world.get::<UiChildren>(root).unwrap().0.as_slice() else {
        panic!("expected two control declarations");
    };

    assert!(world.get::<UiFocusable>(*radio).is_some());
    assert_eq!(world.get::<UiRole>(*radio).unwrap().0, "radio-group");
    assert_eq!(world.get::<Label>(*radio).unwrap().text, "Mode");
    assert_eq!(world.get::<RadioGroup>(*radio).unwrap(), &group);

    assert!(world.get::<UiFocusable>(*icon).is_some());
    assert_eq!(world.get::<UiRole>(*icon).unwrap().0, "button");
    assert_eq!(
        world.get::<IconButton>(*icon).unwrap().accessible_name,
        "Add object"
    );
}

#[test]
fn image_icon_declaration_keeps_asset_path_and_load_state_on_button() {
    let mut world = World::new();
    let entity = WidgetTree::new(
        WidgetNode::icon_button_asset("icons/add.svg", "Add object").with_key("add"),
    )
    .spawn(&mut world, None)
    .unwrap();

    assert_eq!(world.get::<IconButton>(entity).unwrap().icon, "");
    assert_eq!(
        world.get::<IconButton>(entity).unwrap().accessible_name,
        "Add object"
    );
    assert_eq!(
        world.get::<IconButtonAsset>(entity).unwrap().path,
        "icons/add.svg"
    );
    assert_eq!(
        world.get::<IconButtonAssetState>(entity).unwrap().status,
        IconButtonAssetStatus::Hidden
    );
    assert_eq!(world.get::<UiKey>(entity).unwrap().0, "add");
    assert!(world.get::<UiFocusable>(entity).is_some());
}

#[test]
fn slider_declaration_spawns_a_focusable_bounded_control() {
    let slider = Slider::new(0.4, 0.0, 1.0, 0.05).unwrap().with_unit("m");
    let mut world = World::new();
    let entity =
        WidgetTree::new(WidgetNode::slider("Distance", slider.clone()).with_key("distance"))
            .spawn(&mut world, None)
            .unwrap();

    assert_eq!(world.get::<Slider>(entity).unwrap(), &slider);
    assert_eq!(world.get::<Label>(entity).unwrap().text, "Distance");
    assert_eq!(world.get::<UiRole>(entity).unwrap().0, "slider");
    assert!(world.get::<UiFocusable>(entity).is_some());
    assert_eq!(world.get::<UiKey>(entity).unwrap().0, "distance");
    assert_eq!(
        world.get::<UiLayoutNode>(entity).unwrap().size.width,
        UiLength::Px(260.0)
    );
}

#[test]
fn slider_range_declarations_validate_atomically_and_support_integer_values() {
    let mut world = World::new();
    let entity = WidgetTree::new(WidgetNode::integer_slider_range(
        "Count", 4.0, 0.0, 12.0, 1.0,
    ))
    .spawn(&mut world, None)
    .unwrap();
    let slider = world.get::<Slider>(entity).unwrap();
    assert_eq!(slider.kind, NumericFieldKind::Integer);
    assert_eq!(slider.value, 4.0);
    assert_eq!(slider.range(), [0.0, 12.0]);

    let mut empty_world = World::new();
    let mut query = empty_world.query::<Entity>();
    let before = query.iter(&empty_world).count();
    let invalid = WidgetTree::new(WidgetNode::column([
        WidgetNode::button("created only on success"),
        WidgetNode::slider_range("Bad", 0.5, -1.0e308, 1.0e308, 0.1),
    ]));
    assert!(matches!(
        invalid.spawn(&mut empty_world, None),
        Err(WidgetTreeError::InvalidSlider { .. })
    ));
    assert_eq!(query.iter(&empty_world).count(), before);
}

#[test]
fn invalid_number_declaration_does_not_leave_partial_entities() {
    let mut world = World::new();
    let mut query = world.query::<Entity>();
    let before = query.iter(&world).count();
    let tree = WidgetTree::new(WidgetNode::column([
        WidgetNode::button("created only on success"),
        WidgetNode::number("bad", 4.0, 5.0, 1.0, 0.5),
    ]));
    assert!(tree.spawn(&mut world, None).is_err());
    assert_eq!(query.iter(&world).count(), before);
}

#[test]
fn layout_containers_preserve_modes_roles_sizes_and_scroll_clips() {
    let mut world = World::new();
    let root = WidgetTree::new(WidgetNode::column([
        WidgetNode::overlay([WidgetNode::button("Overlay")]),
        WidgetNode::grid(2, [WidgetNode::button("A"), WidgetNode::button("B")]),
        WidgetNode::responsive_grid(80, 3, [WidgetNode::button("Cell")]),
        WidgetNode::flex(UiFlexDirection::Row, true, [WidgetNode::button("Wrap")]),
        WidgetNode::inline([WidgetNode::label("Inline")]),
        WidgetNode::boxed([WidgetNode::button("Boxed")]),
        WidgetNode::property_row("Scale", WidgetNode::number("Value", 1.0, 0.0, 2.0, 0.1)),
        WidgetNode::scroll([WidgetNode::button("Scroll")]).with_height(UiLength::Px(40.0)),
        WidgetNode::row([WidgetNode::button("Custom gap")])
            .with_gap(13.0)
            .with_width(UiLength::Px(320.0)),
    ]))
    .spawn(&mut world, None)
    .unwrap();
    let children = world.get::<UiChildren>(root).unwrap().0.clone();
    assert_eq!(world.get::<UiRole>(root).unwrap().0, "layout-column");
    assert!(matches!(
        world.get::<UiLayoutNode>(children[0]).unwrap().mode,
        UiLayoutMode::Overlay
    ));
    assert!(matches!(
        world.get::<UiLayoutNode>(children[1]).unwrap().mode,
        UiLayoutMode::Grid { columns: 2 }
    ));
    assert_eq!(world.get::<UiRole>(children[1]).unwrap().0, "layout-grid");
    assert!(matches!(
        world.get::<UiLayoutNode>(children[2]).unwrap().mode,
        UiLayoutMode::ResponsiveGrid {
            min_cell_width: 80,
            max_columns: 3
        }
    ));
    assert!(matches!(
        world.get::<UiLayoutNode>(children[3]).unwrap().mode,
        UiLayoutMode::Flex {
            direction: UiFlexDirection::Row,
            wrap: true
        }
    ));
    assert!(matches!(
        world.get::<UiLayoutNode>(children[4]).unwrap().mode,
        UiLayoutMode::Inline
    ));
    assert_eq!(world.get::<UiRole>(children[5]).unwrap().0, "box");
    assert_eq!(world.get::<UiRole>(children[6]).unwrap().0, "property-row");
    let property_children = &world.get::<UiChildren>(children[6]).unwrap().0;
    assert_eq!(
        world.get::<UiRole>(property_children[0]).unwrap().0,
        "property-label"
    );
    assert_eq!(
        world
            .get::<UiLayoutNode>(property_children[1])
            .unwrap()
            .size
            .width,
        UiLength::Fill(1.0)
    );
    let scroll = world.get::<UiLayoutNode>(children[7]).unwrap();
    assert!(scroll.clip_contents);
    assert_eq!(scroll.size.height, UiLength::Px(40.0));
    assert!(world.get::<UiScroll>(children[7]).is_some());
    let custom = world.get::<UiInlineStyle>(children[8]).unwrap();
    assert_eq!(
        custom.0.get(&UiStyleProperty::Gap),
        Some(&UiStyleValue::Scalar(13.0))
    );
    assert_eq!(
        custom.0.get(&UiStyleProperty::Width),
        Some(&UiStyleValue::Length(UiLength::Px(320.0)))
    );
}

#[test]
fn invalid_grid_declaration_is_rejected_before_spawning_any_entities() {
    let mut world = World::new();
    let before = world.iter_entities().count();
    let tree = WidgetTree::new(WidgetNode::column([
        WidgetNode::button("Would spawn"),
        WidgetNode::grid(0, [WidgetNode::button("Invalid")]),
    ]));
    assert!(matches!(
        tree.spawn(&mut world, None),
        Err(WidgetTreeError::InvalidLayout { .. })
    ));
    assert_eq!(world.iter_entities().count(), before);
}

#[test]
fn data_table_declaration_spawns_validated_focusable_model() {
    let mut world = World::new();
    let table = DataTable {
        columns: vec![crate::TableColumn::new("Name", 160.0)],
        rows: vec![crate::TableRow {
            id: 17,
            cells: vec!["Cube".into()],
        }],
        ..DataTable::default()
    };
    let entity = WidgetTree::new(WidgetNode::data_table("Objects", table.clone()))
        .spawn(&mut world, None)
        .unwrap();
    assert!(world.get::<UiFocusable>(entity).is_some());
    assert!(world.get::<UiWheelHandled>(entity).is_some());
    assert_eq!(world.get::<DataTable>(entity).unwrap(), &table);

    let mut invalid = table;
    invalid.rows[0].cells.clear();
    let mut query = world.query::<Entity>();
    let count = query.iter(&world).count();
    assert!(
        WidgetTree::new(WidgetNode::data_table("Invalid", invalid))
            .spawn(&mut world, None)
            .is_err()
    );
    assert_eq!(query.iter(&world).count(), count);
}

#[test]
fn node_graph_declaration_spawns_focusable_editor_and_validates_before_spawn() {
    let mut world = World::new();
    let mut model = NodeGraph::default();
    let source = model.add_node(
        "Source",
        [12.0, 20.0],
        vec![crate::NodeSocket::output("value", crate::SocketType::Float)],
    );
    let sink = model.add_node(
        "Sink",
        [280.0, 40.0],
        vec![crate::NodeSocket::input("value", crate::SocketType::Float)],
    );
    model.create_link(source, 0, sink, 0).unwrap();
    let entity = WidgetTree::new(WidgetNode::node_graph("Shader", model.clone()))
        .spawn(&mut world, None)
        .unwrap();
    assert!(world.get::<UiFocusable>(entity).is_some());
    assert!(world.get::<UiWheelHandled>(entity).is_some());
    assert_eq!(world.get::<Label>(entity).unwrap().text, "Shader");
    assert_eq!(world.get::<NodeGraph>(entity).unwrap(), &model);
    assert!(world.get::<NodeGraphEditorState>(entity).is_some());

    let mut invalid = model;
    invalid.zoom = f32::NAN;
    let mut query = world.query::<Entity>();
    let before = query.iter(&world).count();
    let error = WidgetTree::new(WidgetNode::node_graph("Invalid", invalid))
        .spawn(&mut world, None)
        .unwrap_err();
    assert!(matches!(error, WidgetTreeError::InvalidNodeGraph { .. }));
    assert_eq!(query.iter(&world).count(), before);
}

#[test]
fn canvas_declaration_spawns_focusable_model_and_validates_before_spawn() {
    let mut world = World::new();
    let entity = WidgetTree::new(WidgetNode::canvas_default("Scene"))
        .spawn(&mut world, None)
        .unwrap();
    assert!(world.get::<UiFocusable>(entity).is_some());
    assert!(world.get::<UiWheelHandled>(entity).is_some());
    assert_eq!(world.get::<Label>(entity).unwrap().text, "Scene");
    assert!(world.get::<Canvas>(entity).is_some());

    let mut invalid = Canvas::default();
    invalid.zoom = f32::NAN;
    let mut query = world.query::<Entity>();
    let before = query.iter(&world).count();
    let error = WidgetTree::new(WidgetNode::canvas("Invalid", invalid))
        .spawn(&mut world, None)
        .unwrap_err();
    assert!(matches!(error, WidgetTreeError::InvalidCanvas { .. }));
    assert_eq!(query.iter(&world).count(), before);
}

#[test]
fn color_ramp_declaration_spawns_focusable_editor_and_validates_before_spawn() {
    let mut world = World::new();
    let ramp = WidgetNode::color_ramp_default("Gradient").with_key("gradient");
    let entity = WidgetTree::new(ramp).spawn(&mut world, None).unwrap();
    assert!(world.get::<UiFocusable>(entity).is_some());
    assert_eq!(world.get::<Label>(entity).unwrap().text, "Gradient");
    assert_eq!(
        world.get::<UiLayoutNode>(entity).unwrap().size.height,
        UiLength::Px(220.0)
    );
    assert_eq!(world.get::<ColorRamp>(entity).unwrap().stops.len(), 2);
    assert_eq!(
        world
            .get::<ColorRampEditorState>(entity)
            .unwrap()
            .active_channel,
        ColorChannel::Red
    );

    let mut invalid = ColorRamp::default();
    invalid.stops[0].position = f32::NAN;
    let mut query = world.query::<Entity>();
    let before = query.iter(&world).count();
    let error = WidgetTree::new(WidgetNode::color_ramp("Invalid", invalid))
        .spawn(&mut world, None)
        .unwrap_err();
    assert!(matches!(error, WidgetTreeError::InvalidColorRamp { .. }));
    assert_eq!(query.iter(&world).count(), before);
}

#[test]
fn hsv_color_declaration_spawns_editor_and_rejects_invalid_srgb() {
    let mut world = World::new();
    let entity = WidgetTree::new(WidgetNode::hsv_color("Primary", Rgba(0.2, 0.4, 0.8, 0.6)))
        .spawn(&mut world, None)
        .unwrap();
    assert!(world.get::<UiFocusable>(entity).is_some());
    assert_eq!(world.get::<Label>(entity).unwrap().text, "Primary");
    let color = world.get::<HsvColorEditor>(entity).unwrap().color();
    assert!((color.0 - 0.2).abs() < 1.0e-5);
    assert!((color.1 - 0.4).abs() < 1.0e-5);
    assert!((color.2 - 0.8).abs() < 1.0e-5);
    assert!((color.3 - 0.6).abs() < 1.0e-5);
    assert_eq!(
        world
            .get::<HsvColorEditorState>(entity)
            .unwrap()
            .active_field,
        crate::HsvColorField::Hue
    );

    let mut query = world.query::<Entity>();
    let before = query.iter(&world).count();
    let error = WidgetTree::new(WidgetNode::hsv_color("Invalid", Rgba(1.5, 0.0, 0.0, 1.0)))
        .spawn(&mut world, None)
        .unwrap_err();
    assert!(matches!(error, WidgetTreeError::InvalidHsvColor { .. }));
    assert_eq!(query.iter(&world).count(), before);
}

#[test]
fn curve_editor_declaration_is_focusable_and_validated_before_spawn() {
    let mut world = World::new();
    let entity = WidgetTree::new(WidgetNode::curve_editor_default("Ease"))
        .spawn(&mut world, None)
        .unwrap();
    assert!(world.get::<UiFocusable>(entity).is_some());
    assert_eq!(world.get::<Label>(entity).unwrap().text, "Ease");
    assert_eq!(world.get::<CurveEditor>(entity).unwrap().points.len(), 2);

    let mut invalid = CurveEditor::default();
    invalid.points[0].position[0] = f32::NAN;
    let mut query = world.query::<Entity>();
    let before = query.iter(&world).count();
    let error = WidgetTree::new(WidgetNode::curve_editor("Invalid", invalid))
        .spawn(&mut world, None)
        .unwrap_err();
    assert!(matches!(error, WidgetTreeError::InvalidCurveEditor { .. }));
    assert_eq!(query.iter(&world).count(), before);
}
