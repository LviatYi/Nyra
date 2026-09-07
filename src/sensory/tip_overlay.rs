use crate::MAX_TEXT_COLUMNS;
use crate::job::{JobConfig, Tip};
use crate::sensory::job::{FocusState, JobSensoryState};
use crate::settings_default_values::*;
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, VertexAttributeValues},
    prelude::*,
    render::render_resource::PrimitiveTopology,
    sprite_render::AlphaMode2d,
    window::PrimaryWindow,
};
use rand::RngExt;
use std::{
    f32::consts::{FRAC_PI_2, PI},
    time::Duration,
};
use unicode_width::UnicodeWidthChar;

const COLOR_MAIN_TIP_TEXT: Color = Color::srgb(0.94, 0.96, 1.0);

#[derive(Component)]
pub struct TipOverlay;

#[derive(Component)]
pub struct TipText;

#[derive(Component)]
pub struct ProgressBorder {
    /// Visible fraction of the rounded outline, in the inclusive range `0.0..=1.0`.
    progress: f32,
    geometry: ProgressBorderGeometry,
    path: ProgressBorderPath,
    topology: VisibleBorderTopology,
    background_material: Handle<ColorMaterial>,
    played_material: Handle<ColorMaterial>,
    remaining_material: Handle<ColorMaterial>,
    ripple_mesh: Handle<Mesh>,
    ripple_material: Handle<ColorMaterial>,
    ripple: RippleAnimation,
}

#[derive(Clone, Copy)]
enum RipplePhase {
    Idle,
    Expanding {
        started_at: Duration,
        target_tip_index: usize,
        color: Color,
    },
    Fading {
        started_at: Duration,
        target_tip_index: usize,
        color: Color,
    },
}

struct RippleAnimation {
    phase: RipplePhase,
    displayed_tip_index: usize,
    origin: Vec2,
    directions: Vec<Vec2>,
    boundary_distances: Vec<f32>,
    cover_radius: f32,
}

impl RippleAnimation {
    fn new(displayed_tip_index: usize) -> Self {
        let directions = (0..=TIP_OVERLAY_RIPPLE_SEGMENTS)
            .map(|step| {
                let angle = 2.0 * PI * step as f32 / TIP_OVERLAY_RIPPLE_SEGMENTS as f32;
                Vec2::new(angle.cos(), angle.sin())
            })
            .collect::<Vec<_>>();
        let boundary_distances = vec![0.0; directions.len()];

        Self {
            phase: RipplePhase::Idle,
            displayed_tip_index,
            origin: Vec2::ZERO,
            directions,
            boundary_distances,
            cover_radius: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
struct OutlinePoint {
    position: Vec2,
    outward: Vec2,
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

struct ProgressBorderGeometry {
    samples: Vec<BorderSample>,
}

#[derive(Clone, Copy)]
struct BorderSample {
    /// Normalized distance along the closed centerline.
    progress: f32,
    center: Vec2,
    vertices: BorderVertexPair,
}

/// One cached lap of the border, rebased so that progress `0.0` is the ripple origin.
struct ProgressBorderPath {
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

pub fn setup_overlay(
    mut commands: Commands,
    tips: Res<JobConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    let current_color = tip_color(&tips, 0);
    let next_index = 1 % tips.0.tips.len();
    let next_color = tip_color(&tips, next_index);
    let size = Vec2::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);
    let background_material =
        materials.add(ColorMaterial::from(tip_background_color(current_color)));
    commands.spawn((
        Mesh2d(meshes.add(rounded_rectangle_mesh(size, TIP_OVERLAY_CORNER_RADIUS))),
        MeshMaterial2d(background_material.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
        TipOverlay,
    ));

    let ripple = RippleAnimation::new(0);
    let ripple_mesh = meshes.add(ripple_mesh(ripple.directions.len()));
    let ripple_material = materials.add(border_material(color_with_alpha(current_color, 0.0)));
    commands.spawn((
        Mesh2d(ripple_mesh.clone()),
        MeshMaterial2d(ripple_material.clone()),
        Transform::from_xyz(0.0, 0.0, 0.5),
        TipOverlay,
    ));

    let border_geometry = progress_border_geometry();
    let border_path = border_geometry.path_from(0.0);
    let full_border_topology = VisibleBorderTopology {
        complete_segments: border_path.samples.len() - 1,
        has_partial_segment: false,
    };
    let played_material = materials.add(border_material(next_color));
    commands.spawn((
        Mesh2d(meshes.add(progress_border_mesh(&border_path))),
        MeshMaterial2d(played_material.clone()),
        Transform::from_xyz(0.0, 0.0, 1.0),
        TipOverlay,
    ));
    let remaining_material = materials.add(border_material(current_color));
    commands.spawn((
        Mesh2d(meshes.add(progress_border_mesh(&border_path))),
        MeshMaterial2d(remaining_material.clone()),
        Transform::from_xyz(0.0, 0.0, 2.0),
        ProgressBorder {
            progress: 1.0,
            geometry: border_geometry,
            path: border_path,
            topology: full_border_topology,
            background_material,
            played_material,
            remaining_material,
            ripple_mesh,
            ripple_material,
            ripple,
        },
        TipOverlay,
    ));

    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            TipOverlay,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(truncate_tip(&tips.0.tips[0].tip)),
                TextFont {
                    font: FontSource::SystemUi,
                    font_size: FontSize::Px(17.0),
                    ..default()
                },
                TextColor(COLOR_MAIN_TIP_TEXT),
                TextLayout::no_wrap(),
                Node {
                    max_width: px(136),
                    overflow: Overflow::clip_x(),
                    ..default()
                },
                TipText,
            ));
        });
}

pub(super) fn drag_overlay(
    mouse: Res<ButtonInput<MouseButton>>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        window.start_drag_move();
    }
}

