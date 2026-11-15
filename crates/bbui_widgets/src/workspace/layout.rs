use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceLayout {
    pub version: u32,
    pub root: DockNode,
}

impl WorkspaceLayout {
    pub const VERSION: u32 = 1;
    pub fn new(root: DockNode) -> Self {
        Self {
            version: Self::VERSION,
            root,
        }
    }
    pub fn group(&self, id: &str) -> Option<&TabGroup> {
        self.root.group(id)
    }
    pub fn validate(&self) -> Result<(), WorkspaceError> {
        if self.version != Self::VERSION {
            return Err(WorkspaceError::UnsupportedVersion);
        }
        let mut ids = HashSet::new();
        let mut panes = HashSet::new();
        self.root.validate(&mut ids, &mut panes)
    }
}
