mod selector;

use crate::job::{Job, Jobs};
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const CONDITION_VERSION: u32 = 2;
const DEFAULT_THRESHOLD: f64 = 0.95;
const DEFAULT_POLL_INTERVAL_MS: u64 = 500;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MonitorConfig {
    name: String,
    width: u32,
    height: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConditionConfig {
    version: u32,
    target: CaptureTarget,
    template: PathBuf,
    threshold: f64,
    poll_interval_ms: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
enum CaptureTarget {
    Screen {
        monitor: MonitorConfig,
        region: Region,
    },
    Window {
        title: String,
        region: Region,
    },
}

impl CaptureTarget {
    fn region(&self) -> Region {
        match self {
            Self::Screen { region, .. } | Self::Window { region, .. } => *region,
        }
    }
}

pub enum RecordTarget<'a> {
    AutoWindow,
    Window(&'a str),
    Screen,
}

#[cfg(target_os = "windows")]
struct WindowBounds {
    title: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[cfg(target_os = "windows")]
struct PreparedJobCondition {
    job_index: usize,
    config: ConditionConfig,
    template: Vec<u8>,
}

#[cfg(target_os = "windows")]
struct WatchedJobCondition {
    prepared: PreparedJobCondition,
    next_poll: Instant,
    matched: bool,
    capture_failed: bool,
}

#[derive(Resource)]
pub struct PerceptionObserver {
    matches: Arc<Mutex<Vec<usize>>>,
    stop: Arc<AtomicBool>,
    worker: Mutex<Option<thread::JoinHandle<()>>>,
}

impl PerceptionObserver {
    #[cfg(target_os = "windows")]
    pub fn new(jobs: &Jobs, config_path: &Path) -> Result<Option<Self>, String> {
        let base = config_path.parent().unwrap_or_else(|| Path::new("."));
        let mut conditions = Vec::new();
        for (job_index, job) in jobs.jobs.iter().enumerate() {
            let Job::ImagePerception {
                image: condition, ..
            } = job
            else {
                continue;
            };
            let path = base.join(condition);
            let config = load_condition(&path)?;
            validate_screen_target(&config.target)?;
            let template_path = path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(&config.template);
            let template = xcap::image::open(&template_path)
                .map_err(|error| {
                    format!(
                        "Unable to load template {}: {error}",
                        template_path.display()
                    )
                })?
                .into_rgba8();
            let region = config.target.region();
            if template.width() != region.width || template.height() != region.height
            {
                return Err(format!(
                    "Template size does not match the region in {}",
                    path.display()
                ));
            }
            conditions.push(PreparedJobCondition {
                job_index,
                config,
                template: template.into_raw(),
            });
        }
        if conditions.is_empty() {
            return Ok(None);
        }
        let matches = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_matches = matches.clone();
        let worker_stop = stop.clone();
        let worker = thread::Builder::new()
            .name("perception-tips".into())
            .spawn(move || watch_tip_conditions(conditions, worker_matches, worker_stop))
            .map_err(|error| format!("Unable to start perception watcher: {error}"))?;
        Ok(Some(Self {
            matches,
            stop,
            worker: Mutex::new(Some(worker)),
        }))
    }

    #[cfg(not(target_os = "windows"))]
    pub fn new(jobs: &Jobs, _config_path: &Path) -> Result<Option<Self>, String> {
        if jobs
            .jobs
            .iter()
            .any(|job| matches!(job, Job::ImagePerception { .. }))
        {
            Err("Perception matching currently requires Windows".into())
        } else {
            Ok(None)
        }
    }

    pub fn matched_job_indexes(&self) -> Vec<usize> {
        self.matches.lock().unwrap().clone()
    }
}

impl Drop for PerceptionObserver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.lock().unwrap().take() {
            let _ = worker.join();
        }
    }
}

#[cfg(target_os = "windows")]
fn watch_tip_conditions(
    conditions: Vec<PreparedJobCondition>,
    matches: Arc<Mutex<Vec<usize>>>,
    stop: Arc<AtomicBool>,
) {
    let mut watched = Vec::new();
    for prepared in conditions {
        watched.push(WatchedJobCondition {
            prepared,
            next_poll: Instant::now(),
            matched: false,
            capture_failed: false,
        });
    }
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        for item in &mut watched {
            if item.matched
                && let CaptureTarget::Window { title, .. } = &item.prepared.config.target
                && !window_is_focused(title)
            {
                item.matched = false;
            }
            if now < item.next_poll {
                continue;
            }
            item.next_poll = now + Duration::from_millis(item.prepared.config.poll_interval_ms);
            match capture_target(&item.prepared.config.target) {
                Ok(Some(frame)) => {
                    item.capture_failed = false;
                    item.matched = similarity(&item.prepared.template, frame.as_raw())
                        >= item.prepared.config.threshold;
                }
                Ok(None) => {
                    item.capture_failed = false;
                    item.matched = false;
                }
                Err(error) => {
                    if !item.capture_failed {
                        eprintln!(
                            "[Perception] Capture failed for job {}: {error}",
                            item.prepared.job_index
                        );
                    }
                    item.capture_failed = true;
                    item.matched = false;
                }
            }
        }
        let current = watched
            .iter()
            .filter(|item| item.matched)
            .map(|item| item.prepared.job_index)
            .collect::<Vec<_>>();
        let mut shared = matches.lock().unwrap();
        if *shared != current {
            *shared = current;
        }
        drop(shared);
        let delay = watched
            .iter()
            .map(|item| item.next_poll.saturating_duration_since(Instant::now()))
            .min()
            .unwrap_or(Duration::from_millis(50))
            .min(Duration::from_millis(50));
        thread::sleep(delay);
    }
}

