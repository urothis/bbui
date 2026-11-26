# Native macOS menus

`bbui::native_menu` presents a host-defined menu tree in the macOS application menu bar. BBUI does not prescribe menu names, command IDs, labels, shortcuts, or enabled states. The host constructs a `NativeMenuBar`, calls `sync` on the main thread after its window exists, and drains `take_commands` on its update pass. The bridge queues IDs; the host decides what each command does.

The model supports application-menu items, top-level and nested menus, separators, enabled and hidden items, checked or mixed state, and AppKit key equivalents. It can be rebuilt when the menu structure changes. Title, enabled, visibility, state, and shortcut changes update existing AppKit items in place, preserving an open menu. When `quit_command` is set, the existing native Quit item routes through the host so unsaved-work handling can cancel quitting. System-owned About, Services, and Hide items remain in the application menu.

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

Run `cargo run -p bbui --example native_menu` on macOS. Its File menu enables a save flow, View toggles a checkmark, Recent is nested, and Quit goes through a host guard. On other platforms the same model validates but `sync` reports that no native menu was installed.
