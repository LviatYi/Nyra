mod selector;

pub(super) use crate::reaction::sdk_types::Region;
use crate::reaction::sdk_types::{
    CaptureTarget, ImageCondition as ConditionConfig, MonitorConfig, PositionUnit, WindowRegion,
};
use std::{fs, path::Path};

const DEFAULT_THRESHOLD: f64 = 0.95;
const DEFAULT_POLL_INTERVAL_MS: u64 = 500;

impl WindowRegion {
    fn validate(&self) -> Result<(), String> {
        for value in [self.x, self.y] {
            let valid = value.is_finite()
                && value >= 0.0
                && match self.unit {
                    PositionUnit::Pixels => value <= u32::MAX as f64 && value.fract() == 0.0,
                    PositionUnit::Ratio => value <= 1.0,
                };
            if !valid {
                return Err("Window region x and y must be non-negative integer pixels or ratios between 0 and 1".into());
            }
        }
        validate_region(
            Region {
                x: 0,
                y: 0,
                width: self.width,
                height: self.height,
            },
            u32::MAX,
            u32::MAX,
        )?;
        if matches!(self.unit, PositionUnit::Pixels) {
            self.resolve(u32::MAX, u32::MAX)?;
        }
        Ok(())
    }

    fn resolve(&self, window_width: u32, window_height: u32) -> Result<Region, String> {
        let (x, y) = match self.unit {
            PositionUnit::Pixels => (self.x as u32, self.y as u32),
            PositionUnit::Ratio => (
                (self.x * f64::from(window_width)).round() as u32,
                (self.y * f64::from(window_height)).round() as u32,
            ),
        };
        let region = Region {
            x,
            y,
            width: self.width,
            height: self.height,
        };
        validate_region(region, window_width, window_height)?;
        Ok(region)
    }
}

impl CaptureTarget {
    fn template_size(&self) -> (u32, u32) {
        match self {
            Self::Screen { region, .. } => (region.width, region.height),
            Self::Window { region, .. } => (region.width, region.height),
        }
    }
}

pub enum RecordTarget<'a> {
    AutoWindow,
    Window(&'a str),
    Screen,
}

/// A prepared condition reusable by synchronous Reaction steps.
pub(crate) struct ImageCondition {
    #[cfg(target_os = "windows")]
    config: ConditionConfig,
    #[cfg(target_os = "windows")]
    template: Vec<u8>,
}

