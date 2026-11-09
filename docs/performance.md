# Performance benchmarking

The release-mode Criterion suite is the repeatable CPU comparison harness for the workspace:

```sh
cargo bench -p bui --bench performance --locked
cargo bench -p bui --bench performance --locked -- --save-baseline main
# After a code change, on the same machine:
cargo bench -p bui --bench performance --locked -- --baseline main
```

Open `target/criterion/report/index.html` for distributions and per-workload comparisons. Filter
to one path with `cargo bench -p bui --bench performance --locked -- 'widgets/full_update'`.

| Path | Timed cases |
| --- | --- |
| `bui_core`, `bui_layout` | Transaction undo/redo; idle and one-change layout at 1k/10k nodes |
| `bui_style`, `bui_widgets` | 1k/10k checkboxes; virtual list/table; 1k-node graph pan; active style animation |
| `bui_text` | Bundled-font shaping; expression evaluation; text edit/undo/redo |
| `bui_render` | First, idle, and one-change synchronization for rectangles and image grids |
| `bui_media` | Full PNG/SVG and PNG/GIF/VP8/H.264 poster decoding |
| `bui` | Public plugin headless CPU updates with 200/1k mounted controls |

Scene construction and fixture encoding are outside timed loops; each changed case includes the
edit and its `App::update`. Throughput is input elements or compressed bytes,
not drawn pixels or an FPS estimate. Run comparisons on the same machine with the same feature
set and build profile. Criterion measures CPU wall time; it does not report allocations, live
heap, GPU time, or frame pacing. The standalone harnesses below retain allocator/heap counters,
and the offscreen GPU stress harness retains real GPU timestamp queries.
Proc-macro expansion (`bui_macros`) and filesystem I/O are outside this runtime suite.

Initial release baseline on the local aarch64 development machine (2026-11-09, Rust 1.99,
30 samples per case; mean estimates rounded):

| Workload | Mean CPU time |
| --- | ---: |
| Layout idle / one intrinsic change, 10k nodes | 25.9 µs / 3.31 ms |
| Renderer sync first / idle / one changed rectangle, 10k items | 12.73 ms / 4.60 µs / 1.18 ms |
| Full widget update idle / one changed checkbox, 10k controls | 2.84 ms / 3.28 ms |
| Full widget update idle / graph pan, 1k nodes and 3,984 links | 122 µs / 4.12 ms |
| Public `BuiPlugin` update idle / one toggled checkbox, 1k controls | 1.31 ms / 2.72 ms |
| Wrapped paragraph shaping / 512 × 512 PNG full decode | 183 µs / 1.80 ms |

These are local comparison points, not portable performance targets. The graph pan stays within
the viewport, so each changed iteration paints the same graph-sized workload. The complete
confidence intervals and every case are in the generated Criterion report.

The numbers below were collected by earlier single-run harnesses on 2026-11-08. Use them as
historical context, not as statistical baselines for the new Criterion suite.

## Renderer CPU Measurements

Run `cargo run -p bui_render --example render_sync_bench --release --locked` to measure Bevy UI
entity synchronization for large draw lists. The harness records CPU elapsed time, allocation count
and bytes, peak live Rust heap change, idle update cost, generated render nodes, clip-group count,
and entity-ID churn after one item changes. It does not measure GPU time or memory.

Measurements on the development container (2026-11-08, release build):

| Scene | First sync | Allocations | Bytes allocated | Peak live heap change | Render nodes / clip groups |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1,000 rounded rectangles | 1.91 ms | 1,791 | 5.70 MB | 2.87 MB | 1,000 / 1 |
| 5,000 rounded rectangles | 4.79 ms | 1,983 | 30.69 MB | 15.45 MB | 5,000 / 1 |
| 10,000 rounded rectangles | 8.90 ms | 2,048 | 61.25 MB | 30.83 MB | 10,000 / 1 |
| 4,096 image grid | 4.20 ms | 2,008 | 24.87 MB | 12.52 MB | 4,096 / 1 |

