# Architecture

BUI is a Bevy plugin composed of Rust crates. `bui` is the application-facing entry point. `BuiPlugin` installs ordered systems for style, layout, input, widgets, and rendering. A UI declaration such as `bui! { column { checkbox("Enabled", true) } }` creates a `WidgetTree`; `WidgetTree::spawn` creates Bevy entities under a `UiLayoutRoot`.

## Data flow

1. `bui_core` stores hierarchy, visibility, enabled state, focus, pointer capture, and routed input. `bui` translates Bevy window, keyboard, pointer, IME, and clipboard messages into that input queue.
2. `bui_style` resolves typed theme and state rules, advances per-property transitions, and snaps them when reduced motion is active. `bui_text::UiTextLayout` shapes editable text with Bevy's text pipeline before layout measurement and again at the final width. `bui_layout` writes `UiLayoutResult`; the same rectangles feed hit testing and painting. Window roots update their viewport and DPI from Bevy windows.
3. `bui_widgets` applies routed events to component values, emits `WidgetEvent`s, and builds draw items. Editable text paint carries its shaping width, wrap rule, and text style into `bui_render`. `bui_text` also handles text editing and numeric expressions.
4. `bui_render` sends shapes, text, icons, images, shadows, and curved links to Bevy UI nodes, which Bevy renders on the GPU. `bui_media` loads images and video frames as Bevy assets and advances visible players. `UiWorldSurface` maps a 3D plane back to UI coordinates for pointer routing.
5. `bui` gives controls AccessKit roles, labels, values, disabled/hidden states, and bounds. Screen-reader actions enter the same widget action queue as keyboard and pointer operations.

Long H.264 sources decode through a cancellable player worker and keep at most eight RGBA frames in the playback cache. Seeking replaces the working set; hidden players release decoded frames and stop advancing their clock until visible again. The eager decode utility has a separate explicit frame limit for callers that require the full sequence.

Attach `UiLiveRegion::polite()` or `UiLiveRegion::assertive()` to a changing label or status control to request an AccessKit live announcement; `.atomic()` announces the entire updated value. The example `cargo run -p bui --example accessibility_live` updates a status label at runtime.
Numeric expression errors expose the typed reason as an assertive live description and clear it when the edit is corrected or cancelled.
Number fields and sliders expose both their numeric value and the formatted value with any configured unit.

Menu triggers and submenu items expose their popup and expanded state to AccessKit. A `Tooltip` on a control supplies tooltip text to its accessibility node; standalone tooltips become visible to assistive technology only while shown.
Radio and dropdown choices are stable child entities with individual names, position, selection, enabled state, and bounds. AccessKit choice actions route to the owning widget, so pointer, keyboard, and screen-reader edits use the same value event and undo transaction.
Node sockets are named AccessKit buttons: activate an output socket and then an input socket (in either order) to request a validated graph link. Activating the same socket twice cancels the pending pair; the resulting link edit enters shared history.

`UiFontAsset` requests fonts through Bevy's `AssetServer` while its owner remains visible. The nearest ready font on the UI ancestor chain supplies a `TextFont` handle to editable text shaping and drawing; missing files retain the default-font fallback until an explicit retry or path change. `UiFontAssetState` mirrors loading and failure status for UI presentation.

## Render batching and asset ownership

Bevy's UI renderer owns the glyph atlas set (`FontAtlasSet`) used by its text pipeline; BUI submits Bevy `Text` nodes and font handles and does not create or retain per-widget glyph atlases. Raster images and icons use strong Bevy `Handle<Image>` values in `ImageRegistry`, and generated `ImageNode`s retain cloned handles while they are rendered. Hiding or removing an asset-backed widget releases the BUI cache/registry handle and its generated node; `Assets<Image>` keeps the image alive while any other strong handles remain. BUI currently passes direct image handles and does not pack repeated icons into its own texture atlas. Hosts can use Bevy's `ImageNode` atlas support in their own UI, but `Primitive::Image` and `Primitive::Icon` do not expose atlas regions.

Adjacent Bezier draw items with the same clip and target camera share one shader-backed UI node. Their 32 sampled line segments are uploaded to a Bevy `ShaderBuffer`; a tile index limits each fragment to nearby segments, and the shader composites curves in stable draw order. A non-Bezier item or clip/target change ends the batch, preserving interleaved z order and clipping. Removing the run releases its material and three storage-buffer assets; unchanged geometry keeps those resources while other items update.

