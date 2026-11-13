use super::*;
use std::collections::HashMap;

#[test]
fn custom_socket_types_require_registration_and_explicit_conversions() {
    let temperature = SocketType::custom("demo:temperature");
    let mut registry = NodeGraphTypeRegistry::default();
    assert!(!registry.can_connect(&temperature, &SocketType::Float));
    registry
        .register(SocketTypeDefinition::new(
            "demo:temperature",
            Rgba(0.8, 0.4, 0.1, 1.0),
        ))
        .unwrap();
    assert!(registry.can_connect(&temperature, &temperature));
    assert!(!registry.can_connect(&temperature, &SocketType::Float));
    registry.allow_conversion(temperature.clone(), SocketType::Float);
    assert!(registry.can_connect(&temperature, &SocketType::Float));
    assert!(!registry.can_connect(&SocketType::Float, &temperature));

    let mut graph = NodeGraph::default();
    let source = graph.add_node(
        "Source",
        [0.0, 0.0],
        vec![NodeSocket::output("Heat", temperature)],
    );
    let target = graph.add_node(
        "Target",
        [240.0, 0.0],
        vec![NodeSocket::input("Value", SocketType::Float)],
    );
    assert_eq!(
        graph.create_link(source, 0, target, 0),
        Err(NodeGraphError::IncompatibleSockets)
    );
    graph
        .create_link_with_registry(&registry, source, 0, target, 0)
        .unwrap();
    assert_eq!(
        NodeGraph::from_json(&graph.to_json().unwrap()).unwrap(),
        graph
    );
}

#[test]
fn socket_ids_keep_links_attached_through_schema_reordering() {
    let mut graph = NodeGraph::default();
    let source = graph.add_node(
        "Source",
        [0.0, 0.0],
        vec![
            NodeSocket::output("First", SocketType::Float).with_id(SocketId(10)),
            NodeSocket::output("Second", SocketType::Float).with_id(SocketId(20)),
        ],
    );
    let target = graph.add_node(
        "Target",
        [240.0, 0.0],
        vec![NodeSocket::input("In", SocketType::Float)],
    );
    graph.create_link(source, 1, target, 0).unwrap();
    assert_eq!(graph.links[0].from_socket_id, SocketId(20));
    graph
        .replace_node_sockets(
            source,
            vec![
                NodeSocket::output("Second renamed", SocketType::Float).with_id(SocketId(20)),
                NodeSocket::output("First", SocketType::Float).with_id(SocketId(10)),
            ],
            &HashMap::new(),
        )
        .unwrap();
    assert_eq!(graph.links[0].from_socket, 0);
    assert_eq!(graph.links[0].from_socket_id, SocketId(20));
    let before = graph.clone();
    assert_eq!(
        graph.replace_node_sockets(
            source,
            vec![NodeSocket::output("Different", SocketType::Float).with_id(SocketId(30))],
            &HashMap::new()
        ),
        Err(NodeGraphError::MissingSocket)
    );
    assert_eq!(graph, before);
    graph
        .replace_node_sockets(
            source,
            vec![NodeSocket::output("Replacement", SocketType::Float).with_id(SocketId(30))],
            &HashMap::from([(SocketId(20), SocketId(30))]),
        )
        .unwrap();
    assert_eq!(graph.links[0].from_socket_id, SocketId(30));
}

