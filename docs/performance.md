# Performance and benchmarks

BBUI uses a central dirty journal to avoid rebuilding settled UI. The Criterion suite measures cold setup, idle updates, a single changed entity, layout propagation, renderer synchronization, text/media work, and input at several scene sizes. It also records relevant allocations. The suite measures CPU behavior; GPU readback tests validate rendered pixels, and the render stress example exercises GPU synchronization.

```sh
cargo bench -p bbui --bench performance --locked
cargo run -p bbui_render --example render_sync_bench --release --locked
cargo run -p bbui_widgets --example canvas_stress --release --locked
```

Criterion writes its HTML report to `target/criterion/report/index.html`. Compare the same benchmark on the same machine and toolchain; inspect confidence intervals and allocation counts, not a single elapsed-time sample. The most important regression checks are idle and `change_one`/`one_changed`: an unrelated control should not cause whole-tree work. The [invalidation contract](invalidation.md) explains the expected update scope.

Functional performance gates are the workspace tests and GPU readbacks:

```sh
cargo test -p bbui --test gpu_readback --locked
cargo test -p bbui --test node_graph_gpu_readback --locked
cargo test --workspace --all-targets --locked
```

See [visual captures](visuals/README.md) when a rendering optimization changes layout, clipping, text, or animation timing.
