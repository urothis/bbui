use super::*;

#[test]
pub(super) fn property_rows_and_tabs_keep_accessible_names_and_selection() {
    use bbui_widgets::{WidgetNode, WidgetTree};

    let mut world = World::new();
    let row = WidgetTree::new(WidgetNode::property_row_with_decorator(
        "Scale",
        WidgetNode::number("", 1.0, 0.0, 4.0, 0.1),
    ))
    .spawn(&mut world, None)
    .unwrap();
    let children = world.get::<UiChildren>(row).unwrap().0.clone();
    let control = children[1];
    let decorator = children[2];
    let tabs = WidgetTree::new(
        WidgetNode::column([WidgetNode::icon_button("◇", "Object").with_tab_selected(true)])
            .with_tab_list(),
    )
    .spawn(&mut world, None)
    .unwrap();
    let tab = world.get::<UiChildren>(tabs).unwrap().0[0];
    let vector = WidgetTree::new(WidgetNode::property_row(
        "Location",
        WidgetNode::row([
            WidgetNode::number("X", 0.0, -1.0, 1.0, 0.1),
            WidgetNode::number("Y", 0.0, -1.0, 1.0, 0.1),
        ]),
    ))
    .spawn(&mut world, None)
    .unwrap();
    let axes = world.get::<UiChildren>(vector).unwrap().0[1];
    let axes = world.get::<UiChildren>(axes).unwrap().0.clone();

    sync_accessibility(&mut world);
    assert_eq!(
        world.get::<AccessibilityNode>(control).unwrap().0.label(),
        Some("Scale")
    );
    assert_eq!(
        world.get::<AccessibilityNode>(decorator).unwrap().0.label(),
        Some("Scale keyframe")
    );
    assert_eq!(
        world.get::<AccessibilityNode>(tabs).unwrap().0.role(),
        Role::TabList
    );
    let tab_node = &world.get::<AccessibilityNode>(tab).unwrap().0;
    assert_eq!(tab_node.role(), Role::Tab);
    assert_eq!(tab_node.is_selected(), Some(true));
    assert_eq!(
        world.get::<AccessibilityNode>(axes[0]).unwrap().0.label(),
        Some("Location X")
    );
    assert_eq!(
        world.get::<AccessibilityNode>(axes[1]).unwrap().0.label(),
        Some("Location Y")
    );

    world.get_mut::<UiTab>(tab).unwrap().selected = false;
    sync_accessibility(&mut world);
    assert_eq!(
        world.get::<AccessibilityNode>(tab).unwrap().0.is_selected(),
        Some(false)
    );
}

#[test]
pub(super) fn keyboard_focus_is_mirrored_to_bevys_accessible_focus() {
    let mut app = App::new();
    app.init_resource::<UiFocus>()
        .init_resource::<BbuiFocusMirror>()
        .init_resource::<InputFocus>()
        .add_systems(Update, sync_accessible_focus);
    let window = app.world_mut().spawn_empty().id();
    let control = app.world_mut().spawn((UiNode, UiFocusable)).id();
    {
        let mut focus = app.world_mut().resource_mut::<UiFocus>();
        focus.active_window = Some(window);
        focus.focused = Some(control);
        focus.state_mut(window).focused = Some(control);
    }
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(control));

    {
        let mut focus = app.world_mut().resource_mut::<UiFocus>();
        focus.focused = None;
        focus.state_mut(window).focused = None;
    }
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
}

