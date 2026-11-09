use super::*;

pub(super) fn collection_json(world: &World, owner: Entity) -> Option<String> {
    world
        .get::<ListModel<String>>(owner)
        .and_then(|model| model.to_json().ok())
        .or_else(|| {
            world
                .get::<TreeModel<String>>(owner)
                .and_then(|model| model.to_json().ok())
        })
        .or_else(|| {
            world
                .get::<PreviewGrid>(owner)
                .and_then(|model| model.to_json().ok())
        })
}

pub(super) fn edit_collection(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut World, Entity) -> bool,
) {
    let Some(owner) = collection_owner(world, target) else {
        return;
    };
    let Some(before) = collection_json(world, owner) else {
        return;
    };
    edit_collection_from_snapshot(world, owner, before, emitted, edit);
}

pub(super) fn edit_collection_from_snapshot(
    world: &mut World,
    owner: Entity,
    before: String,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut World, Entity) -> bool,
) {
    if !edit(world, owner) {
        return;
    }
    if let Some(after) = collection_json(world, owner)
        && before != after
    {
        emitted.push(WidgetEvent::CollectionCommitted {
            target: owner,
            before,
            after,
        });
    }
}

pub(super) fn collection_is_renaming(world: &World, owner: Entity) -> bool {
    world
        .get::<CollectionEditorState>(owner)
        .is_some_and(|state| state.rename_id.is_some())
}

pub(super) fn collection_value(world: &World, owner: Entity, id: u64) -> Option<String> {
    world
        .get::<ListModel<String>>(owner)
        .and_then(|model| model.items.iter().find(|item| item.id == id))
        .map(|item| item.value.clone())
        .or_else(|| {
            world
                .get::<TreeModel<String>>(owner)
                .and_then(|model| model.nodes.iter().find(|node| node.id == id))
                .map(|node| node.value.clone())
        })
        .or_else(|| {
            world
                .get::<PreviewGrid>(owner)
                .and_then(|model| model.items.iter().find(|item| item.id == id))
                .map(|item| item.label.clone())
        })
}

pub(super) fn begin_collection_rename(world: &mut World, target: Entity, id: u64) {
    let Some(owner) = collection_owner(world, target) else {
        return;
    };
    let Some(value) = collection_value(world, owner, id) else {
        return;
    };
    if let Some(mut state) = world.get_mut::<CollectionEditorState>(owner) {
        state.rename_id = Some(id);
        state.rename_original.clone_from(&value);
        state.rename_buffer = value;
        state.rename_cursor = state.rename_buffer.chars().count();
        state.rename_select_all = true;
    }
}

pub(super) fn cancel_collection_rename(world: &mut World, target: Entity) {
    let Some(owner) = collection_owner(world, target) else {
        return;
    };
    if let Some(mut state) = world.get_mut::<CollectionEditorState>(owner) {
        *state = CollectionEditorState::default();
    }
}

pub(super) fn collection_rename_text(world: &mut World, owner: Entity, text: &str) {
    if let Some(mut state) = world.get_mut::<CollectionEditorState>(owner)
        && state.rename_id.is_some()
    {
        if state.rename_select_all {
            state.rename_buffer.clear();
            state.rename_cursor = 0;
            state.rename_select_all = false;
        }
        let byte_index = state
            .rename_buffer
            .char_indices()
            .nth(state.rename_cursor)
            .map_or(state.rename_buffer.len(), |(index, _)| index);
        state.rename_buffer.insert_str(byte_index, text);
        state.rename_cursor += text.chars().count();
    }
}

pub(super) fn commit_collection_rename(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(owner) = collection_owner(world, target) else {
        return;
    };
    let Some((id, value)) = world
        .get::<CollectionEditorState>(owner)
        .and_then(|state| state.rename_id.map(|id| (id, state.rename_buffer.clone())))
    else {
        return;
    };
    cancel_collection_rename(world, owner);
    edit_collection(world, owner, emitted, |world, owner| {
        if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
            list.rename(id, value)
        } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
            tree.rename(id, value)
        } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
            grid.rename(id, value)
        } else {
            false
        }
    });
}