fn truncate_tip(value: &str) -> String {
    // TODO_LviatYi: temporary solution
    // wait for animation to be implemented
    if display_width(value) <= MAX_TEXT_COLUMNS {
        return value.to_owned();
    }

    let target = MAX_TEXT_COLUMNS.saturating_sub(3);
    let mut width = 0;
    let mut output = String::new();
    for character in value.chars() {
        let character_width = character.width().unwrap_or(0);
        if width + character_width > target {
            break;
        }
        width += character_width;
        output.push(character);
    }
    output.push_str("...");
    output
}

pub(super) fn process_jobs(
    time: Res<Time<Real>>,
    jobs: Res<JobConfig>,
    mut state: ResMut<JobSensoryState>,
) {
    let changed = process_jobs_at(&jobs, state.bypass_change_detection(), time.elapsed());
    if changed {
        state.set_changed();
    }
}

fn process_jobs_at(jobs: &JobConfig, state: &mut JobSensoryState, now: Duration) -> bool {
    match state.focus_state.as_ref() {
        None => {
            if jobs.is_empty() {
                false
            } else {
                state.restart(now);
                true
            }
        }
        Some(inner_s) => match jobs.0.tips.get(inner_s.current_index) {
            None => {
                state.restart(now);
                true
            }
            Some(tip) => {
                if inner_s.elapsed_at(now) >= Duration::from_secs(tip.show_time()) {
                    let next_index = (inner_s.current_index + 1) % jobs.0.tips.len();
                    state.restart_at(next_index, now);
                    true
                } else {
                    false
                }
            }
        },
    }
}

