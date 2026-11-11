use bbui::{
    BbuiPlugin,
    widgets::{Label, WidgetNode, WidgetTree},
};
use bbui_core::UiLiveRegion;
use bevy::a11y::AccessibilityNode;
use bevy::{
    asset::AssetPlugin,
    clipboard::ClipboardPlugin,
    input::InputPlugin,
    prelude::{App, ImagePlugin, MinimalPlugins},
    window::WindowPlugin,
};

#[test]
fn live_label_updates_are_reflected_with_polite_atomic_accessibility_semantics() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(WindowPlugin {
            primary_window: None,
            ..Default::default()
        })
        .add_plugins((ImagePlugin::default(), InputPlugin, ClipboardPlugin))
        .add_plugins(BbuiPlugin);

    let status = WidgetTree::new(WidgetNode::label("Waiting for a change"))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut()
        .entity_mut(status)
        .insert(UiLiveRegion::polite().atomic());
    app.update();

    let initial = app
        .world()
        .get::<AccessibilityNode>(status)
        .unwrap()
        .0
        .clone();
    assert_eq!(initial.live(), Some(accesskit::Live::Polite));
    assert!(initial.is_live_atomic());
    assert_eq!(initial.value(), Some("Waiting for a change"));

    app.world_mut().get_mut::<Label>(status).unwrap().text = "Saved successfully".into();
    app.update();

    let changed = &app.world().get::<AccessibilityNode>(status).unwrap().0;
    assert_eq!(changed.live(), Some(accesskit::Live::Polite));
    assert!(changed.is_live_atomic());
    assert_eq!(changed.value(), Some("Saved successfully"));
    assert_ne!(
        changed, &initial,
        "the AccessKit node must publish the live value change"
    );
}
