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
use std::f32::consts::{FRAC_PI_2, PI};
use std::time::Instant;
use unicode_width::UnicodeWidthChar;

const COLOR_MAIN_TIP_BACKGROUND: Color = Color::srgba(0.045, 0.055, 0.075, 0.92);
const COLOR_MAIN_TIP_TEXT: Color = Color::srgb(0.94, 0.96, 1.0);
const COLOR_PROGRESS_BORDER: Color = Color::srgb(0.20, 0.88, 0.48);

#[derive(Component)]
pub struct TipOverlay;

#[derive(Component)]
pub struct TipText;

#[derive(Component)]
pub struct ProgressBorder {
    /// Visible fraction of the rounded outline, in the inclusive range `0.0..=1.0`.
    progress: f32,
    index_state: BorderIndexState,
    geometry: ProgressBorderGeometry,
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
    /// Rounded-outline vertices keyed by their normalized distance from the start.
    samples: Vec<(f32, BorderVertexPair)>,
    full_indices: Vec<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct BorderIndexState {
    full_segments: usize,
    has_terminal_segment: bool,
}

#[derive(Clone, Copy)]
struct ProgressBorderSlice {
    index_state: BorderIndexState,
    terminal: Option<BorderVertexPair>,
}

pub fn setup_overlay(
    mut commands: Commands,
    tips: Res<JobConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    let size = Vec2::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);
    commands.spawn((
        Mesh2d(meshes.add(rounded_rectangle_mesh(size, TIP_OVERLAY_CORNER_RADIUS))),
        MeshMaterial2d(materials.add(ColorMaterial::from(COLOR_MAIN_TIP_BACKGROUND))),
        Transform::from_xyz(0.0, 0.0, 0.0),
        TipOverlay,
    ));

    let border_material = ColorMaterial {
        color: COLOR_PROGRESS_BORDER,
        // Keep the background and border in the same transparent render phase so
        // their Z values determine their order.
        alpha_mode: AlphaMode2d::Blend,
        ..default()
    };
    let border_geometry = progress_border_geometry();
    let border_index_state = BorderIndexState {
        full_segments: border_geometry.samples.len() - 1,
        has_terminal_segment: false,
    };
    commands.spawn((
        Mesh2d(meshes.add(progress_border_mesh(&border_geometry))),
        MeshMaterial2d(materials.add(border_material)),
        Transform::from_xyz(0.0, 0.0, 1.0),
        ProgressBorder {
            progress: 1.0,
            index_state: border_index_state,
            geometry: border_geometry,
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

pub(super) fn process_jobs(jobs: Res<JobConfig>, mut state: ResMut<JobSensoryState>) {
    match state.focus_state.as_ref() {
        None => {
            if !jobs.is_empty() {
                state.restart();
            }

            return;
        }
        Some(inner_s) => match jobs.0.tips.get(inner_s.current_index) {
            None => {
                state.restart();
            }
            Some(tip) => {
                if Instant::now()
                    .duration_since(inner_s.focus_at_time)
                    .as_secs_f32()
                    >= tip.show_time() as f32
                {
                    let next_index = (inner_s.current_index + 1) % jobs.0.tips.len();
                    state.restart_at(next_index);
                }
            }
        },
    }
}

pub fn render_job(
    jobs: Res<JobConfig>,
    state: Res<JobSensoryState>,
    mut text: Single<&mut Text, With<TipText>>,
    progress_border: Single<(&mut ProgressBorder, &Mesh2d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(focus_state): Option<&FocusState> = state.focus_state.as_ref() else {
        return;
    };
    let Some(tip): Option<&Tip> = jobs.0.tips.get(focus_state.current_index) else {
        return;
    };

    if state.is_changed() {
        text.0 = truncate_tip(&tip.tip);
    }

    let elapsed = focus_state.focus_at_time.elapsed().as_secs_f32();
    let progress = (1.0 - elapsed / tip.show_time() as f32).clamp(0.0, 1.0);
    let (mut progress_border, border_mesh) = progress_border.into_inner();
    if progress != progress_border.progress {
        progress_border.progress = progress;
        if let Some(mut mesh) = meshes.get_mut(&border_mesh.0) {
            update_progress_border_mesh(&mut mesh, &mut progress_border, progress);
        }
    }
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
    samples.push((0.0, border_vertex_pair(centerline[0])));
    for points in centerline.windows(2) {
        traversed += points[0].position.distance(points[1].position);
        samples.push(((traversed / total_length).min(1.0), border_vertex_pair(points[1])));
    }

    let mut full_indices = Vec::with_capacity((samples.len() - 1) * 6);
    for segment in 0..samples.len() - 1 {
        push_border_segment_indices(&mut full_indices, segment as u32, segment as u32 + 1);
    }

    ProgressBorderGeometry {
        samples,
        full_indices,
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

fn progress_border_mesh(geometry: &ProgressBorderGeometry) -> Mesh {
    let mut positions = Vec::with_capacity(geometry.samples.len() * 2 + 2);
    for (_, pair) in &geometry.samples {
        positions.extend_from_slice(&[pair.outer, pair.inner]);
    }

    // These last two vertices are the only positions changed while progress moves
    // within a cached segment.
    let last_pair = geometry.samples.last().unwrap().1;
    positions.extend_from_slice(&[last_pair.outer, last_pair.inner]);

    triangle_mesh(positions, geometry.full_indices.clone())
}

fn update_progress_border_mesh(mesh: &mut Mesh, border: &mut ProgressBorder, progress: f32) {
    let slice = progress_border_slice(&border.geometry, progress);

    if let Some(terminal) = slice.terminal
        && let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        let terminal_index = border.geometry.samples.len() * 2;
        positions[terminal_index] = terminal.outer;
        positions[terminal_index + 1] = terminal.inner;
    }

    if slice.index_state == border.index_state {
        return;
    }

    let Some(Indices::U32(indices)) = mesh.indices_mut() else {
        return;
    };
    let cached_index_count = slice.index_state.full_segments * 6;
    indices.clear();
    indices.extend_from_slice(&border.geometry.full_indices[..cached_index_count]);
    if slice.index_state.has_terminal_segment {
        push_border_segment_indices(
            indices,
            slice.index_state.full_segments as u32,
            border.geometry.samples.len() as u32,
        );
    }
    border.index_state = slice.index_state;
}

fn progress_border_slice(geometry: &ProgressBorderGeometry, progress: f32) -> ProgressBorderSlice {
    let progress = progress.clamp(0.0, 1.0);
    if progress == 0.0 {
        return ProgressBorderSlice {
            index_state: BorderIndexState {
                full_segments: 0,
                has_terminal_segment: false,
            },
            terminal: None,
        };
    }
    if progress == 1.0 {
        return ProgressBorderSlice {
            index_state: BorderIndexState {
                full_segments: geometry.samples.len() - 1,
                has_terminal_segment: false,
            },
            terminal: None,
        };
    }

    let upper_index = geometry
        .samples
        .partition_point(|(cached_progress, _)| *cached_progress < progress);
    let lower_index = upper_index - 1;
    let (lower_progress, lower_pair) = geometry.samples[lower_index];
    let (upper_progress, upper_pair) = geometry.samples[upper_index];
    let factor = (progress - lower_progress) / (upper_progress - lower_progress);

    ProgressBorderSlice {
        index_state: BorderIndexState {
            full_segments: lower_index,
            has_terminal_segment: true,
        },
        terminal: Some(lower_pair.lerp(upper_pair, factor)),
    }
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
