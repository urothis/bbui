//! Measure widget update and paint cost for 1k/10k controls.
//!
//! Run with `cargo run --release -p bui_widgets --example widget_stress_bench --locked`.
//! The benchmark warms the Bevy schedules, then records idle frames and frames that toggle one
//! checkbox. Allocation totals include the full `App::update` path. Draw-item counts come from
//! `DisplayList`; GPU submission is not included.

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use bevy_app::App;
use bui_core::{
    CorePlugin, UiEnabled, UiFocusable, UiHitBox, UiNode, UiRect, UiTabIndex, UiVisible, set_parent,
};
use bui_layout::{LayoutPlugin, Rect, UiLayoutNode, UiLayoutRoot, UiLayoutStats, UiLength};
use bui_render::DisplayList;
use bui_style::StylePlugin;
use bui_text::TextPlugin;
use bui_widgets::{
    Checkbox, WidgetPlugin,
    actions::{
        advance_panel_animations, advance_tooltips, apply_widget_actions, collect_widget_paint,
        process_routed_widget_events, sync_collection_views, sync_data_table_views,
        sync_file_browser_items, sync_menu_popups, sync_node_graph_views, sync_panel_children,
        sync_text_controls,
    },
};

struct TrackingAllocator;

static TRACK_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

// SAFETY: Allocation operations delegate to the system allocator unchanged. The optional
// counters only observe allocation sizes and do not retain or dereference allocation pointers.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() && TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() && TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !new_pointer.is_null() && TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
            record_allocation(new_size);
        }
        new_pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }
}

fn record_allocation(bytes: usize) {
    ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
    ALLOCATED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
}

struct Scene {
    app: App,
    changed_control: bevy_ecs::entity::Entity,
    controls: usize,
}

#[derive(Clone, Copy)]
struct AllocationStats {
    count: u64,
    bytes: u64,
}

fn main() {
    const FRAMES: usize = 30;
    for controls in [1_000, 10_000] {
        let mut scene = make_scene(controls);
        scene.app.update();
        scene.app.update();

        let idle_time = measure_time(&mut scene, FRAMES, false);
        let changed_time = measure_time(&mut scene, FRAMES, true);
        let idle_allocations = measure_allocations(&mut scene, FRAMES, false);
        let changed_allocations = measure_allocations(&mut scene, FRAMES, true);
        let idle_paint_time = measure_paint_time(&mut scene, FRAMES, false);
        let changed_paint_time = measure_paint_time(&mut scene, FRAMES, true);
        let idle_paint_allocations = measure_paint_allocations(&mut scene, FRAMES, false);
        let changed_paint_allocations = measure_paint_allocations(&mut scene, FRAMES, true);
        let draw_items = scene
            .app
            .world()
            .resource::<DisplayList>()
            .widget_items()
            .len();
        let visible_controls = visible_controls(&mut scene.app, Rect::new(0.0, 0.0, 1280.0, 720.0));
        let layout_nodes = scene
            .app
            .world()
            .resource::<UiLayoutStats>()
            .total_nodes_processed;
        let subsystem_profile = profile_idle_subsystems(&mut scene, FRAMES);

        println!(
            "controls={} visible_controls={} draw_items={} app_idle={:.2} us/frame {:.1} allocs/{:.1} KB; app_changed={:.2} us/frame {:.1} allocs/{:.1} KB; painter_idle={:.2} us/frame {:.1} allocs/{:.1} KB; painter_changed={:.2} us/frame {:.1} allocs/{:.1} KB; layout_nodes={} total; direct idle groups: core={:.2}us style={:.2}us/{:.1} allocs/{:.1}KB layout={:.2}us text={:.2}us widget_sync={:.2}us paint={:.2}us per frame",
            scene.controls,
            visible_controls,
            draw_items,
            micros_per_frame(idle_time, FRAMES),
            idle_allocations.count as f64 / FRAMES as f64,
            idle_allocations.bytes as f64 / FRAMES as f64 / 1024.0,
            micros_per_frame(changed_time, FRAMES),
            changed_allocations.count as f64 / FRAMES as f64,
            changed_allocations.bytes as f64 / FRAMES as f64 / 1024.0,
            micros_per_frame(idle_paint_time, FRAMES),
            idle_paint_allocations.count as f64 / FRAMES as f64,
            idle_paint_allocations.bytes as f64 / FRAMES as f64 / 1024.0,
            micros_per_frame(changed_paint_time, FRAMES),
            changed_paint_allocations.count as f64 / FRAMES as f64,
            changed_paint_allocations.bytes as f64 / FRAMES as f64 / 1024.0,
            layout_nodes,
            subsystem_profile.core,
            subsystem_profile.style,
            subsystem_profile.style_allocations.count as f64 / FRAMES as f64,
            subsystem_profile.style_allocations.bytes as f64 / FRAMES as f64 / 1024.0,
            subsystem_profile.layout,
            subsystem_profile.text,
            subsystem_profile.widget_sync,
            subsystem_profile.paint,
        );
    }
}