#[test]
pub(super) fn checkbox_semantics_follow_value_and_inherited_visibility() {
    let mut world = World::new();
    let parent = world.spawn((UiNode, UiVisible(false))).id();
    let child = world
        .spawn((
            UiNode,
            UiFocusable,
            UiParent(parent),
            Checkbox::new("Visible", true),
        ))
        .id();
    sync_accessibility(&mut world);
    let node = &world.get::<AccessibilityNode>(child).unwrap().0;
    assert_eq!(node.role(), Role::CheckBox);
    assert_eq!(node.label(), Some("Visible"));
    assert_eq!(node.toggled(), Some(Toggled::True));
    assert!(node.is_hidden());
    assert!(!node.supports_action(Action::Click));
    assert!(!node.supports_action(Action::Focus));
    world.get_mut::<UiVisible>(parent).unwrap().0 = true;
    world.get_mut::<Checkbox>(child).unwrap().value = CheckboxValue::Off;
    sync_accessibility(&mut world);
    let node = &world.get::<AccessibilityNode>(child).unwrap().0;
    assert_eq!(node.toggled(), Some(Toggled::False));
    assert!(!node.is_hidden());
    assert!(node.supports_action(Action::Click));
    world.entity_mut(parent).insert(UiEnabled(false));
    sync_accessibility(&mut world);
    let node = &world.get::<AccessibilityNode>(child).unwrap().0;
    assert!(node.is_disabled());
    assert!(!node.supports_action(Action::Click));
    assert!(!node.supports_action(Action::Focus));
    world.entity_mut(parent).insert(UiEnabled(true));
    world.entity_mut(child).insert(UiHitBox {
        window: None,
        rect: bbui_core::UiRect::new(0.0, 0.0, 20.0, 20.0),
        clip: Some(bbui_core::UiRect::ZERO),
        z_index: 0,
    });
    sync_accessibility(&mut world);
    let node = &world.get::<AccessibilityNode>(child).unwrap().0;
    assert!(node.is_hidden());
    assert!(!node.supports_action(Action::Focus));
    world.get_mut::<UiHitBox>(child).unwrap().clip = None;
    sync_accessibility(&mut world);
    assert!(!world.get::<AccessibilityNode>(child).unwrap().0.is_hidden());
}

#[test]
pub(super) fn numeric_expression_error_is_exposed_to_screen_readers() {
    let mut world = World::new();
    let mut field = NumericField::float(3.0).unwrap();
    assert!(field.evaluate_input("1/0", &Default::default()).is_err());
    let entity = world.spawn((UiNode, field)).id();
    sync_accessibility(&mut world);
    let node = &world.get::<AccessibilityNode>(entity).unwrap().0;
    assert_eq!(node.role(), Role::SpinButton);
    assert_eq!(node.description(), Some("division by zero"));
    assert_eq!(node.live(), Some(Live::Assertive));
    assert_eq!(node.numeric_value(), Some(3.0));

    world.get_mut::<NumericField>(entity).unwrap().cancel_edit();
    sync_accessibility(&mut world);
    let node = &world.get::<AccessibilityNode>(entity).unwrap().0;
    assert_eq!(node.description(), None);
    assert_eq!(node.live(), None);
}

#[test]
pub(super) fn live_status_label_announces_changed_value_atomically() {
    let mut world = World::new();
    let status = world
        .spawn((
            UiNode,
            Label::new("Waiting"),
            UiLiveRegion::polite().atomic(),
        ))
        .id();
    sync_accessibility(&mut world);
    let node = &world.get::<AccessibilityNode>(status).unwrap().0;
    assert_eq!(node.live(), Some(Live::Polite));
    assert!(node.is_live_atomic());
    assert_eq!(node.value(), Some("Waiting"));

    world.get_mut::<Label>(status).unwrap().text = "Saved".to_owned();
    sync_accessibility(&mut world);
    let node = &world.get::<AccessibilityNode>(status).unwrap().0;
    assert_eq!(node.value(), Some("Saved"));
    assert_eq!(node.live(), Some(Live::Polite));
    assert!(node.is_live_atomic());
}