#[test]
fn registered_node_migration_preserves_links_and_unknown_types() {
    let mut registry = NodeGraphTypeRegistry::default();
    let definition = NodeTypeDefinition::new(
        "demo:source",
        2,
        NodeCategory::Input,
        vec![NodeSocket::output("New", SocketType::Float).with_id(SocketId(20))],
        |_| crate::WidgetNode::label("body"),
    )
    .with_migration(1, |node| {
        node.sockets = vec![NodeSocket::output("New", SocketType::Float).with_id(SocketId(20))];
        node.data = serde_json::json!({"version": 2});
        Ok(())
    });
    registry.register_node(definition).unwrap();
    assert!(
        registry
            .register_node(NodeTypeDefinition::new(
                "demo:invalid",
                1,
                NodeCategory::General,
                vec![NodeSocket::input("no id", SocketType::Float)],
                |_| crate::WidgetNode::label("body"),
            ))
            .is_err()
    );

    let mut graph = NodeGraph::default();
    let source = graph.add_node(
        "Source",
        [0.0, 0.0],
        vec![NodeSocket::output("Old", SocketType::Float).with_id(SocketId(20))],
    );
    graph.nodes[0].type_id = Some("demo:source".into());
    graph.nodes[0].schema_version = 1;
    let sink = graph.add_node(
        "Sink",
        [200.0, 0.0],
        vec![NodeSocket::input("In", SocketType::Float)],
    );
    graph.nodes[1].type_id = Some("unknown:plugin".into());
    graph.create_link(source, 0, sink, 0).unwrap();
    let unknown = SocketType::custom("unknown:payload");
    let missing_source = graph.add_node(
        "Missing source",
        [0.0, 150.0],
        vec![NodeSocket::output("Out", unknown.clone())],
    );
    let missing_sink = graph.add_node(
        "Missing sink",
        [200.0, 150.0],
        vec![NodeSocket::input("In", unknown)],
    );
    graph
        .create_link(missing_source, 0, missing_sink, 0)
        .unwrap();
    graph.migrate_registered_nodes(&registry).unwrap();
    assert_eq!(graph.nodes[0].schema_version, 2);
    assert_eq!(graph.nodes[0].data["version"], 2);
    assert_eq!(graph.links[0].from_socket_id, SocketId(20));
    assert_eq!(graph.nodes[1].type_id.as_deref(), Some("unknown:plugin"));
    assert_eq!(graph.links.len(), 2);

    let before = graph.clone();
    let mut newer = NodeGraphTypeRegistry::default();
    newer
        .register_node(NodeTypeDefinition::new(
            "demo:source",
            3,
            NodeCategory::Input,
            vec![NodeSocket::output("New", SocketType::Float).with_id(SocketId(20))],
            |_| crate::WidgetNode::label("body"),
        ))
        .unwrap();
    assert!(graph.migrate_registered_nodes(&newer).is_err());
    assert_eq!(graph, before);
}

#[test]
fn color_ramp_edits_stops_and_samples_interpolation() {
    let mut ramp = ColorRamp::default();
    let id = ramp.add_stop(0.5, Rgba(1.0, 0.0, 0.0, 1.0)).unwrap();
    assert_eq!(ramp.stops.len(), 3);
    assert_eq!(ramp.sample(0.5), Rgba(1.0, 0.0, 0.0, 1.0));
    assert!(ramp.move_stop(id, 0.75));
    assert!(ramp.remove_stop(id));
    assert!(!ramp.remove_stop(1));
}

#[test]
fn color_ramp_json_round_trip_preserves_stops_selection_and_mode() {
    let mut ramp = ColorRamp::default();
    let inserted = ramp.add_stop(0.35, Rgba(0.2, 0.4, 0.6, 0.8)).unwrap();
    ramp.set_interpolation(Interpolation::Smooth);
    ramp.set_color_channel(inserted, ColorChannel::Green, 0.55);
    ramp.select_stop(2);

    let restored = ColorRamp::from_json(&ramp.to_json().unwrap()).unwrap();
    assert_eq!(restored, ramp);
    assert_eq!(restored.selected_stop().unwrap().id, 2);
    assert_eq!(restored.interpolation, Interpolation::Smooth);
}

