use super::Region;
use bevy::{
    app::AppExit,
    asset::RenderAssetUsages,
    diagnostic::DiagnosticsPlugin,
    prelude::*,
    render::{
        RenderPlugin,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        settings::{Backends, WgpuSettings},
    },
    window::{
        MonitorSelection, PrimaryWindow, WindowLevel, WindowMode, WindowResolution,
    },
};
use std::sync::{Arc, Mutex};

#[derive(Component)]
struct SelectionRectangle;

#[derive(Resource)]
struct SelectionState {
    start: Option<Vec2>,
    result: Arc<Mutex<Option<Region>>>,
}

#[derive(Resource)]
struct FrozenScreen {
    width: u32,
    height: u32,
    pixels: Option<Vec<u8>>,
}

pub(super) fn select_region(width: u32, height: u32, pixels: Vec<u8>) -> Option<Region> {
    let result = Arc::new(Mutex::new(None));
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(SelectionState {
            start: None,
            result: result.clone(),
        })
        .insert_resource(FrozenScreen {
            width,
            height,
            pixels: Some(pixels),
        })
        .add_plugins(
            DefaultPlugins
                .set(RenderPlugin {
                    render_creation: WgpuSettings {
                        backends: Some(Backends::DX12),
                        ..default()
                    }
                    .into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Nyra Perception - Select Region".into(),
                        resolution: WindowResolution::new(1, 1).with_scale_factor_override(1.0),
                        mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
                        decorations: false,
                        resizable: false,
                        window_level: WindowLevel::AlwaysOnTop,
                        skip_taskbar: true,
                        ..default()
                    }),
                    ..default()
                })
                .disable::<DiagnosticsPlugin>(),
        )
        .add_systems(Startup, setup)
        .add_systems(Update, select_with_mouse)
        .run();
    *result.lock().unwrap()
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut frozen_screen: ResMut<FrozenScreen>,
) {
    commands.spawn(Camera2d);
    let mut image = Image::new(
        Extent3d {
            width: frozen_screen.width,
            height: frozen_screen.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        frozen_screen
            .pixels
            .take()
            .expect("frozen screen pixels should be available during setup"),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = bevy::image::ImageSampler::nearest();
    let frozen_image = images.add(image);
    commands.spawn((
        ImageNode::new(frozen_image),
        GlobalZIndex(-1),
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: px(frozen_screen.width),
            height: px(frozen_screen.height),
            ..default()
        },
    ));
    commands.spawn((
        GlobalZIndex(10),
        Node {
            position_type: PositionType::Absolute,
            left: px(20),
            top: px(20),
            padding: UiRect::axes(px(14), px(9)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.02, 0.02, 0.86)),
        children![(
            Text::new("Drag to record a condition region · Esc to cancel"),
            TextColor(Color::WHITE),
            TextFont {
                font_size: FontSize::Px(18.0),
                ..default()
            },
        )],
    ));
    commands.spawn((
        SelectionRectangle,
        GlobalZIndex(20),
        Node {
            display: Display::None,
            position_type: PositionType::Absolute,
            border: UiRect::all(px(2)),
            ..default()
        },
        BackgroundColor(Color::NONE),
        BorderColor::all(Color::srgb(0.2, 0.75, 1.0)),
    ));
}

fn select_with_mouse(
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut rectangle: Single<&mut Node, With<SelectionRectangle>>,
    mut selection: ResMut<SelectionState>,
    mut exit: MessageWriter<AppExit>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
        return;
    }

    let cursor = window.physical_cursor_position();
    if mouse.just_pressed(MouseButton::Left) {
        selection.start = cursor;
    }

    if let (Some(start), Some(current)) = (selection.start, cursor)
        && mouse.pressed(MouseButton::Left)
    {
        show_rectangle(&mut rectangle, start, current);
    }

    if mouse.just_released(MouseButton::Left) {
        let Some(start) = selection.start.take() else {
            return;
        };
        let Some(end) = cursor else {
            rectangle.display = Display::None;
            return;
        };
        let left = start.x.min(end.x).floor().max(0.0) as u32;
        let top = start.y.min(end.y).floor().max(0.0) as u32;
        let right = start.x.max(end.x).ceil().min(window.physical_width() as f32) as u32;
        let bottom = start.y.max(end.y).ceil().min(window.physical_height() as f32) as u32;
        if right <= left || bottom <= top {
            rectangle.display = Display::None;
            return;
        }
        *selection.result.lock().unwrap() = Some(Region {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        });
        exit.write(AppExit::Success);
    }
}

fn show_rectangle(node: &mut Node, start: Vec2, end: Vec2) {
    let left = start.x.min(end.x);
    let top = start.y.min(end.y);
    node.display = Display::Flex;
    node.left = px(left);
    node.top = px(top);
    node.width = px((start.x - end.x).abs());
    node.height = px((start.y - end.y).abs());
}