#[test]
pub(super) fn menu_popup_and_tooltip_semantics_track_runtime_state() {
    use bbui_widgets::{MenuDisplay, MenuEntry, MenuTree};

    let mut world = World::new();
    let tree = MenuTree::new(vec![MenuEntry::submenu(
        "add",
        "Add",
        vec![MenuEntry::command("cube", "Cube")],
    )]);
    let owner = world
        .spawn((
            UiNode,
            MenuControl::new("Tools", tree, MenuDisplay::Dropdown),
            Tooltip::new("Open tools"),
        ))
        .id();
    let item = world
        .spawn((
            UiNode,
            MenuItemControl {
                owner,
                id: "add".to_owned(),
                label: "Add".to_owned(),
                parent_path: Vec::new(),
                index: 0,
                enabled: true,
                separator: false,
                has_submenu: true,
                shortcut: None,
            },
        ))
        .id();
    let separator = world
        .spawn((
            UiNode,
            MenuItemControl {
                owner,
                id: String::new(),
                label: String::new(),
                parent_path: Vec::new(),
                index: 1,
                enabled: false,
                separator: true,
                has_submenu: false,
                shortcut: None,
            },
        ))
        .id();
    let tooltip = world.spawn((UiNode, Tooltip::new("More options"))).id();
    sync_accessibility(&mut world);
    let menu_node = &world.get::<AccessibilityNode>(owner).unwrap().0;
    assert_eq!(menu_node.has_popup(), Some(HasPopup::Menu));
    assert_eq!(menu_node.is_expanded(), Some(false));
    assert_eq!(menu_node.tooltip(), Some("Open tools"));
    let item_node = &world.get::<AccessibilityNode>(item).unwrap().0;
    assert_eq!(item_node.has_popup(), Some(HasPopup::Menu));
    assert_eq!(item_node.is_expanded(), Some(false));
    assert!(item_node.supports_action(Action::Click));
    let separator_node = &world.get::<AccessibilityNode>(separator).unwrap().0;
    assert_eq!(separator_node.role(), Role::Splitter);
    assert!(!separator_node.supports_action(Action::Click));
    world.get_mut::<MenuItemControl>(item).unwrap().enabled = false;
    sync_accessibility(&mut world);
    let item_node = &world.get::<AccessibilityNode>(item).unwrap().0;
    assert!(item_node.is_disabled());
    assert!(!item_node.supports_action(Action::Click));
    let tooltip_node = &world.get::<AccessibilityNode>(tooltip).unwrap().0;
    assert_eq!(tooltip_node.role(), Role::Tooltip);
    assert!(tooltip_node.is_hidden());

    let mut menu = world.get_mut::<MenuControl>(owner).unwrap();
    menu.open(None);
    menu.session.path = vec![0, 0];
    world.get_mut::<Tooltip>(tooltip).unwrap().visible = true;
    sync_accessibility(&mut world);
    assert_eq!(
        world
            .get::<AccessibilityNode>(owner)
            .unwrap()
            .0
            .is_expanded(),
        Some(true)
    );
    assert_eq!(
        world
            .get::<AccessibilityNode>(item)
            .unwrap()
            .0
            .is_expanded(),
        Some(true)
    );
    assert!(
        !world
            .get::<AccessibilityNode>(tooltip)
            .unwrap()
            .0
            .is_hidden()
    );
}

#[test]
pub(super) fn dropdown_panel_and_progress_expose_named_live_values() {
    let mut world = World::new();
    let dropdown = world
        .spawn((
            UiNode,
            Label::new("Mode"),
            Dropdown::new(vec!["Fast".to_owned(), "Careful".to_owned()], Some(0)),
        ))
        .id();
    let panel = world.spawn((UiNode, Panel::new("Settings", true))).id();
    let progress = world.spawn((UiNode, Progress::new(0.4))).id();
    sync_accessibility(&mut world);

    let dropdown_node = &world.get::<AccessibilityNode>(dropdown).unwrap().0;
    assert_eq!(dropdown_node.label(), Some("Mode"));
    assert_eq!(dropdown_node.value(), Some("Fast"));
    assert_eq!(dropdown_node.has_popup(), Some(HasPopup::Listbox));
    assert_eq!(dropdown_node.is_expanded(), Some(false));
    let panel_node = &world.get::<AccessibilityNode>(panel).unwrap().0;
    assert_eq!(panel_node.label(), Some("Settings"));
    assert_eq!(panel_node.is_expanded(), Some(true));
    let progress_node = &world.get::<AccessibilityNode>(progress).unwrap().0;
    assert_eq!(progress_node.label(), Some("Progress"));
    assert_eq!(progress_node.numeric_value(), Some(f64::from(0.4_f32)));

    world.get_mut::<Dropdown>(dropdown).unwrap().open();
    world.get_mut::<Panel>(panel).unwrap().state.target_expanded = false;
    sync_accessibility(&mut world);
    assert_eq!(
        world
            .get::<AccessibilityNode>(dropdown)
            .unwrap()
            .0
            .is_expanded(),
        Some(true)
    );
    assert_eq!(
        world
            .get::<AccessibilityNode>(panel)
            .unwrap()
            .0
            .is_expanded(),
        Some(false)
    );
}