#[test]
fn color_ramp_json_rejects_invalid_positions_ids_and_versions() {
    let ramp = ColorRamp::default();
    let valid = ramp.to_json().unwrap();
    let mut invalid_position: ColorRampSnapshot = serde_json::from_str(&valid).unwrap();
    invalid_position.stops[0].position = 1.5;
    assert!(ColorRamp::from_json(&serde_json::to_string(&invalid_position).unwrap()).is_err());

    let mut duplicate_id: ColorRampSnapshot = serde_json::from_str(&valid).unwrap();
    duplicate_id.stops[1].id = duplicate_id.stops[0].id;
    assert!(ColorRamp::from_json(&serde_json::to_string(&duplicate_id).unwrap()).is_err());

    let mut future: ColorRampSnapshot = serde_json::from_str(&valid).unwrap();
    future.version = 2;
    assert!(ColorRamp::from_json(&serde_json::to_string(&future).unwrap()).is_err());
}

#[test]
fn hsv_conversion_round_trips_primary_and_mixed_srgb_colors() {
    for color in [
        Rgba(1.0, 0.0, 0.0, 1.0),
        Rgba(0.0, 1.0, 0.0, 0.5),
        Rgba(0.0, 0.0, 1.0, 0.25),
        Rgba(0.2, 0.4, 0.6, 0.8),
        Rgba(0.0, 0.0, 0.0, 1.0),
    ] {
        let hsv = rgb_to_hsv(color);
        let round_trip = hsv_to_rgb(hsv, color.3);
        assert!((round_trip.0 - color.0).abs() < 1.0e-5);
        assert!((round_trip.1 - color.1).abs() < 1.0e-5);
        assert!((round_trip.2 - color.2).abs() < 1.0e-5);
        assert!((round_trip.3 - color.3).abs() < 1.0e-5);
    }
    assert_eq!(rgb_to_hsv(Rgba(0.0, 1.0, 0.0, 1.0))[0], 120.0);
    assert_eq!(rgb_to_hsv(Rgba(0.0, 0.0, 1.0, 1.0))[0], 240.0);
}

#[test]
fn hsv_hex_entry_and_serialization_preserve_color_and_achromatic_hue() {
    let mut editor = HsvColorEditor::default();
    assert!(editor.set_hex("#33669980").unwrap());
    assert_eq!(editor.hex(), "#33669980");
    let color = editor.color();
    assert!((color.0 - 0x33 as f32 / 255.0).abs() < 1.0e-5);
    assert!((color.1 - 0x66 as f32 / 255.0).abs() < 1.0e-5);
    assert!((color.2 - 0x99 as f32 / 255.0).abs() < 1.0e-5);
    assert!((color.3 - 0x80 as f32 / 255.0).abs() < 1.0e-5);
    assert!(editor.set_hsv(HsvColorField::Hue, 217.0));
    assert!(editor.set_hsv(HsvColorField::Saturation, 0.0));
    assert_eq!(editor.hue_degrees, 217.0);
    assert_eq!(editor.hex(), "#99999980");

    let restored = HsvColorEditor::from_json(&editor.to_json().unwrap()).unwrap();
    assert_eq!(restored, editor);
    assert!(parse_hex_color("#abc", 0.25).is_ok());
    assert!(parse_hex_color("not-hex", 1.0).is_err());
    assert!(parse_hex_color("#fff", f32::NAN).is_err());
}

#[test]
fn curve_points_handles_and_view_state_drive_evaluation() {
    let mut curve = CurveEditor::default();
    let id = curve.add_point(0.5, 0.2).unwrap();
    assert!((curve.evaluate(0.25) - 0.1).abs() < 0.02);
    assert!(curve.move_point(id, [0.6, 0.4]));
    assert!(curve.set_handles(id, [-0.1, 0.0], [0.1, 0.0]));
    curve.zoom_by(2.0, [0.5, 0.5]);
    assert_eq!(curve.zoom, 2.0);
    assert!(curve.remove_point(id));
}

