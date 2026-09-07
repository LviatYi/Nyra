use super::{
    geometry::rounded_rectangle_mesh,
    progress_border::{CountdownBorder, ProgressBorderGeometry, progress_border_mesh},
    ripple::{Ripple, ripple_mesh},
    style::{
        color_with_alpha, overlay_material, set_material_color, tip_background_color, tip_color,
    },
};
use crate::{
    MAX_TEXT_COLUMNS,
    job::JobConfig,
    sensory::job::JobSensoryState,
    settings_default_values::{TIP_OVERLAY_CORNER_RADIUS, WINDOW_HEIGHT, WINDOW_WIDTH},
};
use bevy::{prelude::*, window::PrimaryWindow};
use unicode_width::UnicodeWidthChar;

const COLOR_MAIN_TIP_TEXT: Color = Color::srgb(0.94, 0.96, 1.0);

#[derive(Component)]
pub(super) struct TipOverlay;

#[derive(Component)]
pub(super) struct TipText;

#[derive(Component)]
pub(super) struct TipBackground;

#[derive(Component)]
pub(super) struct PlayedBorder;

pub(super) fn setup_overlay(
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
        MeshMaterial2d(background_material),
        Transform::from_xyz(0.0, 0.0, 0.0),
        TipBackground,
        TipOverlay,
    ));

    let ripple = Ripple::new(0);
    let ripple_mesh = meshes.add(ripple_mesh(&ripple));
    let ripple_material = materials.add(overlay_material(color_with_alpha(current_color, 0.0)));
    commands.spawn((
        Mesh2d(ripple_mesh),
        MeshMaterial2d(ripple_material),
        Transform::from_xyz(0.0, 0.0, 0.5),
        ripple,
        TipOverlay,
    ));

    let border_geometry = ProgressBorderGeometry::new();
    let border_path = border_geometry.path_from(0.0);
    let played_mesh = meshes.add(progress_border_mesh(&border_path));
    let countdown_mesh = meshes.add(progress_border_mesh(&border_path));
    commands.spawn((
        Mesh2d(played_mesh),
        MeshMaterial2d(materials.add(overlay_material(next_color))),
        Transform::from_xyz(0.0, 0.0, 1.0),
        PlayedBorder,
        TipOverlay,
    ));
    commands.spawn((
        Mesh2d(countdown_mesh),
        MeshMaterial2d(materials.add(overlay_material(current_color))),
        Transform::from_xyz(0.0, 0.0, 2.0),
        CountdownBorder::new(border_path),
        TipOverlay,
    ));
    commands.insert_resource(border_geometry);

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

pub(super) fn sync_tip_text(
    jobs: Res<JobConfig>,
    state: Res<JobSensoryState>,
    mut text: Single<&mut Text, With<TipText>>,
) {
    if !state.is_changed() {
        return;
    }
    let Some(focus_state) = state.focus_state.as_ref() else {
        return;
    };
    let Some(tip) = jobs.0.tips.get(focus_state.current_index) else {
        return;
    };
    text.0 = truncate_tip(&tip.tip);
}

pub(super) fn sync_tip_colors(
    jobs: Res<JobConfig>,
    state: Res<JobSensoryState>,
    played: Single<&MeshMaterial2d<ColorMaterial>, With<PlayedBorder>>,
    countdown: Single<&MeshMaterial2d<ColorMaterial>, With<CountdownBorder>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if !state.is_changed() {
        return;
    }
    let Some(focus_state) = state.focus_state.as_ref() else {
        return;
    };

    let current_color = tip_color(&jobs, focus_state.current_index);
    let next_index = (focus_state.current_index + 1) % jobs.0.tips.len();
    set_material_color(&mut materials, &played.0, tip_color(&jobs, next_index));
    set_material_color(&mut materials, &countdown.0, current_color);
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

fn display_width(value: &str) -> usize {
    value
        .chars()
        .map(|character| character.width().unwrap_or(0))
        .sum()
}