The earlier per-item-clip baseline recorded 3,210 / 7,566 / 12,685 allocations for 1k / 5k / 10k
rectangles; current counts are lower by 44% / 74% / 84%. At 10k items, allocated bytes fell from
100.17 MB to 61.25 MB, and peak live heap change fell from 50.03 MB to 30.83 MB. First-sync time
was 1.49 / 6.41 / 11.95 ms in the baseline and is 1.91 / 4.79 / 8.90 ms now; the 1k sample is
slower, while the 5k and 10k samples improved. The 4,096-image scene changed from 6,634 allocations
and 28.78 MB to 2,008 allocations and 24.87 MB.

Across these scenes, 300 unchanged updates averaged 4.4–6.0 µs per frame and allocated once (64
bytes total). One changed draw item replaced one render-node entity while retaining its shared clip
group; those updates took 0.41–1.90 ms depending on scene size. Idle updates retained all node and
clip-group IDs. Use the same command and machine when comparing future changes.

The Linux offscreen GPU tests also read `Image` targets back after rendering. On llvmpipe, the
primitive scene's four captures took 32.7–35.9 ms from scheduling readback to callback delivery.
The real widget scene with an open menu, HSV editor, curve editor, color ramp, and list/tree/preview
grid took 74.0–83.4 ms per capture across 12 Dark/Light × DPI 1/2 × Constant/Linear/Smooth curve
variants. These are CPU wall times including Bevy update and queue delay, not GPU timestamps or
standalone GPU durations.
These scenes and the other editor readbacks check sampled pixels with defined color tolerances
rather than storing golden images. GPU timestamps are measured separately by the offscreen stress
harness; the visual coverage contract is in the [feature inventory](feature-inventory.md).

## Canvas Viewport Baseline

Run `cargo run --release -p bui_widgets --example canvas_stress --locked` to measure canvas painting and hit testing with 100,000 rectangles, 100 of them inside a 100 × 100 logical-pixel viewport. On the development container (2026-11-08, release build), 100 warm frames averaged **0.307 ms** for painting and **0.105 ms** for hit testing. A second pass counted **zero allocations and zero allocated bytes** across 100 paint and hit-test frames. Painting produced 101 draw items per frame (100 visible rectangles plus the background) and reused a 26,624-byte draw buffer. These CPU timings exclude Bevy render synchronization and GPU work. The accompanying regression test checks that 10,000 offscreen selected objects produce no draw items.

## Layout Dirty-Propagation Baseline

Run `cargo run --release -p bui_layout --example layout_idle_bench` to compare idle layout updates against one intrinsic-size change per update. The harness creates one root with 2,000 child nodes, warms up one initial layout, then measures 120 idle frames and 120 frames that each change one child's intrinsic width.

Measurements on the development container (2026-11-08, release build):

| Workload | Total time | Time per frame | Geometry assignments |
| --- | ---: | ---: | ---: |
| Idle | 921.459 µs | 7.68 µs | 0 |
| One intrinsic-size change per frame | 71.169 ms | 593.08 µs | 240,120 |

The changed workload assigns geometry to all 2,001 nodes in the affected root per update, keeping descendant rectangles and clips correct after ancestor or measurement changes. Idle frames skip that root traversal. The change collector still checks ECS change ticks for candidate layout entities; these figures measure the current Bevy `Changed`-query implementation on this machine.

## Responsive Layout Stress Measurements

Run `cargo run --release -p bui_layout --example layout_stress_bench --offline` to measure Flex, ResponsiveGrid, and six-level nested-scroll roots with 1,000 and 10,000 total layout nodes. Each scene is warmed for two updates, then measured for 20 idle frames and 20 frames with one input change per frame (an intrinsic width for Flex/Grid, or the innermost scroll offset). Geometry work is read from `UiLayoutStats`. CPU time is measured in a pass without allocator instrumentation; allocations are counted in a second pass. Allocation count and bytes include the full Bevy `App::update` path, not only `compute_layout`, and report total requested allocation traffic rather than peak live heap.

Measurements on the development container (2026-11-08, release build):