Reorderable `PanelGroup` components use stable panel keys to reorder direct children in `UiChildren`. A drag previews the order and commits one `PanelOrderCommitted` event; Ctrl+Arrow uses the same move operation. `PanelOrderStore` persists order by group key across tree rebuilds, while history validates the live key set before consuming an undo or redo step.
Dropdowns expose listbox expansion, panels expose their expansion state, and progress indicators expose a name and numeric value.

## Ownership and frame order

`UiNode` marks each logical control. `UiParent`/`UiChildren` own hierarchy and paint order; `UiKey` supplies identity across tree rebuilds and history restoration. `UiFocus` and routed pointer capture are per window. Widgets own their values and editor selections, while `UiLayoutResult` owns the resolved rectangle, clip, stacking depth, window, and DPI for layout, hit testing, accessibility, and paint. Popup and virtual row entities belong to their editor and are removed when they leave the visible viewport.

`WidgetNode` declarations expose row, column, overlay, fixed and responsive grid, flex, inline, boxed, property-row, and scroll containers. Per-node layout overrides feed `UiLayoutNode`; `box`, `property-row`, and `property-label` roles obtain spacing from the active theme. The same resolved rectangles drive paint clips and hit boxes after resize, scroll, or DPI changes.
Applications can pass a typed `UiInlineStyle` to `WidgetNode::with_style` and use `UiRole`/`UiClasses` with `UiStyleSheet` for shared rules. The `bui::prelude` exports these style types, computed values, transitions, and reduced-motion settings.

`UiSystems` runs Style → TextMeasure → Layout → Input → LayoutAfterInput → StyleAfterInput → LayoutAfterStyle → Text → Widgets → Render. Widget input changes values before virtual controls and accessible nodes synchronize; history records the resulting edits after painting. The render handoff is `DisplayList` draw items, clips, and asset IDs. The Bevy render world receives Bevy UI entities and image assets from `bui_render`; it does not own widget state or perform hit testing.

Captured `MouseMotion` arrives in physical pixels. The screen and world-surface bridges divide each delta by the physical window's current DPI scale before advancing the logical virtual cursor. Capture stores the original OS cursor position and restores it on commit or cancellation. Window blur, removal, or disabling the target cancels routed capture and restores cursor visibility and grab mode.

Widget history groups related value changes into one transaction. Exclusive panel expansion records all affected panels together and refreshes the persisted expansion store on restore. FileBrowser history records its navigation, selection, bookmark, filter, sort, and view state without copying directory entries; undo validates and re-reads the destination directory before consuming the transaction, and reports a restore error if the path has disappeared. Filesystem mutations are emitted as explicit external side effects.

Text buffers keep a local undo stack while focused. On blur or single-line Enter, `WidgetEvent::TextCommitted` records the whole edit session in shared history, including the before/after text and directed selection. A global restore resets the local stack, updates the mirrored control, and restores focus and selection. Cancelled IME preedit does not create a transaction. The `text_history` example exercises this handoff.

## Application usage

Install `DefaultPlugins` and `BuiPlugin`, spawn one `UiLayoutRoot` for each UI viewport, and spawn a `WidgetTree` below that root. Read `WidgetEventQueue` to apply user edits to application data. Switch `Theme` between `ThemeKind::Dark` and `ThemeKind::Light` at runtime. Run `cargo run -p bui --example showcase` for controls and theme transitions (F6 switches theme; F7 toggles reduced motion), `cargo run -p bui --example file_browser` for filesystem navigation, `cargo run -p bui --example color_ramp` for gradient editing, `cargo run -p bui_render --example world_surface` for an offscreen 3D surface, or `cargo run -p bui_media --example media_playback` for Bevy media asset playback.

The [feature inventory](feature-inventory.md) is the completion contract. Components and examples in this architecture are a working foundation; unchecked inventory items still need full interaction, tests, and examples.

On Linux, `cargo test -p bui --test gpu_readback --locked` renders clipped UI shapes into an offscreen Bevy image and checks Dark/Light colors at DPI scales 1 and 2 through WGPU readback. It works with Mesa's software Vulkan adapter, so no desktop display is required.
