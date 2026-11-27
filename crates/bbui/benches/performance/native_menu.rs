use bbui::native_menu::{NativeMenuBar, NativeMenuCommand, NativeMenuEntry, NativeMenuGroup};
use bbui::native_menu_commands::resolve_native_menu;
use bbui::widgets::{Command, CommandRegistry};
use criterion::{BenchmarkId, Criterion};
use std::hint::black_box;

pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("native_menu/resolve_registry");
    for count in [16, 64, 256] {
        let mut registry = CommandRegistry::default();
        let mut entries = Vec::with_capacity(count);
        for index in 0..count {
            let id = format!("command.{index}");
            assert!(
                registry
                    .register(Command::new(&id, format!("Command {index}")))
                    .is_ok()
            );
            entries.push(NativeMenuEntry::Command(NativeMenuCommand::new(id, "")));
        }
        let template = NativeMenuBar {
            menus: vec![NativeMenuGroup::new("actions", "Actions", entries)],
            ..Default::default()
        };
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                black_box(resolve_native_menu(
                    black_box(&template),
                    black_box(&registry),
                ))
            })
        });
    }
    group.finish();
}