| Layout / nodes | Idle µs/frame | Idle allocs / KB per frame | Dirty µs/frame | Dirty allocs / KB per frame | Geometry nodes / dirty frame |
| --- | ---: | ---: | ---: | ---: | ---: |
| Flex / 1,000 | 6.87 | 127 / 6.9 | 309.08 | 1,318 / 348.9 | 1,000 |
| ResponsiveGrid / 1,000 | 6.48 | 127 / 6.9 | 312.85 | 1,160 / 164.1 | 1,000 |
| Nested scroll / 1,000 | 6.79 | 127 / 6.9 | 355.31 | 1,183 / 183.2 | 1,000 |
| Flex / 10,000 | 27.79 | 127 / 6.9 | 3,085.50 | 11,665 / 3,238.8 | 10,000 |
| ResponsiveGrid / 10,000 | 25.41 | 127 / 6.9 | 3,151.32 | 10,166 / 1,396.2 | 10,000 |
| Nested scroll / 10,000 | 25.85 | 127 / 6.9 | 3,568.21 | 10,189 / 1,586.7 | 10,000 |

The measured 10k dirty baseline before the sizing changes was 3.42 ms / 3,316.9 KB per Flex update, 3.48 ms / 1,474.3 KB per ResponsiveGrid update, and 7.53 ms / 3,929.5 KB per nested-scroll update. The current run is 3.09 ms / 3,238.8 KB, 3.15 ms / 1,396.2 KB, and 3.57 ms / 1,586.7 KB respectively. The largest gain is nested scrolling: fixed-size intermediate containers no longer recursively measure their descendants when the exact measured width and height are already known. Flex and ResponsiveGrid also avoid a redundant generic measurement vector, and intrinsic measurement no longer clones each descendant's `UiChildren` list.

Idle frames visit zero layout nodes and leave all six scenes at 127 small App-level allocations and 6.9 KB/frame. Idle time grows from about 6.5–6.9 µs at 1k to 25–28 µs at 10k. The change collector still uses Bevy's `Changed` filter, which checks candidate entities even when no value changed; therefore idle detection has entity-count cost even though it skips geometry traversal. A dirty input conservatively recomputes its complete root so descendant hit boxes, scroll clips, and positions remain synchronized. These figures do not include scene creation, GPU work, or allocation live-heap usage.

## Widget Paint Stress Baseline

Run `cargo run --release -p bui_widgets --example widget_stress_bench --locked` to measure a
responsive-grid scene containing 1,000 or 10,000 checkboxes. The harness warms the Bevy app and
measures idle updates, updates that toggle one checkbox, direct `collect_widget_paint` calls, draw
item count, and the number of laid-out checkboxes intersecting the 1,280 × 720 viewport. Allocation
counts and bytes are total requested allocation traffic per frame; GPU work is excluded.

Measurements on the development container (2026-11-08, release build):

| Controls | Viewport-visible | Draw items | App idle µs / allocs / KB | App changed µs / allocs / KB | Painter idle µs / allocs / KB | Painter changed µs / allocs / KB |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1,000 | 184 | 2,000 | 4,661 / 43,971 / 14,582.3 | 4,724 / 44,972 / 15,001.4 | 325 / 5,025 / 1,924.0 | 376 / 6,026 / 2,343.2 |
| 10,000 | 184 | 20,000 | 46,350 / 415,303 / 149,883.1 | 47,522 / 425,304 / 154,072.7 | 3,432 / 50,029 / 24,230.2 | 5,030 / 60,030 / 28,419.9 |

The app processes 10,001 layout nodes in the 10k case. Only 184 checkbox hit boxes intersect the
viewport, but the painter still emits two items for every control. The idle painter therefore has
linear CPU and allocation cost even when no widget changes; direct painting at 10k takes 3.43 ms
and requests about 24.2 MB per frame. This is the baseline for change-aware paint caching and
viewport culling. Measurements are single-run local baselines, not cross-machine performance
targets.

