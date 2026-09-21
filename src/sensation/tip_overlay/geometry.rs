use crate::settings_default_values::TIP_OVERLAY_CORNER_SEGMENTS;
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use std::f32::consts::{FRAC_PI_2, PI};

#[derive(Clone, Copy)]
pub(super) struct OutlinePoint {
    pub(super) position: Vec2,
    pub(super) outward: Vec2,
}

pub(super) fn rounded_rectangle_mesh(size: Vec2, radius: f32) -> Mesh {
    let outline = rounded_rectangle_outline(size / 2.0, radius);
    let mut positions = Vec::with_capacity(outline.len() + 1);
    positions.push([0.0, 0.0, 0.0]);
    positions.extend(
        outline
            .iter()
            .map(|point| [point.position.x, point.position.y, 0.0]),
    );

    let mut indices = Vec::with_capacity((outline.len() - 1) * 3);
    for index in 1..outline.len() as u32 {
        // The outline runs clockwise, so reversing the two perimeter vertices
        // gives the triangle the counter-clockwise winding expected by Mesh2d.
        indices.extend_from_slice(&[0, index + 1, index]);
    }

    triangle_mesh(positions, indices)
}

pub(super) fn rounded_rectangle_outline(half_size: Vec2, radius: f32) -> Vec<OutlinePoint> {
    let mut points = Vec::with_capacity(TIP_OVERLAY_CORNER_SEGMENTS * 4 + 6);
    points.push(OutlinePoint {
        position: Vec2::new(radius - half_size.x, half_size.y),
        outward: Vec2::Y,
    });
    points.push(OutlinePoint {
        position: Vec2::new(half_size.x - radius, half_size.y),
        outward: Vec2::Y,
    });
    push_corner(
        &mut points,
        Vec2::new(half_size.x - radius, half_size.y - radius),
        radius,
        FRAC_PI_2,
    );
    points.push(OutlinePoint {
        position: Vec2::new(half_size.x, -half_size.y + radius),
        outward: Vec2::X,
    });
    push_corner(
        &mut points,
        Vec2::new(half_size.x - radius, -half_size.y + radius),
        radius,
        0.0,
    );
    points.push(OutlinePoint {
        position: Vec2::new(-half_size.x + radius, -half_size.y),
        outward: Vec2::NEG_Y,
    });
    push_corner(
        &mut points,
        Vec2::new(-half_size.x + radius, -half_size.y + radius),
        radius,
        -FRAC_PI_2,
    );
    points.push(OutlinePoint {
        position: Vec2::new(-half_size.x, half_size.y - radius),
        outward: Vec2::NEG_X,
    });
    push_corner(
        &mut points,
        Vec2::new(-half_size.x + radius, half_size.y - radius),
        radius,
        PI,
    );
    points
}

pub(super) fn rounded_rectangle_ray_distance(
    origin: Vec2,
    direction: Vec2,
    half_size: Vec2,
    radius: f32,
) -> f32 {
    let mut inside_distance = 0.0;
    let mut outside_distance = half_size.length() * 2.0;
    for _ in 0..18 {
        let middle = (inside_distance + outside_distance) / 2.0;
        if rounded_rectangle_contains(origin + direction * middle, half_size, radius) {
            inside_distance = middle;
        } else {
            outside_distance = middle;
        }
    }
    inside_distance
}

pub(super) fn triangle_mesh(positions: Vec<[f32; 3]>, indices: Vec<u32>) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn push_corner(points: &mut Vec<OutlinePoint>, center: Vec2, radius: f32, start_angle: f32) {
    for step in 1..=TIP_OVERLAY_CORNER_SEGMENTS {
        let angle = start_angle - FRAC_PI_2 * step as f32 / TIP_OVERLAY_CORNER_SEGMENTS as f32;
        let outward = Vec2::new(angle.cos(), angle.sin());
        points.push(OutlinePoint {
            position: center + outward * radius,
            outward,
        });
    }
}

fn rounded_rectangle_contains(point: Vec2, half_size: Vec2, radius: f32) -> bool {
    let corner_offset = point.abs() - (half_size - Vec2::splat(radius));
    corner_offset.max(Vec2::ZERO).length_squared() <= radius * radius
}
