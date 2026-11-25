//! Palette ranking and shortcut lookup with a populated registry.

use bbui_core::{UiKeyCode, UiModifiers};
use bbui_widgets::{Command, CommandRegistry, CommandShortcut};
use criterion::{BenchmarkId, Criterion};
use std::hint::black_box;

pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("commands");
    for count in [32, 256, 1024] {
        let mut registry = CommandRegistry::default();
        for index in 0..count {
            let mut command = Command::new(
                format!("command-{index}"),
                format!("Export command {index}"),
            );
            command.category = "File".into();
            if index == 0 {
                command = command.with_shortcut(
                    CommandShortcut::new(
                        UiKeyCode::N,
                        UiModifiers {
                            control: true,
                            ..Default::default()
                        },
                    ),
                    "Ctrl+N",
                );
            }
            assert!(registry.register(command).is_ok());
        }
        assert!(!registry.ranked("export 15").is_empty());
        group.bench_with_input(BenchmarkId::new("search", count), &count, |b, _| {
            b.iter(|| black_box(registry.ranked(black_box("export 15"))))
        });
        group.bench_with_input(BenchmarkId::new("shortcut", count), &count, |b, _| {
            b.iter(|| {
                black_box(registry.resolve_shortcut(
                    UiKeyCode::N,
                    UiModifiers {
                        control: true,
                        ..Default::default()
                    },
                ))
            })
        });
    }
    group.finish();
}
