# UI Invalidation Contract

`bui_core::UiDirty` is the shared change journal. Producers mark an entity or a whole
domain with `UiDirty::mark` or `UiDirty::mark_all`; each consumer drains only its own
domain. The domains are style, layout, text measurement, text layout, text-control
synchronization, widget paint, render synchronization, hit testing, accessibility, and
file-browser previews.
Draining one domain never consumes another subsystem's work.

The scheduled collectors bridge direct public Bevy ECS edits into this journal using
component change ticks and removal observers. Resource changes such as a theme switch or
font update mark the affected domain globally. Internal mutations mark the journal at
their source when a second phase must observe them in the same update. For example,
label synchronization marks text measurement, and a widget-phase text scroll marks text
layout before paint.

Consumers use the narrowest safe scope. Style resolves dirty entities and their
descendants; text shapes dirty buffers; accessibility rebuilds dirty semantic nodes and
their descendants. Layout follows the dirty ancestor path, reflows siblings when their
geometry depends on that path, and reuses cached measurements for independent branches.
Widget paint publishes only changed entity-keyed fragments and updates hierarchy ranks
around moved subtrees. Render synchronization updates those fragments, changed image IDs,
and entity-attached paint nodes while retaining stable order and camera routing. Global
inputs such as a theme switch invalidate their domains. Structural layout changes can
require a full affected-root pass. A stationary pointer
is hit-tested again when its position changes, a changed hit box reaches its position or
previously owned the hover, or inherited interaction state changes. Active transitions,
media playback, pending preview decoding, and preview cache expiry continue to advance
while active. Settled file-browser previews skip synchronization until a related browser,
item, visibility, layout, or theme input changes.

Collection rows and file-browser rows use per-owner component-change stamps as their dirty-owner
set. A stable owner returns before enumerating visible rows; a changed owner rebuilds only its
mounted window. Collection stamps cover the model, layout, theme, key, enabled, and visible state.
File-browser stamps also cover `FileBrowserViewState`. Mounted row components, layouts, hit boxes,
and parent links are written only when their values differ. Those actual changes flow through the
widget paint collector into `UiDirty` for paint and accessibility consumers.

Canvas objects and graph nodes/links use stable child fragments. Routed edits and the public
`edit_canvas_object(world, owner, id, edit)` and
`move_node_graph_node(world, owner, id, position)` APIs record affected IDs at the mutation site.
Canvas updates its spatial index entry; graph updates that node and its incident links. Direct
edits to the public model vectors still produce correct fragments, but need an index rebuild or
signature comparison because a Bevy component change tick does not identify the changed element.
Graph links share bounded 128-curve GPU batches, so a
link edit rebuilds its batch while retaining the other batch entities. Menus, option rows, and
panels use owner change stamps; collapsed panels traverse their own descendants when layout or
animation changes and restore prior clips when children leave the panel. Settled icon and font
assets skip load polling and overlay construction until an input or load state changes.

`FileBrowser` keeps a filtered-position index and a path-to-visible-index lookup. Its index-defining
state is read through `entries()`, `filter()`, `sort()`, and `ascending()`; change it through
`set_filter`, `set_sort`, navigation, `refresh`, and restore methods so the index remains current.

When adding a widget component, register its paint and accessibility change detection
and removal in `bui_widgets::actions`. When adding a new layout or text input, add it to
the corresponding collector or mark its domain when mutating it. Avoid writing a
component through `get_mut` when the value is unchanged: Bevy will otherwise report a
change and invalidate downstream work. Public direct-call functions retain full-world
behavior when no scheduled collector has prepared a dirty batch.

The Criterion suite in [performance.md](performance.md) measures both unchanged and
changed updates. Use the same fixture and machine when checking an optimization;
compare visual and interaction tests as well as elapsed time.