pub(super) fn click_collection_item(
    world: &mut World,
    item: &CollectionItemControl,
    position: [f32; 2],
    click_count: u32,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    let owner = item.owner;
    let layout = world.get::<UiLayoutResult>(item.owner).copied();
    let list_spacing = world
        .get_resource::<Theme>()
        .map_or_else(|| Theme::default().spacing.list, |theme| theme.spacing.list);
    let in_disclosure = item.kind == CollectionItemKind::Tree
        && item.has_children
        && layout.is_some_and(|layout| {
            position[0]
                < layout.rect.x
                    + list_spacing.padding_x
                    + list_spacing.indent * (item.depth as f32 + 1.0)
        });
    edit_collection(world, owner, emitted, |world, owner| {
        if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
            let before = list.selection.clone();
            list.select(item.id, modifiers.control, modifiers.shift);
            before != list.selection
        } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
            let before = tree.selection.clone();
            tree.select(item.id, modifiers.control, modifiers.shift);
            let selected = before != tree.selection;
            let expanded = (in_disclosure || click_count >= 2) && tree.toggle_expanded(item.id);
            selected || expanded
        } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
            let before = grid.selection.clone();
            grid.select(item.id, modifiers.control, modifiers.shift);
            before != grid.selection
        } else {
            false
        }
    });
    if click_count >= 2 && item.kind != CollectionItemKind::Tree {
        emitted.push(WidgetEvent::CollectionItemActivated {
            target: owner,
            id: item.id,
        });
    }
}

