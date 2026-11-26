//! Cost of rebuilding a compact shell when a navigation sheet changes.

use bbui::chrome::{MobileChrome, MobileChromeSlots};
use bbui_widgets::{WidgetNode, WidgetTree};
use bevy::ecs::world::World;
use criterion::Criterion;
use std::hint::black_box;

#[allow(clippy::expect_used)]
pub fn bench(c: &mut Criterion) {
    let chrome = MobileChrome;
    let view = |open: bool| {
        WidgetTree::new(chrome.view(MobileChromeSlots {
            toolbar: WidgetNode::label("Editor"),
            content: WidgetNode::label("Editor content"),
            sheet: open.then(|| {
                WidgetNode::column([
                    WidgetNode::label("Files"),
                    WidgetNode::button("New"),
                    WidgetNode::button("Open"),
                ])
            }),
            navigation: WidgetNode::row([
                WidgetNode::button("Files"),
                WidgetNode::button("Scene"),
                WidgetNode::button("Tools"),
            ]),
        }))
    };
    let mut world = World::new();
    let root = view(false).spawn(&mut world, None).expect("initial shell");
    let mut open = false;
    c.bench_function("chrome/mobile_sheet_reconcile", |b| {
        b.iter(|| {
            open = !open;
            let tree = view(open);
            black_box(tree.reconcile(&mut world, root).expect("shell update"));
        })
    });
}
