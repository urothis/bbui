# Widgets and application state

`WidgetNode` describes controls and layout; `WidgetTree::spawn(world, parent)` mounts them as Bevy entities. `bbui!` is the equivalent Rust declaration macro. The [controls example](../crates/bbui/examples/controls.rs) shows both mount and event handling; the [properties editor](visuals/examples/properties_editor.png) combines the same controls into a full editor.

## Read edits

BBUI owns transient interaction state: focus, text caret, popup state, drag capture, and local undo. Read `WidgetEventQueue` after `UiSystems::Widgets` to update your application model. Events include activation, value changes, committed edits, selection, and operation failures. Use `WidgetActionQueue` when the application needs to drive an existing control through its normal validation and history path. Avoid rebuilding the whole tree for a single value edit.

## Update a declared tree

Give siblings stable, unique keys with `.with_key("object-name")`. Reconcile a replacement declaration with `new_tree.reconcile(world, root_entity)`. Unchanged keyed nodes retain their entities and live widget state even when siblings move. Changed declarations intentionally replace that control's declared value and configuration. New nodes mount; removed nodes cancel active interactions and despawn. Duplicate sibling keys reject the update before any entity changes.

```rust
let tree = WidgetTree::new(WidgetNode::column([
    WidgetNode::text_field("Name", "Cube").with_key("name"),
    WidgetNode::button("Apply").with_key("apply"),
]));
let root = tree.spawn(world, Some(layout_root))?;
// Later, after constructing a new WidgetTree:
new_tree.reconcile(world, root)?;
```

The root must still be a mounted declarative node. For responsive sizing, use `UiLayoutNode`, `UiLength::Fill`, and the grid/flex/scroll constructors; see [layout gallery](visuals/examples/layout_gallery.png). Use [keyframe motion](motion.md) for animated properties and [theme tokens](theme.md) for color and spacing.

## Editing families

Number fields support expression input, precision scrubbing, snapping, units, and shared undo. Menus include fuzzy search and pie selection. Collections and tables virtualize visible rows. Canvas and node editors preserve model IDs through edits. File operations are external filesystem changes; ask for explicit confirmation before destructive actions in your host UI. `FileBrowser::confine_to(root)` restricts navigation and transfer destinations to a canonical subtree. The [file-browser example](../crates/bbui/examples/file_browser.rs) creates a disposable temp folder, confines itself there, confirms deletion, and removes the folder on normal exit.
