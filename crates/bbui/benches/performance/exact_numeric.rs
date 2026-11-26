use bbui_text::expression::{evaluate_exact_i64, evaluate_exact_u64, evaluate_exact_u64_with_vars};
use bbui_widgets::numeric::NumericField;
use criterion::Criterion;
use std::collections::HashMap;
use std::hint::black_box;

#[expect(
    clippy::expect_used,
    reason = "the fixed finite benchmark input must construct"
)]
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
    let variables = HashMap::from([("enabled".to_owned(), 1.0)]);
    c.bench_function("numeric/exact_u64_conditional", |b| {
        b.iter(|| {
            black_box(evaluate_exact_u64_with_vars(
                black_box("x - 1 if enabled else x"),
                black_box(u64::MAX),
                black_box(&variables),
            ))
        })
    });
    let mut exact = NumericField::unsigned(u64::MAX - 2);
    c.bench_function("numeric/exact_u64_step_pair", |b| {
        b.iter(|| {
            black_box(exact.step_by(black_box(1)));
            black_box(exact.step_by(black_box(-1)));
            black_box(exact.typed_value())
        })
    });
    let mut float = NumericField::float(1_000_000.0).expect("finite benchmark value");
    c.bench_function("numeric/float_step_pair", |b| {
        b.iter(|| {
            black_box(float.step_by(black_box(1)));
            black_box(float.step_by(black_box(-1)));
            black_box(float.value)
        })
    });
}
