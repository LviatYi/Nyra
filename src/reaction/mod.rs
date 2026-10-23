mod action;
mod global_hotkey;
mod process_job;
mod runner;
pub(crate) mod sdk_types;

pub(crate) use global_hotkey::GlobalHotkeys;
pub use runner::ReactionRunner;
use sdk_types::{ActionStep, AfterAction, InputAction, ScreenPoint, UiCondition};

use crate::{
    controller::{ActiveJobState, JobProcessing},
    job::JobConfig,
    sensation::tip_overlay::MenuProcessing,
};
use action::execute_action;
use bevy::{app::AppExit, prelude::*};
use serde::Deserialize;
use std::{
    io::Write,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

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
    ScriptFailed(String),
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
        app.add_systems(PreUpdate, poll_hotkeys).add_systems(
            Update,
            update_runner.after(JobProcessing).after(MenuProcessing),
        );
    }
}

fn poll_hotkeys(mut hotkeys: NonSendMut<GlobalHotkeys>) {
    hotkeys.poll();
}

fn update_runner(
    mut runner: ResMut<ReactionRunner>,
    mut hotkeys: NonSendMut<GlobalHotkeys>,
    jobs: Res<JobConfig>,
    active_job: Res<ActiveJobState>,
    mut exit: MessageReader<AppExit>,
) -> Result<(), BevyError> {
    if exit.read().next().is_some() {
        runner.cancel();
        hotkeys.set_active(&[])?;
        return Ok(());
    }

    let reaction = active_job
        .current_index()
        .and_then(|index| jobs.0.jobs.get(index))
        .and_then(|job| job.tip().reaction.as_ref());
    if let Some(reaction) = reaction
        && hotkeys.just_pressed(reaction.hotkey)
    {
        let hotkey = reaction.hotkey;
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
    runner.poll();
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SdkInstruction {
    line: u32,
    method: String,
    args: serde_json::Value,
}

struct SdkExecution<'a> {
    cancelled: &'a AtomicBool,
    started_at: Instant,
    timeout: Duration,
    base_dir: &'a Path,
}

impl SdkExecution<'_> {
    fn check(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err("Script has been cancelled; resident Bun has been reclaimed".into());
        }
        if self.started_at.elapsed() >= self.timeout {
            return Err(format!(
                "Script execution timed out ({} ms)",
                self.timeout.as_millis()
            ));
        }
        Ok(())
    }

    fn delay(&self, duration: Duration) -> Result<(), String> {
        let started_at = Instant::now();
        loop {
            self.check()?;
            let remaining = duration.saturating_sub(started_at.elapsed());
            if remaining.is_zero() {
                return Ok(());
            }
            std::thread::sleep(remaining.min(POLL_INTERVAL));
        }
    }
}

macro_rules! validate_sdk_arguments {
    ($method:expr, $args:expr;) => {
        serde_json::from_value::<[serde_json::Value; 0]>($args.clone())
            .map(|_| ())
            .map_err(|error| format!("SDK {} arguments are invalid: {error}", $method))
    };
    ($method:expr, $args:expr; $($argument_type:ty),+ $(,)?) => {
        serde_json::from_value::<($($argument_type,)+)>($args.clone())
            .map(|_| ())
            .map_err(|error| format!("SDK {} arguments are invalid: {error}", $method))
    };
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
                        $execution:ident, $run_id:ident, $line:ident;
                        $($argument:ident: $argument_type:ty),* $(,)?
                    ) $body:block
                )*
            }
        }
    ) => {
        fn validate_sdk_instruction(instruction: &SdkInstruction) -> Result<(), String> {
            match instruction.method.as_str() {
                $(
                    $method => validate_sdk_arguments!(instruction.method, instruction.args;
                        $($argument_type),*),
                )*
                method => Err(format!("Unknown SDK method: {method}")),
            }
        }

        fn dispatch_sdk_instruction(
            execution: &SdkExecution<'_>,
            run_id: &str,
            instruction: SdkInstruction,
        ) -> Result<(), String> {
            execution.check()?;
            match instruction.method.as_str() {
                $(
                    $method => {
                        decode_sdk_arguments!(instruction.method, instruction.args;
                            $($argument: $argument_type),*);
                        let $execution = execution;
                        let $run_id = run_id;
                        let $line = instruction.line;
                        (|| -> Result<(), String> { $body })()
                    }
                )*
                method => Err(format!("Unknown SDK method: {method}")),
            }
        }
    };
}

include!("sdk.rs");

fn click_screen(execution: &SdkExecution<'_>, point: &ScreenPoint) -> Result<(), String> {
    execution.check()?;
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::{
            Input::KeyboardAndMouse::{
                INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT,
                SendInput,
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

fn log_in_reaction(run_id: &str, line: u32, message: &str) -> Result<(), String> {
    writeln!(
        std::io::stdout().lock(),
        "[Reaction {run_id}] [Line {line}] {message}"
    )
    .map_err(|error| format!("Failed to write to host log: {error}"))
}

fn respond(
    input: &Option<mpsc::SyncSender<Vec<u8>>>,
    run_id: &str,
    request_id: u64,
    result: Result<(), String>,
) -> Result<(), String> {
    let mut response = serde_json::json!({
        "version": 3,
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