#[cfg(target_os = "windows")]
pub fn record_condition(condition_path: &Path, record_target: RecordTarget<'_>) -> Result<(), String> {
    if condition_path
        .extension()
        .is_none_or(|extension| extension != "json")
    {
        return Err(format!(
            "Perception condition path must use the .json extension: {}",
            condition_path.display()
        ));
    }

    let monitor = primary_monitor()?;
    let monitor_name = monitor
        .name()
        .map_err(|error| format!("Unable to read primary monitor name: {error}"))?;
    let monitor_width = monitor
        .width()
        .map_err(|error| format!("Unable to read primary monitor width: {error}"))?;
    let monitor_height = monitor
        .height()
        .map_err(|error| format!("Unable to read primary monitor height: {error}"))?;
    let monitor_x = monitor
        .x()
        .map_err(|error| format!("Unable to read primary monitor position: {error}"))?;
    let monitor_y = monitor
        .y()
        .map_err(|error| format!("Unable to read primary monitor position: {error}"))?;
    let windows = match record_target {
        RecordTarget::AutoWindow => xcap::Window::all()
            .map_err(|error| format!("Unable to enumerate windows: {error}"))?
            .into_iter()
            .filter_map(|window| window_bounds(&window).ok())
            .collect::<Vec<_>>(),
        RecordTarget::Window(title) => {
            if title.trim().is_empty() {
                return Err("Window title cannot be empty".into());
            }
            vec![window_bounds(&window_by_title(title)?)?]
        }
        RecordTarget::Screen => Vec::new(),
    };
    let source = monitor
        .capture_image()
        .map_err(|error| format!("Unable to capture primary monitor before selection: {error}"))?;

    println!(
        "[Perception] Drag on the primary monitor to select the condition region; press Escape to cancel"
    );
    let selector_pixels = source.as_raw().clone();
    let region = selector::select_region(monitor_width, monitor_height, selector_pixels)
        .ok_or_else(|| "Perception region selection was cancelled".to_string())?;
    validate_region(region, monitor_width, monitor_height)?;
    let template = crop_rgba(source.as_raw(), monitor_width, monitor_height, region)?;
    let selected_x = i64::from(monitor_x) + i64::from(region.x);
    let selected_y = i64::from(monitor_y) + i64::from(region.y);
    let window = match record_target {
        RecordTarget::AutoWindow => {
            let window = windows
                .iter()
                .find(|window| {
                    selected_x >= i64::from(window.x)
                        && selected_y >= i64::from(window.y)
                        && selected_x + i64::from(region.width)
                            <= i64::from(window.x) + i64::from(window.width)
                        && selected_y + i64::from(region.height)
                            <= i64::from(window.y) + i64::from(window.height)
                })
                .ok_or("No window contains the selected region; use --screen for an absolute screen region")?;
            if windows.iter().filter(|candidate| candidate.title == window.title).count() > 1 {
                return Err(format!("Multiple windows have the same title: {}", window.title));
            }
            Some(window)
        }
        RecordTarget::Window(_) => windows.first(),
        RecordTarget::Screen => None,
    };
    let target = if let Some(window) = window {
        let relative_x = selected_x - i64::from(window.x);
        let relative_y = selected_y - i64::from(window.y);
        let relative_region = Region {
            x: u32::try_from(relative_x)
                .map_err(|_| "Selected region starts outside the titled window")?,
            y: u32::try_from(relative_y)
                .map_err(|_| "Selected region starts outside the titled window")?,
            width: region.width,
            height: region.height,
        };
        validate_region(relative_region, window.width, window.height)?;
        CaptureTarget::Window {
            title: window.title.clone(),
            region: relative_region,
        }
    } else {
        CaptureTarget::Screen {
            monitor: MonitorConfig {
                name: monitor_name,
                width: monitor_width,
                height: monitor_height,
            },
            region,
        }
    };

    let condition_directory = condition_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(condition_directory).map_err(|error| {
        format!(
            "Unable to create condition directory {}: {error}",
            condition_directory.display()
        )
    })?;
    let stem = condition_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| "Perception condition filename must be valid UTF-8".to_string())?;
    let template_filename = format!("{stem}.png");
    let template_path = condition_directory.join(&template_filename);

    let condition = ConditionConfig {
        version: CONDITION_VERSION,
        target,
        template: PathBuf::from(template_filename),
        threshold: DEFAULT_THRESHOLD,
        poll_interval_ms: DEFAULT_POLL_INTERVAL_MS,
    };
    let mut json = serde_json::to_string_pretty(&condition)
        .map_err(|error| format!("Unable to serialize perception condition: {error}"))?;
    json.push('\n');
    let mut encoded_template = Vec::new();
    {
        use xcap::image::{
            ExtendedColorType, ImageEncoder,
            codecs::png::{CompressionType, FilterType, PngEncoder},
        };

        PngEncoder::new_with_quality(
            &mut encoded_template,
            CompressionType::Fast,
            FilterType::Sub,
        )
        .write_image(
            &template,
            region.width,
            region.height,
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| format!("Unable to encode template as PNG: {error}"))?;
    }
    fs::write(&template_path, encoded_template).map_err(|error| {
        format!(
            "Unable to save template {}: {error}",
            template_path.display()
        )
    })?;
    fs::write(condition_path, json).map_err(|error| {
        format!(
            "Unable to save condition {}: {error}",
            condition_path.display()
        )
    })?;

    println!(
        "[Perception] Condition: {}\n[Perception] Image: {}",
        condition_path.display(),
        template_path.display()
    );
    match &condition.target {
        CaptureTarget::Window { title, region } => println!(
            "[Perception] Window title: {title}\n[Perception] Relative region: x={} y={} width={} height={}",
            region.x, region.y, region.width, region.height
        ),
        CaptureTarget::Screen { region, .. } => println!(
            "[Perception] Screen region: x={} y={} width={} height={}",
            region.x, region.y, region.width, region.height
        ),
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn record_condition(_condition_path: &Path, _record_target: RecordTarget<'_>) -> Result<(), String> {
    Err("Perception recording currently requires Windows".into())
}

#[cfg(target_os = "windows")]
fn window_bounds(window: &xcap::Window) -> Result<WindowBounds, String> {
    let title = window.title().map_err(|error| error.to_string())?;
    if title.trim().is_empty() {
        return Err("Window has no title".into());
    }
    if window.is_minimized().map_err(|error| error.to_string())? {
        return Err(format!("Window is minimized: {title}"));
    }
    Ok(WindowBounds {
        title,
        x: window.x().map_err(|error| error.to_string())?,
        y: window.y().map_err(|error| error.to_string())?,
        width: window.width().map_err(|error| error.to_string())?,
        height: window.height().map_err(|error| error.to_string())?,
    })
}

#[cfg(target_os = "windows")]
pub fn watch_condition(condition_path: &Path) -> Result<(), String> {
    let condition = load_condition(condition_path)?;
    validate_screen_target(&condition.target)?;

    let condition_directory = condition_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let template_path = condition_directory.join(&condition.template);
    let template = xcap::image::open(&template_path)
        .map_err(|error| {
            format!(
                "Unable to load template {}: {error}",
                template_path.display()
            )
        })?
        .into_rgba8();
    let region = condition.target.region();
    if template.width() != region.width || template.height() != region.height {
        return Err(format!(
            "Template is {}x{}, but the configured region is {}x{}",
            template.width(),
            template.height(),
            region.width,
            region.height
        ));
    }

    println!(
        "[Perception] Watching {} with target {:?} every {} ms; threshold={:.4}; press Ctrl+C to stop",
        condition_path.display(),
        condition.target,
        condition.poll_interval_ms,
        condition.threshold
    );
    let mut sample = 0u64;
    loop {
        sample += 1;
        match capture_target(&condition.target) {
            Ok(Some(frame)) => {
                let score = similarity(template.as_raw(), frame.as_raw());
                let state = if score >= condition.threshold {
                    "MATCHED"
                } else {
                    "NOT_MATCHED"
                };
                println!(
                    "[Perception] sample={sample} state={state} score={score:.4} threshold={:.4}",
                    condition.threshold
                );
            }
            Ok(None) => println!(
                "[Perception] sample={sample} state=NOT_MATCHED window is not focused"
            ),
            Err(error) => {
                eprintln!("[Perception] sample={sample} state=UNKNOWN capture failed: {error}")
            }
        }
        thread::sleep(Duration::from_millis(condition.poll_interval_ms));
    }
}

#[cfg(not(target_os = "windows"))]
pub fn watch_condition(_condition_path: &Path) -> Result<(), String> {
    Err("Perception matching currently requires Windows".into())
}

fn load_condition(path: &Path) -> Result<ConditionConfig, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("Unable to read condition {}: {error}", path.display()))?;
    let condition: ConditionConfig = serde_json::from_str(&source)
        .map_err(|error| format!("Unable to parse condition {}: {error}", path.display()))?;
    if condition.version != CONDITION_VERSION {
        return Err(format!(
            "Unsupported perception condition version {}; expected {}",
            condition.version, CONDITION_VERSION
        ));
    }
    if !(0.0..=1.0).contains(&condition.threshold) || !condition.threshold.is_finite() {
        return Err("Perception condition threshold must be between 0 and 1".into());
    }
    if condition.poll_interval_ms == 0 {
        return Err("Perception condition pollIntervalMs must be greater than 0".into());
    }
    match &condition.target {
        CaptureTarget::Screen { monitor, region } => {
            if monitor.name.trim().is_empty() {
                return Err("Perception screen monitor name cannot be empty".into());
            }
            validate_region(*region, monitor.width, monitor.height)?;
        }
        CaptureTarget::Window { title, region } => {
            if title.trim().is_empty() {
                return Err("Perception window title cannot be empty".into());
            }
            validate_region(*region, u32::MAX, u32::MAX)?;
        }
    }
    if condition.template.as_os_str().is_empty() || condition.template.is_absolute() {
        return Err("Perception condition template must be a relative path".into());
    }
    Ok(condition)
}

