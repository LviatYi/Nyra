use super::view::{NoticePlacement, cursor_in_window};
use crate::{
    controller::ActiveJobState,
    settings_default_values::{WINDOW_HEIGHT, WINDOW_WIDTH},
};
use bevy::{
    prelude::*,
    text::{FontSourceTemplate, GenericFontFamily},
    window::PrimaryWindow,
};

pub(super) const ITEM_HEIGHT: f32 = 24.0;
pub(super) const HINT_HEIGHT: f32 = 16.0;
const SLIDE_DURATION: f32 = 0.18;
const CLOSE_DELAY: f32 = 3.0;

struct MenuAction {
    shortcut: &'static str,
    label: &'static str,
    key: KeyCode,
    virtual_key: i32,
    run: fn(),
}

#[derive(Resource)]
pub(crate) struct MenuActions(Vec<MenuAction>);

impl Default for MenuActions {
    fn default() -> Self {
        let mut actions = Self(Vec::new());
        actions.add(
            "R",
            "录制 reaction 方案",
            KeyCode::KeyR,
            b'R' as i32,
            record_reaction_plan,
        );
        actions
    }
}

impl MenuActions {
    pub(crate) fn add(
        &mut self,
        shortcut: &'static str,
        label: &'static str,
        key: KeyCode,
        virtual_key: i32,
        run: fn(),
    ) {
        self.0.push(MenuAction {
            shortcut,
            label,
            key,
            virtual_key,
            run,
        });
    }

    pub(super) fn list_height(&self) -> f32 {
        ITEM_HEIGHT * self.0.len() as f32
    }

    pub(crate) fn total_height(&self) -> f32 {
        self.list_height() + HINT_HEIGHT
    }
}

#[derive(Component, Default, Clone)]
pub(super) struct MenuList;

#[derive(Component, Default, Clone)]
struct MenuItem;

#[derive(Component, Default, Clone)]
pub(super) struct MenuHint;

pub(super) fn menu_item_scenes(actions: &MenuActions) -> impl SceneList {
    actions
        .0
        .iter()
        .map(|action| menu_item_scene(action.shortcut, action.label))
        .collect::<Vec<_>>()
}

fn menu_item_scene(shortcut: &'static str, label: &'static str) -> impl Scene {
    bsn! {
        Node {
            width: percent(100),
            height: px(ITEM_HEIGHT),
            flex_shrink: 0.0,
        }
        MenuItem
        Pickable::IGNORE
        Children [
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(6),
                width: px(12),
                height: px(12),
            }
            BackgroundColor(Color::NONE)
            Pickable::IGNORE
            --
            Text(shortcut)
            TextFont {
                font: FontSourceTemplate::Generic(GenericFontFamily::SystemUi),
                font_size: FontSize::Px(10.0),
            }
            TextColor(Color::srgb(0.94, 0.96, 1.0))
            Node {
                position_type: PositionType::Absolute,
                left: px(16),
                top: px(5),
            }
            Pickable::IGNORE
            --
            Text(label)
            TextFont {
                font: FontSourceTemplate::Generic(GenericFontFamily::SystemUi),
                font_size: FontSize::Px(10.0),
            }
            TextColor(Color::srgb(0.94, 0.96, 1.0))
            Node {
                position_type: PositionType::Absolute,
                right: px(0),
                top: px(5),
            }
            Pickable::IGNORE
        ]
    }
}

#[derive(Resource)]
pub(super) struct ActionMenu {
    pub(super) progress: f32,
    away_time: f32,
    dismissed_until_leave: bool,
    key_was_down: Vec<bool>,
}

impl Default for ActionMenu {
    fn default() -> Self {
        Self {
            progress: 0.0,
            away_time: CLOSE_DELAY,
            dismissed_until_leave: false,
            key_was_down: Vec::new(),
        }
    }
}

