use super::geometry::{OutlinePoint, rounded_rectangle_outline, triangle_mesh};
use crate::{
    job::JobConfig,
    sensory::job::JobSensoryState,
    settings_default_values::{
        TIP_OVERLAY_BORDER_WIDTH, TIP_OVERLAY_CORNER_RADIUS, WINDOW_HEIGHT, WINDOW_WIDTH,
    },
};
use bevy::{
    mesh::{Indices, VertexAttributeValues},
    prelude::*,
};

#[derive(Component)]
pub(super) struct CountdownBorder {
    /// Visible fraction of the rounded outline, in the inclusive range `0.0..=1.0`.
    progress: f32,
    path: ProgressBorderPath,
    topology: VisibleBorderTopology,
}

#[derive(Resource)]
pub(super) struct ProgressBorderGeometry {
    samples: Vec<BorderSample>,
}

#[derive(Clone, Copy)]
struct BorderVertexPair {
    outer: [f32; 3],
    inner: [f32; 3],
}

impl BorderVertexPair {
    fn lerp(self, other: Self, factor: f32) -> Self {
        Self {
            outer: Vec3::from(self.outer)
                .lerp(Vec3::from(other.outer), factor)
                .to_array(),
            inner: Vec3::from(self.inner)
                .lerp(Vec3::from(other.inner), factor)
                .to_array(),
        }
    }
}

#[derive(Clone, Copy)]
struct BorderSample {
    /// Normalized distance along the closed centerline.
    progress: f32,
    center: Vec2,
    vertices: BorderVertexPair,
}

/// One cached lap of the border, rebased so that progress `0.0` is the ripple origin.
pub(super) struct ProgressBorderPath {
    start_progress: f32,
    samples: Vec<BorderPathSample>,
    full_indices: Vec<u32>,
}

#[derive(Clone, Copy)]
struct BorderPathSample {
    /// Distance from this path's start, normalized to one complete lap.
    progress: f32,
    vertices: BorderVertexPair,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct VisibleBorderTopology {
    complete_segments: usize,
    has_partial_segment: bool,
}

struct VisibleBorderSlice {
    topology: VisibleBorderTopology,
    endpoint: Option<BorderVertexPair>,
}

impl CountdownBorder {
    pub(super) fn new(path: ProgressBorderPath) -> Self {
        let topology = VisibleBorderTopology {
            complete_segments: path.samples.len() - 1,
            has_partial_segment: false,
        };
        Self {
            progress: 1.0,
            path,
            topology,
        }
    }

    pub(super) fn restart_at(
        &mut self,
        geometry: &ProgressBorderGeometry,
        start_progress: f32,
        mesh: &mut Mesh,
    ) -> f32 {
        self.path = geometry.path_from(start_progress);
        reset_progress_border_mesh(mesh, &self.path);
        self.topology = VisibleBorderTopology {
            complete_segments: self.path.samples.len() - 1,
            has_partial_segment: false,
        };
        self.path.start_progress
    }
}

impl ProgressBorderGeometry {
    pub(super) fn new() -> Self {
        let half_size = Vec2::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32) / 2.0;
        let centerline_radius = TIP_OVERLAY_CORNER_RADIUS - TIP_OVERLAY_BORDER_WIDTH / 2.0;
        let centerline = rounded_rectangle_outline(
            half_size - Vec2::splat(TIP_OVERLAY_BORDER_WIDTH / 2.0),
            centerline_radius,
        );

        let total_length = centerline
            .windows(2)
            .map(|points| points[0].position.distance(points[1].position))
            .sum::<f32>();
        let mut traversed = 0.0;
        let mut samples = Vec::with_capacity(centerline.len());
        samples.push(BorderSample {
            progress: 0.0,
            center: centerline[0].position,
            vertices: border_vertex_pair(centerline[0]),
        });
        for points in centerline.windows(2) {
            traversed += points[0].position.distance(points[1].position);
            samples.push(BorderSample {
                progress: (traversed / total_length).min(1.0),
                center: points[1].position,
                vertices: border_vertex_pair(points[1]),
            });
        }
        samples.last_mut().unwrap().progress = 1.0;

        Self { samples }
    }

    pub(super) fn center_at(&self, progress: f32) -> Vec2 {
        let (from, to, factor) = self.segment_at(progress);
        from.center.lerp(to.center, factor)
    }

    pub(super) fn path_from(&self, start_progress: f32) -> ProgressBorderPath {
        let start_progress = start_progress.rem_euclid(1.0);
        let start_vertices = self.vertices_at(start_progress);
        let mut samples = Vec::with_capacity(self.samples.len() + 1);
        samples.push(BorderPathSample {
            progress: 0.0,
            vertices: start_vertices,
        });

        // Append the original cached samples in traversal order. Samples before
        // the new start are moved behind the old `1.0` boundary.
        let first_sample_after_start = self
            .samples
            .partition_point(|sample| sample.progress <= start_progress);
        for sample in &self.samples[first_sample_after_start..] {
            let relative_progress = sample.progress - start_progress;
            if relative_progress < 1.0 {
                samples.push(BorderPathSample {
                    progress: relative_progress,
                    vertices: sample.vertices,
                });
            }
        }
        for sample in self.samples.iter().skip(1) {
            let relative_progress = 1.0 - start_progress + sample.progress;
            if relative_progress >= 1.0 {
                break;
            }
            samples.push(BorderPathSample {
                progress: relative_progress,
                vertices: sample.vertices,
            });
        }
        samples.push(BorderPathSample {
            progress: 1.0,
            vertices: start_vertices,
        });

        let mut full_indices = Vec::with_capacity((samples.len() - 1) * 6);
        for segment in 0..samples.len() - 1 {
            push_border_segment_indices(&mut full_indices, segment as u32, segment as u32 + 1);
        }

        ProgressBorderPath {
            start_progress,
            samples,
            full_indices,
        }
    }

