//! BBUI-native docking workspace. The saved layout contains stable IDs, never ECS entities.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use bbui_core::history::History;
use bbui_style::{Rgba, ThemeColorToken};
use bbui_widgets::{PaneId, WorkspaceError, WorkspaceLayout, WorkspaceOperation};
use bevy::prelude::*;

mod accessibility;
#[cfg(all(test, target_os = "linux"))]
mod gpu;
mod input;
#[cfg(test)]
mod tests;
mod view;

type PaneBuilder = Arc<dyn Fn(&mut World, Entity) + Send + Sync>;
type CloseGuard = Arc<dyn Fn() -> bool + Send + Sync>;

struct PaneRegistration {
    title: String,
    build: PaneBuilder,
    can_close: CloseGuard,
}

/// Theme-aware workspace chrome. Applications can choose any built-in semantic color token
/// and adjust the chrome dimensions without replacing the docking behavior.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceAppearance {
    pub tab_height: f32,
    pub tab_min_width: f32,
    pub tab_max_width: f32,
    pub close_width: f32,
    pub splitter_thickness: f32,
    pub group_background: ThemeColorToken,
    pub tab_background: ThemeColorToken,
    pub active_tab_background: ThemeColorToken,
    pub border: ThemeColorToken,
    pub drop_preview: ThemeColorToken,
    pub drop_preview_alpha: f32,
}

#[derive(Component, Debug, Clone, PartialEq)]
pub(crate) struct WorkspaceSplitterSemantic {
    pub id: String,
    pub index: usize,
    pub axis: bbui_widgets::SplitAxis,
    pub fraction: f32,
}

impl Default for WorkspaceAppearance {
    fn default() -> Self {
        Self {
            tab_height: 28.0,
            tab_min_width: 84.0,
            tab_max_width: 160.0,
            close_width: 20.0,
            splitter_thickness: 7.0,
            group_background: ThemeColorToken::Background,
            tab_background: ThemeColorToken::EditorChromeSurface,
            active_tab_background: ThemeColorToken::Surface,
            border: ThemeColorToken::Border,
            drop_preview: ThemeColorToken::Accent,
            drop_preview_alpha: 0.28,
        }
    }
}

/// Runtime companion to the serializable [`WorkspaceLayout`]. Register a pane once by its
/// stable ID; closing it unmounts its UI, while moving it retains its live entity.
#[derive(Resource)]
pub struct Workspace {
    layout: WorkspaceLayout,
    appearance: WorkspaceAppearance,
    registrations: HashMap<PaneId, PaneRegistration>,
    history: History<WorkspaceLayout>,
    windows: HashMap<String, Entity>,
    cameras: HashMap<String, Entity>,
    roots: HashMap<String, Entity>,
    live_panes: HashMap<PaneId, Entity>,
    rebuild_panes: HashSet<PaneId>,
    targets: HashMap<Entity, input::WorkspaceTarget>,
    preview: Option<Entity>,
    painted_background: Option<Rgba>,
    dirty: bool,
    next_id: u64,
}

impl Workspace {
    pub fn new(layout: WorkspaceLayout) -> Result<Self, WorkspaceError> {
        layout.validate()?;
        Ok(Self {
            layout,
            appearance: WorkspaceAppearance::default(),
            registrations: HashMap::new(),
            history: History::default(),
            windows: HashMap::new(),
            cameras: HashMap::new(),
            roots: HashMap::new(),
            live_panes: HashMap::new(),
            rebuild_panes: HashSet::new(),
            targets: HashMap::new(),
            preview: None,
            painted_background: None,
            dirty: true,
            next_id: 1,
        })
    }

    pub fn layout(&self) -> &WorkspaceLayout {
        &self.layout
    }

    pub fn appearance(&self) -> &WorkspaceAppearance {
        &self.appearance
    }

    pub fn set_appearance(&mut self, appearance: WorkspaceAppearance) {
        self.appearance = appearance;
        self.dirty = true;
    }

