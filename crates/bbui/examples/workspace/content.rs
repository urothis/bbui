//! Representative editor panes shared by the interactive example and GPU captures.

use bbui_layout::{UiEdges, UiLength};
use bbui_style::{Rgba, Theme, ThemeKind};
use bbui_widgets::{WidgetNode, WidgetTree};
use bevy::prelude::{Entity, World};

pub fn configure_theme(theme: &mut Theme) {
    theme.font_size = 11.5;
    theme.corner_radius = 3.0;
    theme.spacing.control.padding_x = 6.0;
    theme.spacing.control.padding_y = 2.0;
    theme.spacing.panel.content_padding = 7.0;
    theme.spacing.property_row.label_width = 108.0;
    theme.spacing.property_row.gap = 5.0;
    if theme.kind == ThemeKind::Dark {
        theme.background = Rgba(48.0 / 255.0, 48.0 / 255.0, 48.0 / 255.0, 1.0);
        theme.surface = Rgba(61.0 / 255.0, 61.0 / 255.0, 61.0 / 255.0, 1.0);
        theme.surface_hover = Rgba(84.0 / 255.0, 84.0 / 255.0, 84.0 / 255.0, 1.0);
    }
}

fn field(label: &str, value: f64) -> WidgetNode {
    WidgetNode::property_row_with_decorator(
        label,
        WidgetNode::number("", value, -10_000.0, 10_000.0, 0.1)
            .with_role("centered-number")
            .with_height(UiLength::Px(20.0)),
    )
    .with_height(UiLength::Px(20.0))
}

fn panel(title: &str, children: impl IntoIterator<Item = WidgetNode>) -> WidgetNode {
    WidgetNode::panel(title, true, children)
        .with_width(UiLength::Fill(1.0))
        .with_gap(2.0)
}

pub fn pane(id: &str) -> WidgetNode {
    let body = match id {
        "document" => WidgetNode::column([
            WidgetNode::row([
                WidgetNode::dropdown("", ["Scene", "Animation"], Some(0))
                    .with_role("compact-dropdown")
                    .with_width(UiLength::Px(144.0))
                    .with_height(UiLength::Px(21.0)),
                WidgetNode::label("Cube · Object Mode").with_height(UiLength::Px(21.0)),
            ])
            .with_gap(8.0)
            .with_height(UiLength::Px(24.0)),
            panel(
                "Scene",
                [
                    WidgetNode::label("Collection").with_height(UiLength::Px(20.0)),
                    WidgetNode::label("  ▾  Camera").with_height(UiLength::Px(21.0)),
                    WidgetNode::label("  ▾  Cube").with_height(UiLength::Px(21.0)),
                    WidgetNode::label("  ▾  Light").with_height(UiLength::Px(21.0)),
                ],
            ),
            panel(
                "Timeline",
                [
                    WidgetNode::row([
                        WidgetNode::button("◀")
                            .with_width(UiLength::Px(28.0))
                            .with_height(UiLength::Px(20.0)),
                        WidgetNode::button("▶")
                            .with_width(UiLength::Px(28.0))
                            .with_height(UiLength::Px(20.0)),
                        WidgetNode::label("Frame  1 / 250").with_height(UiLength::Px(20.0)),
                    ])
                    .with_gap(3.0),
                    WidgetNode::slider_range("", 0.25, 0.0, 1.0, 0.01)
                        .with_role("filled-slider")
                        .with_height(UiLength::Px(20.0)),
                ],
            ),
            panel(
                "Render",
                [
                    WidgetNode::label("Output").with_height(UiLength::Px(17.0)),
                    field("Width", 1920.0),
                    field("Height", 1080.0),
                    WidgetNode::property_row_with_decorator(
                        "Samples",
                        WidgetNode::slider_range("", 64.0, 1.0, 128.0, 1.0)
                            .with_role("filled-slider")
                            .with_height(UiLength::Px(20.0)),
                    )
                    .with_height(UiLength::Px(20.0)),
                ],
            ),
        ]),
        "preview" => WidgetNode::column([
            panel(
                "Render Preview",
                [
                    WidgetNode::label("Camera  ·  1920 × 1080").with_height(UiLength::Px(20.0)),
                    WidgetNode::progress(0.72)
                        .with_width(UiLength::Fill(1.0))
                        .with_height(UiLength::Px(20.0)),
                ],
            ),
            panel("View", [field("Exposure", 1.0), field("Gamma", 1.0)]),
        ]),
        "inspector" => WidgetNode::column([
            WidgetNode::text_field("", "Cube")
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Px(21.0)),
            panel(
                "Transform",
                [
                    WidgetNode::label("Location").with_height(UiLength::Px(17.0)),
                    field("X", 0.0),
                    field("Y", 1.25),
                    field("Z", 0.0),
                ],
            ),
            panel(
                "Visibility",
                [WidgetNode::checkbox("Camera", true).with_height(UiLength::Px(20.0))],
            ),
        ]),
        _ => WidgetNode::column([
            WidgetNode::label("Output  ·  Scene ready").with_height(UiLength::Px(19.0)),
            WidgetNode::label("[10:42:18] Loaded scene").with_height(UiLength::Px(18.0)),
            WidgetNode::label("[10:42:19] Viewport updated").with_height(UiLength::Px(18.0)),
            WidgetNode::label("[10:42:19] Render engine ready").with_height(UiLength::Px(18.0)),
            WidgetNode::label("[10:42:20] Autosave complete").with_height(UiLength::Px(18.0)),
            WidgetNode::text_field("Command", "")
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Px(21.0)),
        ]),
    };
    body.with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0))
        .with_gap(5.0)
        .with_padding(UiEdges::all(8.0))
}

pub fn mount(world: &mut World, parent: Entity, id: &str) {
    let _ = WidgetTree::new(pane(id)).spawn(world, Some(parent));
}
