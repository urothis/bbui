//! Measure UI render-pass GPU timestamps and CPU sync cost for large scenes.
//!
//! Run with `cargo run --release -p bui --example render_gpu_stress_bench --locked`.
//! This requires a Vulkan/DX12 adapter with timestamp-query support. The CPU fields exclude
//! the small sleep used to let the asynchronous render thread complete and publish diagnostics.

#[cfg(target_os = "linux")]
mod linux {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    use bevy::{
        app::PluginGroup,
        asset::RenderAssetUsages,
        camera::RenderTarget,
        diagnostic::{DiagnosticPath, DiagnosticsStore},
        ecs::entity::Entity,
        image::Image,
        prelude::{App, Camera2d, DefaultPlugins, WindowPlugin},
        render::{
            RenderPlugin,
            diagnostic::RenderDiagnosticsPlugin,
            render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
            settings::{RenderCreation, WgpuFeatures, WgpuSettings},
        },
        window::ExitCondition,
        winit::WinitPlugin,
    };
    use bui_render::{
        BuiRenderPlugin, ClipRect, DisplayList, DrawItem, ImageRegistry, Primitive,
        RenderTargetCamera,
    };
    use bui_style::Rgba;

    const WIDTH: u32 = 1280;
    const HEIGHT: u32 = 720;
    const FRAMES: usize = 24;
    const GPU_UI_TIME: DiagnosticPath = DiagnosticPath::const_new("render/ui/elapsed_gpu");

    struct TrackingAllocator;

    static TRACK_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
    static ALLOCATION_COUNT: AtomicU64 = AtomicU64::new(0);
    static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
    static LIVE_BYTES: AtomicU64 = AtomicU64::new(0);
    static PEAK_LIVE_BYTES: AtomicU64 = AtomicU64::new(0);

    #[global_allocator]
    static ALLOCATOR: TrackingAllocator = TrackingAllocator;