pub(super) fn handle_collection_key(
    world: &mut World,
    owner: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    if collection_is_renaming(world, owner) {
        match key {
            UiKeyCode::Enter => commit_collection_rename(world, owner, emitted),
            UiKeyCode::Escape => cancel_collection_rename(world, owner),
            UiKeyCode::Backspace => {
                if let Some(mut state) = world.get_mut::<CollectionEditorState>(owner) {
                    if state.rename_select_all {
                        state.rename_buffer.clear();
                        state.rename_cursor = 0;
                        state.rename_select_all = false;
                    } else if state.rename_cursor > 0 {
                        let from = state
                            .rename_buffer
                            .char_indices()
                            .nth(state.rename_cursor - 1)
                            .map_or(0, |(index, _)| index);
                        let to = state
                            .rename_buffer
                            .char_indices()
                            .nth(state.rename_cursor)
                            .map_or(state.rename_buffer.len(), |(index, _)| index);
                        state.rename_buffer.replace_range(from..to, "");
                        state.rename_cursor -= 1;
                    }
                }
            }
            UiKeyCode::Delete => {
                if let Some(mut state) = world.get_mut::<CollectionEditorState>(owner) {
                    if state.rename_select_all {
                        state.rename_buffer.clear();
                        state.rename_cursor = 0;
                        state.rename_select_all = false;
                    } else if let Some((from, ch)) =
                        state.rename_buffer.char_indices().nth(state.rename_cursor)
                    {
                        state
                            .rename_buffer
                            .replace_range(from..from + ch.len_utf8(), "");
                    }
                }
            }
            UiKeyCode::ArrowLeft | UiKeyCode::ArrowRight | UiKeyCode::Home | UiKeyCode::End => {
                if let Some(mut state) = world.get_mut::<CollectionEditorState>(owner) {
                    state.rename_select_all = false;
                    state.rename_cursor = match key {
                        UiKeyCode::ArrowLeft => state.rename_cursor.saturating_sub(1),
                        UiKeyCode::ArrowRight => {
                            (state.rename_cursor + 1).min(state.rename_buffer.chars().count())
                        }
                        UiKeyCode::Home => 0,
                        UiKeyCode::End => state.rename_buffer.chars().count(),
                        _ => state.rename_cursor,
                    };
                }
            }
            _ => {}
        }
        return;
    }
    if key == UiKeyCode::F2 {
        if let Some(id) = collection_active(world, owner) {
            begin_collection_rename(world, owner, id);
        }
        return;
    }
    if (modifiers.control || modifiers.super_key) && key == UiKeyCode::A {
        edit_collection(world, owner, emitted, |world, owner| {
            if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
                let before = list.selection.clone();
                list.selection.selected = list
                    .items
                    .iter()
                    .filter(|item| item.enabled)
                    .map(|item| item.id)
                    .collect();
                list.selection.active = list.selection.selected.iter().next_back().copied();
                list.selection.anchor = list.selection.active;
                before != list.selection
            } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
                let before = tree.selection.clone();
                let visible = tree.visible_ids();
                tree.selection.selected = visible
                    .into_iter()
                    .filter(|id| tree.nodes.iter().any(|node| node.id == *id && node.enabled))
                    .collect();
                tree.selection.active = tree.selection.selected.iter().next_back().copied();
                tree.selection.anchor = tree.selection.active;
                before != tree.selection
            } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
                let before = grid.selection.clone();
                grid.selection.selected = grid
                    .items
                    .iter()
                    .filter(|item| item.enabled)
                    .map(|item| item.id)
                    .collect();
                grid.selection.active = grid.selection.selected.iter().next_back().copied();
                grid.selection.anchor = grid.selection.active;
                before != grid.selection
            } else {
                false
            }
        });
        return;
    }
    match key {
        UiKeyCode::ArrowUp if world.get::<PreviewGrid>(owner).is_some() => {
            let columns = world
                .get::<PreviewGrid>(owner)
                .map_or(1, |grid| grid.columns.max(1)) as i32;
            move_collection_active(world, owner, -columns, modifiers.shift, emitted);
        }
        UiKeyCode::ArrowDown if world.get::<PreviewGrid>(owner).is_some() => {
            let columns = world
                .get::<PreviewGrid>(owner)
                .map_or(1, |grid| grid.columns.max(1)) as i32;
            move_collection_active(world, owner, columns, modifiers.shift, emitted);
        }
        UiKeyCode::ArrowUp => move_collection_active(world, owner, -1, modifiers.shift, emitted),
        UiKeyCode::ArrowDown => move_collection_active(world, owner, 1, modifiers.shift, emitted),
        UiKeyCode::ArrowLeft if world.get::<PreviewGrid>(owner).is_some() => {
            move_collection_active(world, owner, -1, modifiers.shift, emitted)
        }
        UiKeyCode::ArrowRight if world.get::<PreviewGrid>(owner).is_some() => {
            move_collection_active(world, owner, 1, modifiers.shift, emitted)
        }
        UiKeyCode::PageUp | UiKeyCode::PageDown => {
            let amount = collection_page_size(world, owner).max(1) as i32;
            let direction = if key == UiKeyCode::PageUp {
                -amount
            } else {
                amount
            };
            let delta = if let Some(grid) = world.get::<PreviewGrid>(owner) {
                direction * grid.columns.max(1) as i32
            } else {
                direction
            };
            move_collection_active(world, owner, delta, modifiers.shift, emitted);
        }
        UiKeyCode::Home | UiKeyCode::End => {
            let end = key == UiKeyCode::End;
            edit_collection(world, owner, emitted, |world, owner| {
                if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
                    let Some(id) = (if end {
                        list.items.iter().rev().find(|item| item.enabled)
                    } else {
                        list.items.iter().find(|item| item.enabled)
                    })
                    .map(|item| item.id) else {
                        return false;
                    };
                    list.select(id, false, modifiers.shift)
                } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
                    let visible = tree.visible_ids();
                    let id = (if end {
                        visible.iter().rev().find(|id| {
                            tree.nodes
                                .iter()
                                .any(|node| node.id == **id && node.enabled)
                        })
                    } else {
                        visible.iter().find(|id| {
                            tree.nodes
                                .iter()
                                .any(|node| node.id == **id && node.enabled)
                        })
                    })
                    .copied();
                    id.is_some_and(|id| tree.select(id, false, modifiers.shift))
                } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
                    let id = (if end {
                        grid.items.iter().rev().find(|item| item.enabled)
                    } else {
                        grid.items.iter().find(|item| item.enabled)
                    })
                    .map(|item| item.id);
                    id.is_some_and(|id| grid.select(id, false, modifiers.shift))
                } else {
                    false
                }
            });
        }
        UiKeyCode::Enter => {
            if let Some(active) = collection_active(world, owner) {
                if world.get::<TreeModel<String>>(owner).is_some() {
                    edit_collection(world, owner, emitted, |world, owner| {
                        world
                            .get_mut::<TreeModel<String>>(owner)
                            .is_some_and(|mut tree| tree.toggle_expanded(active))
                    });
                } else {
                    emitted.push(WidgetEvent::CollectionItemActivated {
                        target: owner,
                        id: active,
                    });
                }
            }
        }
        UiKeyCode::ArrowRight if world.get::<TreeModel<String>>(owner).is_some() => {
            move_tree_horizontal(world, owner, true, emitted);
        }
        UiKeyCode::ArrowLeft if world.get::<TreeModel<String>>(owner).is_some() => {
            move_tree_horizontal(world, owner, false, emitted);
        }
        UiKeyCode::Escape => cancel_collection_rename(world, owner),
        _ => {}
    }
}

