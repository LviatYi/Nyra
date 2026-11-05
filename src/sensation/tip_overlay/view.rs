use super::{
    OverlayPresentation,
    float_light::{FloatLight, float_light_mesh},
    geometry::{BorderGeometry, icon_marker_mesh},
    menu::{
        ActionMenu, HINT_HEIGHT, MenuActions, MenuHint, MenuList, menu_areas, menu_item_scenes,
    },
    progress_border::{
        BorderDrainValue, BorderFillValue, BorderHead, BorderHeadGlow, BorderOutgoing,
        BorderProgress, BorderTrack, CountdownText,
    },
    style::{border_track_color, color_material, set_material_color, tip_color},
    text_scroll::{TextScroll, TipTextViewport},
};
use crate::sensation::sensation_config::{
    SensationConfig, SensationConfigHandle, layer_color, layer_sprite,
};
use crate::{
    controller::ActiveJobState,
    job::{Job, JobConfig},
    reaction::{GlobalHotkeys, ReactionRunner},
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
use global_hotkey::hotkey::{HotKey, Modifiers};

#[derive(Resource, Default)]
pub(super) struct NoticePlacement {
    pub(super) center: Option<Vec2>,
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
pub(super) struct TipSubtitle;
#[derive(Component, Default, Clone)]
pub(super) struct TipShortcut;
#[derive(Component, Default, Clone)]
pub(super) struct TipSubtitleSuffix;
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
    config: Res<SensationConfigHandle>,
    visual_assets: Res<Assets<SensationConfig>>,
    asset_server: Res<AssetServer>,
    actions: Res<MenuActions>,
) {
    let Some(visuals) = config.resolve(&visual_assets, &asset_server) else {
        return;
    };
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
    let head_position = border.head_position(0.0);
    let icon_center_x = -(WINDOW_WIDTH as f32) / 2.0 + 30.0;
    let list_height = actions.list_height();
    let mut float_light_material = color_material(Color::NONE);
    float_light_material.texture = visuals
        .float_light
        .as_ref()
        .filter(|asset| !asset.path.trim().is_empty())
        .map(|asset| asset_server.load(asset.path.clone()));

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
            Mesh2d({asset_value(float_light_mesh(0.0))})
            MeshMaterial2d<ColorMaterial>({asset_value(float_light_material)})
            Transform::from_xyz(0.0, 0.0, 0.2)
            Visibility::Hidden
            FloatLight
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
            BorderDrainValue::default()
            --
            Mesh2d({asset_value(border.mesh(0.0))})
            MeshMaterial2d<ColorMaterial>(asset_value(color_material(Color::NONE)))
            Transform::from_xyz(0.0, 0.0, 2.0)
            border
            BorderFillValue::default()
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
                Text("")
                TextFont {
                    font: FontSourceTemplate::Generic(GenericFontFamily::SystemUi),
                    font_size: FontSize::Px(4.0),
                }
                TextColor(Color::srgb(0.94, 0.96, 1.0))
                TextLayout::no_wrap()
                Node {
                    position_type: PositionType::Absolute,
                    left: px(66),
                    top: px(22),
                    width: px(123),
                    overflow: Overflow::clip(),
                }
                TipSubtitle
                Pickable::IGNORE
                Children [
                    TextSpan("")
                    TextFont {
                        font: FontSourceTemplate::Generic(GenericFontFamily::SystemUi),
                        font_size: FontSize::Px(7.0),
                    }
                    TipShortcut
                    --
                    TextSpan("")
                    TextFont {
                        font: FontSourceTemplate::Generic(GenericFontFamily::SystemUi),
                        font_size: FontSize::Px(4.0),
                    }
                    TipSubtitleSuffix
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
                --
                Node {
                    position_type: PositionType::Absolute,
                    left: px(10),
                    top: px(WINDOW_HEIGHT),
                    width: px(WINDOW_WIDTH - 20),
                    height: px(list_height),
                    overflow: Overflow::clip(),
                }
                Pickable::IGNORE
                Children [
                    Node {
                        position_type: PositionType::Absolute,
                        top: px(-list_height),
                        width: percent(100),
                        height: px(list_height),
                        flex_direction: FlexDirection::Column,
                    }
                    MenuList
                    Pickable::IGNORE
                    Children [
                        { menu_item_scenes(&actions) }
                    ]
                ]
                --
                Node {
                    position_type: PositionType::Absolute,
                    left: px(10),
                    top: px(WINDOW_HEIGHT),
                    width: px(WINDOW_WIDTH - 20),
                    height: px(HINT_HEIGHT),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                }
                MenuHint
                Pickable::IGNORE
                Children [
                    Text("⌄ 操作")
                    TextFont {
                        font: FontSourceTemplate::Generic(GenericFontFamily::SystemUi),
                        font_size: FontSize::Px(10.0),
                    }
                    TextColor(Color::srgb(0.94, 0.96, 1.0))
                    Pickable::IGNORE
                ]
            ]
        ]
    });
}

