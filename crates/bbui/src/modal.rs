//! Window-scoped modal dialogs composed from ordinary BBUI widgets.

use std::collections::{HashMap, HashSet};

use bbui_core::{
    UiChildren, UiDirty, UiDirtyDomain, UiFocus, UiKey, UiKeyCode, UiModalScopes, UiRoutedEvent,
    UiRoutedEvents, UiSystems, cleanup_removed_node,
};
use bbui_layout::{Rect, UiAlign, UiEdges, UiJustify, UiLayoutNode, UiLayoutRoot, UiLength};
use bbui_style::{Rgba, ThemeColorToken, UiInlineStyle, UiStyleProperty, UiStyleValue};
use bbui_widgets::{WidgetEvent, WidgetEventQueue, WidgetNode, WidgetTree, WidgetTreeError};
use bevy::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModalAction {
    pub id: String,
    pub label: String,
}

impl ModalAction {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// The host owns the decision represented by each action ID.
#[derive(Debug, Clone)]
pub struct ModalDialog {
    pub title: String,
    pub message: String,
    pub actions: Vec<ModalAction>,
    pub dismiss_on_escape: bool,
}

#[derive(Component)]
pub(crate) struct ModalDialogMarker(pub String);

impl ModalDialog {
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        actions: Vec<ModalAction>,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            actions,
            dismiss_on_escape: true,
        }
    }

    pub fn with_escape_dismissal(mut self, enabled: bool) -> Self {
        self.dismiss_on_escape = enabled;
        self
    }

    /// Show one dialog in `window`. Returns after mounting its widgets and trapping focus.
    pub fn show(self, world: &mut World, window: Entity) -> Result<(), ModalError> {
        let Some(window_state) = world.get::<Window>(window) else {
            return Err(ModalError::MissingWindow);
        };
        let dimensions = [
            window_state.resolution.width(),
            window_state.resolution.height(),
        ];
        if self.actions.is_empty() || self.actions.iter().any(|action| action.id.is_empty()) {
            return Err(ModalError::InvalidActions);
        }
        let mut ids = HashSet::new();
        if self.actions.iter().any(|action| !ids.insert(&action.id)) {
            return Err(ModalError::InvalidActions);
        }
        world.init_resource::<ModalDialogs>();
        if world
            .resource::<ModalDialogs>()
            .active
            .contains_key(&window)
        {
            return Err(ModalError::AlreadyOpen);
        }
        let mut backdrop_style = UiInlineStyle::default();
        backdrop_style.set(
            UiStyleProperty::Background,
            UiStyleValue::Color(Rgba(0.0, 0.0, 0.0, 0.62)),
        );
        backdrop_style.set(UiStyleProperty::BorderWidth, UiStyleValue::Scalar(0.0));
        let mut dialog_style = UiInlineStyle::default();
        dialog_style.set_color_token(UiStyleProperty::Background, ThemeColorToken::Surface);
        dialog_style.set_color_token(UiStyleProperty::BorderColor, ThemeColorToken::Border);
        dialog_style.set(UiStyleProperty::BorderWidth, UiStyleValue::Scalar(1.0));
        dialog_style.set(UiStyleProperty::ZIndex, UiStyleValue::Integer(10_002));
        let mut content_style = UiInlineStyle::default();
        content_style.set(UiStyleProperty::ZIndex, UiStyleValue::Integer(10_004));
        let mut action_style = content_style.clone();
        action_style.set_color_token(UiStyleProperty::Background, ThemeColorToken::HoverSurface);
        let buttons = self.actions.iter().enumerate().map(|(index, action)| {
            WidgetNode::button(action.label.clone())
                .with_key(format!("modal:action:{index}"))
                .with_style(action_style.clone())
        });
        let mut dialog_layout = UiLayoutNode::column(12.0);
        dialog_layout.max_size[0] = Some(520.0);
        let card = WidgetNode::column([
            WidgetNode::label(&self.title)
                .with_role("modal-title")
                .with_style(content_style.clone()),
            WidgetNode::label(&self.message)
                .with_role("modal-message")
                .with_style(content_style.clone()),
            WidgetNode::row(buttons)
                .with_gap(8.0)
                .with_style(content_style),
        ])
        .with_role("box")
        .with_style(dialog_style)
        .with_width(UiLength::Fill(1.0))
        .with_padding(UiEdges::all(20.0));
        dialog_layout.z_index = 10_001;
        let dialog = WidgetNode::column([card])
            .with_key("modal:dialog")
            .with_role("modal-dialog")
            .with_accessible_name(&self.title)
            .with_layout(dialog_layout)
            .with_width(UiLength::Px((dimensions[0] * 0.9).min(520.0)));
        let mut root_layout = UiLayoutNode::column(0.0);
        root_layout.align_items = UiAlign::Center;
        root_layout.justify_content = UiJustify::Center;
        root_layout.z_index = 10_000;
        let root = WidgetTree::new(
            WidgetNode::column([dialog])
                .with_key("modal:backdrop")
                .with_role("box")
                .with_layout(root_layout)
                .with_style(backdrop_style)
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Fill(1.0)),
        )
        .spawn(world, None)
        .map_err(ModalError::Widget)?;
        world.entity_mut(root).insert(UiLayoutRoot::new(
            window,
            Rect::new(0.0, 0.0, dimensions[0], dimensions[1]),
            1.0,
        ));
        let Some(dialog_entity) = find_key(world, root, "modal:dialog") else {
            remove_dialog_tree(world, root);
            return Err(ModalError::MissingMountedControl);
        };
        let buttons: Option<Vec<_>> = self
            .actions
            .into_iter()
            .enumerate()
            .map(|(index, action)| {
                find_key(world, root, &format!("modal:action:{index}"))
                    .map(|entity| (entity, action.id))
            })
            .collect();
        let Some(buttons) = buttons else {
            remove_dialog_tree(world, root);
            return Err(ModalError::MissingMountedControl);
        };
        world
            .entity_mut(dialog_entity)
            .insert(ModalDialogMarker(self.title));
        let previous_focus = world
            .get_resource::<UiFocus>()
            .and_then(|focus| focus.state(window))
            .and_then(|state| state.focused);
        world.init_resource::<UiModalScopes>();
        world.resource_mut::<UiModalScopes>().0.insert(window, root);
        world
            .resource_mut::<UiDirty>()
            .mark_all(UiDirtyDomain::Accessibility);
        let mut focus = world.resource_mut::<UiFocus>();
        let state = focus.state_mut(window);
        state.focused = buttons.first().map(|(entity, _)| *entity);
        state.selection = None;
        state.captured = None;
        state.captured_button = None;
        state.press_position = None;
        focus.set_active(window);
        world.resource_mut::<ModalDialogs>().active.insert(
            window,
            ActiveModal {
                root,
                dialog: dialog_entity,
                buttons,
                previous_focus,
                dismiss_on_escape: self.dismiss_on_escape,
            },
        );
        Ok(())
    }
}

