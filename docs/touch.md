# Touch gestures

BBUI routes the first touch through its normal pointer path. Controls keep their existing tap, press, and drag behavior. When a second touch begins inside the same gesture surface, BBUI cancels the pending pointer press and starts a two-finger gesture on that surface. Lifting either finger ends the gesture.

`UiScroll`, `Canvas`, and `NodeGraph` opt in automatically. A scroll surface pans in both axes; Canvas and NodeGraph pan around the moving center and scale around the pinch center. Nested scroll surfaces use the deepest common gesture surface for both fingers. A custom control can opt in by inserting `UiGestureSurface` on the entity that should receive `UiRoutedEvent::TouchGesture`.

The routed event includes `Started`, `Updated`, `Ended`, and `Cancelled` phases, logical center coordinates, logical delta, and a scale ratio. A custom surface can use any subset of those values. Gestures are scoped to the window and cancel when the window loses focus. If a surface becomes unavailable during a gesture, the next touch event cancels it.

Run `cargo run -p bbui --example touch_gestures` on a touch device. The [before](visuals/examples/touch-gestures-start-dark.png), [after dark](visuals/examples/touch-gestures-dark.png), and [after light](visuals/examples/touch-gestures-light.png) captures come from the runnable example; the capture test injects two touches and verifies the canvas zoom changes.