    fn vertices_at(&self, progress: f32) -> BorderVertexPair {
        let (from, to, factor) = self.segment_at(progress);
        from.vertices.lerp(to.vertices, factor)
    }

    fn segment_at(&self, progress: f32) -> (&BorderSample, &BorderSample, f32) {
        let progress = progress.rem_euclid(1.0);
        if progress == 0.0 {
            let first = &self.samples[0];
            return (first, first, 0.0);
        }

        let upper_index = self
            .samples
            .partition_point(|sample| sample.progress < progress);
        let lower = &self.samples[upper_index - 1];
        let upper = &self.samples[upper_index];
        let factor = (progress - lower.progress) / (upper.progress - lower.progress);
        (lower, upper, factor)
    }
}

impl ProgressBorderPath {
    fn slice_at(&self, progress: f32) -> VisibleBorderSlice {
        let progress = progress.clamp(0.0, 1.0);
        if progress == 0.0 {
            return VisibleBorderSlice {
                topology: VisibleBorderTopology {
                    complete_segments: 0,
                    has_partial_segment: false,
                },
                endpoint: None,
            };
        }
        if progress == 1.0 {
            return VisibleBorderSlice {
                topology: VisibleBorderTopology {
                    complete_segments: self.samples.len() - 1,
                    has_partial_segment: false,
                },
                endpoint: None,
            };
        }

        let upper_index = self
            .samples
            .partition_point(|sample| sample.progress < progress);
        let lower_index = upper_index - 1;
        let lower = self.samples[lower_index];
        let upper = self.samples[upper_index];
        let factor = (progress - lower.progress) / (upper.progress - lower.progress);
        VisibleBorderSlice {
            topology: VisibleBorderTopology {
                complete_segments: lower_index,
                has_partial_segment: true,
            },
            endpoint: Some(lower.vertices.lerp(upper.vertices, factor)),
        }
    }
}

pub(super) fn progress_border_mesh(path: &ProgressBorderPath) -> Mesh {
    let mut positions = Vec::with_capacity(path.samples.len() * 2 + 2);
    for sample in &path.samples {
        push_border_vertices(&mut positions, sample.vertices);
    }

    // This final pair is a reusable endpoint for the one partially visible segment.
    push_border_vertices(&mut positions, path.samples.last().unwrap().vertices);
    triangle_mesh(positions, path.full_indices.clone())
}

pub(super) fn update_countdown_border(
    time: Res<Time<Real>>,
    jobs: Res<JobConfig>,
    state: Res<JobSensoryState>,
    border: Single<(&mut CountdownBorder, &Mesh2d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(focus_state) = state.focus_state.as_ref() else {
        return;
    };
    let Some(tip) = jobs.0.tips.get(focus_state.current_index) else {
        return;
    };

    let (mut border, border_mesh) = border.into_inner();
    let elapsed = focus_state.elapsed_at(time.elapsed()).as_secs_f32();
    let progress = (1.0 - elapsed / tip.show_time() as f32).clamp(0.0, 1.0);
    if progress == border.progress {
        return;
    }

    border.progress = progress;
    let Some(mut mesh) = meshes.get_mut(&border_mesh.0) else {
        return;
    };
    let slice = border.path.slice_at(progress);
    update_progress_border_mesh(&mut mesh, &border.path, border.topology, &slice);
    border.topology = slice.topology;
}

fn reset_progress_border_mesh(mesh: &mut Mesh, path: &ProgressBorderPath) {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    else {
        return;
    };
    positions.clear();
    for sample in &path.samples {
        push_border_vertices(positions, sample.vertices);
    }
    push_border_vertices(positions, path.samples.last().unwrap().vertices);

    let Some(Indices::U32(indices)) = mesh.indices_mut() else {
        return;
    };
    indices.clear();
    indices.extend_from_slice(&path.full_indices);
}

fn update_progress_border_mesh(
    mesh: &mut Mesh,
    path: &ProgressBorderPath,
    current_topology: VisibleBorderTopology,
    slice: &VisibleBorderSlice,
) {
    if let Some(endpoint) = slice.endpoint
        && let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        let endpoint_index = path.samples.len() * 2;
        positions[endpoint_index] = endpoint.outer;
        positions[endpoint_index + 1] = endpoint.inner;
    }

    if slice.topology == current_topology {
        return;
    }

    let Some(Indices::U32(indices)) = mesh.indices_mut() else {
        return;
    };
    indices.clear();
    let complete_index_count = slice.topology.complete_segments * 6;
    indices.extend_from_slice(&path.full_indices[..complete_index_count]);
    if slice.topology.has_partial_segment {
        push_border_segment_indices(
            indices,
            slice.topology.complete_segments as u32,
            path.samples.len() as u32,
        );
    }
}

fn border_vertex_pair(point: OutlinePoint) -> BorderVertexPair {
    let offset = point.outward * (TIP_OVERLAY_BORDER_WIDTH / 2.0);
    let outer = point.position + offset;
    let inner = point.position - offset;
    BorderVertexPair {
        outer: [outer.x, outer.y, 0.0],
        inner: [inner.x, inner.y, 0.0],
    }
}

fn push_border_vertices(positions: &mut Vec<[f32; 3]>, vertices: BorderVertexPair) {
    positions.extend_from_slice(&[vertices.outer, vertices.inner]);
}

fn push_border_segment_indices(indices: &mut Vec<u32>, from: u32, to: u32) {
    let outer = from * 2;
    let inner = outer + 1;
    let next_outer = to * 2;
    let next_inner = next_outer + 1;
    indices.extend_from_slice(&[outer, inner, next_outer, inner, next_inner, next_outer]);
}
