use super::{
    geometry::rounded_rectangle_mesh,
    progress_border::{
        CountdownBorder, CountdownBorderShade, ProgressBorderGeometry, progress_border_mesh,
    },
    ripple::{Ripple, ripple_mesh},
    style::{
        color_with_alpha, overlay_material, set_material_color, tip_background_color, tip_color,
    },
    text_scroll::{TextScroll, TipTextViewport},
};
use crate::{
    job::JobConfig,
    sensory::job_manager::ActiveJobState,
    settings_default_values::{TIP_OVERLAY_CORNER_RADIUS, WINDOW_HEIGHT, WINDOW_WIDTH},
};
use bevy::{prelude::*, window::PrimaryWindow};

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
        Mesh2d(countdown_mesh.clone()),
        MeshMaterial2d(materials.add(overlay_material(current_color))),
        Transform::from_xyz(0.0, 0.0, 2.0),
        CountdownBorder::new(border_path),
        TipOverlay,
    ));
    commands.spawn((
        // Sharing the mesh keeps the shade aligned with the shrinking countdown.
        Mesh2d(countdown_mesh),
        MeshMaterial2d(materials.add(overlay_material(Color::srgba(0.0, 0.0, 0.0, 0.0)))),
        Transform::from_xyz(0.0, 0.0, 2.1),
        CountdownBorderShade,
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
            parent
                .spawn((
                    Node {
                        width: px(136),
                        height: percent(100),
                        align_items: AlignItems::Center,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    TipTextViewport,
                ))
                .with_children(|viewport| {
                    viewport.spawn((
                        Text::new(tips.0.tips[0].tip.replace(['\r', '\n'], " ")),
                        TextFont {
                            font: FontSource::SystemUi,
                            font_size: FontSize::Px(17.0),
                            ..default()
                        },
                        TextColor(COLOR_MAIN_TIP_TEXT),
                        TextLayout::no_wrap(),
                        Node {
                            flex_shrink: 0.0,
                            margin: UiRect::horizontal(Val::Auto),
                            ..default()
                        },
                        TextScroll::default(),
                        TipText,
                    ));
                });
        });
}

pub(super) fn sync_tip_text(
    jobs: Res<JobConfig>,
    state: Res<ActiveJobState>,
    mut text: Single<&mut Text, With<TipText>>,
) {
    let Some(active_job) = state.active_job.as_ref() else {
        return;
    };
    let Some(tip) = jobs.0.tips.get(active_job.current_index) else {
        return;
    };
    let content = tip.tip.replace(['\r', '\n'], " ");
    if text.0 != content {
        text.0 = content;
    }
}

pub(super) fn sync_tip_colors(
    jobs: Res<JobConfig>,
    state: Res<ActiveJobState>,
    played: Single<&MeshMaterial2d<ColorMaterial>, With<PlayedBorder>>,
    countdown: Single<&MeshMaterial2d<ColorMaterial>, With<CountdownBorder>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let Some(active_job) = state.active_job.as_ref() else {
        return;
    };

    set_material_color(
        &mut materials,
        &played.0,
        tip_color(&jobs, active_job.preview_next_index),
    );
    set_material_color(
        &mut materials,
        &countdown.0,
        tip_color(&jobs, active_job.current_index),
    );
}

pub(super) fn drag_overlay(
    mouse: Res<ButtonInput<MouseButton>>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        window.start_drag_move();
    }
}
