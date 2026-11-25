//! Cost of rebuilding a compact shell when a navigation sheet changes.

use bbui::chrome::{MobileChrome, MobileNavItem};
use bbui_widgets::{Command, CommandRegistry, WidgetNode, WidgetTree};
use bevy::ecs::world::World;
use criterion::Criterion;
use std::hint::black_box;

#[allow(clippy::expect_used)]
pub fn bench(c: &mut Criterion) {
    let mut registry = CommandRegistry::default();
    assert!(registry.register(Command::new("save", "Save")).is_ok());
    let mut chrome = MobileChrome::new(
        "Editor",
        vec![
            MobileNavItem::new("files", "Files"),
            MobileNavItem::new("scene", "Scene"),
            MobileNavItem::new("tools", "Tools"),
        ],
    );
    let view = |chrome: &MobileChrome| {
        WidgetTree::new(chrome.view(
            &registry,
            WidgetNode::label("Editor content"),
            Some(WidgetNode::column([
                WidgetNode::button("New"),
                WidgetNode::button("Open"),
            ])),
        ))
    };
    let mut world = World::new();
    let root = view(&chrome)
        .spawn(&mut world, None)
        .expect("initial shell");
    let mut open = false;
    c.bench_function("chrome/mobile_sheet_reconcile", |b| {
        b.iter(|| {
            if open {
                chrome.sheet_title = None;
            } else {
                chrome.navigate("files", "Files");
            }
            open = !open;
            let tree = view(&chrome);
            black_box(tree.reconcile(&mut world, root).expect("shell update"));
        })
    });
}
