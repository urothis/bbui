use super::*;

pub(super) fn node_for(world: &World, entity: Entity) -> Option<Node> {
    let label = world.get::<Label>(entity).map(|label| label.text.clone());
    if world.get::<UiTabList>(entity).is_some() {
        return Some(Node::new(Role::TabList));
    }
    if let Some(splitter) = world.get::<WorkspaceSplitterSemantic>(entity) {
        let mut node = Node::new(Role::Splitter);
        node.set_label("Resize workspace split");
        node.set_numeric_value(f64::from(splitter.fraction) * 100.0);
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(100.0);
        node.set_numeric_value_step(5.0);
        node.set_orientation(if splitter.axis == bbui_widgets::SplitAxis::Horizontal {
            accesskit::Orientation::Vertical
        } else {
            accesskit::Orientation::Horizontal
        });
        node.add_action(Action::Focus);
        node.add_action(Action::Increment);
        node.add_action(Action::Decrement);
        node.add_action(Action::SetValue);
        return Some(node);
    }
    if let Some(option) = world.get::<RadioOptionControl>(entity) {
        let mut node = Node::new(Role::RadioButton);
        node.set_label(option.label.clone());
        node.set_toggled(if option.selected {
            Toggled::True
        } else {
            Toggled::False
        });
        node.set_position_in_set(option.index.saturating_add(1));
        if !option.enabled {
            node.set_disabled();
        } else {
            node.add_action(Action::Click);
            node.add_action(Action::Focus);
        }
        return Some(node);
    }
    if let Some(option) = world.get::<DropdownOptionControl>(entity) {
        let mut node = Node::new(Role::ListBoxOption);
        node.set_label(option.label.clone());
        node.set_selected(option.selected);
        node.set_position_in_set(option.index.saturating_add(1));
        if !option.enabled {
            node.set_disabled();
        } else {
            node.add_action(Action::Click);
            node.add_action(Action::Focus);
        }
        return Some(node);
    }
    if let Some(header) = world.get::<DataTableHeaderControl>(entity) {
        let mut node = Node::new(Role::ColumnHeader);
        node.set_label(header.label.clone());
        node.set_column_index(header.column);
        node.set_numeric_value(f64::from(header.width));
        node.set_numeric_value_step(8.0);
        if let Some(ascending) = header.sort_ascending {
            node.set_sort_direction(if ascending {
                SortDirection::Ascending
            } else {
                SortDirection::Descending
            });
        }
        if header.sortable {
            node.add_action(Action::Click);
        }
        node.add_action(Action::Increment);
        node.add_action(Action::Decrement);
        node.add_action(Action::SetValue);
        return Some(node);
    }
    if let Some(cell) = world.get::<DataTableCellControl>(entity) {
        let mut node = Node::new(Role::GridCell);
        node.set_label(format!("{}, {}", cell.column_label, cell.value));
        node.set_value(cell.value.clone());
        node.set_row_index(cell.row_index);
        node.set_column_index(cell.column);
        node.set_selected(cell.selected);
        node.add_action(Action::Click);
        return Some(node);
    }
    if let Some(table) = world.get::<DataTable>(entity) {
        let mut node = Node::new(Role::Grid);
        node.set_label(label.unwrap_or_else(|| "Data table".to_owned()));
        node.set_row_count(table.rows.len());
        node.set_column_count(table.columns.len());
        if table.multi_select {
            node.set_multiselectable();
        }
        node.set_value(format!("{} rows selected", table.selection.selected.len()));
        return Some(node);
    }
    if let Some(control) = world.get::<CanvasObjectControl>(entity) {
        let mut node = Node::new(Role::GraphicsObject);
        node.set_label(control.label.clone());
        node.set_selected(control.selected);
        if control.locked {
            node.set_disabled();
        } else {
            node.add_action(Action::Click);
        }
        return Some(node);
    }
    if let Some(canvas) = world.get::<Canvas>(entity) {
        let mut node = Node::new(Role::Canvas);
        node.set_label(label.unwrap_or_else(|| "Canvas".to_owned()));
        node.set_value(format!(
            "{} objects, {} selected",
            canvas.objects.len(),
            canvas.selected.len()
        ));
        node.add_action(Action::Focus);
        return Some(node);
    }
    if let Some(control) = world.get::<NodeGraphNodeControl>(entity) {
        let mut node = Node::new(Role::Group);
        node.set_label(control.label.clone());
        node.set_selected(control.selected);
        node.set_expanded(!control.collapsed);
        node.add_action(Action::Click);
        node.add_action(if control.collapsed {
            Action::Expand
        } else {
            Action::Collapse
        });
        return Some(node);
    }
    if let Some(control) = world.get::<NodeGraphSocketControl>(entity) {
        let mut node = Node::new(Role::Button);
        node.set_label(format!(
            "{} {:?} socket, {:?}",
            control.label, control.direction, control.socket_type
        ));
        let selected = world
            .get_resource::<AccessibilityLinkSelection>()
            .and_then(|selection| selection.0.as_ref())
            .is_some_and(|(selected, _)| *selected == entity);
        if selected {
            node.set_selected(true);
            node.set_description(
                "Socket selected; activate an opposite-direction socket to create a link",
            );
            node.set_live(Live::Polite);
            node.set_live_atomic();
        } else {
            node.set_description(
                "Activate an output socket, then an input socket to create a link",
            );
        }
        node.add_action(Action::Click);
        node.add_action(Action::Focus);
        return Some(node);
    }
    if let Some(control) = world.get::<NodeGraphLinkControl>(entity) {
        let mut node = Node::new(Role::Button);
        node.set_label(format!(
            "Link from node {} socket {} to node {} socket {}",
            control.from_node.0,
            control.from_socket + 1,
            control.to_node.0,
            control.to_socket + 1
        ));
        node.set_selected(control.selected);
        node.add_action(Action::Click);
        return Some(node);
    }
    if let Some(graph) = world.get::<NodeGraph>(entity) {
        let mut node = Node::new(Role::GraphicsDocument);
        node.set_label(label.unwrap_or_else(|| "Node editor".to_owned()));
        node.set_value(format!(
            "{} nodes, {} links, {} selected",
            graph.nodes.len(),
            graph.links.len(),
            graph.selected.len()
        ));
        node.add_action(Action::Focus);
        return Some(node);
    }
    if let Some(item) = world.get::<CollectionItemControl>(entity) {
        let mut node = Node::new(match item.kind {
            CollectionItemKind::List => Role::ListBoxOption,
            CollectionItemKind::Tree => Role::TreeItem,
            CollectionItemKind::PreviewGrid => Role::GridCell,
        });
        node.set_label(item.label.clone());
        node.set_position_in_set(item.index.saturating_add(1));
        node.set_selected(item.selected);
        if !item.enabled {
            node.set_disabled();
        } else {
            node.add_action(Action::Click);
        }
        if item.kind == CollectionItemKind::Tree {
            node.set_level(item.depth.saturating_add(1));
            if let Some(tree) = world.get::<TreeModel<String>>(item.owner)
                && let Some(current) = tree.nodes.iter().find(|node| node.id == item.id)
            {
                let siblings: Vec<_> = if let Some(parent) = current.parent {
                    tree.nodes
                        .iter()
                        .find(|node| node.id == parent)
                        .map(|node| node.children.clone())
                        .unwrap_or_default()
                } else {
                    tree.nodes
                        .iter()
                        .filter(|node| node.parent.is_none())
                        .map(|node| node.id)
                        .collect()
                };
                if let Some(position) = siblings.iter().position(|id| *id == item.id) {
                    node.set_position_in_set(position + 1);
                    node.set_size_of_set(siblings.len());
                }
            }
            if item.has_children {
                node.set_expanded(item.expanded);
                if item.enabled {
                    node.add_action(if item.expanded {
                        Action::Collapse
                    } else {
                        Action::Expand
                    });
                }
            }
        }
        return Some(node);
    }
    if let Some(list) = world.get::<ListModel<String>>(entity) {
        let mut node = Node::new(Role::ListBox);
        node.set_label(label.unwrap_or_else(|| "List".to_owned()));
        node.set_size_of_set(list.items.len());
        if list.multi_select {
            node.set_multiselectable();
        }
        node.set_value(format!("{} selected", list.selection.selected.len()));
        return Some(node);
    }
    if let Some(tree) = world.get::<TreeModel<String>>(entity) {
        let mut node = Node::new(Role::Tree);
        node.set_label(label.unwrap_or_else(|| "Tree".to_owned()));
        node.set_size_of_set(tree.nodes.len());
        node.set_value(format!("{} selected", tree.selection.selected.len()));
        return Some(node);
    }
    if let Some(grid) = world.get::<PreviewGrid>(entity) {
        let mut node = Node::new(Role::Grid);
        node.set_label(label.unwrap_or_else(|| "Preview grid".to_owned()));
        node.set_size_of_set(grid.items.len());
        if grid.multi_select {
            node.set_multiselectable();
        }
        node.set_value(format!("{} selected", grid.selection.selected.len()));
        return Some(node);
    }
    if let Some(browser) = world.get::<FileBrowser>(entity) {
        let mut node = Node::new(Role::ListBox);
        node.set_label(format!("Files in {}", browser.directory.display()));
        node.set_value(format!("{} selected", browser.selected.len()));
        node.set_size_of_set(browser.visible_count());
        node.set_multiselectable();
        return Some(node);
    }
    if let Some(item) = world.get::<FileBrowserItem>(entity) {
        let mut node = Node::new(Role::ListBoxOption);
        node.set_label(if item.directory {
            format!("{}, folder", item.name)
        } else {
            item.name.clone()
        });
        node.set_position_in_set(item.index.saturating_add(1));
        if item.selected {
            node.set_selected(true);
        }
        node.add_action(Action::Click);
        return Some(node);
    }
    if let Some(menu) = world.get::<MenuControl>(entity) {
        let mut node = Node::new(Role::Button);
        node.set_label(menu.label.clone());
        node.set_has_popup(HasPopup::Menu);
        node.set_expanded(menu.session.open);
        node.add_action(Action::Click);
        node.add_action(Action::ShowContextMenu);
        return Some(node);
    }
    if let Some(item) = world.get::<MenuItemControl>(entity) {
        let mut node = Node::new(if item.separator {
            Role::Splitter
        } else {
            Role::MenuItem
        });
        node.set_label(item.label.clone());
        if let Some(shortcut) = &item.shortcut {
            node.set_description(format!("Shortcut {shortcut}"));
        }
        if let Some(owner) = world.get::<MenuControl>(item.owner) {
            let mut path = item.parent_path.clone();
            path.push(item.index);
            node.set_selected(owner.session.open && owner.session.path == path);
        }
        if item.has_submenu && !item.separator {
            node.set_has_popup(HasPopup::Menu);
            if let Some(owner) = world.get::<MenuControl>(item.owner) {
                let mut path = item.parent_path.clone();
                path.push(item.index);
                node.set_expanded(
                    owner.session.open
                        && owner.session.path.starts_with(&path)
                        && owner.session.path.len() > path.len(),
                );
            }
        }
        if !item.enabled {
            node.set_disabled();
        }
        if item.enabled && !item.separator {
            node.add_action(Action::Click);
        }
        return Some(node);
    }
    if let Some(ramp) = world.get::<ColorRamp>(entity) {
        let mut node = Node::new(Role::Slider);
        node.set_label(label.unwrap_or_else(|| "Color ramp".to_owned()));
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(1.0);
        node.set_numeric_value_step(0.01);
        if let Some(stop) = ramp.selected_stop() {
            node.set_numeric_value(f64::from(stop.position));
            node.set_value(format!(
                "Stop at {:.0}%, RGBA {:.2} {:.2} {:.2} {:.2}, {:?} interpolation",
                stop.position * 100.0,
                stop.color.0,
                stop.color.1,
                stop.color.2,
                stop.color.3,
                ramp.interpolation
            ));
        }
        node.add_action(Action::Increment);
        node.add_action(Action::Decrement);
        node.add_action(Action::SetValue);
        return Some(node);
    }
    if let Some(editor) = world.get::<HsvColorEditor>(entity) {
        let mut node = Node::new(Role::ColorWell);
        node.set_label(label.unwrap_or_else(|| "Color".to_owned()));
        let color = editor.color();
        let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
        node.set_color_value(AccessibleColor {
            red: byte(color.0),
            green: byte(color.1),
            blue: byte(color.2),
            alpha: byte(color.3),
        });
        node.set_value(editor.hex());
        node.add_action(Action::SetValue);
        node.add_action(Action::Increment);
        node.add_action(Action::Decrement);
        return Some(node);
    }
    if let Some(editor) = world.get::<CurveEditor>(entity) {
        let mut node = Node::new(Role::Slider);
        node.set_label(label.unwrap_or_else(|| "Curve editor point".to_owned()));
        node.set_numeric_value_step(0.01);
        if let Some((index, point)) = editor
            .points
            .iter()
            .enumerate()
            .find(|(_, point)| Some(point.id) == editor.selected)
        {
            node.set_numeric_value(f64::from(point.position[1]));
            node.set_value(format!(
                "Point {} of {}, x {:.3}, y {:.3}, {:?} interpolation",
                index + 1,
                editor.points.len(),
                point.position[0],
                point.position[1],
                editor.interpolation
            ));
            node.add_action(Action::Increment);
            node.add_action(Action::Decrement);
            node.add_action(Action::SetValue);
        } else {
            node.set_value(format!(
                "{} points, {:?} interpolation; select a point to edit",
                editor.points.len(),
                editor.interpolation
            ));
        }
        return Some(node);
    }
    if let Some(checkbox) = world.get::<Checkbox>(entity) {
        let mut node = Node::new(Role::CheckBox);
        node.set_label(checkbox.label.clone());
        node.set_toggled(match checkbox.value {
            CheckboxValue::Off => Toggled::False,
            CheckboxValue::On => Toggled::True,
            CheckboxValue::Mixed => Toggled::Mixed,
        });
        if !checkbox.enabled {
            node.set_disabled();
        }
        if checkbox.read_only {
            node.set_read_only();
        }
        if checkbox.enabled && !checkbox.read_only {
            node.add_action(Action::Click);
        }
        return Some(node);
    }
    if let Some(slider) = world.get::<Slider>(entity) {
        let mut node = Node::new(Role::Slider);
        node.set_label(label.unwrap_or_else(|| "Slider".to_owned()));
        node.set_numeric_value(slider.value);
        node.set_value(slider.display());
        let [min, max] = slider.range();
        node.set_min_numeric_value(min);
        node.set_max_numeric_value(max);
        node.set_numeric_value_step(slider.step);
        if let Some(error) = &slider.expression_error {
            node.set_description(error.to_string());
            node.set_live(Live::Assertive);
            node.set_live_atomic();
        }
        if !slider.enabled {
            node.set_disabled();
        }
        if slider.read_only {
            node.set_read_only();
        }
        if slider.enabled && !slider.read_only {
            node.add_action(Action::Increment);
            node.add_action(Action::Decrement);
            node.add_action(Action::SetValue);
        }
        return Some(node);
    }
    if let Some(number) = world.get::<NumericField>(entity) {
        let mut node = Node::new(Role::SpinButton);
        if let Some(label) = label {
            node.set_label(label);
        }
        node.set_numeric_value(number.value);
        node.set_value(number.display());
        if let Some(min) = number.min {
            node.set_min_numeric_value(min);
        }
        if let Some(max) = number.max {
            node.set_max_numeric_value(max);
        }
        node.set_numeric_value_step(number.step);
        if let Some(error) = &number.expression_error {
            node.set_description(error.to_string());
            node.set_live(Live::Assertive);
            node.set_live_atomic();
        }
        if !number.enabled {
            node.set_disabled();
        }
        if number.read_only {
            node.set_read_only();
        }
        if number.enabled && !number.read_only {
            node.add_action(Action::Increment);
            node.add_action(Action::Decrement);
            node.add_action(Action::SetValue);
        }
        return Some(node);
    }
    if let Some(field) = world.get::<TextField>(entity) {
        let mut node = Node::new(Role::TextInput);
        let read_only = field.read_only || world.get::<UiReadOnly>(entity).is_some();
        if let Some(label) = label {
            node.set_label(label);
        }
        node.set_value(field.text.clone());
        if !field.placeholder.is_empty() {
            node.set_placeholder(field.placeholder.clone());
        }
        if !field.enabled {
            node.set_disabled();
        }
        if read_only {
            node.set_read_only();
        }
        if field.enabled && !read_only {
            node.add_action(Action::ReplaceSelectedText);
        }
        return Some(node);
    }
    if let Some(area) = world.get::<TextArea>(entity) {
        let mut node = Node::new(Role::MultilineTextInput);
        let read_only = area.field.read_only || world.get::<UiReadOnly>(entity).is_some();
        if let Some(label) = label {
            node.set_label(label);
        }
        node.set_value(area.field.text.clone());
        if !area.field.enabled {
            node.set_disabled();
        }
        if read_only {
            node.set_read_only();
        }
        if area.field.enabled && !read_only {
            node.add_action(Action::ReplaceSelectedText);
        }
        return Some(node);
    }
    if let Some(dropdown) = world.get::<Dropdown>(entity) {
        let mut node = Node::new(Role::ComboBox);
        if let Some(label) = label {
            node.set_label(label);
        }
        if let Some(value) = dropdown
            .selected
            .and_then(|index| dropdown.options.get(index))
        {
            node.set_value(value.clone());
        }
        if !dropdown.enabled {
            node.set_disabled();
        }
        node.set_has_popup(HasPopup::Listbox);
        node.set_size_of_set(dropdown.options.len());
        node.set_expanded(dropdown.open);
        if dropdown.enabled {
            node.add_action(Action::Click);
        }
        return Some(node);
    }
    if let Some(radio) = world.get::<RadioGroup>(entity) {
        let mut node = Node::new(Role::RadioGroup);
        node.set_size_of_set(radio.options.len());
        if let Some(label) = label {
            node.set_label(label);
        }
        if let Some(value) = radio.selected.and_then(|index| radio.options.get(index)) {
            node.set_value(value.label.clone());
        }
        if !radio.enabled {
            node.set_disabled();
        }
        if radio.enabled {
            node.add_action(Action::Increment);
            node.add_action(Action::Decrement);
        }
        return Some(node);
    }
    if let Some(button) = world.get::<Button>(entity) {
        let tab = world.get::<UiTab>(entity);
        let mut node = Node::new(if tab.is_some() {
            Role::Tab
        } else {
            Role::Button
        });
        node.set_label(button.label.clone());
        if let Some(tab) = tab {
            node.set_selected(tab.selected);
        }
        if !button.enabled {
            node.set_disabled();
        }
        if button.enabled {
            node.add_action(Action::Click);
        }
        return Some(node);
    }
    if let Some(button) = world.get::<IconButton>(entity) {
        let tab = world.get::<UiTab>(entity);
        let mut node = Node::new(if tab.is_some() {
            Role::Tab
        } else {
            Role::Button
        });
        node.set_label(button.accessible_name.clone());
        if let Some(tab) = tab {
            node.set_selected(tab.selected);
        }
        if let Some(state) = world.get::<IconButtonAssetState>(entity) {
            match &state.status {
                IconButtonAssetStatus::Loading => node.set_description("Icon loading"),
                IconButtonAssetStatus::Failed(reason) => {
                    node.set_description(format!("Icon failed to load: {reason}"));
                    node.set_live(Live::Assertive);
                    node.set_live_atomic();
                }
                IconButtonAssetStatus::Hidden | IconButtonAssetStatus::Ready => {}
            }
        }
        if !button.enabled {
            node.set_disabled();
        }
        if button.enabled {
            node.add_action(Action::Click);
        }
        return Some(node);
    }
    if let Some(panel) = world.get::<Panel>(entity) {
        let mut node = Node::new(Role::Group);
        node.set_label(panel.title.clone());
        node.set_expanded(panel.state.target_expanded);
        if !panel.enabled {
            node.set_disabled();
        }
        if panel.enabled {
            node.add_action(if panel.state.target_expanded {
                Action::Collapse
            } else {
                Action::Expand
            });
            if world
                .get::<UiParent>(entity)
                .and_then(|parent| world.get::<PanelGroup>(parent.0))
                .is_some_and(|group| group.reorderable)
            {
                node.add_action(Action::Increment);
                node.add_action(Action::Decrement);
            }
        }
        return Some(node);
    }
    if let Some(progress) = world.get::<Progress>(entity) {
        let mut node = Node::new(Role::ProgressIndicator);
        node.set_label(label.unwrap_or_else(|| "Progress".to_owned()));
        node.set_numeric_value(f64::from(progress.value));
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(1.0);
        return Some(node);
    }
    if let Some(label) = world.get::<Label>(entity) {
        let mut node = Node::new(Role::Label);
        node.set_value(label.text.clone());
        return Some(node);
    }
    if let Some(tooltip) = world.get::<Tooltip>(entity) {
        let mut node = Node::new(Role::Tooltip);
        node.set_label(tooltip.text.clone());
        if !tooltip.visible {
            node.set_hidden();
        }
        return Some(node);
    }
    if world.get::<UiChildren>(entity).is_some() {
        return Some(Node::new(Role::Group));
    }
    None
}

pub(super) fn visible_in_hierarchy(world: &World, entity: Entity) -> bool {
    let mut current = Some(entity);
    let mut depth = 0;
    while let Some(at) = current {
        if world.get::<UiVisible>(at).is_some_and(|visible| !visible.0) {
            return false;
        }
        if world.get::<UiLayoutResult>(at).is_some_and(|layout| {
            layout
                .clip
                .is_some_and(|clip| layout.rect.intersection(clip).is_none())
        }) || world.get::<UiHitBox>(at).is_some_and(|hit| {
            hit.clip
                .is_some_and(|clip| hit.rect.intersection(clip).is_none())
        }) {
            return false;
        }
        current = world.get::<UiParent>(at).map(|parent| parent.0);
        depth += 1;
        if depth > 1024 {
            return false;
        }
    }
    true
}

pub(super) fn enabled_in_hierarchy(world: &World, entity: Entity) -> bool {
    let mut current = Some(entity);
    let mut depth = 0;
    while let Some(at) = current {
        if world.get::<UiEnabled>(at).is_some_and(|enabled| !enabled.0) {
            return false;
        }
        current = world.get::<UiParent>(at).map(|parent| parent.0);
        depth += 1;
        if depth > 1024 {
            return false;
        }
    }
    true
}
