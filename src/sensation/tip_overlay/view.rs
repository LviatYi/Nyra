use super::{
    geometry::{BorderGeometry, icon_marker_mesh},
    progress_border::{
        BorderHead, BorderHeadGlow, BorderOutgoing, BorderProgress, BorderTrack, CountdownText,
    },
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
    asset::asset_value,
    picking::pointer::PointerId,
    prelude::*,
    sprite::{SpriteImageMode, SpriteScalingMode},
    text::{FontSourceTemplate, GenericFontFamily},
    window::{CursorMoved, CursorOptions, PrimaryWindow, WindowEvent, WindowPosition},
};
#[cfg(target_os = "windows")]
use bevy::window::RawHandleWrapper;
#[cfg(target_os = "windows")]
use raw_window_handle::RawWindowHandle;
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{RemovePropW, SetPropW};

#[cfg(target_os = "windows")]
const NON_RUDE_HWND: windows_sys::core::PCWSTR = windows_sys::core::w!("NonRudeHWND");

#[derive(Resource, Default)]
pub(super) struct NoticePlacement {
    center: Option<Vec2>,
    canvas_size: Option<Vec2>,
    drag_pointer: Option<PointerId>,
}

#[derive(Component, Default, Clone)]
pub(super) struct NoticeWorldRoot;
#[derive(Component, Default, Clone)]
pub(super) struct NoticeUiBoard;

#[derive(Component, Default, Clone)]
pub(super) struct TipText;
#[derive(Component, Default, Clone)]
pub(super) struct IconMarker;
#[derive(Component, Default, Clone)]
pub(super) struct BaseArt;
#[derive(Component, Default, Clone)]
pub(super) struct IconBackgroundArt;
#[derive(Component, Default, Clone)]
pub(super) struct IconArt {
    has_source: bool,
}

