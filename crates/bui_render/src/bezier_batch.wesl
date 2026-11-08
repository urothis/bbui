import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(1) @binding(0)
var<storage, read> segments: array<f32>;
@group(1) @binding(1)
var<storage, read> curves: array<f32>;
@group(1) @binding(2)
var<storage, read> tile_indices: array<u32>;
@group(1) @binding(3)
var<uniform> parameters: vec4<f32>;

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

fn composite_curve(
    accumulated: vec4<f32>,
    distance: f32,
    color: vec4<f32>,
    width: f32,
) -> vec4<f32> {
    let half_width = width * 0.5;
    let antialias = max(fwidth(distance), 0.65);
    let coverage = 1.0 - smoothstep(half_width - antialias, half_width + antialias, distance);
    let source_alpha = clamp(color.a * coverage, 0.0, 1.0);
    if source_alpha <= 0.0 {
        return accumulated;
    }
    let source = srgb_to_linear(color.rgb);
    let output_alpha = source_alpha + accumulated.a * (1.0 - source_alpha);
    let output_rgb = (source * source_alpha
        + accumulated.rgb * accumulated.a * (1.0 - source_alpha)) / output_alpha;
    return vec4<f32>(output_rgb, output_alpha);
}

@fragment
fn fragment(input: UiVertexOutput) -> @location(0) vec4<f32> {
    let point = input.uv * input.size;
    let tile_size = parameters.x;
    let columns = u32(parameters.y);
    let rows = u32(parameters.z);
    let tile_x = min(u32(max(floor(point.x / tile_size), 0.0)), columns - 1u);
    let tile_y = min(u32(max(floor(point.y / tile_size), 0.0)), rows - 1u);
    let tile = tile_y * columns + tile_x;
    let header = tile * 2u;
    let first = tile_indices[header];
    let count = tile_indices[header + 1u];
    let index_base = u32(parameters.w) + first;

    var accumulated = vec4<f32>(0.0);
    var current_curve = 0xffffffffu;
    var current_distance = 1.0e20;
    var current_color = vec4<f32>(0.0);
    var current_width = 0.0;
    for (var offset = 0u; offset < count; offset = offset + 1u) {
        let segment_index = tile_indices[index_base + offset];
        let curve_index = segment_index / 32u;
        if curve_index != current_curve {
            if current_curve != 0xffffffffu {
                accumulated = composite_curve(
                    accumulated,
                    current_distance,
                    current_color,
                    current_width,
                );
            }
            current_curve = curve_index;
            current_distance = 1.0e20;
            let curve_base = curve_index * 8u;
            current_color = vec4<f32>(
                curves[curve_base],
                curves[curve_base + 1u],
                curves[curve_base + 2u],
                curves[curve_base + 3u],
            );
            current_width = curves[curve_base + 4u];
        }
        let segment_base = segment_index * 4u;
        let start_point = vec2<f32>(segments[segment_base], segments[segment_base + 1u]);
        let end_point = vec2<f32>(segments[segment_base + 2u], segments[segment_base + 3u]);
        current_distance = min(current_distance, segment_distance(point, start_point, end_point));
    }
    if current_curve != 0xffffffffu {
        accumulated = composite_curve(accumulated, current_distance, current_color, current_width);
    }

    return accumulated;
}
