use super::{
    geometry::{BorderGeometry, icon_marker_mesh},
    progress_border::{BorderProgress, CountdownText},
    style::{base_color, color_material, set_material_color, tip_color},
    text_scroll::{TextScroll, TipTextViewport},
    visual_config::{
        LayerSizing, RotationNoticeVisualConfig, layer_color, layer_image, layer_sprite,
    },
};
use crate::{
    controller::ActiveJobState,
    job::JobConfig,
    settings_default_values::{WINDOW_HEIGHT, WINDOW_WIDTH},
};
use bevy::{
    prelude::*,
    sprite::{SpriteImageMode, SpriteScalingMode},
    text::FontSource,
    window::PrimaryWindow,
};

const ICON_CENTER_X: f32 = -(WINDOW_WIDTH as f32) / 2.0 + 20.0;
const ICON_SIZE: Vec2 = Vec2::splat(26.0);
const CONTENT_COLOR: Color = Color::srgb(0.94, 0.96, 1.0);

#[derive(Component)]
pub(super) struct TipText;
#[derive(Component)]
pub(super) struct BaseFallback;
#[derive(Component)]
pub(super) struct IconFallback;
#[derive(Component)]
pub(super) struct IconMarker;
#[derive(Component)]
pub(super) struct BaseArt;
#[derive(Component)]
pub(super) struct IconBackgroundArt;
#[derive(Component, Default)]
pub(super) struct IconArt {
    has_source: bool,
}
#[derive(Component)]
pub(super) struct CountdownBackground;

pub(super) fn setup_overlay(
    mut commands: Commands,
    jobs: Res<JobConfig>,
    visuals: Res<RotationNoticeVisualConfig>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let theme = tip_color(&jobs, 0);
    let border_width = if visuals.border_width.is_finite()
        && visuals.border_width > 0.0
        && visuals.border_width < WINDOW_HEIGHT as f32 / 4.0
    {
        visuals.border_width
    } else {
        eprintln!("[Sensation] Invalid border width; using 2 pixels");
        2.0
    };
    let geometry = BorderGeometry::load(visuals.border_path.as_deref(), border_width);
    let fill_mesh = meshes.add(geometry.fill_mesh());
    let marker_mesh = meshes.add(icon_marker_mesh());
    let border = BorderProgress::new(geometry);
    let border_mesh = meshes.add(border.mesh());
    let base_material = materials.add(color_material(base_color(
        theme,
        visuals.background_opacity,
    )));
    let marker_material = materials.add(color_material(theme));
    let border_material = materials.add(color_material(theme));

    commands.spawn(Camera2d);
    commands.spawn((
        Mesh2d(fill_mesh),
        MeshMaterial2d(base_material),
        Transform::from_xyz(0.0, 0.0, 0.0),
        BaseFallback,
    ));
    let mut base_art = layer_sprite(
        &asset_server,
        visuals.base_background.as_ref(),
        Vec2::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32),
    );
    base_art.color = layer_color(
        visuals.base_background.as_ref(),
        theme,
        visuals.background_opacity,
    );
    commands.spawn((base_art, Transform::from_xyz(0.0, 0.0, 0.1), BaseArt));

    commands.spawn((
        Sprite::from_color(
            icon_background_color(theme, visuals.background_opacity),
            ICON_SIZE,
        ),
        Transform::from_xyz(ICON_CENTER_X, 0.0, 0.3),
        IconFallback,
    ));
    let mut icon_background =
        layer_sprite(&asset_server, visuals.icon_background.as_ref(), ICON_SIZE);
    icon_background.color = layer_color(
        visuals.icon_background.as_ref(),
        theme,
        visuals.background_opacity,
    );
    commands.spawn((
        icon_background,
        Transform::from_xyz(ICON_CENTER_X, 0.0, 0.4),
        IconBackgroundArt,
    ));
    commands.spawn((
        Mesh2d(marker_mesh),
        MeshMaterial2d(marker_material),
        Transform::from_xyz(ICON_CENTER_X, 0.0, 0.7),
        IconMarker,
    ));
    commands.spawn((
        Sprite::from_color(Color::NONE, Vec2::splat(14.0)),
        Transform::from_xyz(ICON_CENTER_X, 0.0, 0.8),
        IconArt::default(),
    ));
    commands.spawn((
        Mesh2d(border_mesh),
        MeshMaterial2d(border_material),
        Transform::from_xyz(0.0, 0.0, 2.0),
        border,
    ));

    let current_tip = jobs.0.jobs[0].tip().text.replace(['\r', '\n'], " ");
    let countdown_art = layer_image(&asset_server, visuals.countdown_background.as_ref());
    let countdown_stretches = visuals
        .countdown_background
        .as_ref()
        .is_some_and(|asset| !matches!(asset.sizing, LayerSizing::Original));
    let countdown_size = if countdown_stretches {
        percent(100)
    } else {
        Val::Auto
    };
    commands
        .spawn(Node {
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|root| {
            root.spawn(Node {
                width: px(WINDOW_WIDTH),
                height: px(WINDOW_HEIGHT),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|board| {
                board
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(40),
                            top: px(4),
                            width: px(112),
                            height: px(20),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::FlexStart,
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        TipTextViewport,
                    ))
                    .with_children(|viewport| {
                        viewport.spawn((
                            Text(current_tip),
                            TextFont {
                                font: FontSource::SystemUi,
                                font_size: FontSize::Px(13.0),
                                ..default()
                            },
                            TextColor(CONTENT_COLOR),
                            TextLayout::no_wrap(),
                            Node {
                                flex_shrink: 0.0,
                                ..default()
                            },
                            TextScroll::default(),
                            TipText,
                        ));
                    });
                board
                    .spawn(Node {
                        position_type: PositionType::Absolute,
                        right: px(8),
                        bottom: px(3),
                        padding: UiRect::axes(px(3), px(1)),
                        ..default()
                    })
                    .with_children(|countdown| {
                        countdown.spawn((
                            countdown_art,
                            Node {
                                position_type: PositionType::Absolute,
                                right: px(0),
                                bottom: px(0),
                                width: countdown_size,
                                height: countdown_size,
                                ..default()
                            },
                            CountdownBackground,
                        ));
                        countdown.spawn((
                            Text("--".into()),
                            TextFont {
                                font: FontSource::SystemUi,
                                font_size: FontSize::Px(10.0),
                                ..default()
                            },
                            TextColor(theme),
                            CountdownText,
                        ));
                    });
            });
        });
}