impl ImageCondition {
    #[cfg(target_os = "windows")]
    pub(crate) fn prepare(config: ConditionConfig, base_dir: &Path) -> Result<Self, String> {
        validate_condition(&config)?;
        validate_screen_target(&config.target)?;
        let template_path = base_dir.join(&config.template);
        let template = xcap::image::open(&template_path)
            .map_err(|error| {
                format!(
                    "Unable to load template {}: {error}",
                    template_path.display()
                )
            })?
            .into_rgba8();
        let (width, height) = config.target.template_size();
        if template.width() != width || template.height() != height {
            return Err(format!(
                "Template size does not match the region: {}",
                template_path.display()
            ));
        }
        Ok(Self {
            config,
            template: template.into_raw(),
        })
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn prepare(_config: ConditionConfig, _base_dir: &Path) -> Result<Self, String> {
        Err("Perception matching currently requires Windows".into())
    }

    /// None means the target is unavailable, never a successful match.
    #[cfg(target_os = "windows")]
    pub(crate) fn matches(&self) -> Result<Option<bool>, String> {
        capture_target(&self.config.target).map(|frame| {
            frame.map(|frame| similarity(&self.template, frame.as_raw()) >= self.config.threshold)
        })
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn matches(&self) -> Result<Option<bool>, String> {
        Err("Perception matching currently requires Windows".into())
    }
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
pub fn record_condition(
    template_path: &Path,
    record_target: RecordTarget<'_>,
) -> Result<(), String> {
    if template_path
        .extension()
        .is_none_or(|extension| extension != "png")
    {
        return Err(format!(
            "Perception template path must use the .png extension: {}",
            template_path.display()
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
            validate_window_selector(title)?;
            vec![window_bounds(&window_by_selector(title)?)?]
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
            if windows
                .iter()
                .filter(|candidate| candidate.title == window.title)
                .count()
                > 1
            {
                return Err(format!(
                    "Multiple windows have the same title: {}",
                    window.title
                ));
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
            title: match record_target {
                RecordTarget::Window(title) => title.to_string(),
                _ => window.title.clone(),
            },
            region: WindowRegion {
                x: f64::from(relative_region.x),
                y: f64::from(relative_region.y),
                width: relative_region.width,
                height: relative_region.height,
                unit: PositionUnit::Pixels,
            },
        }
    } else {
        CaptureTarget::Screen {
            monitor: MonitorConfig {
                name: Some(monitor_name),
                width: monitor_width,
                height: monitor_height,
            },
            region,
        }
    };

    let template_directory = template_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(template_directory).map_err(|error| {
        format!(
            "Unable to create template directory {}: {error}",
            template_directory.display()
        )
    })?;
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
    fs::write(template_path, encoded_template).map_err(|error| {
        format!(
            "Unable to save template {}: {error}",
            template_path.display()
        )
    })?;
    let target_source = match &target {
        CaptureTarget::Window { title, region } => format!(
            "{{mode: \"window\", title: {title:?}, region: {{x: {}, y: {}, width: {}, height: {}}}}}",
            region.x, region.y, region.width, region.height
        ),
        CaptureTarget::Screen { monitor, region } => format!(
            "{{mode: \"screen\", monitor: {{name: {}, width: {}, height: {}}}, region: {{x: {}, y: {}, width: {}, height: {}}}}}",
            monitor
                .name
                .as_ref()
                .map(|name| format!("{name:?}"))
                .unwrap_or_else(|| "null".into()),
            monitor.width,
            monitor.height,
            region.x,
            region.y,
            region.width,
            region.height
        ),
    };
    let template = template_path.to_string_lossy();
    println!(
        "[Perception] Image: {}\nCopy this condition into your script's before option (template path is relative to the main configuration directory):\n{{\n    kind: \"image\",\n    target: {target_source},\n    template: {template:?},\n    threshold: {DEFAULT_THRESHOLD},\n    pollIntervalMs: {DEFAULT_POLL_INTERVAL_MS},\n}}",
        template_path.display()
    );
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn record_condition(
    _template_path: &Path,
    _record_target: RecordTarget<'_>,
) -> Result<(), String> {
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

fn validate_condition(condition: &ConditionConfig) -> Result<(), String> {
    if !(0.0..=1.0).contains(&condition.threshold) || !condition.threshold.is_finite() {
        return Err("Perception condition threshold must be between 0 and 1".into());
    }
    if condition.timeout_ms == 0
        || condition.poll_interval_ms == 0
        || condition.poll_interval_ms > condition.timeout_ms
    {
        return Err("Image condition requires 0 < pollIntervalMs <= timeoutMs".into());
    }
    match &condition.target {
        CaptureTarget::Screen { monitor, region } => {
            if monitor
                .name
                .as_ref()
                .is_some_and(|name| name.trim().is_empty())
            {
                return Err("Perception screen monitor name cannot be empty".into());
            }
            validate_region(*region, monitor.width, monitor.height)?;
        }
        CaptureTarget::Window { title, region } => {
            validate_window_selector(title)?;
            region.validate()?;
        }
    }
    if condition.template.trim().is_empty() {
        return Err("Image condition template cannot be empty".into());
    }
    Ok(())
}

fn validate_window_selector(title: &str) -> Result<(), String> {
    if title.trim().is_empty() {
        return Err("Perception window title cannot be empty".into());
    }
    Ok(())
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
    let monitor = match config.name.as_deref() {
        Some(name) => monitor_by_name(name)?,
        None => primary_monitor()?,
    };
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
            let window = window_by_selector(title)?;
            if !window.is_focused().map_err(|error| error.to_string())? {
                return Ok(None);
            }
            if window.is_minimized().map_err(|error| error.to_string())? {
                return Err("Target window is minimized".into());
            }
            let width = window.width().map_err(|error| error.to_string())?;
            let height = window.height().map_err(|error| error.to_string())?;
            let region = region.resolve(width, height)?;

            let x = i64::from(window.x().map_err(|error| error.to_string())?) + i64::from(region.x);
            let y = i64::from(window.y().map_err(|error| error.to_string())?) + i64::from(region.y);
            let monitors = xcap::Monitor::all()
                .map_err(|error| format!("Unable to enumerate monitors: {error}"))?;
            for monitor in monitors {
                let monitor_x = i64::from(monitor.x().map_err(|error| error.to_string())?);
                let monitor_y = i64::from(monitor.y().map_err(|error| error.to_string())?);
                let monitor_width = i64::from(monitor.width().map_err(|error| error.to_string())?);
                let monitor_height =
                    i64::from(monitor.height().map_err(|error| error.to_string())?);
                let local_x = x - monitor_x;
                let local_y = y - monitor_y;
                if local_x >= 0
                    && local_y >= 0
                    && local_x + i64::from(region.width) <= monitor_width
                    && local_y + i64::from(region.height) <= monitor_height
                {
                    let frame = monitor
                        .capture_region(local_x as u32, local_y as u32, region.width, region.height)
                        .map_err(|error| error.to_string())?;
                    return if window.is_focused().map_err(|error| error.to_string())? {
                        Ok(Some(frame))
                    } else {
                        Ok(None)
                    };
                }
            }
            Err("Window region is not fully visible on one monitor".into())
        }
    }
}

#[cfg(target_os = "windows")]
fn window_by_selector(title: &str) -> Result<xcap::Window, String> {
    let windows =
        xcap::Window::all().map_err(|error| format!("Unable to enumerate windows: {error}"))?;
    let mut process_matches = windows.iter().filter(|window| {
        window_process_name(window).is_ok_and(|candidate| candidate.eq_ignore_ascii_case(title))
    });
    if let Some(window) = process_matches.next() {
        if process_matches.next().is_some() {
            return Err(format!(
                "Multiple windows match executable filename: {title}; use a unique window title"
            ));
        }
        return Ok(window.clone());
    }
    let mut matches = windows
        .into_iter()
        .filter(|window| window.title().is_ok_and(|candidate| candidate == title));
    let window = matches
        .next()
        .ok_or_else(|| format!("No executable filename or window title matches: {title}"))?;
    if matches.next().is_some() {
        return Err(format!("Multiple windows have the same title: {title}"));
    }
    Ok(window)
}

#[cfg(target_os = "windows")]
fn window_process_name(window: &xcap::Window) -> Result<String, String> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
        },
    };

    let pid = window.pid().map_err(|error| error.to_string())?;
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return Err(format!(
            "Unable to query process {pid}: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut path = vec![0u16; 32_768];
    let mut length = path.len() as u32;
    let result = unsafe { QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length) };
    let error = if result == 0 {
        Some(std::io::Error::last_os_error())
    } else {
        None
    };
    unsafe { CloseHandle(process) };
    if let Some(error) = error {
        return Err(format!(
            "Unable to read executable path for process {pid}: {error}"
        ));
    }
    let path = String::from_utf16_lossy(&path[..length as usize]);
    Path::new(&path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| format!("Process {pid} has no executable filename"))
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
