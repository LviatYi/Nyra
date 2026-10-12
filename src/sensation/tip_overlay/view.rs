use super::{
    geometry::{BorderGeometry, icon_marker_mesh},
    progress_border::{BorderHead, BorderHeadGlow, BorderProgress, BorderTrack, CountdownText},
    style::{border_track_color, color_material, set_material_color, tip_color},
    text_scroll::{TextScroll, TipTextViewport},
    visual_config::{RotationNoticeVisualConfig, layer_color, layer_sprite},
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
    window::{CursorOptions, PrimaryWindow, WindowPosition},
};

const ICON_CENTER_X: f32 = -(WINDOW_WIDTH as f32) / 2.0 + 30.0;
const ICON_SIZE: Vec2 = Vec2::splat(32.0);
const CONTENT_COLOR: Color = Color::srgb(0.94, 0.96, 1.0);
const HEAD_GLOW_RADIUS: f32 = 4.5;

#[derive(Resource, Default)]
pub(super) struct NoticePlacement {
    center: Option<Vec2>,
    canvas_size: Option<Vec2>,
    drag_offset: Option<Vec2>,
}

#[derive(Component)]
pub(super) struct NoticeWorldRoot;
#[derive(Component)]
pub(super) struct NoticeUiBoard;

#[derive(Component)]
pub(super) struct TipText;
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
    let track_mesh = meshes.add(geometry.progress_mesh(1.0));
    let marker_mesh = meshes.add(icon_marker_mesh());
    let border = BorderProgress::new(geometry);
    let border_mesh = meshes.add(border.mesh());
    let head_position = border.head_position().unwrap_or(Vec2::ZERO);
    let outer_glow_mesh = meshes.add(Circle::new(HEAD_GLOW_RADIUS));
    let inner_glow_mesh = meshes.add(Circle::new(3.0));
    let head_core_mesh = meshes.add(Circle::new(1.5));
    let marker_material = materials.add(color_material(theme));
    let border_material = materials.add(color_material(theme));
    let track_material = materials.add(color_material(border_track_color(theme)));
    let outer_glow_material = materials.add(color_material(theme.with_alpha(0.16)));
    let inner_glow_material = materials.add(color_material(theme.with_alpha(0.4)));
    let head_core_material = materials.add(color_material(Color::srgba(1.0, 1.0, 1.0, 0.95)));

    commands.spawn(Camera2d);
    commands.insert_resource(NoticePlacement::default());
    let notice_root = commands
        .spawn((Transform::default(), Visibility::default(), NoticeWorldRoot))
        .id();
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
    commands.spawn((
        base_art,
        Transform::from_xyz(0.0, 0.0, 0.1),
        BaseArt,
        ChildOf(notice_root),
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
        ChildOf(notice_root),
    ));
    commands.spawn((
        Mesh2d(marker_mesh),
        MeshMaterial2d(marker_material),
        Transform::from_xyz(ICON_CENTER_X, 0.0, 0.7),
        IconMarker,
        ChildOf(notice_root),
    ));
    commands.spawn((
        Sprite::from_color(Color::NONE, Vec2::splat(24.0)),
        Transform::from_xyz(ICON_CENTER_X, 0.0, 0.8),
        IconArt::default(),
        ChildOf(notice_root),
    ));
    commands.spawn((
        Mesh2d(track_mesh),
        MeshMaterial2d(track_material),
        Transform::from_xyz(0.0, 0.0, 1.9),
        BorderTrack,
        ChildOf(notice_root),
    ));
    commands.spawn((
        Mesh2d(border_mesh),
        MeshMaterial2d(border_material),
        Transform::from_xyz(0.0, 0.0, 2.0),
        border,
        ChildOf(notice_root),
    ));
    commands
        .spawn((
            Transform::from_xyz(head_position.x, head_position.y, 2.1),
            Visibility::Hidden,
            BorderHead,
            ChildOf(notice_root),
        ))
        .with_children(|head| {
            head.spawn((
                Mesh2d(outer_glow_mesh),
                MeshMaterial2d(outer_glow_material),
                Transform::from_xyz(0.0, 0.0, 0.0),
                BorderHeadGlow(0.16),
            ));
            head.spawn((
                Mesh2d(inner_glow_mesh),
                MeshMaterial2d(inner_glow_material),
                Transform::from_xyz(0.0, 0.0, 0.1),
                BorderHeadGlow(0.4),
            ));
            head.spawn((
                Mesh2d(head_core_mesh),
                MeshMaterial2d(head_core_material),
                Transform::from_xyz(0.0, 0.0, 0.2),
            ));
        });

    let current_tip = jobs.0.jobs[0].tip().text.replace(['\r', '\n'], " ");
    commands
        .spawn(Node {
            width: percent(100),
            height: percent(100),
            ..default()
        })
        .with_children(|root| {
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: px(WINDOW_WIDTH),
                    height: px(WINDOW_HEIGHT),
                    ..default()
                },
                NoticeUiBoard,
            ))
            .with_children(|board| {
                board
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(66),
                            top: px(9),
                            width: px(123),
                            height: px(22),
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
                                font: FontSource::system_ui(),
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
                board.spawn((
                    Text("--".into()),
                    TextFont {
                        font: FontSource::system_ui(),
                        font_size: FontSize::Px(6.0),
                        ..default()
                    },
                    TextColor(theme),
                    Node {
                        position_type: PositionType::Absolute,
                        right: percent(8),
                        bottom: percent(4),
                        ..default()
                    },
                    CountdownText,
                ));
            });
        });
}