fn make_scene(controls: usize) -> Scene {
    let mut app = App::new();
    app.add_plugins((
        CorePlugin,
        StylePlugin,
        LayoutPlugin,
        TextPlugin,
        WidgetPlugin,
    ));
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiEnabled(true),
            UiVisible(true),
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 1280.0, 720.0), 1.0),
            UiLayoutNode {
                clip_contents: true,
                ..UiLayoutNode::responsive_grid(148, 8, 4.0)
            },
        ))
        .id();

    let mut changed_control = None;
    for index in 0..controls {
        let control = app
            .world_mut()
            .spawn((
                UiNode,
                UiEnabled(true),
                UiVisible(true),
                UiFocusable,
                UiTabIndex(index as i32),
                UiLayoutNode::leaf(UiLength::Fill(1.0), UiLength::Px(28.0)),
                Checkbox::new(format!("Control {index:05}"), false),
            ))
            .id();
        set_parent(app.world_mut(), control, root).expect("benchmark hierarchy is valid");
        if index == 0 {
            changed_control = Some(control);
        }
    }

    Scene {
        app,
        changed_control: changed_control.expect("benchmark has controls"),
        controls,
    }
}

fn measure_time(scene: &mut Scene, frames: usize, dirty: bool) -> Duration {
    let started = Instant::now();
    for _ in 0..frames {
        if dirty {
            toggle_one_control(scene);
        }
        scene.app.update();
        black_box(
            scene
                .app
                .world()
                .resource::<DisplayList>()
                .widget_items()
                .len(),
        );
    }
    started.elapsed()
}

fn measure_allocations(scene: &mut Scene, frames: usize, dirty: bool) -> AllocationStats {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    TRACK_ALLOCATIONS.store(true, Ordering::Relaxed);
    for _ in 0..frames {
        if dirty {
            toggle_one_control(scene);
        }
        scene.app.update();
        black_box(
            scene
                .app
                .world()
                .resource::<DisplayList>()
                .widget_items()
                .len(),
        );
    }
    TRACK_ALLOCATIONS.store(false, Ordering::Relaxed);
    AllocationStats {
        count: ALLOCATION_COUNT.load(Ordering::Relaxed),
        bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
    }
}

fn measure_paint_time(scene: &mut Scene, frames: usize, dirty: bool) -> Duration {
    let started = Instant::now();
    for _ in 0..frames {
        if dirty {
            scene.app.world_mut().increment_change_tick();
            toggle_one_control(scene);
        }
        collect_widget_paint(scene.app.world_mut());
        black_box(
            scene
                .app
                .world()
                .resource::<DisplayList>()
                .widget_items()
                .len(),
        );
    }
    started.elapsed()
}