    pub fn register_pane(
        &mut self,
        id: impl Into<PaneId>,
        title: impl Into<String>,
        build: impl Fn(&mut World, Entity) + Send + Sync + 'static,
    ) {
        let id = id.into();
        self.registrations.insert(
            id.clone(),
            PaneRegistration {
                title: title.into(),
                build: Arc::new(build),
                can_close: Arc::new(|| true),
            },
        );
        self.rebuild_panes.insert(id);
        self.dirty = true;
    }

    /// A false result cancels an explicit close request. Native window closure remains final.
    pub fn set_close_guard(
        &mut self,
        id: &PaneId,
        guard: impl Fn() -> bool + Send + Sync + 'static,
    ) -> bool {
        if let Some(registration) = self.registrations.get_mut(id) {
            registration.can_close = Arc::new(guard);
            true
        } else {
            false
        }
    }

    pub fn apply(&mut self, operation: WorkspaceOperation) -> Result<(), WorkspaceError> {
        if let WorkspaceOperation::Close { pane } = &operation
            && self
                .registrations
                .get(pane)
                .is_some_and(|entry| !(entry.can_close)())
        {
            return Err(WorkspaceError::DropRejected);
        }
        let before = self.layout.clone();
        self.layout.apply(operation)?;
        if self.layout != before {
            let _ = self.history.begin("Workspace layout");
            let _ = self
                .history
                .record("workspace", before, self.layout.clone());
            let _ = self.history.commit();
            self.dirty = true;
        }
        Ok(())
    }

    pub fn restore(&mut self, layout: WorkspaceLayout) -> Result<(), WorkspaceError> {
        layout.validate()?;
        self.layout = layout;
        self.history = History::default();
        self.dirty = true;
        Ok(())
    }

    fn cancel_preview(&mut self, before: WorkspaceLayout) {
        self.layout = before;
        self.dirty = true;
    }

    pub fn undo(&mut self) -> bool {
        let Some(changes) = self.history.undo() else {
            return false;
        };
        if let Some((_, layout)) = changes.into_iter().next() {
            self.layout = layout;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        let Some(changes) = self.history.redo() else {
            return false;
        };
        if let Some((_, layout)) = changes.into_iter().next() {
            self.layout = layout;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    fn preview(&mut self, operation: WorkspaceOperation) -> Result<(), WorkspaceError> {
        self.layout.apply(operation)?;
        self.dirty = true;
        Ok(())
    }

    fn finish_preview(&mut self, before: WorkspaceLayout) {
        if before != self.layout {
            let _ = self.history.begin("Resize workspace split");
            let _ = self
                .history
                .record("workspace", before, self.layout.clone());
            let _ = self.history.commit();
        }
    }

    fn next_key(&mut self, prefix: &str) -> String {
        let key = format!("workspace:{prefix}:{}", self.next_id);
        self.next_id += 1;
        key
    }
}

/// Installed by [`crate::BbuiPlugin`]. Does nothing until a [`Workspace`] resource is inserted.
pub struct WorkspacePlugin;

impl Plugin for WorkspacePlugin {
    fn build(&self, app: &mut App) {
        use bevy::ecs::schedule::IntoScheduleConfigs;
        app.init_resource::<input::WorkspaceGesture>()
            .add_systems(PreUpdate, view::sync_workspace)
            .add_systems(
                Update,
                accessibility::route_workspace_accessibility_actions
                    .in_set(bbui_core::UiSystems::Widgets)
                    .before(crate::accessibility::route_accessibility_actions),
            )
            .add_systems(
                Update,
                input::bridge_cross_window_input
                    .in_set(bbui_core::UiSystems::Input)
                    .after(crate::input_bridge::bridge_bevy_input)
                    .before(bbui_core::process_ui_input),
            )
            .add_systems(
                Update,
                input::process_workspace_input
                    .in_set(bbui_core::UiSystems::Widgets)
                    .before(bbui_widgets::actions::process_routed_widget_events),
            );
    }
}