pub fn render_job(
    time: Res<Time<Real>>,
    jobs: Res<JobConfig>,
    state: Res<JobSensoryState>,
    mut text: Single<&mut Text, With<TipText>>,
    progress_border: Single<(&mut ProgressBorder, &Mesh2d)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let Some(focus_state): Option<&FocusState> = state.focus_state.as_ref() else {
        return;
    };
    let Some(tip): Option<&Tip> = jobs.0.tips.get(focus_state.current_index) else {
        return;
    };

    let now = time.elapsed();
    let (mut progress_border, border_mesh) = progress_border.into_inner();
    if state.is_changed() {
        text.0 = truncate_tip(&tip.tip);
        let current_color = tip_color(&jobs, focus_state.current_index);
        let next_index = (focus_state.current_index + 1) % jobs.0.tips.len();
        let next_color = tip_color(&jobs, next_index);
        set_material_color(&mut materials, &progress_border.played_material, next_color);
        set_material_color(
            &mut materials,
            &progress_border.remaining_material,
            current_color,
        );
        if focus_state.current_index != progress_border.ripple.displayed_tip_index {
            begin_ripple_animation(
                &mut progress_border,
                now,
                focus_state.current_index,
                current_color,
            );
            set_material_color(
                &mut materials,
                &progress_border.ripple_material,
                current_color,
            );
            if let Some(mut mesh) = meshes.get_mut(&border_mesh.0) {
                reset_progress_border_mesh(&mut mesh, &progress_border.path);
            }
            progress_border.topology = VisibleBorderTopology {
                complete_segments: progress_border.path.samples.len() - 1,
                has_partial_segment: false,
            };
        }
    }
    update_ripple_animation(&mut progress_border, now, &mut meshes, &mut materials);

    let elapsed = focus_state.elapsed_at(now).as_secs_f32();
    let progress = (1.0 - elapsed / tip.show_time() as f32).clamp(0.0, 1.0);
    if progress != progress_border.progress {
        progress_border.progress = progress;
        if let Some(mut mesh) = meshes.get_mut(&border_mesh.0) {
            let slice = progress_border.path.slice_at(progress);
            update_progress_border_mesh(
                &mut mesh,
                &progress_border.path,
                progress_border.topology,
                &slice,
            );
            progress_border.topology = slice.topology;
        }
    }
}

fn tip_color(jobs: &JobConfig, tip_index: usize) -> Color {
    Color::Srgba(
        Srgba::hex(jobs.resolved_color_at(tip_index))
            .expect("tip colors are validated before startup"),
    )
}

fn tip_background_color(color: Color) -> Color {
    let color = color.to_srgba();
    Color::srgba(
        color.red * TIP_OVERLAY_BACKGROUND_BRIGHTNESS,
        color.green * TIP_OVERLAY_BACKGROUND_BRIGHTNESS,
        color.blue * TIP_OVERLAY_BACKGROUND_BRIGHTNESS,
        TIP_OVERLAY_BACKGROUND_ALPHA,
    )
}

fn border_material(color: Color) -> ColorMaterial {
    ColorMaterial {
        color,
        // Keep every overlay mesh in the transparent phase so Z controls their order.
        alpha_mode: AlphaMode2d::Blend,
        ..default()
    }
}

fn set_material_color(
    materials: &mut Assets<ColorMaterial>,
    handle: &Handle<ColorMaterial>,
    color: Color,
) {
    if let Some(mut material) = materials.get_mut(handle) {
        material.color = color;
    }
}

fn begin_ripple_animation(
    border: &mut ProgressBorder,
    now: Duration,
    target_tip_index: usize,
    color: Color,
) {
    let start_progress = rand::rng().random_range(0.0..1.0);
    border.path = border.geometry.path_from(start_progress);
    let origin = border.geometry.center_at(border.path.start_progress);

    let half_size = Vec2::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32) / 2.0;
    let ripple = &mut border.ripple;
    ripple.origin = origin;
    ripple.cover_radius = 0.0;
    for (direction, boundary_distance) in
        ripple.directions.iter().zip(&mut ripple.boundary_distances)
    {
        *boundary_distance = rounded_rectangle_ray_distance(
            ripple.origin,
            *direction,
            half_size,
            TIP_OVERLAY_CORNER_RADIUS,
        );
        ripple.cover_radius = ripple.cover_radius.max(*boundary_distance);
    }
    ripple.phase = RipplePhase::Expanding {
        started_at: now,
        target_tip_index,
        color,
    };
}