#[test]
pub(super) fn removing_a_control_removes_its_stale_accessibility_node() {
    let mut world = World::new();
    let entity = world.spawn((UiNode, Button::new("Transient"))).id();
    sync_accessibility(&mut world);
    assert!(world.get::<AccessibilityNode>(entity).is_some());
    world.entity_mut(entity).remove::<Button>();
    sync_accessibility(&mut world);
    assert!(world.get::<AccessibilityNode>(entity).is_none());
}

#[test]
pub(super) fn screen_reader_click_queues_the_same_widget_action_as_pointer_input() {
    let mut app = App::new();
    app.add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .init_resource::<WidgetActionQueue>()
        .add_systems(Update, route_accessibility_actions);
    let target = app.world_mut().spawn((UiNode, Button::new("Apply"))).id();
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: NodeId(target.to_bits()),
            data: None,
        }));
    app.update();
    assert_eq!(app.world().resource::<WidgetActionQueue>().len(), 1);
}

#[test]
pub(super) fn screen_reader_menu_item_click_emits_the_selected_command() {
    use bbui_widgets::{MenuDisplay, MenuEntry, MenuTree};

    let mut app = App::new();
    app.add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .init_resource::<WidgetActionQueue>()
        .init_resource::<bbui_widgets::WidgetEventQueue>()
        .add_systems(
            Update,
            (
                route_accessibility_actions,
                bbui_widgets::actions::apply_widget_actions,
            )
                .chain(),
        );
    let mut menu = MenuControl::new(
        "Add",
        MenuTree::new(vec![MenuEntry::command("cube", "Cube")]),
        MenuDisplay::Dropdown,
    );
    menu.open(None);
    let owner = app.world_mut().spawn((UiNode, menu)).id();
    let item = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(owner),
            MenuItemControl {
                owner,
                id: "cube".to_owned(),
                label: "Cube".to_owned(),
                parent_path: Vec::new(),
                index: 0,
                enabled: true,
                separator: false,
                has_submenu: false,
                shortcut: None,
            },
        ))
        .id();
    app.world_mut()
        .get_mut::<MenuControl>(owner)
        .unwrap()
        .session
        .path = vec![0];
    sync_accessibility(app.world_mut());
    assert_eq!(
        app.world()
            .get::<AccessibilityNode>(item)
            .unwrap()
            .0
            .is_selected(),
        Some(true)
    );
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: NodeId(item.to_bits()),
            data: None,
        }));
    app.update();
    assert!(!app.world().get::<MenuControl>(owner).unwrap().session.open);
    assert!(
        app.world()
            .resource::<bbui_widgets::WidgetEventQueue>()
            .0
            .iter()
            .any(
                |event| matches!(event, bbui_widgets::WidgetEvent::MenuCommand { target, command_id }
                if *target == owner && command_id == "cube")
            )
    );
}

#[test]
pub(super) fn radio_group_screen_reader_step_queues_a_selection_change() {
    let mut app = App::new();
    app.add_plugins(bbui_widgets::WidgetPlugin)
        .add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .add_systems(
            Update,
            route_accessibility_actions.before(bbui_widgets::actions::apply_widget_actions),
        );
    let target = app
        .world_mut()
        .spawn((
            UiNode,
            Label::new("Mode"),
            RadioGroup::new(["Fast", "Careful"], Some(0)),
        ))
        .id();
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Increment,
            target_tree: TreeId::ROOT,
            target_node: NodeId(target.to_bits()),
            data: None,
        }));
    app.update();
    assert_eq!(
        app.world().get::<RadioGroup>(target).unwrap().selected,
        Some(1)
    );
}

