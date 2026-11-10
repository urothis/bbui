use super::*;

struct PaintEntityContext<'a> {
    world: &'a mut World,
    entity: Entity,
    layout: UiLayoutResult,
    rect: Rect,
    clip: ClipRect,
    z: i32,
    theme: &'a Theme,
    base_theme: &'a Theme,
    enabled: bool,
    is_box: bool,
    output: &'a mut Vec<DrawItem>,
}

mod entity_editors;
use entity_editors::*;

mod entity_menus;
use entity_menus::*;

mod entity_text;
use entity_text::*;

mod entity_misc;
use entity_misc::*;

mod dirty;
pub use dirty::collect_widget_paint;
pub(super) use dirty::{collect_widget_paint_changes, styled_theme};

mod order;
pub(super) use order::*;

mod full;
pub(super) use full::*;

mod retained;
use retained::*;

mod overlays;
pub(super) use overlays::*;

mod primitives;
pub(super) use primitives::*;

mod table;
pub(super) use table::*;

mod collection;
pub(super) use collection::*;

mod slider;
pub(super) use slider::*;

mod radio;
pub(super) use radio::*;

mod color_ramp;
pub(super) use color_ramp::*;

mod hsv;
pub(super) use hsv::*;

mod curve;
pub(super) use curve::*;

mod node_graph;
pub(super) use node_graph::*;

mod text;
pub(super) use text::*;