The collector now skips repainting when UI component change ticks, theme/interaction inputs, focus
targets, and tooltip ownership are unchanged. It also drops controls whose layout rectangle has no
intersection with its inherited clip. Repeating the same benchmark after those changes gives:

| Controls | Viewport-visible | Draw items | App idle µs / allocs / KB | App changed µs / allocs / KB | Painter idle µs / allocs / KB | Painter changed µs / allocs / KB |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1,000 | 184 | 368 | 4,417 / 38,946 / 12,658.2 | 4,518 / 39,889 / 13,143.1 | 131 / 0 / 0.0 | 224 / 943 / 484.8 |
| 10,000 | 184 | 368 | 44,275 / 365,274 / 125,652.9 | 44,647 / 366,217 / 126,208.0 | 1,325 / 0 / 0.0 | 1,761 / 943 / 555.2 |

For 10k controls, idle painting is 61% faster, with all 50,029 painter allocations and 24.2 MB of
traffic removed. Draw items now match the 184 visible controls (two items each), rather than drawing
all 10,000. A visible checkbox mutation rebuilds in the same call and causes 943 allocations for
the 184 visible controls. Idle fingerprinting still visits the UI entity/component ticks, so its CPU
cost grows with control count; the later Style phase currently dominates whole-app idle time.

The style resolver now fingerprints theme, focus, and style-bearing component change ticks before
building cascades. If the fingerprint matches the last settled pass, both style phases return
without sorting entities or allocating declarations. Active transitions still change their ticks
each frame and continue to advance and resolve; focus or inline-style changes in the input phase
invalidate `StyleAfterInput` in the same update.

Repeating the release benchmark after style dirty gating gives:

| Controls | App idle µs / allocs / KB | App changed µs / allocs / KB | Painter idle µs / allocs / KB | Painter changed µs / allocs / KB |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 | 307.02 / 344 / 19.7 | 407.41 / 1,287 / 504.6 | 132.78 / 0 / 0.0 | 227.88 / 943 / 484.8 |
| 10,000 | 2,983.30 / 344 / 19.7 | 3,495.53 / 1,287 / 574.9 | 1,346.63 / 0 / 0.0 | 1,831.35 / 943 / 555.2 |

At 10k controls, whole-app idle time falls from 44.27 ms to 2.98 ms per frame, and allocation
traffic falls from 365,274 allocations / 125.7 MB to 344 / 19.7 KB. A changed checkbox takes 3.50
ms and 575 KB of allocations. Direct style advancement plus both resolver calls is now 1.58 ms
per idle frame with zero allocations, down from 42.55 ms and 12,164 allocations / 125.6 MB.
Other direct idle groups measure core input 0.06 µs, layout 23.67 µs, text 0.53 µs, widget
synchronization 12.12 µs, and painting 1.346 ms per frame.

The style and paint fingerprints still visit ECS entities to read their component change ticks,
so their CPU cost grows with control count even when they allocate nothing. At 10k controls the
remaining measured idle work is concentrated in style fingerprinting and paint fingerprinting;
GPU submission and scene construction are not included. Group timings are direct function calls,
while whole-app measurements use the scheduled `App::update` path. Measurements are local
single-run data, not cross-machine performance targets.

## Virtualized Model Stress Measurements

Run `cargo run --release -p bui_widgets --example widget_model_stress_bench --locked` to exercise
large list, table, graph, and animated-style models through a warmed BUI app. The list has 10,000
rows, the table has 10,000 × 8 (80,000) cells, the graph has 1,000 nodes and 3,984 links, and the
style scene animates 1,000 controls. The harness reports idle and changed/animated `App::update`
time, global-allocator traffic, peak live Rust heap delta, live Rust heap, entity count,
materialized item count, and draw item count. These are local single-run measurements; global
allocator heap figures do not include Bevy GPU resources or driver memory.

Measurements on the development container (2026-11-08, release build):