pub(super) fn sync_notice(
    jobs: Res<JobConfig>,
    state: Res<ActiveJobState>,
    visuals: Res<RotationNoticeVisualConfig>,
    asset_server: Res<AssetServer>,
    mut text: Single<&mut Text, With<TipText>>,
    mut countdown_color: Single<&mut TextColor, With<CountdownText>>,
    mut sprites: Query<(
        &mut Sprite,
        Option<&BaseArt>,
        Option<&IconBackgroundArt>,
        Option<&mut IconArt>,
    )>,
    materials_query: Query<(
        &MeshMaterial2d<ColorMaterial>,
        Option<&IconMarker>,
        Option<&BorderProgress>,
        Option<&BorderTrack>,
        Option<&BorderHeadGlow>,
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
    for (mut sprite, base_art, icon_background, icon_art) in &mut sprites {
        if base_art.is_some() {
            sprite.color = layer_color(
                visuals.base_background.as_ref(),
                theme,
                visuals.background_opacity,
            );
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
                sprite.custom_size = Some(Vec2::splat(24.0));
                sprite.image_mode = SpriteImageMode::Scale(SpriteScalingMode::FitCenter);
            } else {
                sprite.color = Color::NONE;
            }
        }
    }
    for (handle, marker, border, track, head_glow) in &materials_query {
        let color = if marker.is_some() || border.is_some() {
            theme
        } else if track.is_some() {
            border_track_color(theme)
        } else if let Some(glow) = head_glow {
            theme.with_alpha(glow.0)
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

pub(super) fn sync_overlay_interaction(
    mouse: Res<ButtonInput<MouseButton>>,
    state: Res<ActiveJobState>,
    mut placement: ResMut<NoticePlacement>,
    window: Single<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    world_root: Single<&mut Transform, With<NoticeWorldRoot>>,
    board: Single<&mut Node, With<NoticeUiBoard>>,
) {
    let (window, mut cursor_options) = window.into_inner();
    let canvas_size = window.resolution.size();
    let mut center = placement.center.unwrap_or(canvas_size * 0.5);
    if let Some(previous_size) = placement.canvas_size {
        center += (canvas_size - previous_size) * 0.5;
    }
    placement.canvas_size = Some(canvas_size);

    let cursor = cursor_in_window(window);
    let notice_half_size = Vec2::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32) * 0.5;
    let over_notice = cursor.is_some_and(|cursor| {
        let offset = (cursor - center).abs();
        offset.x <= notice_half_size.x && offset.y <= notice_half_size.y
    });
    if mouse.just_pressed(MouseButton::Left) && over_notice {
        placement.drag_offset = cursor.map(|cursor| center - cursor);
    }
    if state.active_job.is_none() || !mouse.pressed(MouseButton::Left) {
        placement.drag_offset = None;
    } else if let (Some(cursor), Some(offset)) = (cursor, placement.drag_offset) {
        center = cursor + offset;
    }

    let margin = notice_half_size + Vec2::splat(HEAD_GLOW_RADIUS);
    center = center.clamp(margin, (canvas_size - margin).max(margin));
    placement.center = Some(center);

    let mut world_root = world_root.into_inner();
    world_root.translation = Vec3::new(
        center.x - canvas_size.x * 0.5,
        canvas_size.y * 0.5 - center.y,
        0.0,
    );
    let mut board = board.into_inner();
    board.left = px(center.x - notice_half_size.x);
    board.top = px(center.y - notice_half_size.y);

    let over_notice = cursor.is_some_and(|cursor| {
        let offset = (cursor - center).abs();
        offset.x <= notice_half_size.x && offset.y <= notice_half_size.y
    });
    let hit_test = state.active_job.is_some() && (over_notice || placement.drag_offset.is_some());
    if cursor_options.hit_test != hit_test {
        cursor_options.hit_test = hit_test;
    }
}

#[cfg(target_os = "windows")]
fn cursor_in_window(window: &Window) -> Option<Vec2> {
    use windows_sys::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};

    let WindowPosition::At(origin) = window.position else {
        return window.cursor_position();
    };
    // Click-through windows do not receive cursor events, so poll the screen position.
    let mut point = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return None;
    }
    let cursor = (Vec2::new(point.x as f32, point.y as f32) - origin.as_vec2())
        / window.resolution.scale_factor() as f32;
    let size = window.resolution.size();
    (cursor.x >= 0.0 && cursor.y >= 0.0 && cursor.x < size.x && cursor.y < size.y)
        .then_some(cursor)
}

#[cfg(not(target_os = "windows"))]
fn cursor_in_window(window: &Window) -> Option<Vec2> {
    window.cursor_position()
}
