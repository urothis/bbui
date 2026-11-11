//! Interactions common to both properties editor hosts.

use super::*;

fn activate_page(world: &mut World, controls: PropertyControls, selected: usize) {
    for (index, (page, tab)) in controls.pages.into_iter().zip(controls.tabs).enumerate() {
        let active = index == selected;
        if let Some(mut visible) = world.get_mut::<UiVisible>(page)
            && visible.0 != active
        {
            visible.0 = active;
        }
        if let Some(mut role) = world.get_mut::<UiRole>(tab) {
            let expected = tab_role(index, active);
            if role.0 != expected {
                role.0 = expected.into();
            }
        }
        if let Some(mut state) = world.get_mut::<bbui_core::UiTab>(tab) {
            state.selected = active;
        }
    }
}

pub fn process_events(world: &mut World, events: Vec<WidgetEvent>) {
    let Some(controls) = world.get_resource::<PropertyControls>().copied() else {
        return;
    };
    for event in events {
        if let WidgetEvent::Activated { target } = &event {
            let section = world.get::<UiKey>(*target).map(|key| key.0.clone());
            if let Some(index) = TAB_KEYS
                .iter()
                .position(|key| Some(*key) == section.as_deref())
            {
                activate_page(world, controls, index);
                if let Some(mut status) = world.get_mut::<Label>(controls.status) {
                    status.text = format!(
                        "{} · drag values · click tabs to switch pages",
                        TAB_NAMES[index]
                    );
                }
            }
            if section.as_deref() == Some("object-fake")
                && let Some(mut button) = world.get_mut::<IconButton>(*target)
            {
                button.icon = if button.icon == "◇" { "◆" } else { "◇" }.into();
            }
            if section.as_deref() == Some("header-play") {
                let next = if let Some(mut progress) = world.get_mut::<Progress>(controls.progress)
                {
                    let next = if progress.value >= 1.0 {
                        0.25
                    } else {
                        (progress.value + 0.25).min(1.0)
                    };
                    progress.set(next);
                    Some(next)
                } else {
                    None
                };
                if let Some(next) = next
                    && let Some(mut label) = world.get_mut::<Label>(controls.progress_label)
                {
                    label.text = format!("Rendering {}%", (next * 100.0).round() as u32);
                }
            }
            if section.as_deref() == Some("add-modifier")
                && let Some(mut visible) = world.get_mut::<UiVisible>(controls.mirror_modifier)
            {
                visible.0 = !visible.0;
            }
            if section.as_deref() == Some("object-duplicate") {
                let name = world
                    .get::<TextBuffer>(controls.object_name)
                    .map_or("Object".to_owned(), |buffer| buffer.as_str().to_owned());
                if let Some(mut buffer) = world.get_mut::<TextBuffer>(controls.object_name) {
                    buffer.select_all();
                    buffer.insert_text(&format!("{name}.001"));
                }
            }
            if section.as_deref() == Some("object-unlink")
                && let Some(mut buffer) = world.get_mut::<TextBuffer>(controls.object_name)
            {
                buffer.select_all();
                buffer.insert_text("");
            }
        }
        if let WidgetEvent::ValueChanged {
            target,
            after: WidgetValue::Option(Some(index)),
            ..
        } = &event
            && world
                .get::<UiKey>(*target)
                .is_some_and(|key| key.0 == "object-browse")
            && let Some(name) = world
                .get::<Dropdown>(*target)
                .and_then(|dropdown| dropdown.options.get(*index))
                .cloned()
            && let Some(mut buffer) = world.get_mut::<TextBuffer>(controls.object_name)
        {
            buffer.select_all();
            buffer.insert_text(&name);
        }
        if let WidgetEvent::MenuCommand { command_id, .. } = &event {
            let expanded = match command_id.as_str() {
                "find-transform" | "find-visibility" => Some([true, true, false]),
                "find-display" => Some([true, true, true]),
                _ => None,
            };
            if let Some(expanded) = expanded {
                activate_page(world, controls, 0);
                for (panel, open) in [controls.transform, controls.visibility, controls.display]
                    .into_iter()
                    .zip(expanded)
                {
                    if let Some(mut panel) = world.get_mut::<Panel>(panel) {
                        panel.set_expanded(open);
                    }
                }
            }
        }
        info!("properties editor: {event:?}");
    }
}
