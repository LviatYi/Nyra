mod impression;
#[cfg(target_os = "windows")]
mod scaled;
mod selector;
pub(crate) mod types;
pub(crate) use impression::{ImpressionPlugin, ImpressionStore};

use std::{fs, path::Path};
pub(super) use types::Region;
use types::{
    CaptureTarget, ImageAnchor, ImageCondition as ConditionConfig, ImagePosition, ImageSearch,
    MonitorConfig, PositionUnit,
};

const DEFAULT_THRESHOLD: f64 = 0.95;
const DEFAULT_POLL_INTERVAL_MS: u64 = 500;

impl ImagePosition {
    fn validate(&self) -> Result<(), String> {
        for value in [self.x, self.y] {
            let valid = value.is_finite()
                && value >= 0.0
                && match self.unit {
                    PositionUnit::Pixels => value <= u32::MAX as f64 && value.fract() == 0.0,
                    PositionUnit::Ratio => value <= 1.0,
                };
            if !valid {
                return Err(
                    "Image position must use non-negative integer pixels or ratios between 0 and 1"
                        .into(),
                );
            }
        }
        if self.tolerance > 1024 {
            return Err("Image position tolerance cannot exceed 1024 pixels".into());
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
enum ResolvedSearch {
    All,
    Region(Region),
    Position { x: f64, y: f64, tolerance: u32 },
}

#[cfg(target_os = "windows")]
fn resolve_search(
    search: &Option<ImageSearch>,
    width: u32,
    height: u32,
) -> Result<ResolvedSearch, String> {
    match search {
        None => Ok(ResolvedSearch::All),
        Some(ImageSearch::Region { region }) => {
            validate_region(*region, width, height)?;
            Ok(ResolvedSearch::Region(*region))
        }
        Some(ImageSearch::Position { position }) => {
            let (x, y) = match position.unit {
                PositionUnit::Pixels => (position.x, position.y),
                PositionUnit::Ratio => (
                    position.x * f64::from(width),
                    position.y * f64::from(height),
                ),
            };
            if x > f64::from(width) || y > f64::from(height) {
                return Err("Image anchor position is outside the capture target".into());
            }
            Ok(ResolvedSearch::Position {
                x,
                y,
                tolerance: position.tolerance,
            })
        }
    }
}

/// Rectangle containing every valid template placement for the selected search.
#[cfg(target_os = "windows")]
fn candidate_region(
    frame: opencv::core::Size,
    size: opencv::core::Size,
    search: ResolvedSearch,
    anchor: ImageAnchor,
) -> Option<opencv::core::Rect> {
    use opencv::core::Rect;
    if size.width <= 0 || size.height <= 0 || size.width > frame.width || size.height > frame.height
    {
        return None;
    }
    match search {
        ResolvedSearch::All => Some(Rect::new(0, 0, frame.width, frame.height)),
        ResolvedSearch::Region(region) => {
            if i64::from(region.width) < i64::from(size.width)
                || i64::from(region.height) < i64::from(size.height)
            {
                return None;
            }
            Some(Rect::new(
                region.x as i32,
                region.y as i32,
                region.width as i32,
                region.height as i32,
            ))
        }
        ResolvedSearch::Position { x, y, tolerance } => {
            let nominal_x = (x - anchor.x * f64::from(size.width)).round() as i64;
            let nominal_y = (y - anchor.y * f64::from(size.height)).round() as i64;
            let left = (nominal_x - i64::from(tolerance)).max(0);
            let top = (nominal_y - i64::from(tolerance)).max(0);
            let right = (nominal_x + i64::from(tolerance)).min(i64::from(frame.width - size.width));
            let bottom =
                (nominal_y + i64::from(tolerance)).min(i64::from(frame.height - size.height));
            if left > right || top > bottom {
                return None;
            }
            Some(Rect::new(
                left as i32,
                top as i32,
                (right - left + i64::from(size.width)) as i32,
                (bottom - top + i64::from(size.height)) as i32,
            ))
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
    template: opencv::core::Mat,
    #[cfg(target_os = "windows")]
    key: String,
    #[cfg(target_os = "windows")]
    impressions: ImpressionStore,
    #[cfg(target_os = "windows")]
    scaled: Option<scaled::ScaledSearch>,
}

impl ImageCondition {
    #[cfg(target_os = "windows")]
    pub(crate) fn prepare(
        config: ConditionConfig,
        base_dir: &Path,
        impressions: ImpressionStore,
    ) -> Result<Self, String> {
        validate_condition(&config)?;
        validate_screen_target(&config.target)?;
        let template_path = base_dir.join(&config.template);
        let image = xcap::image::open(&template_path)
            .map_err(|error| {
                format!(
                    "Unable to load template {}: {error}",
                    template_path.display()
                )
            })?
            .into_rgba8();
        let mut hash = blake3::Hasher::new();
        hash.update(if config.scale_invariant {
            b"nyra-scaled-search-v2"
        } else {
            b"nyra-fixed-search-v2"
        });
        hash.update(
            &serde_json::to_vec(&(&config.target, &config.search, &config.anchor))
                .map_err(|error| error.to_string())?,
        );
        hash.update(image.as_raw());
        let key = hash.finalize().to_hex().to_string();
        let template = rgba_to_rgb(image.as_raw(), image.height())?;
        let scaled = if config.scale_invariant {
            Some(scaled::ScaledSearch::prepare(&template)?)
        } else {
            None
        };
        Ok(Self {
            config,
            template,
            key,
            impressions,
            scaled,
        })
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn prepare(
        _config: ConditionConfig,
        _base_dir: &Path,
        _impressions: ImpressionStore,
    ) -> Result<Self, String> {
        Err("Perception matching currently requires Windows".into())
    }

    /// None means the target is unavailable or the search is inconclusive.
    #[cfg(target_os = "windows")]
    pub(crate) fn matches(&self) -> Result<Option<bool>, String> {
        use std::io::Write;

        let started = std::time::Instant::now();
        let result = self.matches_once();
        let status = match &result {
            Ok(Some(true)) => "matched",
            Ok(Some(false)) => "not_matched",
            Ok(None) => "inconclusive",
            Err(_) => "error",
        };
        let mut output = std::io::stdout().lock();
        if let Err(error) = &result {
            let _ = writeln!(
                output,
                "[Perception] template={:?} result={status} elapsedMs={} error={error}",
                self.config.template,
                started.elapsed().as_millis()
            );
        } else {
            let _ = writeln!(
                output,
                "[Perception] template={:?} result={status} elapsedMs={}",
                self.config.template,
                started.elapsed().as_millis()
            );
        }
        result
    }

    #[cfg(target_os = "windows")]
    fn matches_once(&self) -> Result<Option<bool>, String> {
        use impression::Offset;
        use opencv::{
            core::{self, Mat, Point, Rect},
            imgproc,
            prelude::*,
        };
        let started = std::time::Instant::now();
        let (image, search) = match capture_target(
            &self.config,
            self.template.size().map_err(|error| error.to_string())?,
        )? {
            Capture::Unavailable => return Ok(None),
            Capture::NoCandidates => return Ok(Some(false)),
            Capture::Frame(image, search) => (image, search),
        };
        let frame = rgba_to_rgb(image.as_raw(), image.height())?;
        if let Some(scaled) = &self.scaled {
            return scaled.matches(
                &frame,
                self.config.threshold.unwrap_or(0.995),
                &self.key,
                &self.impressions,
                started,
                search,
                self.config.anchor,
            );
        }
        let width = self.template.cols();
        let height = self.template.rows();
        let Some(area) = candidate_region(
            frame.size().map_err(|error| error.to_string())?,
            self.template.size().map_err(|error| error.to_string())?,
            search,
            self.config.anchor,
        ) else {
            return Ok(Some(false));
        };
        let denominator = f64::from(width) * f64::from(height) * 3.0 * 255.0 * 255.0;
        let threshold = self.config.threshold.unwrap_or(0.95);
        if let ResolvedSearch::Position { x, y, tolerance } = search {
            let origin_x = (x - self.config.anchor.x * f64::from(width)).round() as i64;
            let origin_y = (y - self.config.anchor.y * f64::from(height)).round() as i64;
            let mut offsets = self.impressions.offsets(&self.key);
            offsets.push(Offset { x: 0, y: 0 });
            for offset in offsets {
                if offset.x.unsigned_abs() > tolerance || offset.y.unsigned_abs() > tolerance {
                    continue;
                }
                let px = origin_x + i64::from(offset.x);
                let py = origin_y + i64::from(offset.y);
                if px < i64::from(area.x)
                    || py < i64::from(area.y)
                    || px + i64::from(width) > i64::from(area.x + area.width)
                    || py + i64::from(height) > i64::from(area.y + area.height)
                {
                    continue;
                }
                let roi = Mat::roi(&frame, Rect::new(px as i32, py as i32, width, height))
                    .map_err(|error| error.to_string())?;
                let mut result = Mat::default();
                imgproc::match_template(
                    &roi,
                    &self.template,
                    &mut result,
                    imgproc::TM_SQDIFF,
                    &core::no_array(),
                )
                .map_err(|error| error.to_string())?;
                let score = 1.0
                    - f64::from(
                        *result
                            .at_2d::<f32>(0, 0)
                            .map_err(|error| error.to_string())?,
                    )
                    .max(0.0)
                        / denominator;
                if score >= threshold {
                    self.impressions.observe(&self.key, offset);
                    return Ok(Some(true));
                }
            }
        }
        let roi = Mat::roi(&frame, area).map_err(|error| error.to_string())?;
        let mut result = Mat::default();
        imgproc::match_template(
            &roi,
            &self.template,
            &mut result,
            imgproc::TM_SQDIFF,
            &core::no_array(),
        )
        .map_err(|error| error.to_string())?;
        let mut min = 0.0;
        let mut location = Point::default();
        core::min_max_loc(
            &result,
            Some(&mut min),
            None,
            Some(&mut location),
            None,
            &core::no_array(),
        )
        .map_err(|error| error.to_string())?;
        if 1.0 - min.max(0.0) / denominator >= threshold {
            if let ResolvedSearch::Position { x, y, .. } = search {
                let origin_x = (x - self.config.anchor.x * f64::from(width)).round() as i32;
                let origin_y = (y - self.config.anchor.y * f64::from(height)).round() as i32;
                self.impressions.observe(
                    &self.key,
                    Offset {
                        x: area.x + location.x - origin_x,
                        y: area.y + location.y - origin_y,
                    },
                );
            }
            Ok(Some(true))
        } else {
            Ok(Some(false))
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn matches(&self) -> Result<Option<bool>, String> {
        Err("Perception matching currently requires Windows".into())
    }
}

#[cfg(target_os = "windows")]
fn rgba_to_rgb(bytes: &[u8], height: u32) -> Result<opencv::core::Mat, String> {
    use opencv::{
        core::{self, Mat},
        imgproc,
        prelude::*,
    };
    let flat = Mat::from_slice(bytes).map_err(|error| error.to_string())?;
    let rgba = flat
        .reshape(
            4,
            i32::try_from(height).map_err(|_| "Image height is too large")?,
        )
        .map_err(|error| error.to_string())?;
    let mut rgb = Mat::default();
    imgproc::cvt_color(
        &rgba,
        &mut rgb,
        imgproc::COLOR_RGBA2RGB,
        0,
        core::AlgorithmHint::ALGO_HINT_DEFAULT,
    )
    .map_err(|error| error.to_string())?;
    Ok(rgb)
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
    let (target_source, position_x, position_y) = if let Some(window) = window {
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
        let title = match record_target {
            RecordTarget::Window(title) => title,
            _ => &window.title,
        };
        (
            format!("{{mode: \"window\", title: {title:?}}}"),
            relative_region.x,
            relative_region.y,
        )
    } else {
        (
            format!("{{mode: \"fullScreen\", monitorName: {monitor_name:?}}}"),
            region.x,
            region.y,
        )
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
    let template = template_path.to_string_lossy();
    println!(
        "[Perception] Image: {}\nCopy this condition into your script's before option (template path is relative to the main configuration directory):\n{{\n    kind: \"image\",\n    target: {target_source},\n    search: {{kind: \"position\", position: {{x: {position_x}, y: {position_y}}}}},\n    template: {template:?},\n    threshold: {DEFAULT_THRESHOLD},\n    pollIntervalMs: {DEFAULT_POLL_INTERVAL_MS},\n}}",
        template_path.display()
    );
    println!(
        "[Perception] To search the primary screen at an unknown scale instead:\n{{kind: \"image\", template: {template:?}, scaleInvariant: true}}"
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
    if condition
        .threshold
        .is_some_and(|threshold| !(0.0..=1.0).contains(&threshold) || !threshold.is_finite())
    {
        return Err("Perception condition threshold must be between 0 and 1".into());
    }
    if !condition.anchor.x.is_finite()
        || !condition.anchor.y.is_finite()
        || !(0.0..=1.0).contains(&condition.anchor.x)
        || !(0.0..=1.0).contains(&condition.anchor.y)
    {
        return Err("Image anchor coordinates must be between 0 and 1".into());
    }
    if let Some(ImageSearch::Position { position }) = &condition.search {
        position.validate()?;
    }
    if let Some(ImageSearch::Region { region }) = &condition.search {
        validate_region(*region, u32::MAX, u32::MAX)?;
    }
    if condition.timeout_ms() == 0
        || condition.poll_interval_ms() == 0
        || condition.poll_interval_ms() > condition.timeout_ms()
    {
        return Err("Image condition requires 0 < pollIntervalMs <= timeoutMs".into());
    }
    match &condition.target {
        CaptureTarget::FullScreen { monitor_name } => {
            if monitor_name
                .as_ref()
                .is_some_and(|name| name.trim().is_empty())
            {
                return Err("Perception screen monitor name cannot be empty".into());
            }
        }
        CaptureTarget::Screen { monitor } => {
            if monitor
                .name
                .as_ref()
                .is_some_and(|name| name.trim().is_empty())
            {
                return Err("Perception screen monitor name cannot be empty".into());
            }
            if monitor.width == 0 || monitor.height == 0 {
                return Err("Monitor dimensions must be positive".into());
            }
        }
        CaptureTarget::Window { title } => validate_window_selector(title)?,
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
    if let CaptureTarget::Screen { monitor } = target {
        screen_monitor(monitor)?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn screen_monitor(config: &MonitorConfig) -> Result<xcap::Monitor, String> {
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
    Ok(monitor)
}

#[cfg(target_os = "windows")]
enum Capture {
    Unavailable,
    NoCandidates,
    Frame(xcap::image::RgbaImage, ResolvedSearch),
}

#[cfg(target_os = "windows")]
fn capture_target(
    config: &ConditionConfig,
    template_size: opencv::core::Size,
) -> Result<Capture, String> {
    use opencv::core::{Rect, Size};
    let (width, height, window, monitor) = match &config.target {
        CaptureTarget::FullScreen { monitor_name } => {
            let monitor = match monitor_name.as_deref() {
                Some(name) => monitor_by_name(name)?,
                None => primary_monitor()?,
            };
            let width = monitor.width().map_err(|error| error.to_string())?;
            let height = monitor.height().map_err(|error| error.to_string())?;
            (width, height, None, Some(monitor))
        }
        CaptureTarget::Screen { monitor } => {
            let selected = screen_monitor(monitor)?;
            (monitor.width, monitor.height, None, Some(selected))
        }
        CaptureTarget::Window { title } => {
            let window = window_by_selector(title)?;
            if !window.is_focused().map_err(|error| error.to_string())? {
                return Ok(Capture::Unavailable);
            }
            if window.is_minimized().map_err(|error| error.to_string())? {
                return Err("Target window is minimized".into());
            }
            let width = window.width().map_err(|error| error.to_string())?;
            let height = window.height().map_err(|error| error.to_string())?;
            (width, height, Some(window), None)
        }
    };
    let frame_size = Size::new(
        i32::try_from(width).map_err(|_| "Target width is too large")?,
        i32::try_from(height).map_err(|_| "Target height is too large")?,
    );
    let search = resolve_search(&config.search, width, height)?;
    let area = if !config.scale_invariant {
        match candidate_region(frame_size, template_size, search, config.anchor) {
            Some(area) => area,
            None => return Ok(Capture::NoCandidates),
        }
    } else if let ResolvedSearch::Region(region) = search {
        Rect::new(
            region.x as i32,
            region.y as i32,
            region.width as i32,
            region.height as i32,
        )
    } else {
        Rect::new(0, 0, frame_size.width, frame_size.height)
    };
    let local_search = match search {
        ResolvedSearch::Position { x, y, tolerance } => ResolvedSearch::Position {
            x: x - f64::from(area.x),
            y: y - f64::from(area.y),
            tolerance,
        },
        _ => ResolvedSearch::All,
    };
    if let Some(monitor) = monitor {
        let image = monitor
            .capture_region(
                area.x as u32,
                area.y as u32,
                area.width as u32,
                area.height as u32,
            )
            .map_err(|error| error.to_string())?;
        return Ok(Capture::Frame(image, local_search));
    }
    let window = window.unwrap();
    let wx = i64::from(window.x().map_err(|error| error.to_string())?);
    let wy = i64::from(window.y().map_err(|error| error.to_string())?);
    for monitor in xcap::Monitor::all().map_err(|error| error.to_string())? {
        let mx = i64::from(monitor.x().map_err(|error| error.to_string())?);
        let my = i64::from(monitor.y().map_err(|error| error.to_string())?);
        let mw = i64::from(monitor.width().map_err(|error| error.to_string())?);
        let mh = i64::from(monitor.height().map_err(|error| error.to_string())?);
        let x = wx + i64::from(area.x) - mx;
        let y = wy + i64::from(area.y) - my;
        if x >= 0 && y >= 0 && x + i64::from(area.width) <= mw && y + i64::from(area.height) <= mh {
            let image = monitor
                .capture_region(x as u32, y as u32, area.width as u32, area.height as u32)
                .map_err(|error| error.to_string())?;
            return if window.is_focused().map_err(|error| error.to_string())? {
                Ok(Capture::Frame(image, local_search))
            } else {
                Ok(Capture::Unavailable)
            };
        }
    }
    Err("Search region is not fully visible on one monitor".into())
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
