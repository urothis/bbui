//! Pixel coverage for real menu-overlay, HSV-editor, and curve-editor widget painters.
#![cfg(target_os = "linux")]

use std::{
    collections::HashMap,
    fs, thread,
    time::{Duration, Instant},
};

use bevy::{
    app::PluginGroup,
    asset::Assets,
    camera::RenderTarget,
    image::Image,
    prelude::{App, Camera2d, DefaultPlugins, Entity, ResMut, Resource, WindowPlugin},
    render::{
        RenderPlugin,
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{TextureFormat, TextureUsages},
        settings::{RenderCreation, WgpuSettings},
    },
    ui::IsDefaultUiCamera,
    window::ExitCondition,
    winit::WinitPlugin,
};
use bui::{
    BuiPlugin,
    widgets::{MenuEntry, WidgetNode, WidgetTree},
};
use bui_core::UiChildren;
use bui_layout::{Rect, UiLayoutNode, UiLayoutResult, UiLayoutRoot, UiLength};
use bui_render::{ClipRect, DisplayList, Primitive, UiPaint};
use bui_style::{Rgba, Theme, ThemeColorToken, ThemeKind};
use bui_widgets::{
    ColorRamp, CurveEditor, DataTable, Dropdown, DropdownOptionControl, DropdownPopupState,
    Interpolation, ListItem, ListModel, MenuControl, PreviewGrid, PreviewGridItem, TableColumn,
    TableRow, TreeModel, TreeNode,
    declarative::{
        CollectionItemControl, CollectionItemKind, DataTableCellControl, DataTableHeaderControl,
    },
};

const TARGET_WIDTH: u32 = 1200;
const TARGET_HEIGHT: u32 = 1200;

#[derive(Resource, Default)]
struct CapturedFrames(HashMap<usize, CapturedFrame>);

#[derive(Clone)]
struct CapturedFrame {
    bytes: Vec<u8>,
    completed_at: Instant,
}

fn pixel_at(pixels: &[u8], x: usize, y: usize) -> &[u8] {
    let bytes_per_row = (TARGET_WIDTH as usize * 4).div_ceil(256) * 256;
    let offset = y * bytes_per_row + x * 4;
    &pixels[offset..offset + 4]
}

fn write_capture_ppm(path: &std::path::Path, pixels: &[u8]) {
    let mut ppm = format!("P6\n{TARGET_WIDTH} {TARGET_HEIGHT}\n255\n").into_bytes();
    ppm.reserve(TARGET_WIDTH as usize * TARGET_HEIGHT as usize * 3);
    for y in 0..TARGET_HEIGHT as usize {
        for x in 0..TARGET_WIDTH as usize {
            ppm.extend_from_slice(&pixel_at(pixels, x, y)[..3]);
        }
    }
    fs::write(path, ppm).expect("write optional widget visual capture");
}

fn assert_color(pixel: &[u8], color: Rgba, what: &str) {
    let expected = [
        (color.0 * 255.0).round() as u8,
        (color.1 * 255.0).round() as u8,
        (color.2 * 255.0).round() as u8,
        (color.3 * 255.0).round() as u8,
    ];
    for (actual, expected) in pixel.iter().zip(expected) {
        assert!(
            actual.abs_diff(expected) <= 4,
            "{what} color differed: actual={pixel:?}, expected={expected:?}"
        );
    }
}

fn assert_near(pixel: &[u8], expected: [u8; 4], tolerance: u8, what: &str) {
    for (actual, expected) in pixel.iter().zip(expected) {
        assert!(
            actual.abs_diff(expected) <= tolerance,
            "{what} color differed: actual={pixel:?}, expected={expected:?} ±{tolerance}"
        );
    }
}

fn assert_color_near_position(
    pixels: &[u8],
    position: [f32; 2],
    color: Rgba,
    tolerance: u8,
    radius: usize,
    what: &str,
) {
    let expected = [
        (color.0 * 255.0).round() as u8,
        (color.1 * 255.0).round() as u8,
        (color.2 * 255.0).round() as u8,
        (color.3 * 255.0).round() as u8,
    ];
    let center_x = position[0].round() as usize;
    let center_y = position[1].round() as usize;
    let x_start = center_x.saturating_sub(radius);
    let x_end = (center_x + radius).min(TARGET_WIDTH as usize - 1);
    let y_start = center_y.saturating_sub(radius);
    let y_end = (center_y + radius).min(TARGET_HEIGHT as usize - 1);
    let found = (y_start..=y_end).any(|y| {
        (x_start..=x_end).any(|x| {
            pixel_at(pixels, x, y)
                .iter()
                .zip(expected)
                .all(|(actual, expected)| actual.abs_diff(expected) <= tolerance)
        })
    });
    assert!(
        found,
        "{what} was not visible near {position:?} with color {expected:?} ±{tolerance}"
    );
}

fn curve_plot(rect: Rect, theme: &Theme) -> Rect {
    let spacing = theme.spacing.editor;
    let left = spacing.gutter_width + spacing.padding_x;
    let vertical = theme.control_height + spacing.padding_y;
    Rect {
        x: rect.x + left,
        y: rect.y + vertical,
        width: (rect.width - left - spacing.padding_x).max(1.0),
        height: (rect.height - vertical - spacing.padding_y).max(1.0),
    }
}

fn ramp_track(rect: Rect) -> Rect {
    Rect {
        x: rect.x + 12.0,
        y: rect.y + 38.0,
        width: (rect.width - 24.0).max(1.0),
        height: 25.0,
    }
}

fn blend(foreground: Rgba, background: Rgba, amount: f32) -> Rgba {
    let amount = amount.clamp(0.0, 1.0);
    let keep = 1.0 - amount;
    Rgba(
        foreground.0 * keep + background.0 * amount,
        foreground.1 * keep + background.1 * amount,
        foreground.2 * keep + background.2 * amount,
        foreground.3 * keep + background.3 * amount,
    )
}

