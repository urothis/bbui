# Accessibility

BUI publishes AccessKit nodes from the same entities used for input, layout, and painting. Bounds follow the resolved layout and window DPI. Hidden or disabled controls expose no actions. `UiFocusable`, `UiTabIndex`, and visible tree order determine Tab traversal; AccessKit Focus and Blur update `UiFocus` for the control's window.

| UI family | Accessible role and operation |
| --- | --- |
| Buttons, checkboxes, panels | Named buttons/check boxes/groups; activate or expand/collapse. Reorderable panels also expose Increment and Decrement. |
| Numbers, sliders, color, ramps, curves | Current value and range or selected point/stop; Increment, Decrement, and SetValue use widget actions. Invalid numeric expressions announce their reason assertively. |
| Text fields and areas | Text input or multiline text input; ReplaceSelectedText uses the same editing path as keyboard input. Read-only fields omit edit actions. |
| Menus, radio groups, dropdowns | Popup state and named choices. Each visible choice reports selection and position; activation emits the normal command or value event. |
| Lists, trees, grids, tables, browser | Virtual rows expose names, selection, position, and available click/expand actions. Large models keep only visible child entities. |
| Canvas and node editor | Objects, nodes, and links expose named virtual controls. Activate an output and then an input socket, in either order, to create a validated, undoable link; activate the same socket twice to cancel. The first socket announces its selected state. |

Focused control value changes update the AccessKit node. Attach `UiLiveRegion::polite()` or `UiLiveRegion::assertive()` to application status labels for change announcements; `.atomic()` announces the entire label. Icon load and numeric expression errors use assertive live descriptions, and the file-browser example marks filesystem status assertive. Run `cargo run -p bui --example accessibility_live` to inspect a changing status label, or `cargo test -p bui --lib accessibility::tests --locked` for semantics and routed action coverage.