#[test]
fn curve_editor_json_round_trip_preserves_points_handles_and_view() {
    let mut curve = CurveEditor::default();
    let id = curve.add_point(0.4, 0.72).unwrap();
    curve.set_handles(id, [-0.12, -0.08], [0.18, 0.14]);
    curve.interpolation = Interpolation::Smooth;
    curve.pan = [0.13, -0.04];
    curve.zoom = 2.25;
    curve.select_point(id);
    curve.validate().unwrap();

    let restored = CurveEditor::from_json(&curve.to_json().unwrap()).unwrap();
    assert_eq!(restored, curve);

    let mut invalid = curve.clone();
    invalid.selected = Some(u64::MAX);
    assert!(invalid.validate().is_err());
    let mut invalid = curve.clone();
    invalid.zoom = f32::NAN;
    assert!(CurveEditor::from_json(&invalid.to_json().unwrap()).is_err());
    let mut invalid = curve.clone();
    invalid.points.swap(0, 1);
    assert!(invalid.validate().is_err());

    let mut unsupported: serde_json::Value =
        serde_json::from_str(&curve.to_json().unwrap()).unwrap();
    unsupported["version"] = serde_json::Value::from(99);
    assert!(CurveEditor::from_json(&unsupported.to_string()).is_err());
}

#[test]
fn node_graph_validates_and_reconnects_typed_links() {
    let mut graph = NodeGraph::default();
    let source = graph.add_node(
        "source",
        [0.0, 0.0],
        vec![NodeSocket::output("value", SocketType::Float)],
    );
    let sink = graph.add_node(
        "sink",
        [1.0, 0.0],
        vec![NodeSocket::input("value", SocketType::Float)],
    );
    let wrong = graph.add_node(
        "wrong",
        [2.0, 0.0],
        vec![NodeSocket::input("value", SocketType::Text)],
    );
    let link = graph.create_link(source, 0, sink, 0).unwrap();
    assert_eq!(
        graph.create_link(source, 0, wrong, 0),
        Err(NodeGraphError::IncompatibleSockets)
    );
    assert_eq!(
        graph.reconnect_link(link, LinkEndpoint::To, wrong, 0),
        Err(NodeGraphError::IncompatibleSockets)
    );
    assert_eq!(graph.links[0].to_node, sink);
    graph.select(source, false);
    graph.pan_by([2.0, -1.0]);
    graph.zoom_by(2.0);
    assert!(graph.remove_node(source));
    assert!(graph.links.is_empty());
}

#[test]
fn node_graph_serialization_preserves_identity_selection_view_and_connections() {
    let mut graph = NodeGraph::default();
    let source = graph.add_node(
        "Source",
        [24.0, 40.0],
        vec![NodeSocket::output("value", SocketType::Float).bottom_right()],
    );
    let sink = graph.add_node(
        "Sink",
        [320.0, 80.0],
        vec![NodeSocket::input("value", SocketType::Float)],
    );
    let link = graph.create_link(source, 0, sink, 0).unwrap();
    graph.select(source, false);
    graph.nodes[1].collapsed = true;
    graph.pan = [18.0, -12.0];
    graph.zoom = 1.4;

    let restored = NodeGraph::from_json(&graph.to_json().unwrap()).unwrap();
    assert_eq!(restored, graph);
    assert_eq!(restored.links[0].id, link);

    let mut legacy: serde_json::Value = serde_json::from_str(&graph.to_json().unwrap()).unwrap();
    legacy["nodes"][0]["sockets"][0]
        .as_object_mut()
        .unwrap()
        .remove("placement");
    let legacy = NodeGraph::from_json(&legacy.to_string()).unwrap();
    assert_eq!(legacy.nodes[0].sockets[0].placement, SocketPlacement::Auto);

    let mut invalid = graph.clone();
    invalid.nodes[0].position[0] = f32::NAN;
    assert!(invalid.to_json().is_err());

    let mut invalid_link = graph.clone();
    invalid_link.links[0].to_socket = usize::MAX;
    let json = serde_json::to_string(&invalid_link).unwrap();
    assert!(NodeGraph::from_json(&json).is_err());
}