pub(super) fn setup_overlay(
    mut commands: Commands,
    visuals: Res<RotationNoticeVisualConfig>,
) {
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
    let track_mesh = geometry.progress_mesh(1.0);
    let border = BorderProgress::new(geometry);
    let head_position = border.head_position();
    let icon_center_x = -(WINDOW_WIDTH as f32) / 2.0 + 30.0;

    commands.insert_resource(NoticePlacement::default());
    commands.spawn_scene_list(bsn_list! {
        Camera2d
        --
        Transform
        Visibility
        NoticeWorldRoot
        Children [
            Sprite {
                color: Color::NONE,
                ..{layer_sprite(
                    visuals.base_background.as_ref(),
                    Vec2::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32),
                )}
            }
            Transform::from_xyz(0.0, 0.0, 0.1)
            BaseArt
            --
            Sprite {
                color: Color::NONE,
                ..{layer_sprite(visuals.icon_background.as_ref(), Vec2::splat(32.0))}
            }
            Transform::from_xyz(icon_center_x, 0.0, 0.4)
            IconBackgroundArt
            --
            Mesh2d(asset_value(icon_marker_mesh()))
            MeshMaterial2d<ColorMaterial>(asset_value(color_material(Color::NONE)))
            Transform::from_xyz(icon_center_x, 0.0, 0.7)
            IconMarker
            --
            Sprite {
                color: Color::NONE,
                custom_size: {Some(Vec2::splat(24.0))},
            }
            Transform::from_xyz(icon_center_x, 0.0, 0.8)
            IconArt
            --
            Mesh2d(asset_value(track_mesh))
            MeshMaterial2d<ColorMaterial>(asset_value(color_material(Color::NONE)))
            Transform::from_xyz(0.0, 0.0, 1.9)
            BorderTrack
            --
            Mesh2d({asset_value(border.outgoing_mesh(1.0))})
            MeshMaterial2d<ColorMaterial>(asset_value(color_material(Color::NONE)))
            Transform::from_xyz(0.0, 0.0, 1.95)
            BorderOutgoing
            --
            Mesh2d({asset_value(border.mesh())})
            MeshMaterial2d<ColorMaterial>(asset_value(color_material(Color::NONE)))
            Transform::from_xyz(0.0, 0.0, 2.0)
            border
            --
            Transform::from_xyz(head_position.x, head_position.y, 2.1)
            Visibility::Visible
            BorderHead
            Children [
                Mesh2d({asset_value(Mesh::from(Circle::new(2.5)))})
                MeshMaterial2d<ColorMaterial>(asset_value(color_material(Color::NONE)))
                Transform::from_xyz(0.0, 0.0, 0.0)
                BorderHeadGlow(0.16)
                --
                Mesh2d({asset_value(Mesh::from(Circle::new(1.5)))})
                MeshMaterial2d<ColorMaterial>(asset_value(color_material(Color::NONE)))
                Transform::from_xyz(0.0, 0.0, 0.1)
                BorderHeadGlow(0.4)
                --
                Mesh2d({asset_value(Mesh::from(Circle::new(0.5)))})
                MeshMaterial2d<ColorMaterial>(
                    asset_value(color_material(Color::srgba(1.0, 1.0, 1.0, 0.95)))
                )
                Transform::from_xyz(0.0, 0.0, 0.2)
            ]
        ]
        --
        Node {
            width: percent(100),
            height: percent(100),
        }
        Pickable::IGNORE
        Children [
            Node {
                position_type: PositionType::Absolute,
                width: px(WINDOW_WIDTH),
                height: px(WINDOW_HEIGHT),
            }
            NoticeUiBoard
            on(start_notice_drag)
            on(drag_notice)
            on(end_notice_drag)
            on(cancel_notice_drag)
            Children [
                Node {
                    position_type: PositionType::Absolute,
                    left: px(66),
                    top: px(9),
                    width: px(123),
                    height: px(22),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::FlexStart,
                    overflow: Overflow::clip(),
                }
                TipTextViewport
                Pickable::IGNORE
                Children [
                    Text
                    TextFont {
                        font: FontSourceTemplate::Generic(GenericFontFamily::SystemUi),
                        font_size: FontSize::Px(13.0),
                    }
                    TextColor(Color::srgb(0.94, 0.96, 1.0))
                    TextLayout::no_wrap()
                    Node {
                        flex_shrink: 0.0,
                    }
                    TextScroll
                    TipText
                    Pickable::IGNORE
                ]
                --
                Text("--")
                TextFont {
                    font: FontSourceTemplate::Generic(GenericFontFamily::SystemUi),
                    font_size: FontSize::Px(6.0),
                }
                TextColor(Color::NONE)
                Node {
                    position_type: PositionType::Absolute,
                    right: percent(8),
                    bottom: percent(4),
                }
                CountdownText
                Pickable::IGNORE
            ]
        ]
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

#[cfg(target_os = "windows")]
pub(super) fn mark_overlay_window_non_rude(
    event: On<Insert<RawHandleWrapper>>,
    windows: Query<&RawHandleWrapper, With<PrimaryWindow>>,
) {
    let Ok(handle) = windows.get(event.entity) else {
        return;
    };
    let RawWindowHandle::Win32(raw) = handle.get_window_handle() else {
        panic!("Primary window has no Win32 handle");
    };
    let hwnd = raw.hwnd.get() as *mut _;

    // Windows must see this property before the full-screen-sized window is shown.
    if unsafe { SetPropW(hwnd, NON_RUDE_HWND, 1 as *mut _) } == 0 {
        panic!("Could not set NonRudeHWND: {}", std::io::Error::last_os_error());
    }
}

#[cfg(target_os = "windows")]
pub(super) fn unmark_overlay_window_non_rude(
    event: On<Remove<RawHandleWrapper>>,
    windows: Query<&RawHandleWrapper>,
) {
    let Ok(handle) = windows.get(event.entity) else {
        return;
    };
    if let RawWindowHandle::Win32(raw) = handle.get_window_handle() {
        unsafe { RemovePropW(raw.hwnd.get() as *mut _, NON_RUDE_HWND) };
    }
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

fn start_notice_drag(
    event: On<PointerDragStart>,
    state: Res<ActiveJobState>,
    mut placement: ResMut<NoticePlacement>,
    mut capture_map: ResMut<PointerCaptureMap>,
) {
    if event.button != PointerButton::Primary
        || state.active_job.is_none()
        || placement.drag_pointer.is_some()
    {
        return;
    }
    placement.drag_pointer = Some(event.pointer.id);
    capture_map.capture(event.pointer.id, event.entity, event.hit.clone());
}

fn drag_notice(
    event: On<PointerDrag>,
    state: Res<ActiveJobState>,
    mut placement: ResMut<NoticePlacement>,
) {
    if event.button == PointerButton::Primary
        && state.active_job.is_some()
        && placement.drag_pointer == Some(event.pointer.id)
        && let Some(center) = placement.center.as_mut()
    {
        *center += event.delta;
    }
}

fn end_notice_drag(
    event: On<PointerDragEnd>,
    mut placement: ResMut<NoticePlacement>,
    mut capture_map: ResMut<PointerCaptureMap>,
) {
    if event.button == PointerButton::Primary
        && placement.drag_pointer == Some(event.pointer.id)
    {
        placement.drag_pointer = None;
        capture_map.release(event.pointer.id);
    }
}

fn cancel_notice_drag(
    event: On<PointerCancel>,
    mut placement: ResMut<NoticePlacement>,
    mut capture_map: ResMut<PointerCaptureMap>,
) {
    if placement.drag_pointer == Some(event.pointer.id) {
        placement.drag_pointer = None;
        capture_map.release(event.pointer.id);
    }
}

pub(super) fn sync_overlay_position(
    state: Res<ActiveJobState>,
    mut placement: ResMut<NoticePlacement>,
    mut capture_map: ResMut<PointerCaptureMap>,
    window: Single<&Window, With<PrimaryWindow>>,
    world_root: Single<&mut Transform, With<NoticeWorldRoot>>,
    board: Single<&mut Node, With<NoticeUiBoard>>,
) {
    if state.active_job.is_none()
        && let Some(pointer) = placement.drag_pointer.take()
    {
        capture_map.release(pointer);
    }
    let canvas_size = window.resolution.size();
    let mut center = placement.center.unwrap_or(canvas_size * 0.5);
    if let Some(previous_size) = placement.canvas_size {
        center += (canvas_size - previous_size) * 0.5;
    }
    placement.canvas_size = Some(canvas_size);

    let notice_half_size = Vec2::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32) * 0.5;

    center = center.clamp(
        notice_half_size,
        (canvas_size - notice_half_size).max(notice_half_size),
    );
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
}

pub(super) fn sync_overlay_hit_test(
    state: Res<ActiveJobState>,
    placement: Res<NoticePlacement>,
    window: Single<(Entity, &Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut window_events: MessageWriter<WindowEvent>,
) {
    let (entity, window, mut cursor_options) = window.into_inner();
    let cursor = cursor_in_window(window);
    let center = placement.center.unwrap_or(window.resolution.size() * 0.5);
    let notice_half_size = Vec2::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32) * 0.5;

    // Keep picking's mouse position current while the native window passes clicks through.
    if state.active_job.is_some() && !cursor_options.hit_test
        && let Some(position) = cursor
    {
        window_events.write(WindowEvent::CursorMoved(CursorMoved {
            window: entity,
            position,
            delta: None,
        }));
    }
    let over_notice = cursor.is_some_and(|cursor| {
        let offset = (cursor - center).abs();
        offset.x <= notice_half_size.x && offset.y <= notice_half_size.y
    });
    let hit_test = state.active_job.is_some() && (over_notice || placement.drag_pointer.is_some());
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