pub(super) fn sync_notice(
    (jobs, state, presentation): (
        Res<JobConfig>,
        Res<ActiveJobState>,
        Res<OverlayPresentation>,
    ),
    config: Res<SensationConfigHandle>,
    visual_assets: Res<Assets<SensationConfig>>,
    asset_server: Res<AssetServer>,
    mut text: Single<&mut Text, (With<TipText>, Without<TipSubtitle>)>,
    mut subtitle: Single<&mut Text, (With<TipSubtitle>, Without<TipText>)>,
    (mut shortcut_text, mut subtitle_suffix): (
        Single<&mut TextSpan, (With<TipShortcut>, Without<TipSubtitleSuffix>)>,
        Single<&mut TextSpan, (With<TipSubtitleSuffix>, Without<TipShortcut>)>,
    ),
    mut viewport: Single<&mut Node, (With<TipTextViewport>, Without<TipSubtitle>)>,
    mut head: Single<&mut Visibility, With<BorderHead>>,
    runner: Option<Res<ReactionRunner>>,
    hotkeys: Option<NonSend<GlobalHotkeys>>,
    mut progress_visibility: Query<
        &mut Visibility,
        (
            Or<(
                With<BorderProgress>,
                With<BorderOutgoing>,
                With<BorderTrack>,
            )>,
            Without<BorderHead>,
        ),
    >,
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
    let banner = *presentation == OverlayPresentation::Banner;
    let job = if banner {
        None
    } else {
        let Some(active) = state.active_job.as_ref() else {
            return;
        };
        Some((active.current_index, &jobs.0.jobs[active.current_index]))
    };
    let Some(visuals) = config.resolve(&visual_assets, &asset_server) else {
        return;
    };
    let initiative = job.is_some_and(|(_, job)| matches!(job, Job::Initiative { .. }));
    let tip = job.map(|(_, job)| job.tip());
    let shortcut = tip
        .and_then(|tip| tip.reaction.as_ref())
        .filter(|reaction| {
            hotkeys
                .as_ref()
                .is_some_and(|hotkeys| hotkeys.is_registered(reaction.hotkey))
        });
    let (caption, key, suffix) =
        if initiative && runner.as_ref().is_some_and(|runner| runner.is_busy()) {
            ("正在工作中", String::new(), "")
        } else if let Some(reaction) = shortcut {
            ("按下 ", compact_hotkey(reaction.hotkey), " 以应对")
        } else if initiative {
            ("暂无可用快捷键", String::new(), "")
        } else {
            ("", String::new(), "")
        };
    if subtitle.0 != caption {
        subtitle.0 = caption.to_owned();
    }
    if shortcut_text.0 != key {
        shortcut_text.0 = key;
    }
    if subtitle_suffix.0 != suffix {
        subtitle_suffix.0 = suffix.to_owned();
    }
    viewport.top = px(if subtitle.0.is_empty() { 9 } else { 3 });
    viewport.height = px(if subtitle.0.is_empty() { 22 } else { 18 });
    **head = if banner || initiative {
        Visibility::Hidden
    } else {
        Visibility::Visible
    };
    for mut visibility in &mut progress_visibility {
        *visibility = if banner {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    let theme = job
        .map(|(index, _)| tip_color(&jobs, index))
        .unwrap_or_else(|| {
            Color::Srgba(Srgba::hex(crate::settings_default_values::DEFAULT_TIP_COLORS[0]).unwrap())
        });
    let content = tip
        .map(|tip| tip.text.replace(['\r', '\n'], " "))
        .unwrap_or_else(|| "NYRA ONLINE".to_owned());
    if text.0 != content {
        text.0 = content;
    }
    countdown_color.0 = if banner { Color::NONE } else { theme };
    for (mut sprite, base_art, icon_background, icon_art) in &mut sprites {
        if base_art.is_some() {
            sprite.color = layer_color(visuals.base_background.as_ref(), theme);
        } else if icon_background.is_some() {
            sprite.color = layer_color(visuals.icon_background.as_ref(), theme);
        } else if let Some(mut icon_art) = icon_art {
            let icon_path = tip
                .and_then(|tip| tip.icon.as_deref())
                .filter(|path| !path.trim().is_empty());
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

fn compact_hotkey(hotkey: HotKey) -> String {
    let mut parts = Vec::new();
    if hotkey.mods.contains(Modifiers::CONTROL) {
        parts.push("CTRL");
    }
    if hotkey.mods.contains(Modifiers::ALT) {
        parts.push("ALT");
    }
    if hotkey.mods.contains(Modifiers::SHIFT) {
        parts.push("SFT");
    }
    if hotkey.mods.contains(Modifiers::META) {
        parts.push("WIN");
    }
    let key = hotkey.key.to_string();
    let key = match key.as_str() {
        "Backquote" => "`",
        "Backslash" => "\\",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Comma" => ",",
        "Equal" => "=",
        "Minus" => "-",
        "Period" => ".",
        "Quote" => "'",
        "Semicolon" => ";",
        "Slash" => "/",
        "Escape" => "ESC",
        "Backspace" => "BKSP",
        "Delete" => "DEL",
        "Insert" => "INS",
        "PageUp" => "PGUP",
        "PageDown" => "PGDN",
        "ArrowUp" => "UP",
        "ArrowDown" => "DOWN",
        "ArrowLeft" => "LEFT",
        "ArrowRight" => "RIGHT",
        "Space" => "SPACE",
        "Enter" => "ENTER",
        "Tab" => "TAB",
        _ => &key,
    };
    let key = key
        .strip_prefix("Key")
        .filter(|key| key.len() == 1)
        .or_else(|| key.strip_prefix("Digit").filter(|key| key.len() == 1))
        .unwrap_or(key);
    parts.push(key);
    parts.join("+")
}

pub(super) fn sync_overlay_visibility(
    state: Res<ActiveJobState>,
    presentation: Res<OverlayPresentation>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    window.visible = *presentation == OverlayPresentation::Banner || state.active_job.is_some();
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
    if event.button == PointerButton::Primary && placement.drag_pointer == Some(event.pointer.id) {
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
    actions: Res<MenuActions>,
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
        (canvas_size - notice_half_size - Vec2::new(0.0, actions.total_height()))
            .max(notice_half_size),
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
    menu: Res<ActionMenu>,
    actions: Res<MenuActions>,
    window: Single<(Entity, &Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut window_events: MessageWriter<WindowEvent>,
) {
    let (entity, window, mut cursor_options) = window.into_inner();
    let cursor = cursor_in_window(window);
    let center = placement.center.unwrap_or(window.resolution.size() * 0.5);

    // Keep picking's mouse position current while the native window passes clicks through.
    if state.active_job.is_some()
        && !cursor_options.hit_test
        && let Some(position) = cursor
    {
        window_events.write(WindowEvent::CursorMoved(CursorMoved {
            window: entity,
            position,
            delta: None,
        }));
    }
    let over_overlay = cursor.is_some_and(|cursor| {
        let (over_notice, over_menu) =
            menu_areas(cursor, center, menu.progress, actions.list_height());
        over_notice || over_menu
    });
    let hit_test = state.active_job.is_some() && (over_overlay || placement.drag_pointer.is_some());
    if cursor_options.hit_test != hit_test {
        cursor_options.hit_test = hit_test;
    }
}

#[cfg(target_os = "windows")]
pub(super) fn cursor_in_window(window: &Window) -> Option<Vec2> {
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
    (cursor.x >= 0.0 && cursor.y >= 0.0 && cursor.x < size.x && cursor.y < size.y).then_some(cursor)
}

#[cfg(not(target_os = "windows"))]
pub(super) fn cursor_in_window(window: &Window) -> Option<Vec2> {
    window.cursor_position()
}
