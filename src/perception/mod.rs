mod selector;

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

const CONDITION_VERSION: u32 = 1;
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
    monitor: MonitorConfig,
    region: Region,
    template: PathBuf,
    threshold: f64,
    poll_interval_ms: u64,
}

#[cfg(target_os = "windows")]
pub fn record_condition(condition_path: &Path) -> Result<(), String> {
    if condition_path.extension().is_none_or(|extension| extension != "json") {
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
        monitor: MonitorConfig {
            name: monitor_name,
            width: monitor_width,
            height: monitor_height,
        },
        region,
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
    fs::write(&template_path, encoded_template)
        .map_err(|error| format!("Unable to save template {}: {error}", template_path.display()))?;
    fs::write(condition_path, json).map_err(|error| {
        format!(
            "Unable to save condition {}: {error}",
            condition_path.display()
        )
    })?;

    println!(
        "[Perception] Recorded region x={} y={} width={} height={}\n[Perception] Condition: {}\n[Perception] Template: {}",
        region.x,
        region.y,
        region.width,
        region.height,
        condition_path.display(),
        template_path.display()
    );
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn record_condition(_condition_path: &Path) -> Result<(), String> {
    Err("Perception recording currently requires Windows".into())
}

#[cfg(target_os = "windows")]
pub fn watch_condition(condition_path: &Path) -> Result<(), String> {
    let condition = load_condition(condition_path)?;
    let monitor = monitor_by_name(&condition.monitor.name)?;
    let monitor_width = monitor
        .width()
        .map_err(|error| format!("Unable to read configured monitor width: {error}"))?;
    let monitor_height = monitor
        .height()
        .map_err(|error| format!("Unable to read configured monitor height: {error}"))?;
    if monitor_width != condition.monitor.width || monitor_height != condition.monitor.height {
        return Err(format!(
            "Configured monitor size is {}x{}, but it is currently {}x{}; record the condition again",
            condition.monitor.width, condition.monitor.height, monitor_width, monitor_height
        ));
    }
    validate_region(condition.region, monitor_width, monitor_height)?;

    let condition_directory = condition_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let template_path = condition_directory.join(&condition.template);
    let template = xcap::image::open(&template_path)
        .map_err(|error| format!("Unable to load template {}: {error}", template_path.display()))?
        .into_rgba8();
    if template.width() != condition.region.width || template.height() != condition.region.height {
        return Err(format!(
            "Template is {}x{}, but the configured region is {}x{}",
            template.width(),
            template.height(),
            condition.region.width,
            condition.region.height
        ));
    }

    println!(
        "[Perception] Watching {} on monitor {} every {} ms; threshold={:.4}; press Ctrl+C to stop",
        condition_path.display(),
        condition.monitor.name,
        condition.poll_interval_ms,
        condition.threshold
    );
    let mut sample = 0u64;
    loop {
        sample += 1;
        match monitor.capture_region(
            condition.region.x,
            condition.region.y,
            condition.region.width,
            condition.region.height,
        ) {
            Ok(frame) => {
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
            Err(error) => eprintln!(
                "[Perception] sample={sample} state=UNKNOWN capture failed: {error}"
            ),
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
            "Perception region ({}, {}, {}, {}) exceeds monitor size {}x{}",
            region.x, region.y, region.width, region.height, monitor_width, monitor_height
        ));
    }
    Ok(())
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
