use bbui_text::expression::{evaluate_exact_i64, evaluate_exact_u64};
use bbui_widgets::numeric::{NumericField, NumericValue};
use criterion::Criterion;
use std::hint::black_box;

pub fn bench(c: &mut Criterion) {
    c.bench_function("numeric/exact_i64_expression", |b| {
        b.iter(|| {
            black_box(evaluate_exact_i64(
                black_box("(x - 1) * 2"),
                black_box(i64::MAX / 2 - 2),
            ))
        })
    });
    c.bench_function("numeric/exact_u64_expression", |b| {
        b.iter(|| black_box(evaluate_exact_u64(black_box("x - 1"), black_box(u64::MAX))))
    });
    let mut field = NumericField::unsigned(u64::MAX - 2);
    c.bench_function("numeric/exact_u64_step", |b| {
        b.iter(|| {
            let _ = field.set_typed_value(black_box(NumericValue::Unsigned(u64::MAX - 1)));
            black_box(field.typed_value())
        })
    });
}
