use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use bevy::prelude::Resource;
use serde::Deserialize;

use super::{
    MAX_LINE_BYTES, MAX_OUTPUT_BYTES, POLL_INTERVAL, RunState, RunStatus, SdkRequest,
    dispatch_sdk_request,
    process_job::{ProcessJob, ScriptSlot},
    respond,
};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

enum WorkerCommand {
    Run { run_id: String, script_id: u32 },
    Cancel,
    Shutdown,
}

enum WorkerEvent {
    Command(WorkerCommand),
    Line { diagnostic: bool, text: String },
    Closed,
    Error(String),
}

#[derive(Deserialize)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum RunnerMessage {
    Ready {
        version: u32,
        script_count: usize,
    },
    StartupFailed {
        version: u32,
        error: String,
    },
    Completed {
        version: u32,
        run_id: String,
    },
    Failed {
        version: u32,
        run_id: String,
        error: String,
    },
    Request {
        version: u32,
        run_id: String,
        request: SdkRequest,
    },
}

struct ActiveRun {
    run_id: String,
    started_at: Instant,
    next_request_id: u64,
    _slot: ScriptSlot,
}

struct ManagedProcess {
    child: Child,
    job: Option<ProcessJob>,
}

impl Drop for ManagedProcess {
    fn drop(&mut self) {
        self.child.stdin.take();
        self.job.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A Bun process preloaded with every configured macro and kept for the Bevy app lifetime.
#[derive(Resource)]
pub struct ReactionRunner {
    script_base: PathBuf,
    script_ids: HashMap<PathBuf, u32>,
    sequence: u64,
    status: Arc<Mutex<RunStatus>>,
    events: Option<mpsc::SyncSender<WorkerEvent>>,
    worker: Option<JoinHandle<()>>,
}

impl ReactionRunner {
    pub fn new(
        config: &super::ReactionConfig,
        config_path: &Path,
        scripts: impl IntoIterator<Item = PathBuf>,
    ) -> Result<Self, String> {
        let executable = std::env::current_exe()
            .map_err(|error| format!("Unable to locate Nyra installation directory: {error}"))?;
        let config_path = std::path::absolute(config_path).map_err(|error| {
            format!("Unable to locate config {}: {error}", config_path.display())
        })?;
        let script_base = config_path.parent().unwrap().to_path_buf();
        let runtime_dir = executable.parent().unwrap().join("runtime");
        let mut script_ids = HashMap::new();
        let mut script_paths = Vec::new();
        for script in scripts {
            let path = resolve_script(&script_base, &script)?;
            if !script_ids.contains_key(&path) {
                let id = u32::try_from(script_paths.len()).map_err(|_| {
                    "The number of configured Reaction scripts exceeds the supported range"
                        .to_string()
                })?;
                script_ids.insert(path.clone(), id);
                script_paths.push(path);
            }
        }

        let status = Arc::new(Mutex::new(RunStatus {
            run_id: None,
            state: RunState::Idle,
        }));
        // if script_paths.is_empty() {
        //     return Ok(Self {
        //         script_base,
        //         script_ids,
        //         sequence: 0,
        //         status,
        //         events: None,
        //         worker: None,
        //     });
        // }
        for name in ["bun.exe", "runner.ts", "bun.json", "bunfig.toml"] {
            let path = runtime_dir.join(name);
            if !path.is_file() {
                return Err(format!(
                    "Missing runtime file {} distributed with Nyra, please reinstall",
                    path.display()
                ));
            }
        }

        let (event_tx, event_rx) = mpsc::sync_channel(64);
        let (startup_tx, startup_rx) = mpsc::sync_channel(1);
        let worker_status = status.clone();
        let worker_events = event_tx.clone();
        let cwd = script_base.clone();
        let worker_runtime_dir = runtime_dir.clone();
        let timeout = Duration::from_millis(config.timeout_ms);
        let worker = thread::Builder::new()
            .name("reaction-runner".into())
            .spawn(move || {
                let result = run_worker(
                    &worker_runtime_dir,
                    &cwd,
                    &script_paths,
                    timeout,
                    event_rx,
                    worker_events,
                    &worker_status,
                    &startup_tx,
                );
                if let Err(error) = result {
                    let _ = startup_tx.try_send(Err(error.clone()));
                    worker_status.lock().unwrap().state = RunState::Failed(error);
                }
            })
            .map_err(|error| {
                format!("Unable to start resident script management thread: {error}")
            })?;

        match startup_rx.recv_timeout(STARTUP_TIMEOUT) {
            Ok(Ok(())) => Ok(Self {
                script_base,
                script_ids,
                sequence: 0,
                status,
                events: Some(event_tx),
                worker: Some(worker),
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(_) => {
                let _ = event_tx.try_send(WorkerEvent::Command(WorkerCommand::Shutdown));
                let _ = worker.join();
                Err("Waiting for resident Bun initialization timed out".into())
            }
        }
    }

    pub fn start(&mut self, script: &Path) -> Result<String, String> {
        self.poll();
        if self.is_busy() {
            return Err(
                "A TS script is already running or cleaning up, this trigger is rejected".into(),
            );
        }
        let path = resolve_script(&self.script_base, script)?;
        let script_id = *self.script_ids.get(&path).ok_or_else(|| {
            format!(
                "The script was not preloaded by the resident Bun: {}",
                path.display()
            )
        })?;
        let events = self
            .events
            .as_ref()
            .ok_or("The current configuration has no runnable Reaction scripts")?;
        self.sequence += 1;
        let run_id = format!("{}-{}", std::process::id(), self.sequence);
        *self.status.lock().unwrap() = RunStatus {
            run_id: Some(run_id.clone()),
            state: RunState::Running,
        };
        if let Err(error) = events.try_send(WorkerEvent::Command(WorkerCommand::Run {
            run_id: run_id.clone(),
            script_id,
        })) {
            self.status.lock().unwrap().state =
                RunState::Failed(format!("Unable to trigger resident Bun: {error}"));
            return Err(format!("Unable to trigger resident Bun: {error}"));
        }
        Ok(run_id)
    }

    pub fn status(&self) -> RunStatus {
        self.status.lock().unwrap().clone()
    }

    pub fn is_busy(&self) -> bool {
        matches!(
            self.status().state,
            RunState::Running | RunState::CleaningUp
        )
    }

    pub fn cancel(&self) {
        if let Some(events) = &self.events {
            let _ = events.try_send(WorkerEvent::Command(WorkerCommand::Cancel));
        }
    }

    pub fn poll(&mut self) {
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.is_finished())
        {
            let worker = self.worker.take().unwrap();
            if worker.join().is_err() {
                self.status.lock().unwrap().state = RunState::Failed(
                    "Resident script management thread exited unexpectedly".into(),
                );
            }
            self.events.take();
        }
    }
}

impl Drop for ReactionRunner {
    fn drop(&mut self) {
        if let Some(events) = self.events.take() {
            let _ = events.try_send(WorkerEvent::Command(WorkerCommand::Shutdown));
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn resolve_script(base: &Path, script: &Path) -> Result<PathBuf, String> {
    if script
        .extension()
        .is_none_or(|ext| ext != "ts" && ext != "mts")
    {
        return Err(format!(
            "The script must be a .ts or .mts file: {}",
            script.display()
        ));
    }
    let path = std::path::absolute(base.join(script)).map_err(|error| {
        format!(
            "Unable to locate script {}: {error}",
            base.join(script).display()
        )
    })?;
    if !path.is_file() {
        return Err(format!(
            "The script file does not exist or is inaccessible: {}",
            path.display()
        ));
    }
    Ok(path)
}

fn run_worker(
    runtime_dir: &Path,
    cwd: &Path,
    scripts: &[PathBuf],
    timeout: Duration,
    event_rx: mpsc::Receiver<WorkerEvent>,
    event_tx: mpsc::SyncSender<WorkerEvent>,
    status: &Mutex<RunStatus>,
    startup: &mpsc::SyncSender<Result<(), String>>,
) -> Result<(), String> {
    let mut bun_config = std::ffi::OsString::from("--config=");
    bun_config.push(runtime_dir.join("bunfig.toml"));
    let mut command = Command::new(runtime_dir.join("bun.exe"));
    command
        .arg("--no-install")
        .arg("--no-env-file")
        .arg(bun_config)
        .arg("run")
        .arg(runtime_dir.join("runner.ts"))
        .args(scripts)
        .current_dir(cwd)
        .env_remove("NODE_OPTIONS")
        .env_remove("BUN_OPTIONS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let child = command
        .spawn()
        .map_err(|error| format!("Unable to start Bun distributed with the program: {error}"))?;
    let mut process = ManagedProcess { child, job: None };
    process.job =
        Some(ProcessJob::attach(&process.child).map_err(|error| {
            format!("Unable to add Bun to the process management group: {error}")
        })?);
    let (input_tx, input_rx) = mpsc::sync_channel::<Vec<u8>>(1);
    let mut input_tx = Some(input_tx);
    let mut stdin = process.child.stdin.take().unwrap();
    let stdout = process.child.stdout.take().unwrap();
    let stderr = process.child.stderr.take().unwrap();
    let input_events = event_tx.clone();
    let input_writer = thread::Builder::new()
        .name("reaction-stdin".into())
        .spawn(move || {
            for bytes in input_rx {
                if let Err(error) = stdin.write_all(&bytes) {
                    let _ = input_events.send(WorkerEvent::Error(format!(
                        "Failed to write to Bun stdin: {error}"
                    )));
                    break;
                }
            }
        })
        .map_err(|error| format!("Unable to start Bun input thread: {error}"))?;
    let stdout_events = event_tx.clone();
    let stdout_reader = thread::Builder::new()
        .name("reaction-stdout".into())
        .spawn(move || read_output(stdout, false, stdout_events))
        .map_err(|error| format!("Unable to read Bun stdout: {error}"))?;
    let stderr_reader = thread::Builder::new()
        .name("reaction-stderr".into())
        .spawn(move || read_output(stderr, true, event_tx))
        .map_err(|error| format!("Unable to read Bun stderr: {error}"))?;

    input_tx
        .as_ref()
        .unwrap()
        .try_send(b"start\n".to_vec())
        .map_err(|error| format!("Unable to initialize Bun channel: {error}"))?;

    let mut active = None::<ActiveRun>;
    let mut ready = false;
    let mut output_bytes = 0usize;
    let mut open_output_streams = 2usize;
    let loop_result = loop {
        if active
            .as_ref()
            .is_some_and(|run| run.started_at.elapsed() >= timeout)
        {
            break Err(format!(
                "Script execution timed out ({} ms)",
                timeout.as_millis()
            ));
        }
        if let Some(exit) = process
            .child
            .try_wait()
            .map_err(|error| format!("Unable to query Bun status: {error}"))?
        {
            break Err(format!("Resident Bun exited unexpectedly: {exit}"));
        }

        match event_rx.recv_timeout(POLL_INTERVAL) {
            Ok(WorkerEvent::Command(WorkerCommand::Shutdown)) => break Ok(()),
            Ok(WorkerEvent::Command(WorkerCommand::Cancel)) => {
                if active.is_some() {
                    status.lock().unwrap().state = RunState::CleaningUp;
                    break Err("Script has been cancelled; resident Bun has been reclaimed".into());
                }
            }
            Ok(WorkerEvent::Command(WorkerCommand::Run { run_id, script_id })) => {
                if !ready || active.is_some() {
                    status.lock().unwrap().state = RunState::Failed(
                        "Resident Bun is not ready or a script is already running".into(),
                    );
                    continue;
                }
                let slot = match ScriptSlot::acquire() {
                    Ok(slot) => slot,
                    Err(error) => {
                        status.lock().unwrap().state = RunState::Failed(error);
                        continue;
                    }
                };
                let command = serde_json::json!({
                    "version": 1,
                    "event": "run",
                    "runId": &run_id,
                    "scriptId": script_id,
                });
                send_json(&input_tx, command)?;
                output_bytes = 0;
                active = Some(ActiveRun {
                    run_id,
                    started_at: Instant::now(),
                    next_request_id: 1,
                    _slot: slot,
                });
            }
            Ok(WorkerEvent::Error(error)) => break Err(error),
            Ok(WorkerEvent::Closed) => {
                open_output_streams = open_output_streams.saturating_sub(1);
                if open_output_streams == 0 {
                    break Err("Resident Bun output channel has been disconnected".into());
                }
            }
            Ok(WorkerEvent::Line { diagnostic, text }) => {
                output_bytes += text.len();
                if output_bytes > MAX_OUTPUT_BYTES {
                    break Err("Script cumulative output exceeds 1 MiB".into());
                }
                if diagnostic {
                    let run_id = active.as_ref().map_or("runner", |run| run.run_id.as_str());
                    eprint!("[Reaction {run_id}] {text}");
                    continue;
                }
                let message: RunnerMessage = serde_json::from_str(&text)
                    .map_err(|error| format!("Script stdout protocol error: {error}"))?;
                match message {
                    RunnerMessage::Ready {
                        version: 1,
                        script_count,
                    } if !ready && script_count == scripts.len() => {
                        ready = true;
                        startup
                            .send(Ok(()))
                            .map_err(|_| "Nyra startup process was interrupted".to_string())?;
                    }
                    RunnerMessage::StartupFailed { version: 1, error } if !ready => {
                        break Err(format!("Resident Bun failed to load script: {error}"));
                    }
                    RunnerMessage::Request {
                        version: 1,
                        run_id,
                        request,
                    } => {
                        let run = active
                            .as_mut()
                            .ok_or("Idle resident Bun sent an SDK request")?;
                        if run.run_id != run_id
                            || request.id != run.next_request_id
                            || request.id > 9_007_199_254_740_991
                        {
                            break Err("SDK request run ID or request ID does not match".into());
                        }
                        run.next_request_id += 1;
                        let id = request.id;
                        let result = dispatch_sdk_request(&run_id, request);
                        respond(&input_tx, &run_id, id, result)?;
                    }
                    RunnerMessage::Completed { version: 1, run_id } => {
                        let run = active
                            .take()
                            .ok_or("Idle resident Bun reported script completion")?;
                        if run.run_id != run_id {
                            break Err("Script completion message run ID does not match".into());
                        }
                        status.lock().unwrap().state = RunState::Completed;
                        output_bytes = 0;
                    }
                    RunnerMessage::Failed {
                        version: 1,
                        run_id,
                        error,
                    } => {
                        let run = active
                            .take()
                            .ok_or("Idle resident Bun reported script failure")?;
                        if run.run_id != run_id {
                            break Err("Script failure message run ID does not match".into());
                        }
                        status.lock().unwrap().state = RunState::Failed(error);
                        output_bytes = 0;
                    }
                    _ => {
                        break Err(
                            "Resident Bun lifecycle message does not match or is duplicated".into(),
                        );
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break Err("Resident Bun management channel has been disconnected".into());
            }
        }
    };

    if active.is_some() {
        status.lock().unwrap().state = RunState::CleaningUp;
    }
    drop(event_rx);
    drop(input_tx.take());
    let kill_result = match process.child.try_wait() {
        Ok(Some(_)) => Ok(()),
        _ => process.child.kill(),
    };
    process.job.take();
    let wait_result = process.child.wait();
    let input_result = input_writer.join();
    let stdout_result = stdout_reader.join();
    let stderr_result = stderr_reader.join();
    if let Err(error) = wait_result {
        return Err(format!(
            "Failed to reclaim Bun: {error}; kill result: {kill_result:?}; loop result: {loop_result:?}"
        ));
    }
    if input_result.is_err() || stdout_result.is_err() || stderr_result.is_err() {
        return Err("Bun standard stream thread exited unexpectedly".into());
    }
    loop_result
}

fn send_json(
    input: &Option<mpsc::SyncSender<Vec<u8>>>,
    value: serde_json::Value,
) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(&value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    input
        .as_ref()
        .ok_or("Bun input channel has been closed")?
        .try_send(bytes)
        .map_err(|error| format!("Failed to send resident Bun command: {error}"))
}

fn read_output(pipe: impl Read, diagnostic: bool, events: mpsc::SyncSender<WorkerEvent>) {
    let mut reader = BufReader::new(pipe);
    loop {
        let mut bytes = Vec::new();
        match reader
            .by_ref()
            .take((MAX_LINE_BYTES + 1) as u64)
            .read_until(b'\n', &mut bytes)
        {
            Ok(0) => {
                let _ = events.send(WorkerEvent::Closed);
                break;
            }
            Ok(_) if bytes.len() > MAX_LINE_BYTES => {
                let _ = events.send(WorkerEvent::Error(
                    "Script output line exceeds 16 KiB".into(),
                ));
                break;
            }
            Ok(_) => match String::from_utf8(bytes) {
                Ok(text) => {
                    if events.send(WorkerEvent::Line { diagnostic, text }).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let _ = events.send(WorkerEvent::Error(
                        "Script output is not valid UTF-8".into(),
                    ));
                    break;
                }
            },
            Err(error) => {
                let _ = events.send(WorkerEvent::Error(format!(
                    "Failed to read script output: {error}"
                )));
                break;
            }
        }
    }
}
