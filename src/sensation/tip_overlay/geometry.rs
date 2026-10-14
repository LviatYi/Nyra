use crate::settings_default_values::{WINDOW_HEIGHT, WINDOW_WIDTH};
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use serde::Deserialize;
use std::{fs, path::Path};

const DEFAULT_BORDER_PATH: &str =
    include_str!("../../../assets/sensation/rotation_notice_border.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BorderPathDefinition {
    canvas: [f32; 2],
    /// Closed polygon in screen coordinates; the first point is not repeated.
    points: Vec<[f32; 2]>,
    /// Authored outer contour, paired with points; overrides generated border width.
    #[serde(default)]
    outer_points: Option<Vec<[f32; 2]>>,
}

#[derive(Clone, Copy)]
struct BorderVertexPair {
    inner: Vec2,
    outer: Vec2,
}

impl BorderVertexPair {
    fn lerp(self, other: Self, factor: f32) -> Self {
        Self {
            inner: self.inner.lerp(other.inner, factor),
            outer: self.outer.lerp(other.outer, factor),
        }
    }
}

#[derive(Clone)]
pub(super) struct BorderGeometry {
    vertices: Vec<BorderVertexPair>,
    cumulative_lengths: Vec<f32>,
    total_length: f32,
    winding: f32,
}

impl BorderGeometry {
    pub(super) fn load(path: Option<&str>, width: f32) -> Self {
        if let Some(path) = path {
            match fs::read_to_string(Path::new("assets").join(path))
                .map_err(|error| error.to_string())
                .and_then(|source| Self::parse(&source, width))
            {
                Ok(geometry) => return geometry,
                Err(error) => {
                    eprintln!("[Sensation] Border path {path}: {error}; using built-in path")
                }
            }
        }
        match Self::parse(DEFAULT_BORDER_PATH, width) {
            Ok(geometry) => geometry,
            Err(error) => {
                eprintln!(
                    "[Sensation] Built-in border with configured width: {error}; using 2 pixels"
                );
                Self::parse(DEFAULT_BORDER_PATH, 2.0).expect("built-in border path must be valid")
            }
        }
    }