#[derive(Debug)]
pub enum ModalError {
    MissingWindow,
    MissingMountedControl,
    AlreadyOpen,
    InvalidActions,
    Widget(WidgetTreeError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalDismissal {
    Action(String),
    Escape,
    Programmatic,
}

#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct ModalEvent {
    pub window: Entity,
    pub dismissal: ModalDismissal,
}

struct ActiveModal {
    root: Entity,
    dialog: Entity,
    buttons: Vec<(Entity, String)>,
    previous_focus: Option<Entity>,
    dismiss_on_escape: bool,
}

/// Read this resource to suspend application input, such as a 3D viewport,
/// while a dialog is open in the same window.
#[derive(Resource, Default)]
pub struct ModalDialogs {
    active: HashMap<Entity, ActiveModal>,
}

impl ModalDialogs {
    pub fn is_open(&self, window: Entity) -> bool {
        self.active.contains_key(&window)
    }
}

fn find_key(world: &World, root: Entity, key: &str) -> Option<Entity> {
    let mut pending = vec![root];
    while let Some(entity) = pending.pop() {
        if world
            .get::<UiKey>(entity)
            .is_some_and(|value| value.0 == key)
        {
            return Some(entity);
        }
        if let Some(children) = world.get::<UiChildren>(entity) {
            pending.extend(children.0.iter().copied());
        }
    }
    None
}

fn remove_dialog_tree(world: &mut World, root: Entity) {
    if let Some(children) = world.get::<UiChildren>(root).cloned() {
        for child in children.0 {
            remove_dialog_tree(world, child);
        }
    }
    if world.get_entity(root).is_ok() {
        cleanup_removed_node(world, root);
        world.despawn(root);
    }
}

/// Close the active dialog without selecting an action.
pub fn dismiss_dialog(world: &mut World, window: Entity) -> bool {
    finish_dialog(world, window, ModalDismissal::Programmatic)
}

fn finish_dialog(world: &mut World, window: Entity, dismissal: ModalDismissal) -> bool {
    let Some(active) = world.resource_mut::<ModalDialogs>().active.remove(&window) else {
        return false;
    };
    world.resource_mut::<UiModalScopes>().0.remove(&window);
    world
        .resource_mut::<UiDirty>()
        .mark_all(UiDirtyDomain::Accessibility);
    remove_dialog_tree(world, active.root);
    let restored = active
        .previous_focus
        .filter(|entity| bbui_core::is_interactable_in_window(world, *entity, window));
    let mut focus = world.resource_mut::<UiFocus>();
    let state = focus.state_mut(window);
    state.focused = restored;
    state.selection = None;
    focus.set_active(window);
    world
        .resource_mut::<Messages<ModalEvent>>()
        .write(ModalEvent { window, dismissal });
    true
}

fn process_modal_events(world: &mut World) {
    if world.resource::<ModalDialogs>().active.is_empty() {
        return;
    }
    let mut dismissals = Vec::new();
    let activated: HashSet<_> = world
        .resource::<WidgetEventQueue>()
        .0
        .iter()
        .filter_map(|event| {
            if let WidgetEvent::Activated { target } = event {
                Some(*target)
            } else {
                None
            }
        })
        .collect();
    for (&window, active) in &world.resource::<ModalDialogs>().active {
        if world.get::<bbui_core::UiNode>(active.root).is_none()
            || world.get::<Window>(window).is_none()
        {
            dismissals.push((window, ModalDismissal::Programmatic));
        } else if let Some((_, action)) = active.buttons.iter().find(|(entity, _)| activated.contains(entity)) {
            dismissals.push((window, ModalDismissal::Action(action.clone())));
        } else if active.dismiss_on_escape && world.resource::<UiRoutedEvents>().events.iter().any(|event| {
            matches!(event, UiRoutedEvent::Key { window: owner, key: UiKeyCode::Escape, pressed: true, .. } if *owner == window)
        }) {
            dismissals.push((window, ModalDismissal::Escape));
        }
    }
    for (window, dismissal) in dismissals {
        let buttons: HashSet<_> = world.resource::<ModalDialogs>().active[&window]
            .buttons
            .iter()
            .map(|(entity, _)| *entity)
            .collect();
        world.resource_mut::<WidgetEventQueue>().0.retain(
            |event| !matches!(event, WidgetEvent::Activated { target } if buttons.contains(target)),
        );
        finish_dialog(world, window, dismissal);
    }
}

fn sync_modal_width(
    windows: Query<&Window>,
    dialogs: Res<ModalDialogs>,
    mut styles: Query<&mut UiInlineStyle>,
) {
    for (&window, active) in &dialogs.active {
        let (Ok(window), Ok(mut style)) = (windows.get(window), styles.get_mut(active.dialog))
        else {
            continue;
        };
        let width =
            UiStyleValue::Length(UiLength::Px((window.resolution.width() * 0.9).min(520.0)));
        if style.0.get(&UiStyleProperty::Width) != Some(&width) {
            style.set(UiStyleProperty::Width, width);
        }
    }
}

pub struct ModalPlugin;

impl Plugin for ModalPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ModalDialogs>()
            .init_resource::<UiModalScopes>()
            .add_message::<ModalEvent>()
            .add_systems(PreUpdate, sync_modal_width)
            .add_systems(
                Update,
                process_modal_events
                    .in_set(UiSystems::Widgets)
                    .after(bbui_widgets::actions::apply_widget_actions),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bbui_core::{UiInputEvent, UiInputQueue, UiModifiers, UiPointerButton};
    use bbui_layout::UiLayoutResult;
    use bevy::{
        asset::AssetPlugin, clipboard::ClipboardPlugin, input::InputPlugin, window::PrimaryWindow,
    };

    fn app() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins(WindowPlugin {
                primary_window: None,
                ..default()
            })
            .add_plugins((ImagePlugin::default(), InputPlugin, ClipboardPlugin))
            .add_plugins(crate::BbuiPlugin);
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        let base = WidgetTree::new(WidgetNode::button("Behind dialog"))
            .spawn(app.world_mut(), None)
            .unwrap();
        app.world_mut().entity_mut(base).insert(UiLayoutRoot::new(
            window,
            Rect::new(0.0, 0.0, 1280.0, 720.0),
            1.0,
        ));
        app.update();
        (app, window, base)
    }

