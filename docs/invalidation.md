# UI Invalidation Contract

`bui_core::UiDirty` is the shared change journal. Producers mark an entity or a whole
domain with `UiDirty::mark` or `UiDirty::mark_all`; each consumer drains only its own
domain. The domains are style, layout, text measurement, text layout, text-control
synchronization, widget paint, render synchronization, hit testing, and accessibility.
Draining one domain never consumes another subsystem's work.

The scheduled collectors bridge direct public Bevy ECS edits into this journal using
component change ticks and removal observers. Resource changes such as a theme switch or
font update mark the affected domain globally. Internal mutations mark the journal at
their source when a second phase must observe them in the same update. For example,
label synchronization marks text measurement, and a widget-phase text scroll marks text
layout before paint.

Consumers use the narrowest safe scope. Style resolves dirty entities and their
descendants; text shapes dirty buffers; accessibility rebuilds dirty semantic nodes and
their descendants. Layout recomputes owning roots when a change can affect sibling
placement. Widget paint and render synchronization skip settled frames and rebuild when
their input changes. A stationary pointer is hit-tested again only after a pointer or
hit-geometry change. Active transitions and media playback continue to advance while
active.

When adding a widget component, register its paint and accessibility change detection
and removal in `bui_widgets::actions`. When adding a new layout or text input, add it to
the corresponding collector or mark its domain when mutating it. Avoid writing a
component through `get_mut` when the value is unchanged: Bevy will otherwise report a
change and invalidate downstream work. Public direct-call functions retain full-world
behavior when no scheduled collector has prepared a dirty batch.

The Criterion suite in [performance.md](performance.md) measures both unchanged and
changed updates. Use the same fixture and machine when checking an optimization;
compare visual and interaction tests as well as elapsed time.