fn curve_screen_position(plot: Rect, position: [f32; 2]) -> [f32; 2] {
    [
        plot.x + position[0] * plot.width,
        plot.y + (1.0 - position[1]) * plot.height,
    ]
}

fn collection_item(
    world: &bevy::prelude::World,
    owner: Entity,
    id: u64,
) -> (Entity, Rect, CollectionItemControl) {
    let children = world.get::<UiChildren>(owner).unwrap();
    let entity = children
        .0
        .iter()
        .copied()
        .find(|entity| {
            world
                .get::<CollectionItemControl>(*entity)
                .is_some_and(|item| item.id == id)
        })
        .expect("collection item should be mounted in the widget tree");
    let control = world.get::<CollectionItemControl>(entity).unwrap().clone();
    let rect = world
        .get::<bui_layout::UiLayoutResult>(entity)
        .unwrap()
        .rect;
    (entity, rect, control)
}

fn table_header(world: &bevy::prelude::World, owner: Entity, column: usize) -> (Entity, Rect) {
    let children = world.get::<UiChildren>(owner).unwrap();
    let entity = children
        .0
        .iter()
        .copied()
        .find(|entity| {
            world
                .get::<DataTableHeaderControl>(*entity)
                .is_some_and(|header| header.column == column)
        })
        .expect("virtualized table header should be visible");
    let rect = world
        .get::<bui_layout::UiLayoutResult>(entity)
        .unwrap()
        .rect;
    (entity, rect)
}

fn table_cell(
    world: &bevy::prelude::World,
    owner: Entity,
    row_id: u64,
    column: usize,
) -> (Entity, Rect, DataTableCellControl) {
    let children = world.get::<UiChildren>(owner).unwrap();
    let entity = children
        .0
        .iter()
        .copied()
        .find(|entity| {
            world
                .get::<DataTableCellControl>(*entity)
                .is_some_and(|cell| cell.row_id == row_id && cell.column == column)
        })
        .expect("virtualized table cell should be visible");
    let rect = world
        .get::<bui_layout::UiLayoutResult>(entity)
        .unwrap()
        .rect;
    let cell = world.get::<DataTableCellControl>(entity).unwrap().clone();
    (entity, rect, cell)
}

fn dropdown_option(
    world: &bevy::prelude::World,
    owner: Entity,
    index: usize,
) -> (Entity, Rect, DropdownOptionControl) {
    let children = world.get::<UiChildren>(owner).unwrap();
    let entity = children
        .0
        .iter()
        .copied()
        .find(|entity| {
            world
                .get::<DropdownOptionControl>(*entity)
                .is_some_and(|option| option.index == index)
        })
        .expect("visible dropdown option should be mounted");
    let rect = world
        .get::<bui_layout::UiLayoutResult>(entity)
        .unwrap()
        .rect;
    let option = world.get::<DropdownOptionControl>(entity).unwrap().clone();
    (entity, rect, option)
}

fn assert_table_header_text_visible(pixels: &[u8], rect: Rect, theme: &Theme) -> usize {
    let background = blend(theme.surface, theme.background, 0.12);
    let background = [
        (background.0 * 255.0).round() as u8,
        (background.1 * 255.0).round() as u8,
        (background.2 * 255.0).round() as u8,
    ];
    let text = [
        (theme.text.0 * 255.0).round() as u8,
        (theme.text.1 * 255.0).round() as u8,
        (theme.text.2 * 255.0).round() as u8,
    ];
    let start_x = rect.x.ceil() as usize + 3;
    let end_x = (rect.x + rect.width - 24.0).floor().max(rect.x) as usize;
    let start_y = rect.y.ceil() as usize + 2;
    let end_y = (rect.y + rect.height - 2.0).floor() as usize;
    let glyph_pixels = (start_y..end_y)
        .flat_map(|y| (start_x..end_x).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let actual = pixel_at(pixels, *x, *y);
            let from_background = (0..3)
                .map(|channel| actual[channel].abs_diff(background[channel]))
                .map(u16::from)
                .sum::<u16>();
            let from_text = (0..3)
                .map(|channel| actual[channel].abs_diff(text[channel]))
                .map(u16::from)
                .sum::<u16>();
            from_background >= 36 && from_text < from_background
        })
        .count();
    assert!(
        glyph_pixels >= 5,
        "expected sorted table header glyph pixels, found {glyph_pixels}"
    );
    glyph_pixels
}

fn assert_menu_label_visible(pixels: &[u8], rect: Rect, theme: &Theme) -> usize {
    let surface = [
        (theme.surface.0 * 255.0).round() as u8,
        (theme.surface.1 * 255.0).round() as u8,
        (theme.surface.2 * 255.0).round() as u8,
    ];
    let text = [
        (theme.text.0 * 255.0).round() as u8,
        (theme.text.1 * 255.0).round() as u8,
        (theme.text.2 * 255.0).round() as u8,
    ];
    let start_x = rect.x.ceil() as usize + 6;
    let end_x = (rect.x + 100.0).floor() as usize;
    let start_y = rect.y.ceil() as usize + 2;
    let end_y = (rect.y + rect.height - 2.0).floor() as usize;
    let glyph_pixels = (start_y..end_y)
        .flat_map(|y| (start_x..end_x).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let actual = pixel_at(pixels, *x, *y);
            let from_surface = (0..3)
                .map(|channel| actual[channel].abs_diff(surface[channel]))
                .map(u16::from)
                .sum::<u16>();
            let from_text = (0..3)
                .map(|channel| actual[channel].abs_diff(text[channel]))
                .map(u16::from)
                .sum::<u16>();
            from_surface >= 45 && from_text < from_surface
        })
        .count();
    assert!(
        glyph_pixels >= 5,
        "expected visible menu label glyph pixels, found {glyph_pixels}"
    );
    glyph_pixels
}