pub(super) fn collection_active(world: &World, owner: Entity) -> Option<u64> {
    world
        .get::<ListModel<String>>(owner)
        .and_then(|model| model.selection.active)
        .or_else(|| {
            world
                .get::<TreeModel<String>>(owner)
                .and_then(|model| model.selection.active)
        })
        .or_else(|| {
            world
                .get::<PreviewGrid>(owner)
                .and_then(|model| model.selection.active)
        })
}

pub(super) fn move_collection_active(
    world: &mut World,
    owner: Entity,
    delta: i32,
    extend: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    edit_collection(world, owner, emitted, |world, owner| {
        if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
            let before = list.selection.clone();
            for _ in 0..delta.unsigned_abs() {
                list.move_active(delta.signum(), extend);
            }
            before != list.selection
        } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
            let before = tree.selection.clone();
            for _ in 0..delta.unsigned_abs() {
                tree.move_active(delta.signum(), extend);
            }
            before != tree.selection
        } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
            let before = grid.selection.clone();
            for _ in 0..delta.unsigned_abs() {
                grid.move_active(delta.signum(), extend);
            }
            before != grid.selection
        } else {
            false
        }
    });
}

pub(super) fn move_tree_horizontal(
    world: &mut World,
    owner: Entity,
    expand: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(active) = collection_active(world, owner) else {
        return;
    };
    edit_collection(world, owner, emitted, |world, owner| {
        let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) else {
            return false;
        };
        let Some(node) = tree.nodes.iter().find(|node| node.id == active).cloned() else {
            return false;
        };
        if expand {
            if !node.children.is_empty() && !node.expanded {
                tree.toggle_expanded(active)
            } else if let Some(child) = node.children.first().copied() {
                tree.select(child, false, false)
            } else {
                false
            }
        } else if node.expanded && !node.children.is_empty() {
            tree.toggle_expanded(active)
        } else if let Some(parent) = node.parent {
            tree.select(parent, false, false)
        } else {
            false
        }
    });
}

pub(super) fn collection_page_size(world: &World, owner: Entity) -> usize {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let height = world.get::<UiLayoutResult>(owner).map_or(240.0, |layout| {
        layout.rect.height - theme.spacing.collection.header_height
    });
    let row_height = collection_row_height(world, owner, &theme).max(1.0);
    (height.max(row_height) / row_height).floor() as usize
}

