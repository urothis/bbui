use super::*;
use std::collections::{HashMap, HashSet};

/// Validate only new or changed controls; unchanged file-browser and graph declarations
/// must not rescan the filesystem or serialize large models during an idle reconciliation.
pub(super) fn preflight_reconcile(
    world: &World,
    entity: Entity,
    node: &WidgetNode,
) -> Result<(), WidgetTreeError> {
    validate_sibling_keys(node)?;
    let previous = world
        .get::<WidgetElement>(entity)
        .ok_or(WidgetTreeError::MissingRoot)?;
    if previous.spec != node.spec {
        validate_spec(node)?;
        if let WidgetSpec::FileBrowser { directory } = &node.spec
            && let Some(root) = world
                .get::<FileBrowser>(entity)
                .and_then(FileBrowser::confinement_root)
        {
            let mut browser =
                FileBrowser::new(directory).map_err(|error| WidgetTreeError::FileBrowser {
                    operation: error.operation,
                    path: error.path,
                    message: error.source.to_string(),
                })?;
            browser
                .confine_to(root)
                .map_err(|error| WidgetTreeError::FileBrowser {
                    operation: error.operation,
                    path: error.path,
                    message: error.source.to_string(),
                })?;
        }
    }
    let old_children = world
        .get::<UiChildren>(entity)
        .map_or_else(Vec::new, |children| {
            children
                .0
                .iter()
                .copied()
                .filter(|child| world.get::<WidgetElement>(*child).is_some())
                .collect()
        });
    let keyed: HashMap<_, _> = old_children
        .iter()
        .filter_map(|child| {
            world
                .get::<UiKey>(*child)
                .map(|key| (key.0.as_str(), *child))
        })
        .collect();
    for (index, declaration) in node.children.iter().enumerate() {
        let existing = declaration
            .key
            .as_deref()
            .and_then(|key| keyed.get(key).copied())
            .or_else(|| {
                declaration
                    .key
                    .is_none()
                    .then(|| old_children.get(index).copied())
                    .flatten()
                    .filter(|child| world.get::<UiKey>(*child).is_none())
            });
        if let Some(child) = existing {
            preflight_reconcile(world, child, declaration)?;
        } else {
            validate_node(declaration)?;
        }
    }
    Ok(())
}

