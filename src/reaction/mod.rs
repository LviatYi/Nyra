mod global_hotkey;
mod process_job;
mod runner;

pub(crate) use global_hotkey::GlobalHotkeys;
pub use runner::ReactionRunner;

use crate::{job::JobConfig, sensory::ActiveJobState};
use bevy::{app::AppExit, prelude::*};
use global_hotkey::HotkeyEvent;
use serde::Deserialize;
use std::{io::Write, sync::mpsc, time::Duration};

const MAX_LINE_BYTES: usize = 16 * 1024;
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ReactionConfig {
    pub timeout_ms: u64,
}

impl Default for ReactionConfig {
    fn default() -> Self {
        Self { timeout_ms: 30_000 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunState {
    Idle,
    Running,
    CleaningUp,
    Completed,
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct RunStatus {
    pub run_id: Option<String>,
    pub state: RunState,
}

pub struct ReactionPlugin;

impl Plugin for ReactionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, update_runner);
    }
}

fn update_runner(
    mut runner: ResMut<ReactionRunner>,
    mut hotkeys: ResMut<GlobalHotkeys>,
    jobs: Res<JobConfig>,
    active_job: Res<ActiveJobState>,
    mut exit: MessageReader<AppExit>,
) {
    if exit.read().next().is_some() {
        runner.cancel();
        let _ = hotkeys.set_active(None);
        return;
    }

    let reaction = active_job
        .current_index()
        .and_then(|index| jobs.0.tips.get(index))
        .and_then(|tip| tip.reaction.as_ref());
    let desired_hotkey = reaction.map(|reaction| reaction.hotkey.clone());
    if let Err(error) = hotkeys.set_active(desired_hotkey.clone()) {
        eprintln!("[Reaction] {error}");
    }
    for event in hotkeys.drain() {
        match event {
            HotkeyEvent::Registered(Some(hotkey)) => {
                println!("[Reaction] Current global hotkey: {hotkey}");
            }
            HotkeyEvent::Registered(None) => {}
            HotkeyEvent::RegistrationFailed(error) => eprintln!("[Reaction] {error}"),
            HotkeyEvent::Triggered(hotkey) if desired_hotkey.as_ref() == Some(&hotkey) => {
                let Some(reaction) = reaction else {
                    continue;
                };
                match runner.start(&reaction.script) {
                    Ok(run_id) => println!(
                        "[Reaction {run_id}] Hotkey {hotkey} triggered: {}",
                        reaction.script.display()
                    ),
                    Err(error) => {
                        eprintln!("[Reaction] Hotkey {hotkey} failed to trigger: {error}")
                    }
                }
            }
            HotkeyEvent::Triggered(_) => {}
        }
    }
    runner.poll();
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SdkRequest {
    id: u64,
    method: String,
    args: serde_json::Value,
}

macro_rules! decode_sdk_arguments {
    ($method:expr, $args:expr;) => {
        let _: [serde_json::Value; 0] = serde_json::from_value($args).map_err(|error| {
            format!("SDK {} arguments are invalid: {error}", $method)
        })?;
    };
    ($method:expr, $args:expr; $($argument:ident: $argument_type:ty),+ $(,)?) => {
        let ($($argument,)+): ($($argument_type,)+) =
            serde_json::from_value($args).map_err(|error| {
                format!("SDK {} arguments are invalid: {error}", $method)
            })?;
    };
}

macro_rules! define_sdk {
    (
        types {
            $(
                $(#[doc = $type_doc:literal])*
                struct $type_name:ident {
                    $(
                        $(#[doc = $field_doc:literal])*
                        $field:ident: $field_type:ty
                    ),* $(,)?
                }
            )*
        }
        context $context:ident {
            properties {
                $(
                    $(#[doc = $property_doc:literal])*
                    readonly $property:ident: $property_type:ty;
                )*
            }
            methods {
                $(
                    $(#[doc = $method_doc:literal])*
                    $method:literal(
                        $run_id:ident;
                        $($argument:ident: $argument_type:ty),* $(,)?
                    ) $body:block
                )*
            }
        }
    ) => {
        $(
            $(#[doc = $type_doc])*
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase", deny_unknown_fields)]
            struct $type_name {
                $(
                    $(#[doc = $field_doc])*
                    $field: $field_type,
                )*
            }
        )*

        fn dispatch_sdk_request(run_id: &str, request: SdkRequest) -> Result<(), String> {
            match request.method.as_str() {
                $(
                    $method => {
                        decode_sdk_arguments!(request.method, request.args;
                            $($argument: $argument_type),*);
                        let $run_id = run_id;
                        (|| -> Result<(), String> { $body })()
                    }
                )*
                method => Err(format!("Unknown SDK method: {method}")),
            }
        }
    };
}

include!("sdk.rs");

fn click_screen(point: &ScreenPoint) -> Result<(), String> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::{
            Input::KeyboardAndMouse::{
                INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
                MOUSEINPUT, SendInput,
            },
            WindowsAndMessaging::SetCursorPos,
        };

        if SetCursorPos(point.x, point.y) == 0 {
            return Err(format!(
                "Failed to move the cursor to ({}, {}): {}",
                point.x,
                point.y,
                std::io::Error::last_os_error()
            ));
        }

        let mouse_input = |flags| INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        };
        let inputs = [
            mouse_input(MOUSEEVENTF_LEFTDOWN),
            mouse_input(MOUSEEVENTF_LEFTUP),
        ];
        let inserted = SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
        if inserted != inputs.len() as u32 {
            let error = std::io::Error::last_os_error();
            if inserted == 1 {
                let release = mouse_input(MOUSEEVENTF_LEFTUP);
                let _ = SendInput(1, &release, std::mem::size_of::<INPUT>() as i32);
            }
            return Err(format!(
                "Failed to click at ({}, {}): SendInput inserted {inserted} of {} events ({})",
                point.x,
                point.y,
                inputs.len(),
                error
            ));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = point;
        Err("Reaction click currently requires Windows".into())
    }
}

fn log_in_reaction(run_id: &str, message: &str) -> Result<(), String> {
    writeln!(std::io::stdout().lock(), "[Reaction {run_id}] {message}")
        .map_err(|error| format!("Failed to write to host log: {error}"))
}

fn respond(
    input: &Option<mpsc::SyncSender<Vec<u8>>>,
    run_id: &str,
    request_id: u64,
    result: Result<(), String>,
) -> Result<(), String> {
    let mut response = serde_json::json!({
        "version": 1,
        "runId": run_id,
        "event": "response",
        "requestId": request_id,
        "ok": result.is_ok(),
    });
    if let Err(error) = result {
        response["error"] = error.into();
    }
    let mut bytes = serde_json::to_vec(&response).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    input
        .as_ref()
        .ok_or("Bun input channel has been closed")?
        .try_send(bytes)
        .map_err(|error| format!("Unable to send SDK response: {error}"))
}
