use super::{NodeCategory, NodeModel, NodeSocket, SocketType};
use crate::WidgetNode;
use crate::actions::WidgetValue;
use crate::declarative::WidgetSpec;
use crate::{NumericFieldKind, WidgetEvent};
use bbui_style::Rgba;
use bevy_ecs::prelude::{Resource, World};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

type NodeBodyFactory = Arc<dyn Fn(&NodeModel) -> WidgetNode + Send + Sync>;
type NodeBodyEventHandler =
    Arc<dyn Fn(&World, &WidgetEvent, &NodeModel) -> Option<serde_json::Value> + Send + Sync>;
type NodeMigration = Arc<dyn Fn(&mut NodeModel) -> Result<(), String> + Send + Sync>;
type SocketEditorFactory = Arc<dyn Fn(&NodeSocket) -> WidgetNode + Send + Sync>;
type SocketEditorReader =
    Arc<dyn Fn(&World, &WidgetEvent) -> Option<serde_json::Value> + Send + Sync>;

#[derive(Clone)]
pub struct SocketEditorDefinition {
    build: SocketEditorFactory,
    read: SocketEditorReader,
}

impl SocketEditorDefinition {
    pub fn new(
        build: impl Fn(&NodeSocket) -> WidgetNode + Send + Sync + 'static,
        read: impl Fn(&World, &WidgetEvent) -> Option<serde_json::Value> + Send + Sync + 'static,
    ) -> Self {
        Self {
            build: Arc::new(build),
            read: Arc::new(read),
        }
    }

    pub fn build(&self, socket: &NodeSocket) -> WidgetNode {
        (self.build)(socket)
    }
    pub fn read(&self, world: &World, event: &WidgetEvent) -> Option<serde_json::Value> {
        (self.read)(world, event)
    }
}

#[derive(Clone)]
pub struct NodeTypeDefinition {
    pub id: String,
    pub version: u32,
    pub category: NodeCategory,
    pub sockets: Vec<NodeSocket>,
    body: NodeBodyFactory,
    on_event: Option<NodeBodyEventHandler>,
    migrations: HashMap<u32, NodeMigration>,
}

impl NodeTypeDefinition {
    pub fn new(
        id: impl Into<String>,
        version: u32,
        category: NodeCategory,
        sockets: Vec<NodeSocket>,
        body: impl Fn(&NodeModel) -> WidgetNode + Send + Sync + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            version,
            category,
            sockets,
            body: Arc::new(body),
            on_event: None,
            migrations: HashMap::new(),
        }
    }

    /// Return replacement node data for an event raised by a descendant body control.
    /// Returning None leaves the model unchanged.
    pub fn with_event_handler(
        mut self,
        handler: impl Fn(&World, &WidgetEvent, &NodeModel) -> Option<serde_json::Value>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.on_event = Some(Arc::new(handler));
        self
    }

    pub fn handle_event(
        &self,
        world: &World,
        event: &WidgetEvent,
        node: &NodeModel,
    ) -> Option<serde_json::Value> {
        self.on_event
            .as_ref()
            .and_then(|handler| handler(world, event, node))
    }

    /// Register a single schema step. The callback may edit data and sockets, including
    /// their stable IDs. Every version between the saved and current schema needs a step.
    pub fn with_migration(
        mut self,
        from_version: u32,
        migrate: impl Fn(&mut NodeModel) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        self.migrations.insert(from_version, Arc::new(migrate));
        self
    }

    pub fn migrate(&self, node: &mut NodeModel) -> Result<(), String> {
        if node.schema_version > self.version {
            return Err(format!("node {} uses a newer schema version", self.id));
        }
        while node.schema_version < self.version {
            let step = self.migrations.get(&node.schema_version).ok_or_else(|| {
                format!(
                    "missing migration for {} version {}",
                    self.id, node.schema_version
                )
            })?;
            step(node)?;
            node.schema_version += 1;
        }
        Ok(())
    }

    pub fn build_body(&self, node: &NodeModel) -> WidgetNode {
        (self.body)(node)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SocketTypeDefinition {
    pub id: String,
    pub color: Rgba,
}

impl SocketTypeDefinition {
    pub fn new(id: impl Into<String>, color: Rgba) -> Self {
        Self {
            id: id.into(),
            color,
        }
    }
}

/// Runtime definitions for socket visuals and explicitly permitted conversions.
/// Graph files store type IDs; callbacks and colors are registered by the application.
#[derive(Resource, Clone)]
pub struct NodeGraphTypeRegistry {
    types: HashMap<String, SocketTypeDefinition>,
    nodes: HashMap<String, NodeTypeDefinition>,
    conversions: HashSet<(SocketType, SocketType)>,
    editors: HashMap<SocketType, SocketEditorDefinition>,
}

impl Default for NodeGraphTypeRegistry {
    fn default() -> Self {
        let mut registry = Self {
            types: HashMap::new(),
            nodes: HashMap::new(),
            conversions: HashSet::new(),
            editors: HashMap::new(),
        };
        registry.register_editor(
            SocketType::Boolean,
            SocketEditorDefinition::new(
                |socket| {
                    WidgetNode::checkbox(
                        &socket.name,
                        socket
                            .value
                            .as_ref()
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false),
                    )
                },
                |_, event| match event {
                    WidgetEvent::ValueChanged {
                        after: WidgetValue::Bool(value),
                        ..
                    } => Some(serde_json::Value::Bool(*value)),
                    _ => None,
                },
            ),
        );
        for (socket_type, kind) in [
            (SocketType::Integer, NumericFieldKind::Integer),
            (SocketType::Float, NumericFieldKind::Float),
        ] {
            registry.register_editor(
                socket_type,
                SocketEditorDefinition::new(
                    move |socket| {
                        WidgetNode::new(WidgetSpec::Number {
                            label: socket.name.clone(),
                            value: socket
                                .value
                                .as_ref()
                                .and_then(serde_json::Value::as_f64)
                                .unwrap_or(0.0),
                            min: None,
                            max: None,
                            step: if kind == NumericFieldKind::Integer {
                                1.0
                            } else {
                                0.01
                            },
                            kind,
                        })
                    },
                    move |_, event| match event {
                        WidgetEvent::ValueChanged {
                            after: WidgetValue::Number(value),
                            ..
                        } if value.is_finite() => {
                            if kind == NumericFieldKind::Integer {
                                Some(serde_json::Value::from(*value as i64))
                            } else {
                                serde_json::Number::from_f64(*value).map(serde_json::Value::Number)
                            }
                        }
                        _ => None,
                    },
                ),
            );
        }
        registry.register_editor(
            SocketType::Text,
            SocketEditorDefinition::new(
                |socket| {
                    WidgetNode::text_field(
                        &socket.name,
                        socket
                            .value
                            .as_ref()
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or(""),
                    )
                },
                |_, event| match event {
                    WidgetEvent::TextChanged { text, .. } => {
                        Some(serde_json::Value::String(text.clone()))
                    }
                    _ => None,
                },
            ),
        );
        registry
    }
}

