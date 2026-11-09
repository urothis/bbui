import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(1) @binding(0)
var<uniform> points_01: vec4<f32>;
@group(1) @binding(1)
var<uniform> points_23: vec4<f32>;
@group(1) @binding(2)
var<uniform> color: vec4<f32>;
@group(1) @binding(3)
var<uniform> style: vec4<f32>;

fn bezier_point(t: f32) -> vec2<f32> {
    let inverse = 1.0 - t;
    let p0 = points_01.xy;
    let p1 = points_01.zw;
    let p2 = points_23.xy;
    let p3 = points_23.zw;
    return p0 * (inverse * inverse * inverse)
        + p1 * (3.0 * inverse * inverse * t)
        + p2 * (3.0 * inverse * t * t)
        + p3 * (t * t * t);
}

fn segment_distance(point: vec2<f32>, start_point: vec2<f32>, end_point: vec2<f32>) -> f32 {
    let segment = end_point - start_point;
    let length_squared = dot(segment, segment);
    if length_squared <= 0.000001 {
        return length(point - start_point);
    }
    let t = clamp(dot(point - start_point, segment) / length_squared, 0.0, 1.0);
    return length(point - (start_point + segment * t));
}

fn srgb_channel_to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        return channel / 12.92;
    }
    return pow((channel + 0.055) / 1.055, 2.4);
}

fn srgb_to_linear(rgb: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        srgb_channel_to_linear(rgb.r),
        srgb_channel_to_linear(rgb.g),
        srgb_channel_to_linear(rgb.b),
    );
}

@fragment
fn fragment(input: UiVertexOutput) -> @location(0) vec4<f32> {
    let point = input.uv * input.size;
    var previous = bezier_point(0.0);
    var distance = 1.0e20;
    for (var index = 1; index <= 32; index = index + 1) {
        let current = bezier_point(f32(index) / 32.0);
        distance = min(distance, segment_distance(point, previous, current));
        previous = current;
    }

    let half_width = style.x * 0.5;
    let antialias = clamp(fwidth(distance), 0.65, 1.25);
    let coverage = 1.0 - smoothstep(half_width - antialias, half_width + antialias, distance);
    return vec4<f32>(srgb_to_linear(color.rgb), color.a * coverage);
}
