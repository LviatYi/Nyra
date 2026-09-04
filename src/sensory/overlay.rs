use crate::MAX_TEXT_COLUMNS;
use crate::job::{JobConfig, Tip};
use crate::sensory::job::{FocusState, JobSensoryState};
use crate::settings_default_values::{WINDOW_HEIGHT, WINDOW_WIDTH};
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*,
    render::render_resource::PrimitiveTopology, sprite_render::AlphaMode2d, window::PrimaryWindow,
};
use std::f32::consts::{FRAC_PI_2, PI};
use std::time::Instant;
use unicode_width::UnicodeWidthChar;

const BORDER_WIDTH: f32 = 3.0;
const CORNER_RADIUS: f32 = 12.0;
const CORNER_SEGMENTS: usize = 12;
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
}

#[derive(Clone, Copy)]
struct OutlinePoint {
    position: Vec2,
    outward: Vec2,
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
        Mesh2d(meshes.add(rounded_rectangle_mesh(size, CORNER_RADIUS))),
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
    commands.spawn((
        Mesh2d(meshes.add(progress_border_mesh(1.0))),
        MeshMaterial2d(materials.add(border_material)),
        Transform::from_xyz(0.0, 0.0, 1.0),
        ProgressBorder { progress: 1.0 },
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
                    .as_secs()
                    > tip.show_time()
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
            *mesh = progress_border_mesh(progress);
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

fn progress_border_mesh(length: f32) -> Mesh {
    let half_size = Vec2::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32) / 2.0;
    let centerline_radius = CORNER_RADIUS - BORDER_WIDTH / 2.0;
    let centerline = rounded_rectangle_outline(
        half_size - Vec2::splat(BORDER_WIDTH / 2.0),
        centerline_radius,
    );
    let visible_outline = outline_prefix(&centerline, length);

    let mut positions = Vec::with_capacity(visible_outline.len() * 2);
    for point in &visible_outline {
        let offset = point.outward * (BORDER_WIDTH / 2.0);
        let outer = point.position + offset;
        let inner = point.position - offset;
        positions.push([outer.x, outer.y, 0.0]);
        positions.push([inner.x, inner.y, 0.0]);
    }

    let mut indices = Vec::with_capacity(visible_outline.len().saturating_sub(1) * 6);
    for index in 0..visible_outline.len().saturating_sub(1) as u32 {
        let outer = index * 2;
        let inner = outer + 1;
        let next_outer = outer + 2;
        let next_inner = outer + 3;
        indices.extend_from_slice(&[outer, inner, next_outer, inner, next_inner, next_outer]);
    }

    triangle_mesh(positions, indices)
}

fn rounded_rectangle_outline(half_size: Vec2, radius: f32) -> Vec<OutlinePoint> {
    let mut points = Vec::with_capacity(CORNER_SEGMENTS * 4 + 6);
    points.push(OutlinePoint {
        position: Vec2::new(0.0, half_size.y),
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
    points.push(OutlinePoint {
        position: Vec2::new(0.0, half_size.y),
        outward: Vec2::Y,
    });
    points
}

fn push_corner(points: &mut Vec<OutlinePoint>, center: Vec2, radius: f32, start_angle: f32) {
    for step in 1..=CORNER_SEGMENTS {
        let angle = start_angle - FRAC_PI_2 * step as f32 / CORNER_SEGMENTS as f32;
        let outward = Vec2::new(angle.cos(), angle.sin());
        points.push(OutlinePoint {
            position: center + outward * radius,
            outward,
        });
    }
}

fn outline_prefix(outline: &[OutlinePoint], length: f32) -> Vec<OutlinePoint> {
    let target_length = outline
        .windows(2)
        .map(|points| points[0].position.distance(points[1].position))
        .sum::<f32>()
        * length.clamp(0.0, 1.0);

    if target_length == 0.0 {
        return Vec::new();
    }

    let mut result = vec![outline[0]];
    let mut remaining = target_length;
    for points in outline.windows(2) {
        let segment_length = points[0].position.distance(points[1].position);
        if remaining >= segment_length {
            result.push(points[1]);
            remaining -= segment_length;
            continue;
        }

        let factor = remaining / segment_length;
        result.push(OutlinePoint {
            position: points[0].position.lerp(points[1].position, factor),
            outward: points[0]
                .outward
                .lerp(points[1].outward, factor)
                .normalize(),
        });
        break;
    }
    result
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
