# Keyframe motion

`bbui_motion` plays typed `MotionClip`s made from `KeyframeTrack`s. A track binds a style property or custom ECS value; each keyframe sets a time, value, and curve for the next segment. Active clips are sampled while running, and changed values mark their affected UI domains. The [curve comparison](visuals/animations/keyframe_curves.webp) and [panel collapse](visuals/animations/properties_transform.webp) are captures of the running examples.

```rust
use std::time::Duration;
use bbui::prelude::*;

let binding = style_motion_binding(UiStyleProperty::Background);
let track = KeyframeTrack::new([
    Keyframe::at(Duration::ZERO, UiStyleValue::Color(start))
        .curve(MotionCurve::EaseOut),
    Keyframe::at(Duration::from_millis(200), UiStyleValue::Color(accent)),
])?;
let clip = MotionClip::new().track(binding, track);
let handle = world.resource_mut::<MotionPlayer>()
    .play(entity, clip, MotionPriority::Explicit);
```

Use the returned handle to pause, resume, reverse, seek, change speed or delay, repeat, or cancel. `MotionEvents` reports completion and cancellation. `MotionCurve` offers linear, easing presets, steps, and validated cubic Bézier curves. Register a `MotionBinding` with read/write closures and dirty domains for a custom ECS value; implement `MotionValue` for its interpolation.

`UiAutoAnimations` creates transitions for hover, state, and theme changes from theme motion tokens. Explicit clips own conflicting properties until they finish. `UiReducedMotion(true)` or zero theme motion scale snaps active UI clips. Panels animate `UiContentReveal` as well as paint, so following controls move with the collapsing panel.
