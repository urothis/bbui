# Incremental update contract

`bbui_core::UiDirty` records which entity or domain changed. Style, layout, text measurement, text layout, text-control sync, widget paint, render sync, hit testing, accessibility, and file previews drain independent domains. Component change collectors and removal observers bridge public Bevy ECS edits into that journal.

A stable frame skips settled widget paint, layout, text shaping, render fragments, and AccessKit nodes. A changed subtree invalidates its affected layout path and dependent siblings; unaffected branches keep cached measurements. Widget paint stores entity-keyed fragments, and render sync updates changed fragments and image IDs. Canvas objects and graph nodes/links have stable child fragments; direct edit APIs mark affected IDs at mutation time.

Visible rows in lists, trees, tables, and the file browser are virtualized. Their owner changes update the visible window and reuse stable row entities. Active motion, media playback, pending previews, and preview expiry still advance while active. World-space accessibility bounds refresh when a surface, view camera, or physical window changes, even when the logical UI layout does not.

An inherited `UiFontAsset` change repaints its text descendants, including when an offscreen font owner drops its handle and those descendants return to the bundled fallback.

To extend the framework, mark the right dirty domain when mutating a value internally and register public component changes/removals with the owning collector. Avoid calling `get_mut` merely to check a value: Bevy will mark that component changed. Check [Criterion benchmarks](performance.md) for idle and one-change workloads before and after performance changes.
