use super::*;

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
        vec![NodeSocket::output("value", SocketType::Float)],
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

    let mut invalid = graph.clone();
    invalid.nodes[0].position[0] = f32::NAN;
    assert!(invalid.to_json().is_err());

    let mut invalid_link = graph.clone();
    invalid_link.links[0].to_socket = usize::MAX;
    let json = serde_json::to_string(&invalid_link).unwrap();
    assert!(NodeGraph::from_json(&json).is_err());
}