| Workload | Model items | Entities / materialized | Draw items | Idle µs / allocs / KB | Changed µs / allocs / KB | Peak live heap delta / live heap |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Long list | 10,000 rows | 128 / 32 | 64 | 345 / 11,016 / 791.2 | 370 / 11,624 / 829.4 | 533 / 2,410 KB |
| Long table | 80,000 cells | 512 / 224 | 701 | 89 / 356 / 20.7 | 1,016 / 12,885 / 2,477.4 | 396 / 7,111 KB |
| Node graph | 4,984 nodes and links | 512 / 35 | 4,564 | 118 / 355 / 20.6 | 1,512 / 2,753 / 4,819.7 | 1,676 / 5,939 KB |
| Style animation | 1,000 controls | 2,048 / 1,000 | 2,000 | 344 / 344 / 19.8 | 3,021 / 24,408 / 12,729.6 | 444 / 7,496 KB |

Owner-level change stamps now skip virtual-view construction while model, layout, and theme inputs
are stable. Graph geometry computes socket endpoints once per node before building visible link
controls. Against the original stress baseline, table idle time fell from 2.82 ms to 89 µs and
allocation traffic from 95,983 allocations / 5.72 MB to 356 / 20.7 KB. Graph idle time fell from
2.80 ms to 118 µs and allocation traffic from 15,450 / 6.18 MB to 355 / 20.6 KB. A changed table
update measured 1.02 ms, and a panned graph update measured 1.51 ms. The list and transition scenes
remain useful controls for comparing unrelated widget paths. These numbers measure app updates;
they do not include GPU submission or driver memory.

## Offscreen GPU Stress Measurements

Run `cargo run --release -p bui --example render_gpu_stress_bench --locked` to render 10,000
rounded rectangles, 4,096 images, and a graph-like scene of 1,000 node bodies plus 4,000 Bezier
links into an offscreen 1,280 × 720 target. The harness reports CPU app-update time, allocation
traffic and peak live Rust heap, plus `render/ui/elapsed_gpu` timestamp queries (24 samples per
result here). Animated CPU time includes replacing the display list with a per-frame variant before
the app update, but excludes timestamp polling and the deliberate 2 ms inter-frame sleep.

Measurements on the development container (2026-11-08, release build, Vulkan llvmpipe software
adapter):

| Scene | Mode | CPU µs/frame | GPU UI ms avg (range) | Allocations / frame | Allocated KB / frame | Peak live heap delta KB |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 10,000 rectangles | Idle | 23,841 | 7.336 (3.461–10.588) | 612 | 958.3 | 863.8 |
| 10,000 rectangles | Animated | 87,595 | 6.996 (5.396–9.162) | 20,646 | 19,291.3 | 37,039.3 |
| 4,096 images | Idle | 11,248 | 2.509 (1.833–3.365) | 613 | 450.9 | 354.6 |
| 4,096 images | Animated | 13,487 | 2.443 (1.866–3.297) | 616 | 2,946.9 | 1,669.4 |
| 4,000 Bezier links + 1,000 nodes | Idle | 45,993 | 36.734 (34.648–39.622) | 623 | 186.6 | 92.1 |
| 4,000 Bezier links + 1,000 nodes | Animated | 51,695 | 36.371 (34.961–38.829) | 2,631 | 4,368.7 | 1,226.5 |

The links share a batch when adjacent in draw order and use the same clip and target camera. This
scene produces one Bezier `MaterialNode` and three storage buffers for all 4,000 curves, instead of
4,000 material nodes (or the earlier 96,000 tessellation line entities). The shader reads a
tile-indexed list of tessellated segments and composites the curves in their original z order.
Changed node-body colors leave the link buffers untouched. Compared with the preceding per-link
material run, the link scene's measured GPU time fell from 172.779 / 179.044 ms to 36.734 / 36.371
ms for idle / animated frames; CPU update time fell from 199.242 / 216.343 ms to 45.993 / 51.695
ms. The 4k case is still a substantial workload on llvmpipe, so software-adapter time is a
regression baseline rather than a hardware performance target. The reported peak is live Rust heap
delta, not driver memory or process RSS. GPU timing is available because this adapter supports
timestamp queries; on adapters without them, the harness reports `gpu=unavailable` rather than
substituting wall time.