impl NodeGraphTypeRegistry {
    pub fn register_editor(&mut self, socket_type: SocketType, editor: SocketEditorDefinition) {
        self.editors.insert(socket_type, editor);
    }

    pub fn editor(&self, socket_type: &SocketType) -> Option<&SocketEditorDefinition> {
        self.editors.get(socket_type)
    }
    pub fn register_node(&mut self, definition: NodeTypeDefinition) -> Result<(), String> {
        if definition.id.is_empty() || !definition.id.contains(':') || definition.version == 0 {
            return Err(
                "node type IDs must be namespaced and have a positive schema version".into(),
            );
        }
        if self.nodes.contains_key(&definition.id) {
            return Err(format!("node type {} is already registered", definition.id));
        }
        let mut ids = HashSet::new();
        if definition
            .sockets
            .iter()
            .any(|socket| socket.id.0 == 0 || !ids.insert(socket.id))
        {
            return Err(format!(
                "node type {} needs unique, nonzero socket IDs",
                definition.id
            ));
        }
        self.nodes.insert(definition.id.clone(), definition);
        Ok(())
    }

    pub fn node(&self, id: &str) -> Option<&NodeTypeDefinition> {
        self.nodes.get(id)
    }
    pub fn register(&mut self, definition: SocketTypeDefinition) -> Result<(), String> {
        if definition.id.is_empty() || !definition.id.contains(':') {
            return Err(
                "custom socket type IDs must be namespaced, such as app:temperature".into(),
            );
        }
        if self.types.contains_key(&definition.id) {
            return Err(format!(
                "socket type {} is already registered",
                definition.id
            ));
        }
        self.types.insert(definition.id.clone(), definition);
        Ok(())
    }

    pub fn get(&self, socket_type: &SocketType) -> Option<&SocketTypeDefinition> {
        match socket_type {
            SocketType::Custom(id) => self.types.get(id),
            _ => None,
        }
    }

    pub fn color(&self, socket_type: &SocketType) -> Option<Rgba> {
        self.get(socket_type).map(|definition| definition.color)
    }

    pub fn allow_conversion(&mut self, from: SocketType, to: SocketType) {
        self.conversions.insert((from, to));
    }

    pub fn can_connect(&self, from: &SocketType, to: &SocketType) -> bool {
        if matches!(from, SocketType::Custom(id) if !self.types.contains_key(id))
            || matches!(to, SocketType::Custom(id) if !self.types.contains_key(id))
        {
            return false;
        }
        from == to
            || matches!(from, SocketType::Any)
            || matches!(to, SocketType::Any)
            || self.conversions.contains(&(from.clone(), to.clone()))
    }
}
