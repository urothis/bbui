use super::*;

pub(super) fn curve_plot_rect(rect: Rect, theme: &Theme) -> Rect {
    let spacing = theme.spacing.editor;
    let left = spacing.gutter_width + spacing.padding_x;
    let right = spacing.padding_x;
    let vertical = theme.control_height + spacing.padding_y;
    Rect {
        x: rect.x + left,
        y: rect.y + vertical,
        width: (rect.width - left - right).max(1.0),
        height: (rect.height - vertical - spacing.padding_y).max(1.0),
    }
}

pub(super) fn curve_data_position(
    editor: &CurveEditor,
    plot: Rect,
    position: [f32; 2],
) -> [f32; 2] {
    editor.view_to_curve([
        (position[0] - plot.x) / plot.width,
        1.0 - (position[1] - plot.y) / plot.height,
    ])
}

pub(super) fn curve_screen_position(
    editor: &CurveEditor,
    plot: Rect,
    position: [f32; 2],
) -> [f32; 2] {
    let view = editor.curve_to_view(position);
    [
        plot.x + view[0] * plot.width,
        plot.y + (1.0 - view[1]) * plot.height,
    ]
}

pub(super) fn begin_curve_editor_pointer(
    world: &mut World,
    window: Entity,
    target: Entity,
    position: [f32; 2],
    modifiers: UiModifiers,
) {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let (before, kind) = {
        let (Some(layout), Some(mut editor)) = (
            world.get::<UiLayoutResult>(target).copied(),
            world.get_mut::<CurveEditor>(target),
        ) else {
            return;
        };
        let plot = curve_plot_rect(layout.rect, &theme);
        if !plot.contains(position[0], position[1]) {
            return;
        }
        let before = editor.clone();
        let selected = editor.selected;
        let mut kind = selected.and_then(|id| {
            (editor.interpolation == Interpolation::Smooth)
                .then(|| editor.points.iter().find(|point| point.id == id))
                .flatten()
                .and_then(|point| {
                    let point_position = curve_screen_position(&editor, plot, point.position);
                    let left = curve_screen_position(
                        &editor,
                        plot,
                        [
                            point.position[0] + point.left_handle[0],
                            point.position[1] + point.left_handle[1],
                        ],
                    );
                    let right = curve_screen_position(
                        &editor,
                        plot,
                        [
                            point.position[0] + point.right_handle[0],
                            point.position[1] + point.right_handle[1],
                        ],
                    );
                    if distance(left, position) <= 8.0 {
                        Some(CurveEditorDragKind::LeftHandle(id))
                    } else if distance(right, position) <= 8.0 {
                        Some(CurveEditorDragKind::RightHandle(id))
                    } else if distance(point_position, position) <= 9.0 {
                        Some(CurveEditorDragKind::Point(id))
                    } else {
                        None
                    }
                })
        });
        if kind.is_none() {
            kind = editor
                .points
                .iter()
                .map(|point| {
                    (
                        distance(
                            curve_screen_position(&editor, plot, point.position),
                            position,
                        ),
                        point.id,
                    )
                })
                .filter(|(distance, _)| *distance <= 9.0)
                .min_by(|left, right| left.0.total_cmp(&right.0))
                .map(|(_, id)| CurveEditorDragKind::Point(id));
        }
        if let Some(CurveEditorDragKind::Point(id))
        | Some(CurveEditorDragKind::LeftHandle(id))
        | Some(CurveEditorDragKind::RightHandle(id)) = kind
        {
            editor.selected = Some(id);
        } else if kind.is_none() && modifiers.shift {
            let data = curve_data_position(&editor, plot, position);
            kind = editor
                .add_point(data[0], data[1])
                .map(CurveEditorDragKind::Point);
        }
        let Some(kind) = kind else {
            return;
        };
        (before, kind)
    };
    world
        .resource_mut::<WidgetInteractionState>()
        .curve_editor_drags
        .insert(
            window,
            CurveEditorDrag {
                target,
                before,
                kind,
            },
        );
}

pub(super) fn begin_curve_editor_pan(
    world: &mut World,
    window: Entity,
    target: Entity,
    position: [f32; 2],
) {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let (Some(layout), Some(editor)) = (
        world.get::<UiLayoutResult>(target).copied(),
        world.get::<CurveEditor>(target).cloned(),
    ) else {
        return;
    };
    if !curve_plot_rect(layout.rect, &theme).contains(position[0], position[1]) {
        return;
    }
    world
        .resource_mut::<WidgetInteractionState>()
        .curve_editor_drags
        .insert(
            window,
            CurveEditorDrag {
                target,
                before: editor,
                kind: CurveEditorDragKind::Pan,
            },
        );
}