#[test]
pub(super) fn radio_and_dropdown_options_expose_and_route_individual_actions() {
    let mut app = App::new();
    app.add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .init_resource::<WidgetActionQueue>()
        .add_systems(Update, route_accessibility_actions);
    let radio = app
        .world_mut()
        .spawn((UiNode, RadioGroup::new(["Fast", "Careful"], Some(0))))
        .id();
    let radio_option = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(radio),
            RadioOptionControl {
                owner: radio,
                index: 1,
                label: "Careful".to_owned(),
                enabled: true,
                selected: false,
            },
        ))
        .id();
    let dropdown = app
        .world_mut()
        .spawn((UiNode, Dropdown::new(["One", "Two"], Some(0))))
        .id();
    let window = app.world_mut().spawn_empty().id();
    for owner in [radio, dropdown] {
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect: bbui_layout::Rect::new(0.0, 0.0, 120.0, 40.0),
            clip: None,
            z: 0,
            content_size: [120.0, 40.0],
            dpi_scale: 1.0,
        });
    }
    let dropdown_option = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(dropdown),
            DropdownOptionControl {
                owner: dropdown,
                index: 1,
                label: "Two".to_owned(),
                selected: false,
                highlighted: true,
                enabled: true,
            },
        ))
        .id();
    sync_accessibility(app.world_mut());
    let radio_node = &app
        .world()
        .get::<AccessibilityNode>(radio_option)
        .unwrap()
        .0;
    assert_eq!(radio_node.role(), Role::RadioButton);
    assert_eq!(radio_node.label(), Some("Careful"));
    assert_eq!(radio_node.toggled(), Some(Toggled::False));
    assert_eq!(radio_node.position_in_set(), Some(2));
    assert_eq!(
        app.world()
            .get::<AccessibilityNode>(radio)
            .unwrap()
            .0
            .size_of_set(),
        Some(2)
    );
    let dropdown_node = &app
        .world()
        .get::<AccessibilityNode>(dropdown_option)
        .unwrap()
        .0;
    assert_eq!(dropdown_node.role(), Role::ListBoxOption);
    assert_eq!(dropdown_node.label(), Some("Two"));
    assert_eq!(dropdown_node.is_selected(), Some(false));
    assert_eq!(dropdown_node.position_in_set(), Some(2));
    assert_eq!(
        app.world()
            .get::<AccessibilityNode>(dropdown)
            .unwrap()
            .0
            .size_of_set(),
        Some(2)
    );
    for entity in [radio_option, dropdown_option] {
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: NodeId(entity.to_bits()),
                data: None,
            }));
    }
    app.update();
    assert_eq!(app.world().resource::<WidgetActionQueue>().len(), 2);

    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Focus,
            target_tree: TreeId::ROOT,
            target_node: NodeId(dropdown_option.to_bits()),
            data: None,
        }));
    app.update();
    assert_eq!(app.world().resource::<UiFocus>().focused, Some(dropdown));
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Blur,
            target_tree: TreeId::ROOT,
            target_node: NodeId(dropdown_option.to_bits()),
            data: None,
        }));
    app.update();
    assert_eq!(app.world().resource::<UiFocus>().focused, None);

    app.world_mut()
        .get_mut::<RadioOptionControl>(radio_option)
        .unwrap()
        .enabled = false;
    sync_accessibility(app.world_mut());
    assert!(
        app.world()
            .get::<AccessibilityNode>(radio_option)
            .unwrap()
            .0
            .is_disabled()
    );
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: NodeId(radio_option.to_bits()),
            data: None,
        }));
    app.update();
    assert_eq!(app.world().resource::<WidgetActionQueue>().len(), 2);
}

#[test]
pub(super) fn reorderable_panel_exposes_screen_reader_move_actions() {
    let mut app = App::new();
    app.add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .init_resource::<WidgetActionQueue>()
        .add_systems(Update, route_accessibility_actions);
    let group = app
        .world_mut()
        .spawn((
            UiNode,
            PanelGroup::with_panels(["one", "two"]).with_reordering("settings"),
        ))
        .id();
    let mut panel = Panel::new("One", true);
    panel.persistence_key = Some("one".into());
    let panel = app.world_mut().spawn((UiNode, panel)).id();
    bbui_core::set_parent(app.world_mut(), panel, group).unwrap();
    sync_accessibility(app.world_mut());
    let node = &app.world().get::<AccessibilityNode>(panel).unwrap().0;
    assert!(node.supports_action(Action::Increment));
    assert!(node.supports_action(Action::Decrement));
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Increment,
            target_tree: TreeId::ROOT,
            target_node: NodeId(panel.to_bits()),
            data: None,
        }));
    app.update();
    assert_eq!(app.world().resource::<WidgetActionQueue>().len(), 1);
}