    fn parse(source: &str, width: f32) -> Result<Self, String> {
        let definition: BorderPathDefinition =
            serde_json::from_str(source).map_err(|error| error.to_string())?;
        let size = Vec2::from_array(definition.canvas);
        if size != Vec2::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32) {
            return Err("path canvas must match the current overlay size".into());
        }
        if !width.is_finite() || width <= 0.0 || width >= size.min_element() / 4.0 {
            return Err("border width is outside the supported range".into());
        }
        if definition.points.len() < 3 {
            return Err("path needs at least three vertices".into());
        }
        if definition
            .outer_points
            .as_ref()
            .is_some_and(|outer| outer.len() != definition.points.len())
        {
            return Err("inner and outer contours must have matching vertex counts".into());
        }
        let points = definition
            .points
            .iter()
            .map(|[x, y]| Vec2::new(*x - size.x / 2.0, size.y / 2.0 - *y))
            .collect::<Vec<_>>();
        if points.iter().any(|point| {
            !point.is_finite() || point.x.abs() > size.x / 2.0 || point.y.abs() > size.y / 2.0
        }) {
            return Err("path vertices must lie inside the window".into());
        }
        let area = points
            .iter()
            .enumerate()
            .map(|(index, point)| point.perp_dot(points[(index + 1) % points.len()]))
            .sum::<f32>();
        if area.abs() < 0.01 {
            return Err("path has no enclosed area".into());
        }
        let winding = area.signum();
        let mut vertices = Vec::with_capacity(points.len() + 1);
        for index in 0..points.len() {
            let previous = points[(index + points.len() - 1) % points.len()];
            let current = points[index];
            let next = points[(index + 1) % points.len()];
            if let Some(outer_points) = &definition.outer_points {
                let [x, y] = outer_points[index];
                let outer = Vec2::new(x - size.x / 2.0, size.y / 2.0 - y);
                if !outer.is_finite()
                    || outer.x.abs() > size.x / 2.0
                    || outer.y.abs() > size.y / 2.0
                {
                    return Err("outer contour vertices must lie inside the window".into());
                }
                vertices.push(BorderVertexPair {
                    inner: current,
                    outer,
                });
                continue;
            }
            let incoming = (current - previous).normalize_or_zero();
            let outgoing = (next - current).normalize_or_zero();
            if incoming == Vec2::ZERO || outgoing == Vec2::ZERO {
                return Err("path must not contain duplicate adjacent vertices".into());
            }
            let outward = |direction: Vec2| {
                let left = Vec2::new(-direction.y, direction.x);
                left * -winding
            };
            let first_normal = outward(incoming);
            let second_normal = outward(outgoing);
            let denominator = 1.0 + first_normal.dot(second_normal);
            if denominator <= 0.0001 {
                return Err("path corner is too sharp for an outward join".into());
            }
            let outer = current + (first_normal + second_normal) * (width / denominator);
            if outer.x.abs() > size.x / 2.0 + 0.01 || outer.y.abs() > size.y / 2.0 + 0.01 {
                return Err("outward border would leave the window".into());
            }
            vertices.push(BorderVertexPair {
                inner: current,
                outer,
            });
        }
        vertices.push(vertices[0]);
        let mut cumulative_lengths = Vec::with_capacity(vertices.len());
        cumulative_lengths.push(0.0);
        for pair in vertices.windows(2) {
            cumulative_lengths.push(
                cumulative_lengths.last().copied().unwrap() + pair[0].inner.distance(pair[1].inner),
            );
        }
        let total_length = *cumulative_lengths.last().unwrap();
        Ok(Self {
            vertices,
            cumulative_lengths,
            total_length,
            winding,
        })
    }

    pub(super) fn progress_mesh(&self, progress: f32) -> Mesh {
        self.progress_range_mesh(0.0, progress)
    }

    pub(super) fn progress_range_mesh(&self, start: f32, end: f32) -> Mesh {
        let start_distance = start.clamp(0.0, 1.0) * self.total_length;
        let end_distance = end.clamp(0.0, 1.0) * self.total_length;
        if start_distance >= end_distance {
            let point = self.pair_at(start_distance);
            return triangle_mesh(
                vec![
                    [point.inner.x, point.inner.y, 0.0],
                    [point.outer.x, point.outer.y, 0.0],
                ],
                Vec::new(),
            );
        }
        let first_inside = self
            .cumulative_lengths
            .partition_point(|length| *length <= start_distance);
        let last_inside = self
            .cumulative_lengths
            .partition_point(|length| *length < end_distance);
        let mut visible = Vec::with_capacity(last_inside - first_inside + 2);
        visible.push(self.pair_at(start_distance));
        visible.extend_from_slice(&self.vertices[first_inside..last_inside]);
        visible.push(self.pair_at(end_distance));
        let mut positions = Vec::with_capacity(visible.len() * 2);
        for vertex in &visible {
            positions.push([vertex.inner.x, vertex.inner.y, 0.0]);
            positions.push([vertex.outer.x, vertex.outer.y, 0.0]);
        }
        let mut indices = Vec::with_capacity(visible.len().saturating_sub(1) * 6);
        for index in 0..visible.len().saturating_sub(1) {
            let start = index as u32 * 2;
            if self.winding < 0.0 {
                indices.extend_from_slice(&[
                    start + 1,
                    start,
                    start + 3,
                    start,
                    start + 2,
                    start + 3,
                ]);
            } else {
                indices.extend_from_slice(&[
                    start,
                    start + 1,
                    start + 2,
                    start + 1,
                    start + 3,
                    start + 2,
                ]);
            }
        }
        triangle_mesh(positions, indices)
    }

    pub(super) fn progress_head(&self, progress: f32) -> Vec2 {
        let pair = self.pair_at(progress.clamp(0.0, 1.0) * self.total_length);
        (pair.inner + pair.outer) * 0.5
    }

    fn pair_at(&self, distance: f32) -> BorderVertexPair {
        let complete = self
            .cumulative_lengths
            .partition_point(|length| *length <= distance);
        let segment = complete.saturating_sub(1).min(self.vertices.len() - 2);
        let start = self.cumulative_lengths[segment];
        let end = self.cumulative_lengths[segment + 1];
        self.vertices[segment].lerp(
            self.vertices[segment + 1],
            ((distance - start) / (end - start)).clamp(0.0, 1.0),
        )
    }
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

pub(super) fn icon_marker_mesh() -> Mesh {
    let positions = vec![
        [-5.0, 5.0, 0.0],
        [5.0, 5.0, 0.0],
        [5.0, -5.0, 0.0],
        [-5.0, -5.0, 0.0],
        [-2.0, 2.0, 0.0],
        [2.0, 2.0, 0.0],
        [2.0, -2.0, 0.0],
        [-2.0, -2.0, 0.0],
    ];
    let mut indices = vec![
        0, 1, 4, 1, 5, 4, 1, 2, 5, 2, 6, 5, 2, 3, 6, 3, 7, 6, 3, 0, 7, 0, 4, 7,
    ];
    for triangle in indices.chunks_exact_mut(3) {
        triangle.swap(1, 2);
    }
    triangle_mesh(positions, indices)
}