fn validate_region(region: Region, monitor_width: u32, monitor_height: u32) -> Result<(), String> {
    if region.width == 0 || region.height == 0 {
        return Err("Perception region width and height must be greater than 0".into());
    }
    let right = region
        .x
        .checked_add(region.width)
        .ok_or("Perception region horizontal bounds overflow")?;
    let bottom = region
        .y
        .checked_add(region.height)
        .ok_or("Perception region vertical bounds overflow")?;
    if right > monitor_width || bottom > monitor_height {
        return Err(format!(
            "Perception region ({}, {}, {}, {}) exceeds target size {}x{}",
            region.x, region.y, region.width, region.height, monitor_width, monitor_height
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn validate_screen_target(target: &CaptureTarget) -> Result<(), String> {
    if let CaptureTarget::Screen { monitor, region } = target {
        screen_monitor(monitor, *region)?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn screen_monitor(config: &MonitorConfig, region: Region) -> Result<xcap::Monitor, String> {
    let monitor = monitor_by_name(&config.name)?;
    let width = monitor.width().map_err(|error| error.to_string())?;
    let height = monitor.height().map_err(|error| error.to_string())?;
    if width != config.width || height != config.height {
        return Err(format!(
            "Configured monitor size is {}x{}, but it is currently {}x{}",
            config.width, config.height, width, height
        ));
    }
    validate_region(region, width, height)?;
    Ok(monitor)
}

#[cfg(target_os = "windows")]
fn capture_target(target: &CaptureTarget) -> Result<Option<xcap::image::RgbaImage>, String> {
    match target {
        CaptureTarget::Screen { monitor, region } => {
            let monitor = screen_monitor(monitor, *region)?;
            monitor
                .capture_region(region.x, region.y, region.width, region.height)
                .map(Some)
                .map_err(|error| error.to_string())
        }
        CaptureTarget::Window { title, region } => {
            let window = window_by_title(title)?;
            if !window.is_focused().map_err(|error| error.to_string())? {
                return Ok(None);
            }
            if window.is_minimized().map_err(|error| error.to_string())? {
                return Err(format!("Window is minimized: {title}"));
            }
            let width = window.width().map_err(|error| error.to_string())?;
            let height = window.height().map_err(|error| error.to_string())?;
            validate_region(*region, width, height)?;

            let x = i64::from(window.x().map_err(|error| error.to_string())?)
                + i64::from(region.x);
            let y = i64::from(window.y().map_err(|error| error.to_string())?)
                + i64::from(region.y);
            let monitors = xcap::Monitor::all()
                .map_err(|error| format!("Unable to enumerate monitors: {error}"))?;
            for monitor in monitors {
                let monitor_x = i64::from(monitor.x().map_err(|error| error.to_string())?);
                let monitor_y = i64::from(monitor.y().map_err(|error| error.to_string())?);
                let monitor_width = i64::from(monitor.width().map_err(|error| error.to_string())?);
                let monitor_height = i64::from(monitor.height().map_err(|error| error.to_string())?);
                let local_x = x - monitor_x;
                let local_y = y - monitor_y;
                if local_x >= 0
                    && local_y >= 0
                    && local_x + i64::from(region.width) <= monitor_width
                    && local_y + i64::from(region.height) <= monitor_height
                {
                    let frame = monitor
                        .capture_region(
                            local_x as u32,
                            local_y as u32,
                            region.width,
                            region.height,
                        )
                        .map_err(|error| error.to_string())?;
                    return if window.is_focused().map_err(|error| error.to_string())? {
                        Ok(Some(frame))
                    } else {
                        Ok(None)
                    };
                }
            }
            Err(format!("Window region is not fully visible on one monitor: {title}"))
        }
    }
}

#[cfg(target_os = "windows")]
fn window_is_focused(title: &str) -> bool {
    window_by_title(title)
        .and_then(|window| window.is_focused().map_err(|error| error.to_string()))
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn window_by_title(title: &str) -> Result<xcap::Window, String> {
    let windows = xcap::Window::all()
        .map_err(|error| format!("Unable to enumerate windows: {error}"))?;
    let mut matches = windows
        .into_iter()
        .filter(|window| window.title().is_ok_and(|candidate| candidate == title));
    let window = matches.next().ok_or_else(|| format!("Window not found: {title}"))?;
    if matches.next().is_some() {
        return Err(format!("Multiple windows have the same title: {title}"));
    }
    Ok(window)
}

fn crop_rgba(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    region: Region,
) -> Result<Vec<u8>, String> {
    const CHANNELS: usize = 4;
    let source_stride = source_width as usize * CHANNELS;
    let expected_source_len = source_stride
        .checked_mul(source_height as usize)
        .ok_or("Frozen screen dimensions exceed the supported size")?;
    if source.len() != expected_source_len {
        return Err(format!(
            "Frozen screen contains {} bytes; expected {expected_source_len}",
            source.len()
        ));
    }

    let row_bytes = region.width as usize * CHANNELS;
    let mut cropped = Vec::with_capacity(
        row_bytes
            .checked_mul(region.height as usize)
            .ok_or("Selected region dimensions exceed the supported size")?,
    );
    let left = region.x as usize * CHANNELS;
    for y in region.y as usize..(region.y + region.height) as usize {
        let row_start = y * source_stride + left;
        cropped.extend_from_slice(&source[row_start..row_start + row_bytes]);
    }
    Ok(cropped)
}

fn similarity(template: &[u8], frame: &[u8]) -> f64 {
    if template.len() != frame.len() || template.is_empty() {
        return 0.0;
    }
    let mut difference = 0u64;
    let mut channel_count = 0u64;
    for (template_pixel, frame_pixel) in template.chunks_exact(4).zip(frame.chunks_exact(4)) {
        for channel in 0..3 {
            difference += template_pixel[channel].abs_diff(frame_pixel[channel]) as u64;
            channel_count += 1;
        }
    }
    1.0 - difference as f64 / (channel_count * u8::MAX as u64) as f64
}

#[cfg(target_os = "windows")]
fn primary_monitor() -> Result<xcap::Monitor, String> {
    xcap::Monitor::all()
        .map_err(|error| format!("Unable to enumerate monitors: {error}"))?
        .into_iter()
        .find(|monitor| monitor.is_primary().unwrap_or(false))
        .ok_or_else(|| "Unable to find the primary monitor".to_string())
}

#[cfg(target_os = "windows")]
fn monitor_by_name(name: &str) -> Result<xcap::Monitor, String> {
    xcap::Monitor::all()
        .map_err(|error| format!("Unable to enumerate monitors: {error}"))?
        .into_iter()
        .find(|monitor| monitor.name().is_ok_and(|candidate| candidate == name))
        .ok_or_else(|| format!("Configured monitor is not available: {name}"))
}
