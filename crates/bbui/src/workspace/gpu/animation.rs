//! Deterministic 60 Hz input frames for offscreen workspace animation captures.

use super::*;
use bbui_core::{UiInputEvent, UiInputQueue, UiKey, UiPointerButton};
use bbui_layout::{Rect, UiLayoutResult};
use std::{f32::consts::TAU, path::Path};

const FRAMES: usize = 120;
const FPS: f64 = 60.0;

fn rect(world: &mut World, key: &str) -> Result<Rect, String> {
    let entity = {
        let mut query = world.query::<(Entity, &UiKey)>();
        query
            .iter(world)
            .find(|(_, candidate)| candidate.0 == key)
            .map(|(entity, _)| entity)
    }
    .ok_or_else(|| format!("missing workspace key {key}"))?;
    world
        .get::<UiLayoutResult>(entity)
        .map(|result| result.rect)
        .ok_or_else(|| format!("missing layout for workspace key {key}"))
}

fn center(rect: Rect) -> [f32; 2] {
    [rect.x + rect.width * 0.5, rect.y + rect.height * 0.5]
}

fn move_pointer(app: &mut App, window: Entity, position: [f32; 2], frame: usize) {
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::PointerMove {
            window,
            position,
            time_seconds: frame as f64 / FPS,
        });
}

fn button(app: &mut App, window: Entity, position: [f32; 2], frame: usize, pressed: bool) {
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed,
            position,
            time_seconds: frame as f64 / FPS,
        });
}

fn save_frame(
    app: &mut App,
    target: &Handle<Image>,
    directory: &Path,
    capture_index: usize,
    frame: usize,
) {
    app.update();
    app.update();
    let bytes = super::capture(app, target, capture_index);
    super::save_png(directory.join(format!("{frame:03}.png")), &bytes);
    app.world_mut()
        .resource_mut::<Frames>()
        .0
        .remove(&capture_index);
}

fn write_cursor_path(directory: &Path, positions: &[[f32; 2]]) -> Result<(), String> {
    let rows = positions
        .iter()
        .map(|point| format!("{:.2},{:.2}\n", point[0], point[1]))
        .collect::<String>();
    std::fs::write(directory.join("cursor.csv"), rows).map_err(|error| error.to_string())
}

fn dock(
    app: &mut App,
    target: &Handle<Image>,
    camera: Entity,
    output: &Path,
) -> Result<(), String> {
    std::fs::create_dir_all(output).map_err(|error| error.to_string())?;
    let tab = center(rect(app.world_mut(), "ws:tab:documents:preview")?);
    let group = rect(app.world_mut(), "ws:group:inspector")?;
    let destination = [group.x + group.width * 0.88, group.y + group.height * 0.46];
    let mut cursor = Vec::with_capacity(FRAMES);
    for frame in 0..FRAMES {
        let position = if frame < 13 {
            tab
        } else if frame <= 79 {
            let t = (frame - 13) as f32 / 66.0;
            let eased = t * t * (3.0 - 2.0 * t);
            [
                tab[0] + (destination[0] - tab[0]) * eased,
                tab[1] + (destination[1] - tab[1]) * eased,
            ]
        } else {
            destination
        };
        if frame == 10 || (13..=79).contains(&frame) {
            move_pointer(app, camera, position, frame);
        }
        if frame == 12 {
            button(app, camera, position, frame, true);
        } else if frame == 80 {
            button(app, camera, position, frame, false);
        }
        save_frame(app, target, output, frame + 1_000, frame);
        cursor.push(position);
    }
    write_cursor_path(output, &cursor)?;
    if app
        .world()
        .resource::<Workspace>()
        .layout()
        .group("documents")
        .is_none_or(|group| group.panes.contains(&PaneId::from("preview")))
    {
        return Err("drag did not dock Preview outside Documents".to_owned());
    }
    Ok(())
}

fn resize(
    app: &mut App,
    target: &Handle<Image>,
    camera: Entity,
    output: &Path,
) -> Result<(), String> {
    std::fs::create_dir_all(output).map_err(|error| error.to_string())?;
    let start = center(rect(app.world_mut(), "ws:splitter:main-split:0")?);
    let mut cursor = Vec::with_capacity(FRAMES);
    let mut changed = false;
    for frame in 0..FRAMES {
        let position = if (13..=107).contains(&frame) {
            let phase = (frame - 13) as f32 / 94.0 * TAU;
            [start[0] + phase.sin() * 90.0, start[1]]
        } else {
            start
        };
        if frame == 10 || (13..=107).contains(&frame) {
            move_pointer(app, camera, position, frame);
        }
        if frame == 12 {
            button(app, camera, position, frame, true);
        } else if frame == 108 {
            button(app, camera, position, frame, false);
        }
        save_frame(app, target, output, frame + 2_000, frame);
        if let DockNode::Split { shares, .. } = &app.world().resource::<Workspace>().layout().root {
            changed |= (shares[0] - 2.0).abs() > 0.01;
        }
        cursor.push(position);
    }
    write_cursor_path(output, &cursor)?;
    if !changed {
        return Err("splitter drag did not resize the workspace".to_owned());
    }
    Ok(())
}

pub(super) fn capture_frames(
    app: &mut App,
    target: &Handle<Image>,
    camera: Entity,
    output: &Path,
) -> Result<(), String> {
    app.world_mut()
        .resource_mut::<input::WorkspaceGesture>()
        .hover = None;
    app.world_mut().resource_mut::<Workspace>().dirty = true;
    for _ in 0..3 {
        app.update();
    }
    let original = app.world().resource::<Workspace>().layout().clone();
    dock(app, target, camera, &output.join("dock"))?;
    app.world_mut()
        .resource_mut::<Workspace>()
        .restore(original)
        .map_err(|error| error.to_string())?;
    for _ in 0..3 {
        app.update();
    }
    resize(app, target, camera, &output.join("resize"))
}
