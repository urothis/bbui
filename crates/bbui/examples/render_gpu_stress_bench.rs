//! Measure UI render-pass GPU timestamps and CPU sync cost for large scenes.
//!
//! Run with `cargo run --release -p bbui --example render_gpu_stress_bench --locked`.
//! This requires a Vulkan/DX12 adapter with timestamp-query support. The CPU fields exclude
//! the small sleep used to let the asynchronous render thread complete and publish diagnostics.

#[cfg(target_os = "linux")]
mod linux {
    use stats_alloc::{Stats, StatsAlloc};
    use std::alloc::System;
    use std::thread;
    use std::time::{Duration, Instant};

    use bbui_render::{
        BbuiRenderPlugin, ClipRect, DisplayList, DrawItem, ImageRegistry, Primitive,
        RenderTargetCamera,
    };
    use bbui_style::Rgba;
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

    const WIDTH: u32 = 1280;
    const HEIGHT: u32 = 720;
    const FRAMES: usize = 24;
    const GPU_UI_TIME: DiagnosticPath = DiagnosticPath::const_new("render/ui/elapsed_gpu");

    #[global_allocator]
    static ALLOCATOR: StatsAlloc<System> = StatsAlloc::system();

    fn live_bytes(stats: Stats) -> usize {
        let live = stats.bytes_allocated as i128 - stats.bytes_deallocated as i128
            + stats.bytes_reallocated as i128;
        usize::try_from(live.max(0)).unwrap_or(usize::MAX)
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
        app.add_plugins((RenderDiagnosticsPlugin, BbuiRenderPlugin));
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
        let before = ALLOCATOR.stats();
        let baseline_live = live_bytes(before);
        let mut peak_live = baseline_live;

        for frame in 0..frames {
            let started = Instant::now();
            if animate {
                app.world_mut()
                    .resource_mut::<DisplayList>()
                    .replace_camera_items(Some(camera), variant_items(items, frame));
            }
            app.update();
            peak_live = peak_live.max(live_bytes(ALLOCATOR.stats()));
            elapsed += started.elapsed();
            record_gpu_sample(app, &mut samples);
            thread::sleep(Duration::from_millis(2));
        }
        let delta = ALLOCATOR.stats() - before;
        let count = delta.allocations + delta.reallocations;
        let bytes = delta.bytes_allocated;
        let peak = peak_live.saturating_sub(baseline_live);
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
            "cpu_update={:.2}us/frame {gpu}; {:.1}allocs/{:.1}KB sampled_peak_live+{:.1}KB",
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