fn measure_paint_allocations(scene: &mut Scene, frames: usize, dirty: bool) -> AllocationStats {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    TRACK_ALLOCATIONS.store(true, Ordering::Relaxed);
    for _ in 0..frames {
        if dirty {
            scene.app.world_mut().increment_change_tick();
            toggle_one_control(scene);
        }
        collect_widget_paint(scene.app.world_mut());
        black_box(
            scene
                .app
                .world()
                .resource::<DisplayList>()
                .widget_items()
                .len(),
        );
    }
    TRACK_ALLOCATIONS.store(false, Ordering::Relaxed);
    AllocationStats {
        count: ALLOCATION_COUNT.load(Ordering::Relaxed),
        bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
    }
}

fn toggle_one_control(scene: &mut Scene) {
    if let Some(mut checkbox) = scene
        .app
        .world_mut()
        .get_mut::<Checkbox>(scene.changed_control)
    {
        checkbox.toggle();
    }
}

fn visible_controls(app: &mut App, viewport: Rect) -> usize {
    let viewport = UiRect::new(viewport.x, viewport.y, viewport.width, viewport.height);
    let mut query = app.world_mut().query::<(&Checkbox, &UiHitBox)>();
    query
        .iter(app.world())
        .filter(|(_, hitbox)| {
            hitbox
                .rect
                .intersection(viewport)
                .and_then(|rect| {
                    hitbox
                        .clip
                        .map_or(Some(rect), |clip| rect.intersection(clip))
                })
                .is_some()
        })
        .count()
}

#[derive(Clone, Copy)]
struct SubsystemProfile {
    core: f64,
    style: f64,
    style_allocations: AllocationStats,
    layout: f64,
    text: f64,
    widget_sync: f64,
    paint: f64,
}

fn profile_idle_subsystems(scene: &mut Scene, frames: usize) -> SubsystemProfile {
    fn measure(
        scene: &mut Scene,
        frames: usize,
        mut run: impl FnMut(&mut bevy_ecs::world::World),
    ) -> (f64, AllocationStats) {
        for _ in 0..2 {
            run(scene.app.world_mut());
        }
        let started = Instant::now();
        for _ in 0..frames {
            run(scene.app.world_mut());
        }
        let elapsed = started.elapsed().as_secs_f64() * 1_000_000.0 / frames as f64;
        ALLOCATION_COUNT.store(0, Ordering::Relaxed);
        ALLOCATED_BYTES.store(0, Ordering::Relaxed);
        TRACK_ALLOCATIONS.store(true, Ordering::Relaxed);
        for _ in 0..frames {
            run(scene.app.world_mut());
        }
        TRACK_ALLOCATIONS.store(false, Ordering::Relaxed);
        (
            elapsed,
            AllocationStats {
                count: ALLOCATION_COUNT.load(Ordering::Relaxed),
                bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
            },
        )
    }

    let (core, _) = measure(scene, frames, bui_core::process_ui_input);
    let (style, style_allocations) = measure(scene, frames, |world| {
        bui_style::advance_ui_style_transitions(world);
        bui_style::resolve_ui_styles(world);
        bui_style::resolve_ui_styles(world);
    });
    let (layout, _) = measure(scene, frames, bui_layout::compute_layout);
    let (text, _) = measure(scene, frames, |world| {
        bui_text::measure_text_intrinsics(world);
        bui_text::process_text_events(world);
        bui_text::update_text_layouts(world);
    });
    let (widget_sync, _) = measure(scene, frames, |world| {
        process_routed_widget_events(world);
        advance_tooltips(world);
        apply_widget_actions(world);
        advance_panel_animations(world);
        sync_panel_children(world);
        sync_menu_popups(world);
        sync_file_browser_items(world);
        sync_collection_views(world);
        sync_data_table_views(world);
        sync_node_graph_views(world);
        sync_text_controls(world);
    });
    let (paint, _) = measure(scene, frames, collect_widget_paint);
    SubsystemProfile {
        core,
        style,
        style_allocations,
        layout,
        text,
        widget_sync,
        paint,
    }
}

fn micros_per_frame(elapsed: Duration, frames: usize) -> f64 {
    elapsed.as_secs_f64() * 1_000_000.0 / frames as f64
}