fn assert_option_label_visible(
    pixels: &[u8],
    rect: Rect,
    background: Rgba,
    theme: &Theme,
    label: &str,
) -> usize {
    let surface = [
        (background.0 * 255.0).round() as u8,
        (background.1 * 255.0).round() as u8,
        (background.2 * 255.0).round() as u8,
    ];
    let text = [
        (theme.text.0 * 255.0).round() as u8,
        (theme.text.1 * 255.0).round() as u8,
        (theme.text.2 * 255.0).round() as u8,
    ];
    let start_x = rect.x.ceil() as usize + 4;
    let end_x = (rect.x + 100.0).floor() as usize;
    let start_y = rect.y.ceil() as usize + 2;
    let end_y = (rect.y + rect.height - 2.0).floor() as usize;
    let glyph_pixels = (start_y..end_y)
        .flat_map(|y| (start_x..end_x).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let actual = pixel_at(pixels, *x, *y);
            let from_surface = (0..3)
                .map(|channel| actual[channel].abs_diff(surface[channel]))
                .map(u16::from)
                .sum::<u16>();
            let from_text = (0..3)
                .map(|channel| actual[channel].abs_diff(text[channel]))
                .map(u16::from)
                .sum::<u16>();
            from_surface >= 36 && from_text < from_surface
        })
        .count();
    assert!(
        glyph_pixels >= 5,
        "expected visible {label:?} dropdown label pixels, found {glyph_pixels}"
    );
    glyph_pixels
}

