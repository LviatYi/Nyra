use crate::job::JobConfig;
use crate::{MAX_TEXT_COLUMNS, RotationState, TipText};
use bevy::{prelude::*, window::PrimaryWindow};
use unicode_width::UnicodeWidthChar;

const BORDER_WIDTH: f32 = 3.0;
const CORNER_RADIUS: f32 = 12.0;

#[derive(Component)]
pub struct Overlay;

pub fn setup_overlay(mut commands: Commands, tips: Res<JobConfig>) {
    commands.spawn(Camera2d);

    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(BORDER_WIDTH)),
                border_radius: BorderRadius::all(px(CORNER_RADIUS)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.045, 0.055, 0.075, 0.92)),
            BorderColor::all(progress_color(1.0)),
            Overlay,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(truncate_tip(&tips.0.tips[0].tip)),
                TextFont {
                    font: FontSource::SystemUi,
                    font_size: FontSize::Px(17.0),
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.96, 1.0)),
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

pub(super) fn rotate_tips(
    time: Res<Time>,
    tips: Res<JobConfig>,
    mut state: ResMut<RotationState>,
    mut overlay: Single<&mut Visibility, With<Overlay>>,
    mut text: Single<&mut Text, With<TipText>>,
    mut border_color: Single<&mut BorderColor, With<Overlay>>,
) {
    state.elapsed += time.delta_secs();

    let mut changed = false;
    while state.elapsed >= tips.0.tips[state.current].interval {
        state.elapsed -= tips.0.tips[state.current].interval;
        state.current = (state.current + 1) % tips.0.tips.len();
        changed = true;
    }

    let tip = &tips.0.tips[state.current];
    if changed {
        text.0 = truncate_tip(&tip.tip);
    }

    let showing = state.elapsed < tip.show_time();
    **overlay = if showing {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };

    if showing {
        let remaining = (1.0 - state.elapsed / tip.show_time()).clamp(0.0, 1.0);
        **border_color = BorderColor::all(progress_color(remaining));
    }
}

fn progress_color(remaining: f32) -> Color {
    if remaining > 0.5 {
        Color::srgb(0.20, 0.88, 0.48)
    } else if remaining > 0.2 {
        Color::srgb(1.0, 0.72, 0.18)
    } else {
        Color::srgb(1.0, 0.25, 0.25)
    }
}

fn display_width(value: &str) -> usize {
    value
        .chars()
        .map(|character| character.width().unwrap_or(0))
        .sum()
}