pub(super) fn collection_row_height(world: &World, owner: Entity, theme: &Theme) -> f32 {
    if let Some(model) = world.get::<ListModel<String>>(owner) {
        if model.row_height == 22.0 {
            theme.spacing.list.row_height
        } else {
            model.row_height
        }
    } else if let Some(model) = world.get::<TreeModel<String>>(owner) {
        if model.row_height == 22 {
            theme.spacing.list.row_height
        } else {
            model.row_height.max(1) as f32
        }
    } else if let Some(model) = world.get::<PreviewGrid>(owner) {
        let height = if model.cell_height == 104.0 {
            theme.spacing.collection.preview_cell_height
        } else {
            model.cell_height
        };
        height + theme.spacing.collection.item_gap
    } else {
        theme.spacing.list.row_height
    }
}

pub(super) fn begin_collection_drag(
    world: &mut World,
    window: Entity,
    target: Entity,
    position: [f32; 2],
) {
    let Some(item) = world.get::<CollectionItemControl>(target).cloned() else {
        return;
    };
    let Some(before) = collection_json(world, item.owner) else {
        return;
    };
    world
        .resource_mut::<WidgetInteractionState>()
        .collection_drags
        .insert(
            window,
            CollectionDrag {
                owner: item.owner,
                id: item.id,
                kind: item.kind,
                source: target,
                start: position,
                before,
                dragging: false,
            },
        );
}

pub(super) fn update_collection_drag(world: &mut World, window: Entity, position: [f32; 2]) {
    if let Some(drag) = world
        .resource_mut::<WidgetInteractionState>()
        .collection_drags
        .get_mut(&window)
        && distance(drag.start, position) >= 5.0
    {
        drag.dragging = true;
    }
}

pub(super) fn collection_item_at_position(
    world: &mut World,
    owner: Entity,
    window: Entity,
    position: [f32; 2],
) -> Option<(CollectionItemControl, Rect)> {
    let mut query = world.query::<(Entity, &CollectionItemControl, &UiHitBox)>();
    query
        .iter(world)
        .filter(|(_, item, hit)| {
            item.owner == owner
                && hit.window.is_none_or(|candidate| candidate == window)
                && hit.contains(position)
        })
        .max_by_key(|(_, _, hit)| hit.z_index)
        .map(|(_, item, hit)| (item.clone(), hit.rect))
}

