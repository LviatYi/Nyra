use super::{
    geometry::rounded_rectangle_mesh,
    progress_border::{
        CountdownBorder, CountdownBorderShade, ProgressBorderGeometry, ProgressBorderPath,
        progress_border_mesh,
    },
    ripple::{Ripple, ripple_mesh},
    style::{
        color_with_alpha, overlay_material, set_material_color, tip_background_color, tip_color,
    },
    text_scroll::{TextScroll, TipTextViewport},
};
use crate::{
    job::JobConfig,
    sensation::job_manager::ActiveJobState,
    settings_default_values::{TIP_OVERLAY_CORNER_RADIUS, WINDOW_HEIGHT, WINDOW_WIDTH},
};
use bevy::{prelude::*, text::FontSourceTemplate, window::PrimaryWindow};

const COLOR_MAIN_TIP_TEXT: Color = Color::srgb(0.94, 0.96, 1.0);

#[derive(Component, Default, Clone)]
pub(super) struct TipOverlay;

#[derive(Component, Default, Clone)]
pub(super) struct TipText;

#[derive(Component, Default, Clone)]
pub(super) struct TipBackground;

#[derive(Component, Default, Clone)]
pub(super) struct PlayedBorder;

struct OverlaySceneAssets {
    background_mesh: Handle<Mesh>,
    background_material: Handle<ColorMaterial>,
    ripple_mesh: Handle<Mesh>,
    ripple_material: Handle<ColorMaterial>,
    played_mesh: Handle<Mesh>,
    played_material: Handle<ColorMaterial>,
    countdown_mesh: Handle<Mesh>,
    countdown_material: Handle<ColorMaterial>,
    shade_material: Handle<ColorMaterial>,
}

pub(super) fn setup_overlay(
    mut commands: Commands,
    tips: Res<JobConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let current_color = tip_color(&tips, 0);
    let next_index = 1 % tips.0.tips.len();
    let next_color = tip_color(&tips, next_index);
    let size = Vec2::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);
    let background_material =
        materials.add(ColorMaterial::from(tip_background_color(current_color)));
    let background_mesh = meshes.add(rounded_rectangle_mesh(size, TIP_OVERLAY_CORNER_RADIUS));

    let ripple = Ripple::new(0);
    let ripple_mesh = meshes.add(ripple_mesh(&ripple));
    let ripple_material = materials.add(overlay_material(color_with_alpha(current_color, 0.0)));

    let border_geometry = ProgressBorderGeometry::new();
    let border_path = border_geometry.path_from(0.0);
    let played_mesh = meshes.add(progress_border_mesh(&border_path));
    let countdown_mesh = meshes.add(progress_border_mesh(&border_path));
    let scene_assets = OverlaySceneAssets {
        background_mesh,
        background_material,
        ripple_mesh,
        ripple_material,
        played_mesh,
        played_material: materials.add(overlay_material(next_color)),
        countdown_mesh,
        countdown_material: materials.add(overlay_material(current_color)),
        shade_material: materials.add(overlay_material(Color::srgba(0.0, 0.0, 0.0, 0.0))),
    };
    let current_tip = tips.0.tips[0].tip.replace(['\r', '\n'], " ");

    commands.spawn_scene_list(overlay_scene(scene_assets, current_tip, border_path));
    commands.insert_resource(border_geometry);
}

fn overlay_scene(
    assets: OverlaySceneAssets,
    current_tip: String,
    countdown_path: ProgressBorderPath,
) -> impl SceneList {
    let OverlaySceneAssets {
        background_mesh,
        background_material,
        ripple_mesh,
        ripple_material,
        played_mesh,
        played_material,
        countdown_mesh,
        countdown_material,
        shade_material,
    } = assets;
    // Sharing this handle keeps the shade aligned with the shrinking countdown mesh.
    let shade_mesh = countdown_mesh.clone();

    bsn_list![
        Camera2d,
        (
            Mesh2d(background_mesh)
            MeshMaterial2d::<ColorMaterial>(background_material)
            Transform::from_xyz(0.0, 0.0, 0.0)
            TipBackground
            TipOverlay
        ),
        (
            Mesh2d(ripple_mesh)
            MeshMaterial2d::<ColorMaterial>(ripple_material)
            Transform::from_xyz(0.0, 0.0, 0.5)
            template(|_| Ok(Ripple::new(0)))
            TipOverlay
        ),
        (
            Mesh2d(played_mesh)
            MeshMaterial2d::<ColorMaterial>(played_material)
            Transform::from_xyz(0.0, 0.0, 1.0)
            PlayedBorder
            TipOverlay
        ),
        (
            Mesh2d(countdown_mesh)
            MeshMaterial2d::<ColorMaterial>(countdown_material)
            Transform::from_xyz(0.0, 0.0, 2.0)
            template(move |_| Ok(CountdownBorder::new(countdown_path.clone())))
            TipOverlay
        ),
        (
            Mesh2d(shade_mesh)
            MeshMaterial2d::<ColorMaterial>(shade_material)
            Transform::from_xyz(0.0, 0.0, 2.1)
            CountdownBorderShade
            TipOverlay
        ),
        (
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
            }
            TipOverlay
            Children [
                (
                    Node {
                        width: px(136),
                        height: percent(100),
                        align_items: AlignItems::Center,
                        overflow: Overflow::clip(),
                    }
                    TipTextViewport
                    Children [
                        (
                            Text(current_tip)
                            TextFont {
                                font: FontSourceTemplate::SystemUi,
                                font_size: FontSize::Px(17.0),
                            }
                            TextColor(COLOR_MAIN_TIP_TEXT)
                            template_value(TextLayout::no_wrap())
                            Node {
                                flex_shrink: 0.0,
                                margin: UiRect::horizontal(Val::Auto),
                            }
                            TextScroll
                            TipText
                        )
                    ]
                )
            ]
        )
    ]
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
