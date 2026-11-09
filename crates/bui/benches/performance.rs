//! CPU performance measurements across the BUI workspace.
//! Run with `cargo bench -p bui --bench performance --locked`.

use criterion::{Criterion, criterion_group, criterion_main};
use std::time::Duration;

#[path = "performance/mod.rs"]
mod suite;

fn workspace_benchmarks(c: &mut Criterion) {
    suite::layout::bench(c);
    suite::integration::bench(c);
    suite::render::bench(c);
    suite::widgets::bench(c);
    suite::text_media::bench(c);
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(30)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    targets = workspace_benchmarks
}
criterion_main!(benches);
