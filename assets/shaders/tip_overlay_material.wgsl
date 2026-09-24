#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
    mesh2d_view_bindings::view,
}

#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping
#endif
#ifdef SRGB_OUTPUT
#import bevy_render::color_operations::linear_to_srgb
#endif
#ifdef OKLAB_OUTPUT
#import bevy_render::color_operations::linear_rgb_to_oklab
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> clip_half_size: vec2<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> corner_radius: f32;

fn rounded_rectangle_distance(point: vec2<f32>) -> f32 {
    let corner = abs(point) - (clip_half_size - vec2(corner_radius));
    return length(max(corner, vec2(0.0)))
        + min(max(corner.x, corner.y), 0.0)
        - corner_radius;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let distance = rounded_rectangle_distance(mesh.world_position.xy);
    let antialias_width = max(fwidth(distance), 0.0001);
    let coverage = 1.0 - smoothstep(-antialias_width, antialias_width, distance);
    var output_color = vec4(color.rgb, color.a * coverage);

    if output_color.a <= 0.0 {
        discard;
    }

#ifdef TONEMAP_IN_SHADER
    output_color = tonemapping::tone_mapping(output_color, view.color_grading);
#endif
#ifdef SRGB_OUTPUT
    output_color = vec4(linear_to_srgb(output_color.rgb), output_color.a);
#endif
#ifdef OKLAB_OUTPUT
    output_color = vec4(linear_rgb_to_oklab(output_color.rgb), output_color.a);
#endif
    return output_color;
}