pub(super) fn finish_collection_drag(
    world: &mut World,
    drag: CollectionDrag,
    position: [f32; 2],
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    if cancelled || !drag.dragging {
        return;
    }
    let window = world
        .get::<UiLayoutResult>(drag.source)
        .map_or(Entity::PLACEHOLDER, |layout| layout.window);
    let Some((drop_target, drop_rect)) =
        collection_item_at_position(world, drag.owner, window, position)
    else {
        return;
    };
    if drop_target.id == drag.id || drop_target.kind != drag.kind {
        return;
    }
    let insert_after = position[1] >= drop_rect.y + drop_rect.height * 0.5;
    let requested_index = drop_target.index + usize::from(insert_after);
    let list_spacing = world
        .get_resource::<Theme>()
        .map_or_else(|| Theme::default().spacing.list, |theme| theme.spacing.list);
    let tree_into_target = drag.kind == CollectionItemKind::Tree
        && position[0]
            > drop_rect.x
                + list_spacing.indent * 2.0
                + drop_target.depth as f32 * list_spacing.indent;
    edit_collection_from_snapshot(world, drag.owner, drag.before, emitted, |world, owner| {
        if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
            let selected = list.select(drag.id, false, false);
            let source_index = list.items.iter().position(|item| item.id == drag.id);
            let target_index = list.items.iter().position(|item| item.id == drop_target.id);
            let requested =
                target_index.map_or(requested_index, |index| index + usize::from(insert_after));
            let destination = match source_index {
                Some(source) if source < requested => requested.saturating_sub(1),
                _ => requested,
            };
            list.reorder(drag.id, destination) || selected
        } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
            let node_parent = tree
                .nodes
                .iter()
                .find(|node| node.id == drop_target.id)
                .and_then(|node| node.parent);
            let destination = if tree_into_target {
                Some(drop_target.id)
            } else {
                node_parent
            };
            let requested = if destination == Some(drop_target.id) {
                tree.nodes
                    .iter()
                    .find(|node| node.id == drop_target.id)
                    .map_or(0, |node| node.children.len())
            } else if let Some(parent) = destination {
                tree.nodes
                    .iter()
                    .find(|node| node.id == parent)
                    .and_then(|node| node.children.iter().position(|id| *id == drop_target.id))
                    .unwrap_or(0)
            } else {
                tree.nodes
                    .iter()
                    .filter(|node| node.parent.is_none())
                    .position(|node| node.id == drop_target.id)
                    .unwrap_or(0)
            };
            let requested = if destination == node_parent && insert_after {
                requested.saturating_add(1)
            } else {
                requested
            };
            let source_sibling_index = if let Some(parent) = destination {
                tree.nodes
                    .iter()
                    .find(|node| node.id == parent)
                    .and_then(|node| node.children.iter().position(|id| *id == drag.id))
            } else {
                tree.nodes
                    .iter()
                    .filter(|node| node.parent.is_none())
                    .position(|node| node.id == drag.id)
            };
            let index = if destination
                == tree
                    .nodes
                    .iter()
                    .find(|node| node.id == drag.id)
                    .and_then(|node| node.parent)
                && source_sibling_index.is_some_and(|source| source < requested)
            {
                requested.saturating_sub(1)
            } else {
                requested
            };
            let selected = tree.select(drag.id, false, false);
            let moved = tree.move_node(drag.id, destination, index);
            if moved
                && tree_into_target
                && let Some(parent) = tree.nodes.iter_mut().find(|node| node.id == drop_target.id)
            {
                parent.expanded = true;
            }
            moved || selected
        } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
            let selected = grid.select(drag.id, false, false);
            let source_index = grid.items.iter().position(|item| item.id == drag.id);
            let target_index = grid.items.iter().position(|item| item.id == drop_target.id);
            let requested =
                target_index.map_or(requested_index, |index| index + usize::from(insert_after));
            let destination = match source_index {
                Some(source) if source < requested => requested.saturating_sub(1),
                _ => requested,
            };
            grid.reorder(drag.id, destination) || selected
        } else {
            false
        }
    });
}

pub(super) fn scroll_collection(
    world: &mut World,
    target: Entity,
    delta_y: f32,
    unit: bui_core::UiWheelUnit,
) {
    if !delta_y.is_finite() || delta_y == 0.0 {
        return;
    }
    let Some(owner) = collection_owner(world, target) else {
        return;
    };
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let row_height = collection_row_height(world, owner, &theme).max(1.0);
    let rows = match unit {
        bui_core::UiWheelUnit::Lines => delta_y.abs().ceil() as usize,
        bui_core::UiWheelUnit::Pixels => (delta_y.abs() / row_height).ceil().max(1.0) as usize,
    };
    if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
        let distance = rows as f32 * row_height;
        list.scroll_offset = if delta_y > 0.0 {
            (list.scroll_offset - distance).max(0.0)
        } else {
            list.scroll_offset + distance
        };
    } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
        tree.scroll_rows = if delta_y > 0.0 {
            tree.scroll_rows.saturating_sub(rows)
        } else {
            tree.scroll_rows.saturating_add(rows)
        };
    } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
        grid.scroll_rows = if delta_y > 0.0 {
            grid.scroll_rows.saturating_sub(rows)
        } else {
            grid.scroll_rows.saturating_add(rows)
        };
    }
}

pub(super) fn scroll_collection_by_rows(world: &mut World, target: Entity, delta: i32) {
    let Some(owner) = collection_owner(world, target) else {
        return;
    };
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let row_height = collection_row_height(world, owner, &theme);
    if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
        list.scroll_offset = (list.scroll_offset + delta as f32 * row_height).max(0.0);
    } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
        tree.scroll_rows = tree.scroll_rows.saturating_add_signed(delta as isize);
    } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
        grid.scroll_rows = grid.scroll_rows.saturating_add_signed(delta as isize);
    }
}
