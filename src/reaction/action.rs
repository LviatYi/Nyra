use std::time::{Duration, Instant};

use crate::perception::ImageCondition;

use super::{
    ActionStep, AfterAction, InputAction, POLL_INTERVAL, ScreenPoint, SdkExecution, UiCondition,
    click_screen, log_in_reaction,
};

pub(super) fn execute_action(
    execution: &SdkExecution<'_>,
    run_id: &str,
    line: u32,
    step: ActionStep,
) -> Result<(), String> {
    if step
        .message
        .as_ref()
        .is_some_and(|message| message.encode_utf16().count() > 2000)
    {
        return Err("Operation message cannot exceed 2000 UTF-16 code units".into());
    }
    if let Some(before) = step.before {
        wait_condition(execution, before).map_err(|error| format!("Before action: {error}"))?;
    }
    let description = match step.action {
        InputAction::Click(input) => {
            let point = input.point;
            if let Some(repeat) = input.repeat {
                for i in 0..repeat.count {
                    click_screen(execution, &point)?;
                    if i < repeat.count - 1 {
                        execution.delay(Duration::from_millis(repeat.interval_ms))?;
                    }
                }
            } else {
                click_screen(execution, &point)?;
            }
            format!("CLICK AT ({}, {})", point.x, point.y)
        }
        InputAction::Drag(input) => {
            let from = input.from;
            let to = input.to;
            let duration_ms = input.duration_ms;
            drag_screen(execution, &from, &to, Duration::from_millis(duration_ms))?;
            format!("DRAG FROM ({}, {}) TO ({}, {})", from.x, from.y, to.x, to.y)
        }
    };
    if let Some(AfterAction::Delay(condition)) = step.after {
        execution
            .delay(Duration::from_millis(condition.duration_ms))
            .map_err(|error| format!("After action: {error}"))?;
    }
    log_in_reaction(
        run_id,
        line,
        step.message.as_deref().unwrap_or(&description),
    )
}

fn wait_condition(execution: &SdkExecution<'_>, condition: UiCondition) -> Result<(), String> {
    match condition {
        UiCondition::Delay(condition) => {
            execution.delay(Duration::from_millis(condition.duration_ms))
        }
        UiCondition::Image(condition) => {
            let path = condition.path;
            let timeout_ms = condition.timeout_ms;
            let poll_interval_ms = condition.poll_interval_ms;
            if path.trim().is_empty() {
                return Err("Image condition path cannot be empty".into());
            }
            if timeout_ms == 0 || poll_interval_ms == 0 || poll_interval_ms > timeout_ms {
                return Err("Image condition requires 0 < pollIntervalMs <= timeoutMs".into());
            }
            let started_at = Instant::now();
            let timeout = Duration::from_millis(timeout_ms);
            let poll_interval = Duration::from_millis(poll_interval_ms);
            let image = ImageCondition::load(&execution.base_dir.join(&path))?;
            loop {
                execution.check()?;
                if started_at.elapsed() >= timeout {
                    return Err(format!(
                        "Image condition timed out after {timeout_ms} ms: {path}"
                    ));
                }
                let matched = image.matches()?;
                execution.check()?;
                if started_at.elapsed() >= timeout {
                    return Err(format!(
                        "Image condition timed out after {timeout_ms} ms: {path}"
                    ));
                }
                if matched == Some(true) {
                    return Ok(());
                }
                execution.delay(poll_interval.min(timeout.saturating_sub(started_at.elapsed())))?;
            }
        }
    }
}

#[cfg(windows)]
fn drag_screen(
    execution: &SdkExecution<'_>,
    from: &ScreenPoint,
    to: &ScreenPoint,
    duration: Duration,
) -> Result<(), String> {
    use windows_sys::Win32::UI::{
        Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT,
            SendInput,
        },
        WindowsAndMessaging::SetCursorPos,
    };

    execution.check()?;
    let move_to = |x, y| {
        if unsafe { SetCursorPos(x, y) } == 0 {
            Err(format!(
                "Unable to move cursor: {}",
                std::io::Error::last_os_error()
            ))
        } else {
            Ok(())
        }
    };
    let button = |flags| {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        };
        if unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) } != 1 {
            Err(format!(
                "Unable to change mouse button state: {}",
                std::io::Error::last_os_error()
            ))
        } else {
            Ok(())
        }
    };

    move_to(from.x, from.y)?;
    execution.check()?;
    button(MOUSEEVENTF_LEFTDOWN)?;
    let result = (|| {
        let started_at = Instant::now();
        loop {
            execution.check()?;
            let elapsed = started_at.elapsed();
            if elapsed >= duration {
                return move_to(to.x, to.y);
            }
            let fraction = elapsed.as_secs_f64() / duration.as_secs_f64();
            let x = f64::from(from.x) + (f64::from(to.x) - f64::from(from.x)) * fraction;
            let y = f64::from(from.y) + (f64::from(to.y) - f64::from(from.y)) * fraction;
            move_to(x.round() as i32, y.round() as i32)?;
            execution.delay(POLL_INTERVAL.min(duration.saturating_sub(elapsed)))?;
        }
    })();
    // A held button must be released even when movement, waiting or cancellation fails.
    let release = button(MOUSEEVENTF_LEFTUP);
    match (result, release) {
        (Err(error), Err(release)) => Err(format!("{error}; mouse release also failed: {release}")),
        (Err(error), _) | (_, Err(error)) => Err(error),
        _ => Ok(()),
    }
}

#[cfg(not(windows))]
fn drag_screen(
    execution: &SdkExecution<'_>,
    _from: &ScreenPoint,
    _to: &ScreenPoint,
    _duration: Duration,
) -> Result<(), String> {
    execution.check()?;
    Err("Reaction drag currently requires Windows".into())
}
