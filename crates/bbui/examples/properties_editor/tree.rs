//! Full editor widget tree, shared by the desktop and world displays.

use super::*;

pub(super) fn build_tree(extra_panel: Option<EditorExtraPanel>) -> WidgetTree {
    let menu = EditorChrome::menu_bar([
        EditorChrome::flat_menu(
            "File",
            [
                MenuEntry::command("new", "New Scene"),
                MenuEntry::command("open", "Open…"),
                MenuEntry::command("save", "Save Scene"),
            ],
        )
        .with_width(UiLength::Px(43.0))
        .with_height(UiLength::Px(20.0)),
        EditorChrome::flat_menu(
            "Edit",
            [
                MenuEntry::command("undo", "Undo"),
                MenuEntry::command("redo", "Redo"),
            ],
        )
        .with_width(UiLength::Px(43.0))
        .with_height(UiLength::Px(20.0)),
        EditorChrome::flat_menu(
            "Render",
            [
                MenuEntry::command("render", "Render Image"),
                MenuEntry::command("animation", "Render Animation"),
            ],
        )
        .with_width(UiLength::Px(60.0))
        .with_height(UiLength::Px(20.0)),
        EditorChrome::flat_menu("Help", [MenuEntry::command("about", "About BBUI")])
            .with_width(UiLength::Px(45.0))
            .with_height(UiLength::Px(20.0)),
        WidgetNode::label("")
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Px(20.0)),
        WidgetNode::icon_button("", "Advance render preview")
            .with_key("header-play")
            .with_role("header-play-icon")
            .with_width(UiLength::Px(25.0))
            .with_height(UiLength::Px(20.0)),
        EditorChrome::flat_menu(
            "⌕",
            [
                MenuEntry::command("find-transform", "Transform"),
                MenuEntry::command("find-visibility", "Visibility"),
                MenuEntry::command("find-display", "Viewport Display"),
            ],
        )
        .with_width(UiLength::Px(25.0))
        .with_height(UiLength::Px(20.0)),
    ])
    .with_gap(0.0)
    .with_padding(UiEdges::all(4.0))
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Px(28.0));

    let transform = EditorChrome::primary_panel(
        "Transform",
        true,
        [
            WidgetNode::label("").with_height(UiLength::Px(2.0)),
            WidgetNode::label("Location").with_height(UiLength::Px(17.0)),
            WidgetNode::label("").with_height(UiLength::Px(3.0)),
            axis("X", 0.0, "m"),
            axis("Y", 1.25, "m"),
            axis("Z", 0.0, "m"),
            WidgetNode::label("Rotation").with_height(UiLength::Px(17.0)),
            WidgetNode::label("").with_height(UiLength::Px(3.0)),
            axis("X", 0.0, "°"),
            axis("Y", 0.0, "°"),
            axis("Z", 90.0, "°"),
            WidgetNode::property_row_with_decorator(
                "Scale",
                WidgetNode::slider_range("", 1.0, 0.0, 4.0, 0.01)
                    .with_role("filled-slider")
                    .with_height(UiLength::Px(20.0)),
            )
            .with_height(UiLength::Px(20.0)),
            WidgetNode::property_row_with_decorator(
                "Mode",
                WidgetNode::dropdown("", ["Object Mode", "Edit Mode", "Sculpt Mode"], Some(0))
                    .with_role("compact-dropdown")
                    .with_height(UiLength::Px(20.0)),
            )
            .with_height(UiLength::Px(20.0)),
        ],
    )
    .with_key("transform")
    .with_gap(1.0)
    .with_width(UiLength::Fill(1.0));

    let visibility = WidgetNode::panel(
        "Visibility",
        true,
        [
            WidgetNode::column([
                WidgetNode::label("").with_height(UiLength::Px(5.0)),
                WidgetNode::row([
                    WidgetNode::checkbox("▸", true)
                        .with_role("segmented-toggle")
                        .with_width(UiLength::Fill(1.0)),
                    WidgetNode::checkbox("◉", true)
                        .with_role("segmented-toggle")
                        .with_width(UiLength::Fill(1.0)),
                    WidgetNode::checkbox("▣", true)
                        .with_role("segmented-toggle")
                        .with_width(UiLength::Fill(1.0)),
                ])
                .with_role("visibility-toggle-strip")
                .with_gap(1.0)
                .with_height(UiLength::Px(20.0)),
                WidgetNode::row([check("Camera", true), check("Diffuse", true)])
                    .with_gap(4.0)
                    .with_height(UiLength::Px(20.0)),
                WidgetNode::row([check("Glossy", true), check("Transmission", true)])
                    .with_gap(4.0)
                    .with_height(UiLength::Px(20.0)),
                WidgetNode::row([check("Volume Scatter", false), check("Shadow", true)])
                    .with_gap(4.0)
                    .with_height(UiLength::Px(20.0)),
            ])
            .with_gap(2.0)
            .with_padding(UiEdges::horizontal_vertical(8.0, 0.0))
            .with_width(UiLength::Fill(1.0)),
            EditorChrome::sub_panel(
                "Mask",
                true,
                [check("Shadow Catcher", false), check("Holdout", false)],
            )
            .with_padding(UiEdges {
                left: 8.0,
                top: 28.0,
                right: 8.0,
                bottom: 8.0,
            })
            .with_gap(0.0)
            .with_width(UiLength::Fill(1.0)),
            EditorChrome::sub_panel(
                "Motion Blur",
                true,
                [WidgetNode::property_row_with_decorator(
                    "Steps",
                    WidgetNode::number("", 1.0, 1.0, 64.0, 1.0)
                        .with_role("centered-number")
                        .with_height(UiLength::Px(20.0)),
                )
                .with_height(UiLength::Px(20.0))],
            )
            .with_padding(UiEdges {
                left: 8.0,
                top: 28.0,
                right: 8.0,
                bottom: 8.0,
            })
            .with_gap(0.0)
            .with_width(UiLength::Fill(1.0)),
            WidgetNode::label("").with_height(UiLength::Px(5.0)),
        ],
    )
    .with_key("visibility")
    .with_gap(2.0)
    .with_padding(UiEdges {
        left: 0.0,
        top: 28.0,
        right: 0.0,
        bottom: 8.0,
    })
    .with_width(UiLength::Fill(1.0));

    let mut inspector_items = vec![
        WidgetNode::row([
            WidgetNode::dropdown("", ["Cube", "Camera", "Light"], Some(0))
                .with_key("object-browse")
                .with_role("icon-dropdown")
                .with_width(UiLength::Px(34.0)),
            WidgetNode::text_field("", "Cube")
                .with_key("object-name")
                .with_width(UiLength::Fill(1.0)),
            WidgetNode::icon_button("◇", "Keep object data")
                .with_key("object-fake")
                .with_width(UiLength::Px(20.0)),
            WidgetNode::icon_button("▣", "Duplicate object data")
                .with_key("object-duplicate")
                .with_width(UiLength::Px(20.0)),
            WidgetNode::icon_button("×", "Unlink object data")
                .with_key("object-unlink")
                .with_width(UiLength::Px(20.0)),
        ])
        .with_gap(1.0)
        .with_height(UiLength::Px(21.0)),
        transform,
        visibility,
        WidgetNode::panel(
            "Viewport Display",
            false,
            [check("Show Name", false), check("Show Axis", true)],
        )
        .with_key("viewport")
        .with_width(UiLength::Fill(1.0)),
    ];
    if let Some(panel) = extra_panel {
        inspector_items.push(panel.node.with_key(panel.key));
    }
    let inspector = WidgetNode::column(inspector_items)
        .with_key(PAGE_KEYS[0])
        .with_gap(1.0)
        .with_width(UiLength::Fill(1.0));

    let modifiers = modifier_page();
    let material = material_page();
    let outliner = outliner_page();
    let preferences = preferences_page();
    let nodes = nodes_page();

    let content = WidgetNode::row([
        EditorChrome::tab_rail([
            toolbar_button(0, "▣"),
            toolbar_asset_button(1, "icons/modifiers.svg"),
            toolbar_button(2, "◕"),
            toolbar_button(3, "☷"),
            toolbar_asset_button(4, "icons/settings.svg"),
            toolbar_button(5, "✣"),
        ])
        .with_gap(1.0)
        .with_width(UiLength::Px(29.0)),
        WidgetNode::scroll([inspector, modifiers, material, outliner, preferences, nodes])
            .with_padding(UiEdges {
                left: 0.0,
                top: 10.0,
                right: 8.0,
                bottom: 0.0,
            })
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0)),
    ])
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Fill(1.0));

    WidgetTree::new(
        WidgetNode::column([
            menu,
            content,
            WidgetNode::column([
                WidgetNode::overlay([
                    EditorChrome::progress(0.25)
                        .with_key("render-progress")
                        .with_width(UiLength::Fill(1.0))
                        .with_height(UiLength::Px(18.0)),
                    WidgetNode::label("Rendering 25%")
                        .with_key("render-progress-label")
                        .with_role("centered-label")
                        .with_width(UiLength::Fill(1.0))
                        .with_height(UiLength::Px(18.0)),
                ])
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Px(18.0)),
                WidgetNode::row([
                    WidgetNode::label("i").with_width(UiLength::Px(16.0)),
                    WidgetNode::label("Drag numeric values · click tabs to switch pages")
                        .with_key("property-status")
                        .with_width(UiLength::Fill(1.0)),
                ])
                .with_gap(4.0)
                .with_height(UiLength::Px(20.0)),
            ])
            .with_gap(4.0)
            .with_padding(UiEdges::all(8.0))
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Px(72.0)),
        ])
        .with_gap(0.0)
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0)),
    )
}