fn update_ripple_animation(
    border: &mut ProgressBorder,
    now: Duration,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) {
    let mut phase = border.ripple.phase;
    if let RipplePhase::Expanding {
        started_at,
        target_tip_index,
        color,
    } = phase
    {
        let linear_progress = (now.saturating_sub(started_at).as_secs_f32()
            / TIP_OVERLAY_RIPPLE_EXPAND_DURATION.as_secs_f32())
        .clamp(0.0, 1.0);
        let eased_progress = 1.0 - (1.0 - linear_progress).powi(3);
        if let Some(mut mesh) = meshes.get_mut(&border.ripple_mesh) {
            update_ripple_mesh(
                &mut mesh,
                &border.ripple,
                border.ripple.cover_radius * eased_progress,
            );
        }

        if linear_progress == 1.0 {
            set_material_color(
                materials,
                &border.background_material,
                tip_background_color(color),
            );
            phase = RipplePhase::Fading {
                started_at: started_at + TIP_OVERLAY_RIPPLE_EXPAND_DURATION,
                target_tip_index,
                color,
            };
        }
    }

    if let RipplePhase::Fading {
        started_at,
        target_tip_index,
        color,
    } = phase
    {
        let fade_progress = (now.saturating_sub(started_at).as_secs_f32()
            / TIP_OVERLAY_RIPPLE_FADE_DURATION.as_secs_f32())
        .clamp(0.0, 1.0);
        let alpha = (1.0 - fade_progress).powi(2);
        set_material_color(
            materials,
            &border.ripple_material,
            color_with_alpha(color, alpha),
        );
        if fade_progress == 1.0 {
            border.ripple.displayed_tip_index = target_tip_index;
            phase = RipplePhase::Idle;
        }
    }

    border.ripple.phase = phase;
}

fn ripple_mesh(perimeter_vertex_count: usize) -> Mesh {
    let positions = vec![[0.0, 0.0, 0.0]; perimeter_vertex_count + 1];
    let mut indices = Vec::with_capacity((perimeter_vertex_count - 1) * 3);
    for perimeter_index in 0..perimeter_vertex_count - 1 {
        indices.extend_from_slice(&[0, perimeter_index as u32 + 1, perimeter_index as u32 + 2]);
    }
    triangle_mesh(positions, indices)
}

fn update_ripple_mesh(mesh: &mut Mesh, ripple: &RippleAnimation, radius: f32) {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    else {
        return;
    };
    positions[0] = [ripple.origin.x, ripple.origin.y, 0.0];
    for (index, (direction, boundary_distance)) in ripple
        .directions
        .iter()
        .zip(&ripple.boundary_distances)
        .enumerate()
    {
        let point = ripple.origin + *direction * radius.min(*boundary_distance);
        positions[index + 1] = [point.x, point.y, 0.0];
    }
}

fn rounded_rectangle_ray_distance(
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

fn rounded_rectangle_contains(point: Vec2, half_size: Vec2, radius: f32) -> bool {
    let corner_offset = point.abs() - (half_size - Vec2::splat(radius));
    corner_offset.max(Vec2::ZERO).length_squared() <= radius * radius
}

fn color_with_alpha(color: Color, alpha: f32) -> Color {
    let color = color.to_srgba();
    Color::srgba(color.red, color.green, color.blue, alpha)
}

fn rounded_rectangle_mesh(size: Vec2, radius: f32) -> Mesh {
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

fn progress_border_geometry() -> ProgressBorderGeometry {
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

    ProgressBorderGeometry { samples }
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

impl ProgressBorderGeometry {
    fn center_at(&self, progress: f32) -> Vec2 {
        let (from, to, factor) = self.segment_at(progress);
        from.center.lerp(to.center, factor)
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

    fn path_from(&self, start_progress: f32) -> ProgressBorderPath {
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

fn progress_border_mesh(path: &ProgressBorderPath) -> Mesh {
    let mut positions = Vec::with_capacity(path.samples.len() * 2 + 2);
    for sample in &path.samples {
        push_border_vertices(&mut positions, sample.vertices);
    }

    // This final pair is a reusable endpoint for the one partially visible segment.
    push_border_vertices(&mut positions, path.samples.last().unwrap().vertices);
    triangle_mesh(positions, path.full_indices.clone())
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

fn rounded_rectangle_outline(half_size: Vec2, radius: f32) -> Vec<OutlinePoint> {
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

fn triangle_mesh(positions: Vec<[f32; 3]>, indices: Vec<u32>) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn display_width(value: &str) -> usize {
    value
        .chars()
        .map(|character| character.width().unwrap_or(0))
        .sum()
}
