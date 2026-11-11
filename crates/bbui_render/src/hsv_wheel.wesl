struct UiVertexOutput {
    @location(0) uv: vec2<f32>,
    @location(1) border_widths: vec4<f32>,
    @location(2) border_radius_x: vec4<f32>,
    @location(3) border_radius_y: vec4<f32>,
    @location(4) @interpolate(flat) size: vec2<f32>,
    @builtin(position) position: vec4<f32>,
}

struct HsvWheelMaterial {
    parameters: vec4<f32>,
}

@group(1) @binding(0)
var<uniform> material: HsvWheelMaterial;

const TAU: f32 = 6.283185307179586;

fn hsv_to_srgb(hue_degrees: f32, saturation: f32, value: f32) -> vec3<f32> {
    let hue = hue_degrees / 60.0;
    let sector = floor(hue);
    let fraction = hue - sector;
    let p = value * (1.0 - saturation);
    let q = value * (1.0 - saturation * fraction);
    let t = value * (1.0 - saturation * (1.0 - fraction));

    if sector < 1.0 {
        return vec3<f32>(value, t, p);
    } else if sector < 2.0 {
        return vec3<f32>(q, value, p);
    } else if sector < 3.0 {
        return vec3<f32>(p, value, t);
    } else if sector < 4.0 {
        return vec3<f32>(p, q, value);
    } else if sector < 5.0 {
        return vec3<f32>(t, p, value);
    }
    return vec3<f32>(value, p, q);
}

fn srgb_channel_to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        return channel / 12.92;
    }
    return pow((channel + 0.055) / 1.055, 2.4);
}

fn srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        srgb_channel_to_linear(color.r),
        srgb_channel_to_linear(color.g),
        srgb_channel_to_linear(color.b),
    );
}

fn circle_coverage(distance: f32, radius: f32) -> f32 {
    let antialias = max(fwidth(distance), 0.65);
    return 1.0 - smoothstep(radius - antialias, radius + antialias, distance);
}

@fragment
fn fragment(input: UiVertexOutput) -> @location(0) vec4<f32> {
    let parameters = material.parameters;
    let hue_degrees = parameters.x;
    let selected_saturation = parameters.y;
    let selected_value = parameters.z;
    let size = min(input.size.x, input.size.y);
    let outer_radius = size * 0.5;
    let disk_radius = outer_radius * 0.7941176471;
    let ring_inner_radius = outer_radius - 10.5;
    let point = (input.uv - vec2<f32>(0.5)) * input.size;
    let radius = length(point);
    let angle = atan2(point.y, point.x);
    let hue_at_point = fract(angle / TAU + 1.0) * 360.0;

    let ring_alpha = circle_coverage(radius, outer_radius)
        * smoothstep(ring_inner_radius - 0.65, ring_inner_radius + 0.65, radius);
    let disk_alpha = circle_coverage(radius, disk_radius);

    var srgb_color = vec3<f32>(0.0);
    var alpha = 0.0;
    if ring_alpha > 0.0 {
        srgb_color = hsv_to_srgb(hue_at_point, 1.0, 1.0);
        alpha = ring_alpha;
    } else if disk_alpha > 0.0 {
        let saturation = clamp(radius / disk_radius, 0.0, 1.0);
        srgb_color = hsv_to_srgb(hue_degrees, saturation, selected_value);
        alpha = disk_alpha;
    }

    let hue_angle = hue_degrees * (TAU / 360.0);
    let hue_direction = vec2<f32>(cos(hue_angle), sin(hue_angle));
    let hue_selector_center = hue_direction * (outer_radius - 5.0);
    let hue_selector_distance = length(point - hue_selector_center);
    let saturation_selector_center = hue_direction * selected_saturation * disk_radius;
    let saturation_selector_distance = length(point - saturation_selector_center);

    if hue_selector_distance <= 6.5 {
        srgb_color = vec3<f32>(0.0);
        alpha = 1.0;
        if hue_selector_distance <= 4.5 {
            srgb_color = vec3<f32>(1.0);
        }
        if hue_selector_distance <= 3.0 {
            srgb_color = hsv_to_srgb(hue_degrees, 1.0, 1.0);
        }
    }

    if saturation_selector_distance <= 5.0 {
        srgb_color = vec3<f32>(0.0);
        alpha = 1.0;
        if saturation_selector_distance <= 3.5 {
            srgb_color = vec3<f32>(1.0);
        }
    }

    return vec4<f32>(srgb_to_linear(srgb_color), alpha);
}