    // SAFETY: All pointers and layouts are forwarded unchanged to System. Atomics only observe sizes.
    unsafe impl GlobalAlloc for TrackingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc(layout) };
            if !pointer.is_null() {
                record_allocation(layout.size());
            }
            pointer
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc_zeroed(layout) };
            if !pointer.is_null() {
                record_allocation(layout.size());
            }
            pointer
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
            if !new_pointer.is_null() {
                if TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
                    ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
                    ALLOCATED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
                }
                update_live(new_size, layout.size());
            }
            new_pointer
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            unsafe { System.dealloc(pointer, layout) };
            update_live(0, layout.size());
        }
    }

    fn record_allocation(bytes: usize) {
        if TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
        }
        update_live(bytes, 0);
    }

    fn update_live(add: usize, remove: usize) {
        let live = if add > remove {
            LIVE_BYTES.fetch_add((add - remove) as u64, Ordering::Relaxed) + (add - remove) as u64
        } else if remove > add {
            LIVE_BYTES.fetch_sub((remove - add) as u64, Ordering::Relaxed) - (remove - add) as u64
        } else {
            LIVE_BYTES.load(Ordering::Relaxed)
        };
        let mut peak = PEAK_LIVE_BYTES.load(Ordering::Relaxed);
        while live > peak {
            match PEAK_LIVE_BYTES.compare_exchange_weak(
                peak,
                live,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => peak = actual,
            }
        }
    }

    struct AppHarness {
        app: App,
        camera: Entity,
        icon_asset: u64,
    }

    #[derive(Clone, Copy)]
    struct CpuStats {
        micros_per_frame: f64,
        allocations_per_frame: f64,
        kb_per_frame: f64,
        peak_live_kb: f64,
    }

    #[derive(Default)]
    struct FrameSamples {
        last_time: Option<Instant>,
        gpu_ms: Vec<f64>,
    }

    pub(super) fn run() {
        let mut harness = make_app();
        let image_id = harness.icon_asset;
        for (name, mut items) in [
            ("rects-10000", build_rect_scene(10_000)),
            ("image-grid-4096", build_image_grid(4_096, image_id)),
            ("graph-like-links-4000", build_link_scene(1_000, 4)),
        ] {
            let count = items.len();
            harness
                .app
                .world_mut()
                .resource_mut::<DisplayList>()
                .replace_camera_items(Some(harness.camera), items.clone());
            for _ in 0..12 {
                harness.app.update();
                thread::sleep(Duration::from_millis(2));
            }
            let idle = measure(&mut harness.app, harness.camera, FRAMES, &items, false);
            let animated = measure(&mut harness.app, harness.camera, FRAMES, &items, true);
            println!(
                "{name} primitives={count} idle: {} animated: {}",
                format_report(idle),
                format_report(animated)
            );
            items.clear();
        }
    }

    fn make_app() -> AppHarness {
        let mut app = App::new();
        let settings = WgpuSettings {
            force_fallback_adapter: true,
            features: WgpuSettings::default().features
                | WgpuFeatures::TIMESTAMP_QUERY
                | WgpuFeatures::TIMESTAMP_QUERY_INSIDE_PASSES,
            ..Default::default()
        };
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..Default::default()
                })
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(Box::new(settings)),
                    ..Default::default()
                })
                .disable::<WinitPlugin>()
                .disable::<bevy::pbr::PbrPlugin>(),
        );
        app.add_plugins((RenderDiagnosticsPlugin, BuiRenderPlugin));
        app.finish();
        app.cleanup();

        let mut target =
            Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
        target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let target = app
            .world_mut()
            .resource_mut::<bevy::asset::Assets<Image>>()
            .add(target);
        let icon = app
            .world_mut()
            .resource_mut::<bevy::asset::Assets<Image>>()
            .add(Image::new_fill(
                Extent3d {
                    width: 2,
                    height: 2,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[160, 190, 230, 255],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
            ));
        let icon_asset = 1;
        app.world_mut()
            .resource_mut::<ImageRegistry>()
            .insert(icon_asset, icon);
        let camera = app
            .world_mut()
            .spawn((Camera2d, RenderTarget::Image(target.clone().into())))
            .id();
        app.world_mut().resource_mut::<RenderTargetCamera>().0 = Some(camera);

        AppHarness {
            app,
            camera,
            icon_asset,
        }
    }

    fn measure(
        app: &mut App,
        camera: Entity,
        frames: usize,
        items: &[DrawItem],
        animate: bool,
    ) -> (CpuStats, Vec<f64>) {
        let mut samples = FrameSamples::default();
        let mut elapsed = Duration::ZERO;
        let baseline_live = LIVE_BYTES.load(Ordering::Relaxed);
        PEAK_LIVE_BYTES.store(baseline_live, Ordering::Relaxed);
        ALLOCATION_COUNT.store(0, Ordering::Relaxed);
        ALLOCATED_BYTES.store(0, Ordering::Relaxed);
        TRACK_ALLOCATIONS.store(true, Ordering::Relaxed);

        for frame in 0..frames {
            let started = Instant::now();
            if animate {
                app.world_mut()
                    .resource_mut::<DisplayList>()
                    .replace_camera_items(Some(camera), variant_items(items, frame));
            }
            app.update();
            elapsed += started.elapsed();
            record_gpu_sample(app, &mut samples);
            thread::sleep(Duration::from_millis(2));
        }
        TRACK_ALLOCATIONS.store(false, Ordering::Relaxed);

        let count = ALLOCATION_COUNT.load(Ordering::Relaxed);
        let bytes = ALLOCATED_BYTES.load(Ordering::Relaxed);
        let peak = PEAK_LIVE_BYTES
            .load(Ordering::Relaxed)
            .saturating_sub(baseline_live);
        let cpu = CpuStats {
            micros_per_frame: elapsed.as_secs_f64() * 1_000_000.0 / frames as f64,
            allocations_per_frame: count as f64 / frames as f64,
            kb_per_frame: bytes as f64 / frames as f64 / 1024.0,
            peak_live_kb: peak as f64 / 1024.0,
        };
        (cpu, samples.gpu_ms)
    }

    fn record_gpu_sample(app: &App, samples: &mut FrameSamples) {
        let Some(measurement) = app
            .world()
            .get_resource::<DiagnosticsStore>()
            .and_then(|store| store.get_measurement(&GPU_UI_TIME))
        else {
            return;
        };
        if samples.last_time == Some(measurement.time) {
            return;
        }
        samples.last_time = Some(measurement.time);
        samples.gpu_ms.push(measurement.value);
    }

    fn format_report((cpu, gpu): (CpuStats, Vec<f64>)) -> String {
        let gpu = if gpu.is_empty() {
            "gpu=unavailable (adapter did not publish timestamp queries)".to_owned()
        } else {
            let mean = gpu.iter().sum::<f64>() / gpu.len() as f64;
            let min = gpu.iter().copied().fold(f64::INFINITY, f64::min);
            let max = gpu.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            format!("gpu_ui={mean:.3}ms avg [{min:.3},{max:.3}] n={}", gpu.len())
        };
        format!(
            "cpu_update={:.2}us/frame {gpu}; {:.1}allocs/{:.1}KB peak_live+{:.1}KB",
            cpu.micros_per_frame, cpu.allocations_per_frame, cpu.kb_per_frame, cpu.peak_live_kb,
        )
    }

    fn variant_items(items: &[DrawItem], frame: usize) -> Vec<DrawItem> {
        let mut changed = items.to_vec();
        let tint = if frame.is_multiple_of(2) {
            Rgba(0.9, 0.25, 0.20, 1.0)
        } else {
            Rgba(0.2, 0.6, 0.9, 1.0)
        };
        for item in &mut changed {
            if let Primitive::RoundedRect { color, .. } = &mut item.primitive {
                *color = tint;
            }
        }
        changed
    }

    fn build_rect_scene(count: usize) -> Vec<DrawItem> {
        let clip = full_clip();
        (0..count)
            .map(|index| {
                let column = index % 128;
                let row = index / 128;
                let rect = ClipRect {
                    x: column as f32 * 10.0,
                    y: row as f32 * 7.0,
                    width: 8.0,
                    height: 6.0,
                };
                DrawItem {
                    clip,
                    z: index as i32,
                    primitive: Primitive::RoundedRect {
                        rect,
                        radius: 1.0,
                        color: Rgba(0.18, 0.42, 0.68, 1.0),
                    },
                }
            })
            .collect()
    }

    fn build_image_grid(count: usize, asset_id: u64) -> Vec<DrawItem> {
        let clip = full_clip();
        (0..count)
            .map(|index| {
                let column = index % 64;
                let row = index / 64;
                DrawItem {
                    clip,
                    z: index as i32,
                    primitive: Primitive::Image {
                        rect: ClipRect {
                            x: column as f32 * 20.0,
                            y: row as f32 * 12.0,
                            width: 18.0,
                            height: 10.0,
                        },
                        asset_id,
                    },
                }
            })
            .collect()
    }

    fn build_link_scene(nodes: usize, links_per_node: usize) -> Vec<DrawItem> {
        let clip = full_clip();
        let mut items = Vec::with_capacity(nodes + nodes * links_per_node);
        for index in 0..nodes {
            let column = index % 32;
            let row = index / 32;
            let x = column as f32 * 39.0;
            let y = row as f32 * 23.0;
            items.push(DrawItem {
                clip,
                z: index as i32,
                primitive: Primitive::RoundedRect {
                    rect: ClipRect {
                        x,
                        y,
                        width: 30.0,
                        height: 18.0,
                    },
                    radius: 2.0,
                    color: Rgba(0.2, 0.25, 0.32, 1.0),
                },
            });
        }
        for link in 0..nodes * links_per_node {
            let source = link % nodes;
            let target = (source + 1 + link / nodes % 8) % nodes;
            let from = [
                (source % 32) as f32 * 39.0 + 30.0,
                (source / 32) as f32 * 23.0 + 9.0,
            ];
            let to = [
                (target % 32) as f32 * 39.0,
                (target / 32) as f32 * 23.0 + 9.0,
            ];
            let dx = (to[0] - from[0]).abs().max(36.0) * 0.4;
            items.push(DrawItem {
                clip,
                z: (nodes + link) as i32,
                primitive: Primitive::Bezier {
                    points: [from, [from[0] + dx, from[1]], [to[0] - dx, to[1]], to],
                    width: 1.5,
                    color: Rgba(0.42, 0.62, 0.82, 1.0),
                },
            });
        }
        items
    }

    fn full_clip() -> ClipRect {
        ClipRect {
            x: 0.0,
            y: 0.0,
            width: WIDTH as f32,
            height: HEIGHT as f32,
        }
    }
}

#[cfg(target_os = "linux")]
fn main() {
    linux::run();
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("render_gpu_stress_bench currently supports the Linux offscreen renderer");
}
