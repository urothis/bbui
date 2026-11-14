use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceWindow {
    pub id: String,
    pub root: DockNode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceLayout {
    pub version: u32,
    pub windows: Vec<WorkspaceWindow>,
}

impl WorkspaceLayout {
    pub const VERSION: u32 = 1;
    pub fn new(main: DockNode) -> Self {
        Self {
            version: Self::VERSION,
            windows: vec![WorkspaceWindow {
                id: "main".into(),
                root: main,
            }],
        }
    }
    pub fn group(&self, id: &str) -> Option<&TabGroup> {
        self.windows.iter().find_map(|window| window.root.group(id))
    }
    pub fn validate(&self) -> Result<(), WorkspaceError> {
        if self.version != Self::VERSION {
            return Err(WorkspaceError::UnsupportedVersion);
        }
        if self.windows.is_empty() {
            return Err(WorkspaceError::InvalidContainer);
        }
        let mut ids = HashSet::new();
        let mut panes = HashSet::new();
        for window in &self.windows {
            if window.id.is_empty() || !ids.insert(window.id.clone()) {
                return Err(WorkspaceError::DuplicateOrEmptyId);
            }
            window.root.validate(&mut ids, &mut panes)?;
        }
        Ok(())
    }
}