#[test]
fn widget_menu_editors_and_collections_render_in_dark_light_and_dpi_variants() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..Default::default()
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                    force_fallback_adapter: true,
                    ..Default::default()
                })),
                ..Default::default()
            })
            .disable::<WinitPlugin>()
            .disable::<bevy::pbr::PbrPlugin>(),
    )
    .add_plugins(BuiPlugin)
    .init_resource::<CapturedFrames>();
    app.finish();
    app.cleanup();

    let mut target = Image::new_target_texture(
        TARGET_WIDTH,
        TARGET_HEIGHT,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    let camera = app
        .world_mut()
        .spawn((
            Camera2d,
            RenderTarget::Image(target.clone().into()),
            IsDefaultUiCamera,
        ))
        .id();

    let mut list_model = ListModel::<String> {
        row_height: 28.0,
        items: vec![
            ListItem {
                id: 1,
                value: "Files".to_owned(),
                enabled: true,
            },
            ListItem {
                id: 2,
                value: "Selected list row".to_owned(),
                enabled: true,
            },
            ListItem {
                id: 3,
                value: "Textures".to_owned(),
                enabled: true,
            },
        ],
        ..Default::default()
    };
    assert!(list_model.select(2, false, false));

    let mut tree_model = TreeModel::<String> {
        nodes: vec![
            TreeNode {
                id: 10,
                parent: None,
                children: vec![11],
                value: "Scene".to_owned(),
                expanded: true,
                enabled: true,
            },
            TreeNode {
                id: 11,
                parent: Some(10),
                children: Vec::new(),
                value: "Selected child".to_owned(),
                expanded: false,
                enabled: true,
            },
            TreeNode {
                id: 12,
                parent: None,
                children: Vec::new(),
                value: "Materials".to_owned(),
                expanded: false,
                enabled: true,
            },
        ],
        ..Default::default()
    };
    assert!(tree_model.select(11, false, false));

    let mut preview_grid = PreviewGrid {
        cell_width: 112.0,
        cell_height: 104.0,
        items: vec![
            PreviewGridItem {
                tint: [0.90, 0.20, 0.10, 1.0],
                ..PreviewGridItem::new(20, "Warm")
            },
            PreviewGridItem {
                tint: [0.10, 0.75, 0.25, 1.0],
                ..PreviewGridItem::new(21, "Selected tile")
            },
            PreviewGridItem {
                tint: [0.20, 0.35, 0.95, 1.0],
                ..PreviewGridItem::new(22, "Cool")
            },
        ],
        ..Default::default()
    };
    assert!(preview_grid.select(21, false, false));

    let tree = WidgetTree::new(WidgetNode::column([
        WidgetNode::row([
            WidgetNode::menu(
                "File",
                [
                    MenuEntry::command("open", "Open").with_shortcut("Ctrl+O"),
                    MenuEntry::command("save", "Save").with_shortcut("Ctrl+S"),
                    MenuEntry::command("quit", "Quit"),
                ],
            ),
            WidgetNode::hsv_color("Primary", Rgba(0.20, 0.40, 0.80, 1.0)),
        ]),
        WidgetNode::curve_editor_default("Ease"),
        WidgetNode::color_ramp_default("Gradient"),
        WidgetNode::row([
            WidgetNode::list("Assets", list_model),
            WidgetNode::tree("Hierarchy", tree_model),
            WidgetNode::preview_grid("Previews", preview_grid),
        ]),
    ]));
    let root = tree.spawn(app.world_mut(), None).unwrap();
    {
        let mut layout = app.world_mut().get_mut::<UiLayoutNode>(root).unwrap();
        layout.size.width = UiLength::Px(TARGET_WIDTH as f32);
        layout.size.height = UiLength::Px(TARGET_HEIGHT as f32);
    }
    app.world_mut().entity_mut(root).insert((
        UiLayoutRoot::new(
            camera,
            Rect::new(0.0, 0.0, TARGET_WIDTH as f32, TARGET_HEIGHT as f32),
            1.0,
        ),
        UiPaint(Primitive::RoundedRect {
            rect: ClipRect {
                x: 0.0,
                y: 0.0,
                width: TARGET_WIDTH as f32,
                height: TARGET_HEIGHT as f32,
            },
            radius: 0.0,
            color: Theme::default().background,
        }),
    ));
    let mut data_table = DataTable {
        columns: [
            ("Name", 160.0),
            ("Type", 110.0),
            ("Status", 110.0),
            ("Owner", 110.0),
            ("Modified", 110.0),
            ("Size", 110.0),
        ]
        .into_iter()
        .map(|(label, width)| TableColumn::new(label, width))
        .collect(),
        rows: (1..=96)
            .map(|id| TableRow {
                id,
                cells: vec![
                    format!("Asset {id:04}"),
                    if id % 2 == 0 { "Mesh" } else { "Image" }.to_owned(),
                    match id % 3 {
                        0 => "Clean",
                        1 => "Modified",
                        _ => "Ready",
                    }
                    .to_owned(),
                    format!("Scene {}", id % 8),
                    format!("2026-11-{:02}", id % 28 + 1),
                    format!("{} KB", id * 17),
                ],
            })
            .collect(),
        row_height: 24,
        scroll_x: 170.0,
        active_column: 3,
        ..Default::default()
    };
    assert!(data_table.select_cell(40, 3, false, false));
    assert!(data_table.sort_by(2, true));
    let active_row_index = data_table.rows.iter().position(|row| row.id == 40).unwrap();
    data_table.scroll_rows = active_row_index.saturating_sub(6);
    let table_viewport = Rect::new(650.0, 20.0, 540.0, 390.0);
    let table_root = WidgetTree::new(WidgetNode::data_table("Virtual inventory", data_table))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut()
        .entity_mut(table_root)
        .insert(UiLayoutRoot::new(camera, table_viewport, 1.0));
    {
        let mut layout = app.world_mut().get_mut::<UiLayoutNode>(table_root).unwrap();
        layout.size.width = UiLength::Px(table_viewport.width);
        layout.size.height = UiLength::Px(table_viewport.height);
        layout.clip_contents = true;
        layout.z_index = 8;
    }
    // A second viewport shares the target camera but keeps this dropdown near the lower-right
    // edge. Its popup must flip above the trigger, stay clipped to this inset viewport, and
    // retain stable selected/highlighted option rows.
    let dropdown_window = app.world_mut().spawn_empty().id();
    let dropdown_viewport = Rect::new(850.0, 700.0, 330.0, 470.0);
    let dropdown_viewport_root = app
        .world_mut()
        .spawn(UiLayoutRoot::new(dropdown_window, dropdown_viewport, 1.0))
        .id();
    let dropdown = WidgetTree::new(WidgetNode::dropdown(
        "Quality",
        (0..10).map(|index| format!("Quality {index}")),
        Some(1),
    ))
    .spawn(app.world_mut(), None)
    .unwrap();
    {
        let mut model = app.world_mut().get_mut::<Dropdown>(dropdown).unwrap();
        model.open();
        model.highlighted = Some(4);
    }
    app.world_mut().entity_mut(dropdown).insert(UiLayoutResult {
        window: dropdown_window,
        rect: Rect::new(1060.0, 1120.0, 130.0, 28.0),
        clip: None,
        z: 20,
        content_size: [130.0, 28.0],
        dpi_scale: 1.0,
    });
    let root_children = app.world().get::<UiChildren>(root).unwrap().0.clone();
    let header = root_children[0];
    let curve = root_children[1];
    let ramp_widget = root_children[2];
    let collection_row = root_children[3];
    let collection_widgets = app
        .world()
        .get::<UiChildren>(collection_row)
        .unwrap()
        .0
        .clone();
    let list_widget = collection_widgets[0];
    let tree_widget = collection_widgets[1];
    let preview_grid_widget = collection_widgets[2];
    {
        let mut layout = app.world_mut().get_mut::<UiLayoutNode>(curve).unwrap();
        layout.size.width = UiLength::Px(680.0);
        layout.size.height = UiLength::Px(340.0);
    }
    let header_children = app.world().get::<UiChildren>(header).unwrap().0.clone();
    let menu = header_children[0];
    let editor = header_children[1];
    {
        let mut curve_editor = app.world_mut().get_mut::<CurveEditor>(curve).unwrap();
        curve_editor.points[0].position = [0.0, 0.16];
        curve_editor.points[1].position = [1.0, 0.26];
        let middle = curve_editor.add_point(0.5, 0.78).unwrap();
        assert!(curve_editor.set_handles(middle, [-0.13, -0.24], [0.13, 0.16]));
        curve_editor.selected = Some(middle);
    }
    {
        let mut ramp = app.world_mut().get_mut::<ColorRamp>(ramp_widget).unwrap();
        assert!(ramp.set_color(1, Rgba(0.90, 0.10, 0.10, 1.0)));
        assert!(ramp.set_color(2, Rgba(0.10, 0.20, 0.90, 1.0)));
        assert!(ramp.add_stop(0.5, Rgba(0.10, 0.80, 0.20, 1.0)).is_some());
        ramp.interpolation = Interpolation::Smooth;
    }
    app.world_mut()
        .get_mut::<MenuControl>(menu)
        .unwrap()
        .session
        .open(None);

    for _ in 0..10 {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }
    for (scenario, (kind, dpi_scale)) in [
        (ThemeKind::Dark, 1.0),
        (ThemeKind::Dark, 2.0),
        (ThemeKind::Light, 1.0),
        (ThemeKind::Light, 2.0),
    ]
    .into_iter()
    .enumerate()
    {
        let theme = Theme::new(kind);
        *app.world_mut().resource_mut::<Theme>() = theme.clone();
        app.world_mut()
            .get_mut::<UiLayoutRoot>(root)
            .unwrap()
            .dpi_scale = dpi_scale;
        app.world_mut()
            .get_mut::<UiLayoutRoot>(table_root)
            .unwrap()
            .dpi_scale = dpi_scale;
        app.world_mut()
            .get_mut::<UiLayoutRoot>(dropdown_viewport_root)
            .unwrap()
            .dpi_scale = dpi_scale;
        app.world_mut()
            .get_mut::<bui_layout::UiLayoutResult>(dropdown)
            .unwrap()
            .dpi_scale = dpi_scale;
        if let Primitive::RoundedRect { color, .. } =
            &mut app.world_mut().get_mut::<UiPaint>(root).unwrap().0
        {
            *color = theme.background;
        }
        for _ in 0..3 {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }

        for (mode_index, interpolation) in [
            Interpolation::Constant,
            Interpolation::Linear,
            Interpolation::Smooth,
        ]
        .into_iter()
        .enumerate()
        {
            app.world_mut()
                .get_mut::<CurveEditor>(curve)
                .unwrap()
                .interpolation = interpolation;
            for _ in 0..3 {
                app.update();
                thread::sleep(Duration::from_millis(5));
            }

            let (wheel_rect, popup_item_rect, curve_rect, curve_model, ramp_rect, ramp_model) = {
                let display_items = app.world().resource::<DisplayList>().items();
                let wheels = display_items
                    .iter()
                    .filter_map(|item| match &item.primitive {
                        Primitive::HsvWheel { rect, .. } => Some(*rect),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    wheels.len(),
                    1,
                    "the HSV widget must emit one wheel primitive"
                );

                let menu_control = app.world().get::<MenuControl>(menu).unwrap();
                assert!(menu_control.session.open);
                let popup_item = menu_control
                    .items
                    .iter()
                    .find(|entity| {
                        app.world()
                            .get::<bui_widgets::MenuItemControl>(**entity)
                            .is_some_and(|item| item.id == "open")
                    })
                    .copied()
                    .expect("opened menu should expose its Open item");
                assert!(
                    app.world()
                        .get::<bui_core::UiVisible>(popup_item)
                        .unwrap()
                        .0
                );
                let popup_rect = app
                    .world()
                    .get::<bui_layout::UiLayoutResult>(popup_item)
                    .unwrap()
                    .rect;
                let curve_rect = app
                    .world()
                    .get::<bui_layout::UiLayoutResult>(curve)
                    .unwrap()
                    .rect;
                let curve_model = app.world().get::<CurveEditor>(curve).unwrap().clone();
                let ramp_rect = app
                    .world()
                    .get::<bui_layout::UiLayoutResult>(ramp_widget)
                    .unwrap()
                    .rect;
                let ramp_model = app.world().get::<ColorRamp>(ramp_widget).unwrap().clone();
                (
                    wheels[0],
                    popup_rect,
                    curve_rect,
                    curve_model,
                    ramp_rect,
                    ramp_model,
                )
            };

            let frame = scenario * 3 + mode_index;
            let (pixels, readback_latency) = {
                let readback_started = Instant::now();
                let readback = app
                    .world_mut()
                    .spawn(Readback::texture(target.clone()))
                    .observe(
                        move |captured: bevy::ecs::observer::On<ReadbackComplete>,
                              mut frames: ResMut<CapturedFrames>| {
                            frames.0.insert(
                                frame,
                                CapturedFrame {
                                    bytes: captured.data.clone(),
                                    completed_at: Instant::now(),
                                },
                            );
                        },
                    )
                    .id();
                let deadline = Instant::now() + Duration::from_secs(10);
                while Instant::now() < deadline
                    && !app
                        .world()
                        .resource::<CapturedFrames>()
                        .0
                        .contains_key(&frame)
                {
                    app.update();
                    thread::sleep(Duration::from_millis(5));
                }
                let captured = app
                    .world()
                    .resource::<CapturedFrames>()
                    .0
                    .get(&frame)
                    .cloned()
                    .expect("widget GPU readback did not complete before the deadline");
                let latency = captured.completed_at.duration_since(readback_started);
                app.world_mut().entity_mut(readback).despawn();
                (captured.bytes, latency)
            };
            if mode_index == 0
                && let Some(directory) = std::env::var_os("BUI_WIDGET_CAPTURE_DIR")
            {
                let directory = std::path::PathBuf::from(directory);
                fs::create_dir_all(&directory).expect("create widget capture directory");
                write_capture_ppm(
                    &directory.join(format!("{kind:?}-dpi{}.ppm", dpi_scale as u32)),
                    &pixels,
                );
            }

            let popup = *app
                .world()
                .get::<DropdownPopupState>(dropdown)
                .expect("open dropdown should expose viewport and popup geometry");
            assert!(popup.popup.open);
            assert_eq!(popup.popup.side, bui_widgets::menus::PopupSide::Above);
            assert_eq!(popup.viewport, dropdown_viewport);
            assert_eq!(popup.visible_count, 8);
            assert!(popup.popup.bounds.x >= popup.viewport.x);
            assert!(popup.popup.bounds.y >= popup.viewport.y);
            assert!(
                popup.popup.bounds.x + popup.popup.bounds.width
                    <= popup.viewport.x + popup.viewport.width
            );
            assert!(
                popup.popup.bounds.y + popup.popup.bounds.height
                    <= popup.viewport.y + popup.viewport.height
            );
            let (normal_entity, normal_rect, normal_option) =
                dropdown_option(app.world(), dropdown, 0);
            let (_, selected_rect, selected_option) = dropdown_option(app.world(), dropdown, 1);
            let (_, highlighted_rect, highlighted_option) =
                dropdown_option(app.world(), dropdown, 4);
            assert!(!normal_option.selected && !normal_option.highlighted);
            assert!(selected_option.selected && !selected_option.highlighted);
            assert!(!highlighted_option.selected && highlighted_option.highlighted);
            assert_eq!(
                app.world()
                    .get::<bui_layout::UiLayoutResult>(normal_entity)
                    .unwrap()
                    .clip,
                Some(popup.popup.bounds),
                "option text and fill must be clipped to popup bounds"
            );
            let selected_color = theme.color(ThemeColorToken::MenuSelectedSurface);
            for (rect, color, description) in [
                (normal_rect, theme.surface, "normal dropdown option"),
                (selected_rect, selected_color, "selected dropdown option"),
                (
                    highlighted_rect,
                    selected_color,
                    "highlighted dropdown option",
                ),
            ] {
                assert_color(
                    pixel_at(
                        &pixels,
                        (rect.x + rect.width - 12.0).round() as usize,
                        (rect.y + rect.height * 0.5).round() as usize,
                    ),
                    color,
                    description,
                );
            }
            assert_option_label_visible(
                &pixels,
                selected_rect,
                selected_color,
                &theme,
                "Quality 1",
            );
            assert_option_label_visible(
                &pixels,
                highlighted_rect,
                selected_color,
                &theme,
                "Quality 4",
            );
            assert_color(
                pixel_at(
                    &pixels,
                    (popup.viewport.x + popup.viewport.width + 5.0) as usize,
                    (selected_rect.y + selected_rect.height * 0.5).round() as usize,
                ),
                theme.background,
                "dropdown popup remains clipped at viewport edge",
            );

            let menu_x = (popup_item_rect.x + popup_item_rect.width - 12.0).floor() as usize;
            let menu_y = (popup_item_rect.y + popup_item_rect.height * 0.5).floor() as usize;
            assert_color(
                pixel_at(&pixels, menu_x, menu_y),
                theme.surface,
                "menu overlay row",
            );
            let menu_text_pixels = assert_menu_label_visible(&pixels, popup_item_rect, &theme);

            let center_x = wheel_rect.x + wheel_rect.width * 0.5;
            let outer_radius = wheel_rect.width.min(wheel_rect.height) * 0.5;
            let ring_x = center_x.floor() as usize;
            let ring_y = (wheel_rect.y + 5.0).floor() as usize;
            let disk_x = center_x.floor() as usize;
            let disk_y = (wheel_rect.y + outer_radius).floor() as usize;
            assert_near(
                pixel_at(&pixels, ring_x, ring_y),
                [128, 0, 255, 255],
                10,
                "actual HSV editor hue ring",
            );
            assert_near(
                pixel_at(&pixels, disk_x, disk_y),
                [204, 204, 204, 255],
                6,
                "actual HSV editor value center",
            );

            let curve_layout = app
                .world()
                .get::<bui_layout::UiLayoutResult>(curve)
                .unwrap();
            assert_eq!(curve_layout.dpi_scale, dpi_scale);
            assert_eq!(
                app.world()
                    .get::<bui_layout::UiLayoutResult>(editor)
                    .unwrap()
                    .dpi_scale,
                dpi_scale
            );
            assert_eq!(wheel_rect.width, 136.0);
            assert_eq!(wheel_rect.height, 136.0);

            let plot = curve_plot(curve_rect, &theme);
            let curve_z = curve_layout.z;
            let (curve_line_count, bezier_count) = app
                .world()
                .resource::<DisplayList>()
                .items()
                .iter()
                .filter(|item| item.z == curve_z.saturating_add(3))
                .fold((0, 0), |(lines, beziers), item| match &item.primitive {
                    Primitive::Line { from, to, .. }
                        if [from, to].iter().all(|point| {
                            point[0] >= plot.x - 1.0
                                && point[0] <= plot.x + plot.width + 1.0
                                && point[1] >= plot.y - 1.0
                                && point[1] <= plot.y + plot.height + 1.0
                        }) =>
                    {
                        (lines + 1, beziers)
                    }
                    Primitive::Bezier { .. } => (lines, beziers + 1),
                    _ => (lines, beziers),
                });
            match interpolation {
                Interpolation::Constant => {
                    assert_eq!(curve_line_count, 4);
                    assert_eq!(bezier_count, 0);
                }
                Interpolation::Linear => {
                    assert_eq!(curve_line_count, 2);
                    assert_eq!(bezier_count, 0);
                }
                Interpolation::Smooth => {
                    assert_eq!(curve_line_count, 0);
                    assert_eq!(bezier_count, 2);
                }
            }

            for sample_x in [0.25, 0.75] {
                let sample =
                    curve_screen_position(plot, [sample_x, curve_model.evaluate(sample_x)]);
                assert_color_near_position(
                    &pixels,
                    sample,
                    theme.accent,
                    34,
                    3,
                    "actual curve path",
                );
            }
            let selected_point = curve_model
                .points
                .iter()
                .find(|point| Some(point.id) == curve_model.selected)
                .unwrap();
            assert_color_near_position(
                &pixels,
                curve_screen_position(plot, selected_point.position),
                theme.accent,
                20,
                2,
                "selected curve point",
            );
            if interpolation == Interpolation::Smooth {
                for handle in [selected_point.left_handle, selected_point.right_handle] {
                    assert_color_near_position(
                        &pixels,
                        curve_screen_position(
                            plot,
                            [
                                selected_point.position[0] + handle[0],
                                selected_point.position[1] + handle[1],
                            ],
                        ),
                        theme.text_muted,
                        28,
                        2,
                        "smooth curve handle",
                    );
                }
            }

            assert_eq!(ramp_model.interpolation, Interpolation::Smooth);
            let ramp_track = ramp_track(ramp_rect);
            let selected_stop = ramp_model.selected_stop().unwrap();
            assert_eq!(selected_stop.position, 0.5);
            let stop_marker_y = ramp_track.y + ramp_track.height + 10.0;
            for stop in &ramp_model.stops {
                let marker_color = if stop.id == selected_stop.id {
                    theme.accent
                } else {
                    stop.color
                };
                assert_color(
                    pixel_at(
                        &pixels,
                        (ramp_track.x + stop.position * ramp_track.width).round() as usize,
                        stop_marker_y.round() as usize,
                    ),
                    marker_color,
                    "color ramp stop marker",
                );
            }
            for segment in [8_usize, 24, 39] {
                let sample = (segment as f32 + 0.5) / 48.0;
                assert_color(
                    pixel_at(
                        &pixels,
                        (ramp_track.x + ramp_track.width * sample).floor() as usize,
                        (ramp_track.y + ramp_track.height * 0.5).floor() as usize,
                    ),
                    ramp_model.sample(sample),
                    "color ramp interpolation sample",
                );
            }

            let (_, selected_list_rect, selected_list_item) =
                collection_item(app.world(), list_widget, 2);
            let (_, unselected_list_rect, unselected_list_item) =
                collection_item(app.world(), list_widget, 1);
            assert_eq!(selected_list_item.kind, CollectionItemKind::List);
            assert!(selected_list_item.selected);
            assert!(!unselected_list_item.selected);
            assert_color(
                pixel_at(
                    &pixels,
                    (selected_list_rect.x + selected_list_rect.width - 12.0).round() as usize,
                    (selected_list_rect.y + selected_list_rect.height * 0.5).round() as usize,
                ),
                blend(theme.surface, theme.accent, 0.28),
                "selected virtualized list row",
            );
            assert_color(
                pixel_at(
                    &pixels,
                    (unselected_list_rect.x + unselected_list_rect.width - 12.0).round() as usize,
                    (unselected_list_rect.y + unselected_list_rect.height * 0.5).round() as usize,
                ),
                theme.surface,
                "unselected virtualized list row",
            );

            let (_, selected_tree_rect, selected_tree_item) =
                collection_item(app.world(), tree_widget, 11);
            assert_eq!(selected_tree_item.kind, CollectionItemKind::Tree);
            assert!(selected_tree_item.selected);
            assert_eq!(selected_tree_item.depth, 1);
            assert_color(
                pixel_at(
                    &pixels,
                    (selected_tree_rect.x + selected_tree_rect.width - 12.0).round() as usize,
                    (selected_tree_rect.y + selected_tree_rect.height * 0.5).round() as usize,
                ),
                blend(theme.surface, theme.accent, 0.28),
                "selected tree child row",
            );

            let (_, warm_tile_rect, warm_tile) =
                collection_item(app.world(), preview_grid_widget, 20);
            let (selected_tile_entity, selected_tile_rect, selected_tile) =
                collection_item(app.world(), preview_grid_widget, 21);
            assert_eq!(warm_tile.kind, CollectionItemKind::PreviewGrid);
            assert!(!warm_tile.selected);
            assert!(selected_tile.selected);
            let collection_spacing = theme.spacing.collection;
            let preview_padding = collection_spacing.preview_padding;
            let swatch_padding = collection_spacing.padding;
            let warm_swatch = Rect {
                x: warm_tile_rect.x + preview_padding + swatch_padding,
                y: warm_tile_rect.y + preview_padding + swatch_padding,
                width: warm_tile_rect.width - 2.0 * (preview_padding + swatch_padding),
                height: warm_tile_rect.height
                    - collection_spacing.preview_caption_height
                    - 2.0 * (preview_padding + swatch_padding),
            };
            assert_color(
                pixel_at(
                    &pixels,
                    (warm_swatch.x + warm_swatch.width * 0.5).round() as usize,
                    (warm_swatch.y + warm_swatch.height * 0.5).round() as usize,
                ),
                Rgba(warm_tile.tint[0], warm_tile.tint[1], warm_tile.tint[2], 1.0),
                "preview grid swatch",
            );
            let selected_overlay = app
                .world()
                .resource::<DisplayList>()
                .items()
                .iter()
                .any(|item| {
                    item.z
                        == app
                            .world()
                            .get::<bui_layout::UiLayoutResult>(selected_tile_entity)
                            .unwrap()
                            .z
                            .saturating_add(3)
                        && matches!(
                            &item.primitive,
                            Primitive::RoundedRect { color, .. }
                                if *color == Rgba(theme.accent.0, theme.accent.1, theme.accent.2, 0.16)
                        )
                });
            assert!(
                selected_overlay,
                "selected preview tile should paint an accent overlay"
            );
            assert!(
                selected_tile_rect.x < TARGET_WIDTH as f32
                    && selected_tile_rect.y < TARGET_HEIGHT as f32
            );

            let table_model = app.world().get::<DataTable>(table_root).unwrap();
            let table_layout = app
                .world()
                .get::<bui_layout::UiLayoutResult>(table_root)
                .unwrap();
            assert_eq!(table_layout.rect, table_viewport);
            assert_eq!(table_layout.dpi_scale, dpi_scale);
            assert_eq!(table_model.sort, Some((2, true)));
            assert_eq!(table_model.scroll_x, 170.0);
            let table_children = app.world().get::<UiChildren>(table_root).unwrap_or_else(|| {
                let table_layout = app
                    .world()
                    .get::<bui_layout::UiLayoutResult>(table_root)
                    .unwrap();
                let generated = app
                    .world()
                    .iter_entities()
                    .filter(|entity| {
                        entity.get::<DataTableHeaderControl>().is_some()
                            || entity.get::<DataTableCellControl>().is_some()
                    })
                    .map(|entity| entity.id())
                    .collect::<Vec<_>>();
                panic!(
                    "DataTable view did not mount children: layout={table_layout:?}, generated controls={generated:?}"
                );
            });
            let headers = table_children
                .0
                .iter()
                .filter_map(|entity| {
                    app.world()
                        .get::<DataTableHeaderControl>(*entity)
                        .cloned()
                        .map(|header| (*entity, header))
                })
                .collect::<Vec<_>>();
            let cells = table_children
                .0
                .iter()
                .filter_map(|entity| {
                    app.world()
                        .get::<DataTableCellControl>(*entity)
                        .cloned()
                        .map(|cell| (*entity, cell))
                })
                .collect::<Vec<_>>();
            let visible_columns = headers
                .iter()
                .map(|(_, header)| header.column)
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(visible_columns, [1, 2, 3, 4, 5].into());
            let visible_rows = cells
                .iter()
                .map(|(_, cell)| cell.row_id)
                .collect::<std::collections::BTreeSet<_>>();
            assert!(visible_rows.len() < table_model.rows.len());
            assert!(
                visible_rows.contains(&40),
                "selected row 40 should be visible; scroll={}, selected={:?}, visible={visible_rows:?}",
                table_model.scroll_rows,
                table_model.selection.active,
            );
            assert!(!visible_rows.contains(&table_model.rows[0].id));
            let (sorted_header_entity, sorted_header_rect) =
                table_header(app.world(), table_root, 2);
            let sorted_header = app
                .world()
                .get::<DataTableHeaderControl>(sorted_header_entity)
                .unwrap();
            assert_eq!(sorted_header.sort_ascending, Some(true));
            let (active_header_entity, active_header_rect) =
                table_header(app.world(), table_root, table_model.active_column);
            let active_header = app
                .world()
                .get::<DataTableHeaderControl>(active_header_entity)
                .unwrap();
            assert!(active_header.active);
            assert!(headers.len() < table_model.columns.len());
            assert!(
                app.world()
                    .resource::<DisplayList>()
                    .items()
                    .iter()
                    .any(|item| {
                        matches!(
                            &item.primitive,
                            Primitive::TextWrapped { text, .. } if text == "Status  ↑"
                        )
                    })
            );
            assert_table_header_text_visible(&pixels, sorted_header_rect, &theme);
            let active_header_bg = blend(theme.surface, theme.selection, 0.34);
            assert_color(
                pixel_at(
                    &pixels,
                    (active_header_rect.x + active_header_rect.width - 14.0).round() as usize,
                    (active_header_rect.y + active_header_rect.height * 0.5).round() as usize,
                ),
                active_header_bg,
                "active virtualized table header",
            );
            let resize_divider = app
                .world()
                .resource::<DisplayList>()
                .items()
                .iter()
                .any(|item| {
                    matches!(
                        &item.primitive,
                        Primitive::Line { from, to, color, .. }
                            if (from[0] - (active_header_rect.x + active_header_rect.width - 1.0)).abs() < 0.1
                                && (to[0] - from[0]).abs() < 0.1
                                && *color == theme.focus
                    )
                });
            assert!(
                resize_divider,
                "active table column should emit its focus-colored resize divider"
            );

            let (selected_cell_entity, selected_cell_rect, selected_cell) =
                table_cell(app.world(), table_root, 40, 3);
            assert!(selected_cell.selected && selected_cell.active);
            assert_eq!(
                app.world()
                    .get::<bui_style::UiComputedStyle>(selected_cell_entity)
                    .unwrap()
                    .color(bui_style::UiStyleProperty::Background),
                Some(theme.selection)
            );
            let selected_cell_layout = app
                .world()
                .get::<bui_layout::UiLayoutResult>(selected_cell_entity)
                .unwrap();
            let cell_clip = selected_cell_layout.clip.unwrap();
            assert!(cell_clip.x >= table_viewport.x && cell_clip.y >= table_viewport.y);
            assert!(cell_clip.x + cell_clip.width <= table_viewport.x + table_viewport.width);
            let selected_cell_color = theme.selection;
            assert_color(
                pixel_at(
                    &pixels,
                    (selected_cell_rect.x + selected_cell_rect.width - 14.0).round() as usize,
                    (selected_cell_rect.y + selected_cell_rect.height * 0.5).round() as usize,
                ),
                selected_cell_color,
                "selected virtualized table cell",
            );
            assert_color(
                pixel_at(
                    &pixels,
                    (selected_cell_rect.x + 1.0).round() as usize,
                    (selected_cell_rect.y + selected_cell_rect.height * 0.5).round() as usize,
                ),
                theme.focus,
                "active table cell focus marker",
            );
            let unselected_row_id = visible_rows
                .iter()
                .copied()
                .find(|row_id| *row_id != 40)
                .expect("a second table row should be visible for the unselected cell check");
            let (_, unselected_cell_rect, unselected_cell) =
                table_cell(app.world(), table_root, unselected_row_id, 4);
            assert!(!unselected_cell.selected);
            assert_color(
                pixel_at(
                    &pixels,
                    (unselected_cell_rect.x + unselected_cell_rect.width - 14.0).round() as usize,
                    (unselected_cell_rect.y + unselected_cell_rect.height * 0.5).round() as usize,
                ),
                theme.surface,
                "unselected virtualized table cell",
            );
            let (partial_header_entity, partial_header_rect) =
                table_header(app.world(), table_root, 1);
            let partial_header = app
                .world()
                .get::<DataTableHeaderControl>(partial_header_entity)
                .unwrap();
            assert!(partial_header_rect.x < table_viewport.x);
            let partial_header_layout = app
                .world()
                .get::<bui_layout::UiLayoutResult>(partial_header_entity)
                .unwrap();
            assert!(partial_header_layout.clip.unwrap().x >= table_viewport.x);
            assert_eq!(partial_header.column, 1);

            println!(
                "widget scene {kind:?} dpi={dpi_scale} {interpolation:?}: menu={:?}, HSV ring={:?}, curve/ramp, selected collections and DataTable headers/cells visible, readback round-trip={:?} (label pixels={menu_text_pixels}, virtual rows={})",
                pixel_at(&pixels, menu_x, menu_y),
                pixel_at(&pixels, ring_x, ring_y),
                readback_latency,
                visible_rows.len(),
            );
        }
    }
}
