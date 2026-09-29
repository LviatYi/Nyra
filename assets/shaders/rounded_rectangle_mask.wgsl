#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> half_size: vec2<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> corner_radius: f32;

fn rounded_rectangle_distance(point: vec2<f32>) -> f32 {
    let corner = abs(point) - (half_size - vec2(corner_radius));
    return length(max(corner, vec2(0.0)))
        + min(max(corner.x, corner.y), 0.0)
        - corner_radius;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let distance = rounded_rectangle_distance(mesh.world_position.xy);
    let distance_gradient = vec2(dpdx(distance), dpdy(distance));
    let antialias_width = max(length(distance_gradient) * 0.5, 0.0001);
    let coverage = 1.0 - smoothstep(-antialias_width, antialias_width, distance);
    return vec4(0.0, 0.0, 0.0, coverage);
}