    #[test]
    fn dialog_traps_focus_blocks_background_and_restores_focus() {
        let (mut app, window, base) = app();
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(base);
        ModalDialog::new(
            "Unsaved work",
            "Save changes before closing?",
            vec![
                ModalAction::new("save", "Save"),
                ModalAction::new("cancel", "Cancel"),
            ],
        )
        .show(app.world_mut(), window)
        .unwrap();
        app.update();
        assert!(app.world().resource::<ModalDialogs>().is_open(window));
        let active = &app.world().resource::<ModalDialogs>().active[&window];
        assert!(!bbui_core::is_interactable_in_window(
            app.world(),
            base,
            window
        ));
        let first = active.buttons[0].0;
        let second = active.buttons[1].0;
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(first)
        );
        let root_rect = app.world().get::<UiLayoutResult>(active.root).unwrap().rect;
        let dialog_entity = find_key(app.world(), active.root, "modal:dialog").unwrap();
        let dialog_rect = app
            .world()
            .get::<UiLayoutResult>(dialog_entity)
            .unwrap()
            .rect;
        assert!(dialog_rect.x > root_rect.x && dialog_rect.right() < root_rect.right());
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set(400.0, 500.0);
        app.update();
        app.update();
        let resized = app
            .world()
            .get::<UiLayoutResult>(dialog_entity)
            .unwrap()
            .rect;
        assert_eq!(resized.width, 360.0);
        assert_eq!(resized.x, 20.0);
        assert_eq!(
            app.world()
                .get::<bevy::a11y::AccessibilityNode>(dialog_entity)
                .unwrap()
                .0
                .role(),
            accesskit::Role::Dialog,
        );
        assert!(
            app.world()
                .get::<bevy::a11y::AccessibilityNode>(base)
                .unwrap()
                .0
                .is_hidden()
        );
        let behind = app.world().get::<UiLayoutResult>(base).unwrap().rect;
        let position = [
            behind.x + behind.width * 0.5,
            behind.y + behind.height * 0.5,
        ];
        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: true,
                position,
                time_seconds: 1.0,
            });
        app.update();
        assert!(
            !app.world()
                .resource::<UiRoutedEvents>()
                .events
                .iter()
                .any(|event| {
                    matches!(event, UiRoutedEvent::PointerDown { target, .. } if *target == base)
                })
        );
        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Key {
                window,
                key: UiKeyCode::Tab,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            });
        app.update();
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(second)
        );
        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Key {
                window,
                key: UiKeyCode::Enter,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            });
        app.update();
        assert!(!app.world().resource::<ModalDialogs>().is_open(window));
        assert!(
            !app.world()
                .resource::<ModalDialogs>()
                .active
                .contains_key(&window)
        );
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(base)
        );
        assert!(bbui_core::is_interactable_in_window(
            app.world(),
            base,
            window
        ));
        assert!(
            app.world()
                .resource::<Messages<ModalEvent>>()
                .iter_current_update_messages()
                .any(|event| event.dismissal == ModalDismissal::Action("cancel".into()))
        );
    }

    #[test]
    fn escape_dismisses_only_when_enabled() {
        let (mut app, window, _) = app();
        let dialog = ModalDialog::new(
            "Delete",
            "Delete this area?",
            vec![ModalAction::new("delete", "Delete")],
        )
        .with_escape_dismissal(false);
        dialog.show(app.world_mut(), window).unwrap();
        app.update();
        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Key {
                window,
                key: UiKeyCode::Escape,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            });
        app.update();
        assert!(
            app.world()
                .resource::<ModalDialogs>()
                .active
                .contains_key(&window)
        );
        assert!(dismiss_dialog(app.world_mut(), window));
        assert!(!dismiss_dialog(app.world_mut(), window));
    }
}