pub(super) fn update_curve_editor_drag(
    world: &mut World,
    window: Entity,
    position: [f32; 2],
    delta: [f32; 2],
) {
    let Some((target, kind)) = world
        .resource::<WidgetInteractionState>()
        .curve_editor_drags
        .get(&window)
        .map(|drag| (drag.target, drag.kind))
    else {
        return;
    };
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let Some(plot) = world
        .get::<UiLayoutResult>(target)
        .map(|layout| curve_plot_rect(layout.rect, &theme))
    else {
        return;
    };
    if let Some(mut editor) = world.get_mut::<CurveEditor>(target) {
        match kind {
            CurveEditorDragKind::Point(id) => {
                let data = curve_data_position(&editor, plot, position);
                editor.move_point(id, data);
            }
            CurveEditorDragKind::LeftHandle(id) | CurveEditorDragKind::RightHandle(id) => {
                let data = curve_data_position(&editor, plot, position);
                if let Some(point) = editor.points.iter().find(|point| point.id == id).copied() {
                    let offset = [data[0] - point.position[0], data[1] - point.position[1]];
                    let (left, right) = if kind == CurveEditorDragKind::LeftHandle(id) {
                        (offset, point.right_handle)
                    } else {
                        (point.left_handle, offset)
                    };
                    editor.set_handles(id, left, right);
                }
            }
            CurveEditorDragKind::Pan => {
                editor.pan_by([delta[0] / plot.width, -delta[1] / plot.height]);
            }
        }
    }
}

pub(super) fn finish_curve_editor_drag(
    world: &mut World,
    drag: CurveEditorDrag,
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    if cancelled {
        if let Some(mut editor) = world.get_mut::<CurveEditor>(drag.target) {
            *editor = drag.before;
        }
        return;
    }
    let Some(editor) = world.get::<CurveEditor>(drag.target) else {
        return;
    };
    if !curve_editor_content_changed(&drag.before, editor) {
        return;
    }
    let (Ok(before), Ok(after)) = (drag.before.to_json(), editor.to_json()) else {
        return;
    };
    emitted.push(WidgetEvent::CurveEditorCommitted {
        target: drag.target,
        before,
        after,
    });
}

pub(super) fn curve_editor_content_changed(before: &CurveEditor, after: &CurveEditor) -> bool {
    before.points != after.points
        || before.interpolation != after.interpolation
        || before.pan != after.pan
        || before.zoom != after.zoom
}

pub(super) fn edit_curve_editor(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut CurveEditor) -> bool,
) {
    let Some(before_editor) = world.get::<CurveEditor>(target).cloned() else {
        return;
    };
    let changed = world
        .get_mut::<CurveEditor>(target)
        .is_some_and(|mut editor| edit(&mut editor));
    if !changed
        || !world
            .get::<CurveEditor>(target)
            .is_some_and(|after| curve_editor_content_changed(&before_editor, after))
    {
        return;
    }
    if let (Ok(before), Some(after)) = (before_editor.to_json(), world.get::<CurveEditor>(target))
        && let Ok(after) = after.to_json()
    {
        emitted.push(WidgetEvent::CurveEditorCommitted {
            target,
            before,
            after,
        });
    }
}

pub(super) fn zoom_curve_editor_at(
    world: &mut World,
    target: Entity,
    position: [f32; 2],
    delta_y: f32,
    emitted: &mut Vec<WidgetEvent>,
) {
    if !delta_y.is_finite() || delta_y == 0.0 {
        return;
    }
    let Some(layout) = world.get::<UiLayoutResult>(target).copied() else {
        return;
    };
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let plot = curve_plot_rect(layout.rect, &theme);
    let anchor = [
        (position[0] - plot.x) / plot.width,
        1.0 - (position[1] - plot.y) / plot.height,
    ];
    let factor = 1.1_f32.powf(delta_y.clamp(-8.0, 8.0));
    edit_curve_editor(world, target, emitted, |editor| {
        let before = (editor.pan, editor.zoom);
        editor.zoom_by(factor, anchor);
        before != (editor.pan, editor.zoom)
    });
}

pub(super) fn distance(left: [f32; 2], right: [f32; 2]) -> f32 {
    (left[0] - right[0]).hypot(left[1] - right[1])
}
