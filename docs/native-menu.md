# Native macOS menus

`bbui::native_menu` presents a host-defined menu tree in the macOS application menu bar. The host constructs a `NativeMenuBar`, calls `sync` on the main thread after its window exists, and drains `take_commands` on its update pass. The bridge queues IDs; the host decides what each command does. In a Bevy system, request a `NonSend` resource to keep that system on the main thread; the runnable example demonstrates this.

For commands also shown in a BBUI command palette, use `bbui::native_menu_commands::sync_native_menu_from_registry`. The host's menu template determines placement, separators, submenu names, visibility, and checkmarks. `CommandRegistry` supplies each command's label, enabled state, and shortcut by ID. Letter, digit, punctuation, navigation, and F1–F35 shortcuts have AppKit equivalents; unknown keys return explicit errors. Missing command IDs also return explicit errors. The host still handles command activation and can call `record_activation` before dispatching an ID. This keeps native menus and palettes in sync without forcing an application menu structure on every host.

The model supports application-menu items, top-level and nested menus, separators, enabled and hidden items, checked or mixed state, and AppKit key equivalents. It can be rebuilt when the menu structure changes. Title, enabled, visibility, state, and shortcut changes update existing AppKit items in place, preserving an open menu. Native menu validation respects disabled host commands even when AppKit automatically validates the application menu. When `quit_command` is set, the existing native Quit item routes through the host so unsaved-work handling can cancel quitting. System-owned About, Services, and Hide items remain in the application menu.

```rust
use bbui::native_menu::{NativeMenuBar, NativeMenuCommand, NativeMenuEntry, NativeMenuGroup};

let menu = NativeMenuBar {
    menus: vec![NativeMenuGroup::new(
        "file",
        "File",
        vec![NativeMenuEntry::Command(NativeMenuCommand::new("file.open", "Open…"))],
    )],
    quit_command: Some("app.quit".into()),
    ..Default::default()
};
let _installed = bbui::native_menu::sync(&menu)?;
for command_id in bbui::native_menu::take_commands() {
    // Dispatch through the host's command registry.
}
# Ok::<(), bbui::native_menu::NativeMenuError>(())
```

Run `cargo run -p bbui --example native_menu` on macOS. File > New enables Save. Open Recent is nested, View toggles a checkmark, and Quit goes through a host guard. The example window shows the current document and grid states. On other platforms the same model validates but `sync` reports that no native menu was installed.
