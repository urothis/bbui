use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetTree {
    pub root: WidgetNode,
}

impl WidgetTree {
    pub fn new(root: WidgetNode) -> Self {
        Self { root }
    }

    /// Spawn every declaration node as a Bevy entity, preserving tree order in `UiChildren`.
    ///
    /// The root is also attached to `parent` when supplied. Number declarations are validated
    /// before any entity is created, so invalid values cannot leave behind a partial tree.
    pub fn spawn(
        &self,
        world: &mut World,
        parent: Option<Entity>,
    ) -> Result<Entity, WidgetTreeError> {
        if let Some(parent) = parent
            && world.get::<UiNode>(parent).is_none()
        {
            return Err(WidgetTreeError::Hierarchy(UiHierarchyError::MissingParent));
        }
        validate_node(&self.root)?;
        spawn_node(world, &self.root, parent)
    }

    /// Reconcile a previously mounted tree in place. Matching keyed children retain their
    /// entities, focus, scroll position, and widget state. Changed declarations update those
    /// controls; removed nodes cancel active interactions before despawning.
    pub fn reconcile(&self, world: &mut World, root: Entity) -> Result<Entity, WidgetTreeError> {
        if world.get::<WidgetElement>(root).is_none() {
            return Err(WidgetTreeError::MissingRoot);
        }
        if world.get::<WidgetElement>(root).is_some_and(|element| {
            std::mem::discriminant(&element.spec) != std::mem::discriminant(&self.root.spec)
        }) {
            return Err(WidgetTreeError::RootTypeChanged);
        }
        preflight_reconcile(world, root, &self.root)?;
        reconcile_node(world, root, &self.root)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WidgetTreeError {
    MissingRoot,
    RootTypeChanged,
    DuplicateKey(String),
    InvalidLayout {
        key: Option<String>,
        message: &'static str,
    },
    InvalidNumberField {
        key: Option<String>,
    },
    InvalidSlider {
        key: Option<String>,
        message: &'static str,
    },
    Hierarchy(UiHierarchyError),
    FileBrowser {
        operation: &'static str,
        path: PathBuf,
        message: String,
    },
    InvalidColorRamp {
        key: Option<String>,
        message: String,
    },
    InvalidHsvColor {
        key: Option<String>,
        message: String,
    },
    InvalidCurveEditor {
        key: Option<String>,
        message: String,
    },
    InvalidCollection {
        key: Option<String>,
        message: String,
    },
    InvalidDataTable {
        key: Option<String>,
        message: String,
    },
    InvalidNodeGraph {
        key: Option<String>,
        message: String,
    },
    InvalidCanvas {
        key: Option<String>,
        message: String,
    },
    InvalidMenu {
        key: Option<String>,
        message: String,
    },
}