pub(super) fn menu_areas(
    cursor: Vec2,
    center: Vec2,
    progress: f32,
    list_height: f32,
) -> (bool, bool) {
    let origin = center - Vec2::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32) * 0.5;
    let local = cursor - origin;
    let over_notice = local.x >= 0.0
        && local.x < WINDOW_WIDTH as f32
        && local.y >= 0.0
        && local.y < WINDOW_HEIGHT as f32;
    let over_menu = local.x >= 10.0
        && local.x < WINDOW_WIDTH as f32 - 10.0
        && local.y >= WINDOW_HEIGHT as f32
        && local.y < WINDOW_HEIGHT as f32 + HINT_HEIGHT + list_height * progress;
    (over_notice, over_menu)
}

pub(super) fn update_menu(
    time: Res<Time>,
    state: Res<ActiveJobState>,
    actions: Res<MenuActions>,
    placement: Res<NoticePlacement>,
    window: Single<&Window, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<ActionMenu>,
    mut list: Single<&mut Node, With<MenuList>>,
    mut hint: Single<&mut Node, (With<MenuHint>, Without<MenuList>)>,
) {
    let window = window.into_inner();
    let cursor = cursor_in_window(window);
    let center = placement.center.unwrap_or(window.resolution.size() * 0.5);
    let list_height = actions.list_height();
    let (over_notice, over_menu) = cursor
        .map(|cursor| menu_areas(cursor, center, menu.progress, list_height))
        .unwrap_or((false, false));
    let within_full_menu = cursor.is_some_and(|cursor| {
        let (over_notice, over_menu) = menu_areas(cursor, center, 1.0, list_height);
        over_notice || over_menu
    });
    let hovering = over_notice || over_menu;
    let active = state.active_job.is_some();

    if !active {
        menu.progress = 0.0;
        menu.away_time = CLOSE_DELAY;
        menu.dismissed_until_leave = false;
    } else {
        if !within_full_menu {
            menu.dismissed_until_leave = false;
        }
        if hovering && !menu.dismissed_until_leave {
            menu.away_time = 0.0;
        } else {
            menu.away_time = (menu.away_time + time.delta_secs()).min(CLOSE_DELAY);
        }
        if mouse.just_pressed(MouseButton::Middle) && over_menu && menu.progress > 0.0 {
            menu.dismissed_until_leave = true;
            menu.away_time = CLOSE_DELAY;
        }
        let open = !menu.dismissed_until_leave && (hovering || menu.away_time < CLOSE_DELAY);
        let step = time.delta_secs() / SLIDE_DURATION;
        menu.progress = if open {
            (menu.progress + step).min(1.0)
        } else {
            (menu.progress - step).max(0.0)
        };

        if mouse.just_pressed(MouseButton::Left)
            && over_menu
            && let Some(cursor) = cursor
        {
            let local_y = cursor.y - center.y + WINDOW_HEIGHT as f32 * 0.5;
            if local_y < WINDOW_HEIGHT as f32 + list_height * menu.progress {
                let row_y = local_y - WINDOW_HEIGHT as f32 + (1.0 - menu.progress) * list_height;
                if let Some(action) = actions.0.get((row_y / ITEM_HEIGHT) as usize) {
                    (action.run)();
                }
            }
        }
    }

    menu.key_was_down.resize(actions.0.len(), false);
    for (index, action) in actions.0.iter().enumerate() {
        let key_down = keyboard.pressed(action.key) || native_key_pressed(action.virtual_key);
        if active && hovering && key_down && !menu.key_was_down[index] {
            (action.run)();
        }
        menu.key_was_down[index] = key_down;
    }

    list.top = px((menu.progress - 1.0) * list_height);
    hint.top = px(WINDOW_HEIGHT as f32 + menu.progress * list_height);
}

fn record_reaction_plan() {
    println!("hello world");
}

#[cfg(target_os = "windows")]
fn native_key_pressed(virtual_key: i32) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    unsafe { GetAsyncKeyState(virtual_key) as u16 & 0x8000 != 0 }
}

#[cfg(not(target_os = "windows"))]
fn native_key_pressed(_virtual_key: i32) -> bool {
    false
}