fn icon_background_color(theme: Color, opacity: f32) -> Color {
    let theme = theme.to_srgba();
    Color::srgba(
        theme.red * 0.15,
        theme.green * 0.15,
        theme.blue * 0.15,
        (opacity * 0.75).clamp(0.0, 1.0),
    )
}

pub(super) fn sync_notice(
    jobs: Res<JobConfig>,
    state: Res<ActiveJobState>,
    visuals: Res<RotationNoticeVisualConfig>,
    asset_server: Res<AssetServer>,
    mut text: Single<&mut Text, With<TipText>>,
    mut countdown_color: Single<&mut TextColor, With<CountdownText>>,
    mut countdown_art: Single<&mut ImageNode, With<CountdownBackground>>,
    mut sprites: Query<(
        &mut Sprite,
        Option<&BaseArt>,
        Option<&IconFallback>,
        Option<&IconBackgroundArt>,
        Option<&mut IconArt>,
    )>,
    materials_query: Query<(
        &MeshMaterial2d<ColorMaterial>,
        Option<&BaseFallback>,
        Option<&IconMarker>,
        Option<&BorderProgress>,
    )>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let Some(active) = state.active_job.as_ref() else {
        return;
    };
    let tip = jobs.0.jobs[active.current_index].tip();
    let theme = tip_color(&jobs, active.current_index);
    let content = tip.text.replace(['\r', '\n'], " ");
    if text.0 != content {
        text.0 = content;
    }
    countdown_color.0 = theme;
    countdown_art.color = layer_color(
        visuals.countdown_background.as_ref(),
        theme,
        visuals.background_opacity,
    );
    for (mut sprite, base_art, icon_fallback, icon_background, icon_art) in &mut sprites {
        if base_art.is_some() {
            sprite.color = layer_color(
                visuals.base_background.as_ref(),
                theme,
                visuals.background_opacity,
            );
        } else if icon_fallback.is_some() {
            sprite.color = icon_background_color(theme, visuals.background_opacity);
        } else if icon_background.is_some() {
            sprite.color = layer_color(
                visuals.icon_background.as_ref(),
                theme,
                visuals.background_opacity,
            );
        } else if let Some(mut icon_art) = icon_art {
            let icon_path = tip.icon.as_deref().filter(|path| !path.trim().is_empty());
            icon_art.has_source = icon_path.is_some();
            if let Some(path) = icon_path {
                sprite.image = asset_server.load(path.to_owned());
                sprite.color = theme;
                sprite.custom_size = Some(Vec2::splat(14.0));
                sprite.image_mode = SpriteImageMode::Scale(SpriteScalingMode::FitCenter);
            } else {
                sprite.color = Color::NONE;
            }
        }
    }
    for (handle, base, marker, border) in &materials_query {
        let color = if base.is_some() {
            base_color(theme, visuals.background_opacity)
        } else if marker.is_some() || border.is_some() {
            theme
        } else {
            continue;
        };
        set_material_color(&mut materials, &handle.0, color);
    }
}

pub(super) fn sync_overlay_visibility(
    state: Res<ActiveJobState>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    window.visible = state.active_job.is_some();
}

pub(super) fn sync_icon_placeholder(
    image: Single<(&Sprite, &IconArt)>,
    images: Res<Assets<Image>>,
    marker: Single<&mut Visibility, With<IconMarker>>,
) {
    let (sprite, icon) = image.into_inner();
    *marker.into_inner() = if icon.has_source && images.get(&sprite.image).is_some() {
        Visibility::Hidden
    } else {
        Visibility::Visible
    };
}

pub(super) fn drag_overlay(
    mouse: Res<ButtonInput<MouseButton>>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        window.start_drag_move();
    }
}
