# Typed node graphs

`NodeGraph` stores node and socket IDs, links, viewport state, and value backed inputs. Applications register runtime definitions in `NodeGraphTypeRegistry`. The [node graph example](../crates/bbui/examples/node_graph.rs) and its [rendered preview](visuals/examples/node_graph.png) show a registered node with bottom sockets, an editable Factor input, and a BBUI checkbox in its body.

## Define types and nodes

- Give each custom socket type a namespaced ID, such as `app:temperature`, and register a `SocketTypeDefinition` with its color. Built in types include Boolean, Integer, Float, Vector2, Vector3, Color, and Text.
- Use `allow_conversion(from, to)` for each permitted directed conversion. Exact type matches work by default. A converted link shades continuously from the output socket color to the input socket color, including when selected. The graph validates a connection before creating it. A rejected drag shows the target half of the preview in red and an X over the socket.
- Give each registered node a namespaced ID and a positive schema version. Every socket in a `NodeTypeDefinition` needs a unique, nonzero `SocketId`. IDs belong to the socket's meaning, not its position in the vector.
- The node body factory returns a `WidgetNode` tree. These are ordinary BBUI controls with normal layout, focus, and input. `with_event_handler` maps control events back to the node's serializable `data` value.

## Input values

`NodeSocket::with_value` stores the fallback value for an unconnected input. The registry supplies default editors for Boolean, Integer, Float, and Text. Register a `SocketEditorDefinition` for other built in or custom types. Its factory creates a BBUI widget tree; its reader converts widget events to a JSON value. Editors disappear while their input has a link, leaving the fallback value intact for later use.

For bottom ports, call `bottom_left()` or `bottom_right()` on a socket. These sit along the bottom edge, away from the corners. Their edge tangents leave the node vertically.

## Saved graphs and migrations

`to_json` and `from_json` preserve node type IDs, schema versions, node data, socket IDs, socket values, and links. Old graphs without socket IDs are upgraded on load. Unknown registered types stay in the saved graph so a missing plugin does not discard data.

When a registered node schema changes, increment its version and add a migration for every version step with `NodeTypeDefinition::with_migration`. A migration may replace or reorder sockets, but connected socket IDs must still resolve. `migrate_registered_nodes` applies all steps to a clone and commits only when the resulting graph and links validate. `replace_node_sockets_with_registry` is available for a single node; its ID map explicitly redirects links when a socket gets a new ID. Failed migrations leave the original graph untouched.