#[test]
pub(super) fn icon_button_exposes_an_accessible_name_and_disabled_state() {
    let mut world = World::new();
    let mut icon = IconButton::new("plus", "Add object");
    icon.enabled = false;
    let entity = world.spawn((UiNode, icon)).id();
    sync_accessibility(&mut world);
    let node = &world.get::<AccessibilityNode>(entity).unwrap().0;
    assert_eq!(node.role(), Role::Button);
    assert_eq!(node.label(), Some("Add object"));
    assert!(node.is_disabled());
    assert!(!node.supports_action(Action::Click));

    world.entity_mut(entity).insert(IconButtonAssetState {
        status: IconButtonAssetStatus::Loading,
    });
    sync_accessibility(&mut world);
    let node = &world.get::<AccessibilityNode>(entity).unwrap().0;
    assert_eq!(node.description(), Some("Icon loading"));

    world.entity_mut(entity).insert(IconButtonAssetState {
        status: IconButtonAssetStatus::Failed("missing.svg".into()),
    });
    sync_accessibility(&mut world);
    let node = &world.get::<AccessibilityNode>(entity).unwrap().0;
    assert_eq!(node.description(), Some("Icon failed to load: missing.svg"));
    assert_eq!(node.live(), Some(Live::Assertive));
}

#[test]
pub(super) fn slider_exposes_bounded_value_and_read_only_state() {
    let mut world = World::new();
    let mut slider = Slider::new(4.0, 0.0, 10.0, 0.5).unwrap().with_unit("dB");
    slider.read_only = true;
    let entity = world.spawn((UiNode, Label::new("Gain"), slider)).id();
    sync_accessibility(&mut world);
    let node = &world.get::<AccessibilityNode>(entity).unwrap().0;
    assert_eq!(node.role(), Role::Slider);
    assert_eq!(node.label(), Some("Gain"));
    assert_eq!(node.numeric_value(), Some(4.0));
    assert_eq!(node.value(), Some("4 dB"));
    assert_eq!(node.min_numeric_value(), Some(0.0));
    assert_eq!(node.max_numeric_value(), Some(10.0));
    assert_eq!(node.numeric_value_step(), Some(0.5));
    assert!(node.is_read_only());
    assert!(!node.supports_action(Action::Increment));
    assert!(!node.supports_action(Action::Decrement));
    assert!(!node.supports_action(Action::SetValue));
}

#[test]
pub(super) fn disabled_and_read_only_controls_do_not_advertise_edit_actions() {
    let mut world = World::new();
    let mut checkbox = Checkbox::new("Locked", false);
    checkbox.read_only = true;
    let checkbox = world.spawn((UiNode, checkbox)).id();
    let mut number = NumericField::float(2.0).unwrap();
    number.enabled = false;
    let number = world.spawn((UiNode, number)).id();
    let mut field = TextField::new("draft");
    field.read_only = true;
    let field = world.spawn((UiNode, field)).id();
    let component_only_read_only = world
        .spawn((UiNode, TextField::new("external"), UiReadOnly))
        .id();
    let mut area = TextArea::new("notes");
    area.field.enabled = false;
    let area = world.spawn((UiNode, area)).id();
    let mut dropdown = Dropdown::new(["One", "Two"], Some(0));
    dropdown.enabled = false;
    let dropdown = world.spawn((UiNode, dropdown)).id();
    let mut radio = RadioGroup::new(["One", "Two"], Some(0));
    radio.enabled = false;
    let radio = world.spawn((UiNode, radio)).id();
    let mut button = Button::new("Run");
    button.enabled = false;
    let button = world.spawn((UiNode, button)).id();
    sync_accessibility(&mut world);

    for (entity, forbidden) in [
        (checkbox, Action::Click),
        (number, Action::SetValue),
        (number, Action::Increment),
        (field, Action::ReplaceSelectedText),
        (component_only_read_only, Action::ReplaceSelectedText),
        (area, Action::ReplaceSelectedText),
        (dropdown, Action::Click),
        (radio, Action::Increment),
        (button, Action::Click),
    ] {
        let node = &world.get::<AccessibilityNode>(entity).unwrap().0;
        assert!(node.is_disabled() || node.is_read_only());
        assert!(
            !node.supports_action(forbidden),
            "{entity:?} advertises {forbidden:?}"
        );
    }
}
