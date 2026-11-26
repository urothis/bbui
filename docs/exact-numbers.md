# Exact integer controls

Use `NumericField::integer(i64)` or `NumericField::unsigned(u64)` for full-width values. The declarative equivalents are `WidgetNode::exact_i64(label, value)` and `WidgetNode::exact_u64(label, value)`. Read `typed_value()` and write with `set_typed_value(NumericValue::Integer(...))` or `NumericValue::Unsigned(...)`. The field's `value: f64` remains a projection for older rendering and layout code; it can round values above `2^53` and must not be used as the source of truth for integer data.

Typed editing accepts checked integer arithmetic with parentheses, unary signs, `+`, `-`, `*`, `/`, and `x` for the current value. Invalid input stays in the edit buffer with an error. Keyboard stepping, scrubbing, bounded slider movement, clipboard text, action events, undo/redo, and accessibility values use the typed integer. Bounds and defaults can be set through the typed field methods. A float-only API rejects integer arguments beyond the exact `f64` range so accidental conversion cannot silently alter a full-width value.

The legacy `NumericField::new` constructor accepts integer values and bounds only within the exact `f64` range. Unsigned fields reject negative input, and integer steps must be positive, whole, and exactly representable. Use the typed constructors and `set_typed_bounds` for values outside that range.

The [dark](visuals/examples/exact-numbers-dark.png) and [light](visuals/examples/exact-numbers-light.png) captures come from `cargo run -p bbui --example exact_numbers`. Run `cargo bench -p bbui --bench performance --locked -- numeric/` for the parser and step measurements.