pub(super) fn reconcile_node(
    world: &mut World,
    entity: Entity,
    node: &WidgetNode,
) -> Result<Entity, WidgetTreeError> {
    let Some(previous) = world.get::<WidgetElement>(entity).cloned() else {
        return Err(WidgetTreeError::MissingRoot);
    };
    if std::mem::discriminant(&previous.spec) != std::mem::discriminant(&node.spec) {
        let parent = world
            .get::<bbui_core::UiParent>(entity)
            .map(|parent| parent.0);
        let replacement = spawn_node(world, node, parent)?;
        remove_subtree(world, entity);
        return Ok(replacement);
    }

    let props = WidgetProps::from(node);
    let old_props = world.get::<WidgetProps>(entity).cloned();
    let spec_changed = previous.spec != node.spec;
    if previous.key != node.key || spec_changed {
        if spec_changed {
            // A changed declaration is an explicit replacement of the control's value and
            // configuration. An unchanged declaration leaves live interaction state untouched.
            if matches!(previous.spec, WidgetSpec::Menu { .. }) {
                let virtual_children =
                    world
                        .get::<UiChildren>(entity)
                        .map_or_else(Vec::new, |children| {
                            children
                                .0
                                .iter()
                                .copied()
                                .filter(|child| world.get::<MenuItemControl>(*child).is_some())
                                .collect()
                        });
                for child in virtual_children {
                    remove_subtree(world, child);
                }
            }
            insert_control(world, entity, &node.spec)?;
            if matches!(
                node.spec,
                WidgetSpec::Container {
                    kind: WidgetContainer::MenuBar,
                    ..
                }
            ) {
                world.entity_mut(entity).insert(UiRole::new("menu-bar"));
            } else if matches!(
                previous.spec,
                WidgetSpec::Container {
                    kind: WidgetContainer::MenuBar,
                    ..
                }
            ) && node.role.is_none()
            {
                world.entity_mut(entity).remove::<UiRole>();
            }
            if let Some(role) = &node.role {
                world.entity_mut(entity).insert(UiRole::new(role.clone()));
            }
            if let WidgetSpec::Menu { tree, .. } = &node.spec {
                spawn_menu_items(world, entity, tree);
            }
        }
        world.entity_mut(entity).insert(WidgetElement {
            key: node.key.clone(),
            spec: node.spec.clone(),
        });
        match &node.key {
            Some(key) => {
                world.entity_mut(entity).insert(UiKey(key.clone()));
            }
            None => {
                world.entity_mut(entity).remove::<UiKey>();
            }
        }
    }
    if old_props.as_ref() != Some(&props) || (spec_changed && props.layout.is_none()) {
        let mut layout = props.layout.unwrap_or_else(|| layout_for(&node.spec));
        if props.scrollable {
            layout.clip_contents = true;
        }
        if old_props
            .as_ref()
            .is_none_or(|old| old.layout != props.layout || old.scrollable != props.scrollable)
            || (spec_changed && props.layout.is_none())
        {
            world.entity_mut(entity).insert(layout);
        }
        if old_props
            .as_ref()
            .is_none_or(|old| old.style != props.style)
        {
            match &props.style {
                Some(style) => {
                    world.entity_mut(entity).insert(style.clone());
                }
                None => {
                    world.entity_mut(entity).remove::<UiInlineStyle>();
                }
            }
        }
        if old_props
            .as_ref()
            .is_none_or(|old| old.accessible_name != props.accessible_name)
        {
            match &props.accessible_name {
                Some(name) => {
                    world
                        .entity_mut(entity)
                        .insert(UiAccessibleName(name.clone()));
                }
                None => {
                    world.entity_mut(entity).remove::<UiAccessibleName>();
                }
            }
        }
        if old_props.as_ref().is_none_or(|old| old.role != props.role) {
            match &props.role {
                Some(role) => {
                    world.entity_mut(entity).insert(UiRole::new(role.clone()));
                }
                None => {
                    world.entity_mut(entity).remove::<UiRole>();
                }
            }
        }
        if old_props
            .as_ref()
            .is_none_or(|old| old.tab_selected != props.tab_selected)
        {
            match props.tab_selected {
                Some(selected) => {
                    world.entity_mut(entity).insert(UiTab { selected });
                }
                None => {
                    world.entity_mut(entity).remove::<UiTab>();
                }
            }
        }
        if old_props
            .as_ref()
            .is_none_or(|old| old.tab_list != props.tab_list)
        {
            if props.tab_list {
                world.entity_mut(entity).insert(UiTabList);
            } else {
                world.entity_mut(entity).remove::<UiTabList>();
            }
        }
        if old_props
            .as_ref()
            .is_none_or(|old| old.scrollable != props.scrollable)
        {
            if props.scrollable {
                world.entity_mut(entity).insert(UiScroll::new());
            } else {
                world.entity_mut(entity).remove::<UiScroll>();
            }
        }
        world.entity_mut(entity).insert(props);
    }

    let old_children = world
        .get::<UiChildren>(entity)
        .map_or_else(Vec::new, |children| {
            children
                .0
                .iter()
                .copied()
                .filter(|child| world.get::<WidgetElement>(*child).is_some())
                .collect()
        });
    let mut keyed = HashMap::new();
    for child in &old_children {
        if let Some(key) = world.get::<UiKey>(*child) {
            keyed.insert(key.0.clone(), *child);
        }
    }
    let mut used = HashSet::new();
    let mut ordered = Vec::with_capacity(node.children.len());
    for (index, declaration) in node.children.iter().enumerate() {
        let existing = declaration
            .key
            .as_ref()
            .and_then(|key| keyed.get(key).copied())
            .or_else(|| {
                declaration
                    .key
                    .is_none()
                    .then(|| old_children.get(index).copied())
                    .flatten()
                    .filter(|child| world.get::<UiKey>(*child).is_none())
            })
            .filter(|child| used.insert(*child));
        let child = match existing {
            Some(child) => reconcile_node(world, child, declaration)?,
            None => spawn_node(world, declaration, Some(entity))?,
        };
        if world
            .get::<Panel>(entity)
            .is_some_and(|panel| !panel.state.expanded)
            && world
                .get::<UiVisible>(child)
                .is_some_and(|visible| visible.0)
        {
            world.entity_mut(child).insert(UiVisible(false));
        }
        ordered.push(child);
    }
    for old in old_children {
        if !ordered.contains(&old) {
            remove_subtree(world, old);
        }
    }
    let virtual_children = world
        .get::<UiChildren>(entity)
        .map_or_else(Vec::new, |children| {
            children
                .0
                .iter()
                .copied()
                .filter(|child| world.get::<WidgetElement>(*child).is_none())
                .collect()
        });
    ordered.extend(virtual_children);
    if world
        .get::<UiChildren>(entity)
        .is_none_or(|children| children.0 != ordered)
    {
        world.entity_mut(entity).insert(UiChildren(ordered));
    }
    Ok(entity)
}

fn remove_subtree(world: &mut World, entity: Entity) {
    let children = world
        .get::<UiChildren>(entity)
        .map_or_else(Vec::new, |children| children.0.clone());
    for child in children {
        remove_subtree(world, child);
    }
    if world.get_entity(entity).is_ok() {
        cleanup_removed_node(world, entity);
        world.despawn(entity);
    }
}
