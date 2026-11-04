use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use opencv::{
    core::{self, Mat, Point, Rect, Scalar, Size},
    imgproc,
    prelude::*,
};

use super::impression::{ImpressionStore, ScaleImpression};
use super::{ResolvedSearch, candidate_region, types::ImageAnchor};

const SEARCH_BUDGET: Duration = Duration::from_millis(250);
const SCALE_STEP: f64 = 1.12;
const SHAPE_THRESHOLD: f64 = 0.25;
const CANDIDATES_PER_SCALE: usize = 8;
const CACHE_BYTES: usize = 64 * 1024 * 1024;

struct Variant {
    rgb: Mat,
    edges: Mat,
}

struct SearchState {
    frame_size: Size,
    sizes: Vec<Size>,
    variants: Vec<Option<Variant>>,
    cache_bytes: usize,
    cursor: usize,
}

pub(super) struct ScaledSearch {
    original: Mat,
    state: Mutex<SearchState>,
}

fn cv_error(error: opencv::Error) -> String {
    format!("OpenCV scaled image matching failed: {error}")
}

fn edges(rgb: &Mat) -> Result<Mat, String> {
    let mut gray = Mat::default();
    imgproc::cvt_color(
        rgb,
        &mut gray,
        imgproc::COLOR_RGB2GRAY,
        0,
        core::AlgorithmHint::ALGO_HINT_DEFAULT,
    )
    .map_err(cv_error)?;
    let mut result = Mat::default();
    imgproc::canny(&gray, &mut result, 50.0, 100.0, 3, false).map_err(cv_error)?;
    Ok(result)
}

fn resize(original: &Mat, size: Size) -> Result<Mat, String> {
    let mut result = Mat::default();
    let interpolation = if size.width < original.cols() || size.height < original.rows() {
        imgproc::INTER_AREA
    } else {
        imgproc::INTER_LINEAR
    };
    imgproc::resize(original, &mut result, size, 0.0, 0.0, interpolation).map_err(cv_error)?;
    Ok(result)
}

impl ScaledSearch {
    pub(super) fn prepare(original: &Mat) -> Result<Self, String> {
        let template_edges = edges(original)?;
        if core::count_non_zero(&template_edges).map_err(cv_error)? < 4 {
            return Err(
                "Full-screen scaled matching needs a template with identifiable edges".into(),
            );
        }
        Ok(Self {
            original: original.try_clone().map_err(cv_error)?,
            state: Mutex::new(SearchState {
                frame_size: Size::new(0, 0),
                sizes: Vec::new(),
                variants: Vec::new(),
                cache_bytes: 0,
                cursor: 0,
            }),
        })
    }

    pub(super) fn matches(
        &self,
        frame: &Mat,
        threshold: f64,
        key: &str,
        impressions: &ImpressionStore,
        started: Instant,
        search: ResolvedSearch,
        anchor: ImageAnchor,
    ) -> Result<Option<bool>, String> {
        let frame_edges = edges(frame)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Scaled search state is unavailable")?;
        let frame_size = Size::new(frame.cols(), frame.rows());
        if state.frame_size != frame_size {
            state.frame_size = frame_size;
            state.sizes = scale_sizes(self.original.size().map_err(cv_error)?, frame_size);
            state.variants = (0..state.sizes.len()).map(|_| None).collect();
            state.cache_bytes = 0;
            state.cursor = 0;
        }
        if state.sizes.is_empty() {
            return Ok(None);
        }

        // A remembered match is still checked against the current frame.
        if let Some(previous) = impressions.scale(key) {
            let size = Size::new(previous.width, previous.height);
            if size.width > 0
                && size.height > 0
                && size.width <= frame.cols()
                && size.height <= frame.rows()
            {
                let template = resize(&self.original, size)?;
                let template_edges = edges(&template)?;
                if core::count_non_zero(&template_edges).map_err(cv_error)? >= 4 {
                    if let Some(found) = verify_candidate(
                        frame,
                        &frame_edges,
                        &template,
                        &template_edges,
                        Point::new(previous.x, previous.y),
                        threshold,
                        search,
                        anchor,
                    )? {
                        impressions.observe_scale(key, found);
                        return Ok(Some(true));
                    }
                }
            }
        }

        let count = state.sizes.len();
        let mut processed = 0;
        while processed < count {
            // Always attempt at least one scale so expensive captures cannot starve the search.
            if processed > 0 && started.elapsed() >= SEARCH_BUDGET {
                break;
            }
            let index = state.cursor;
            state.cursor = (state.cursor + 1) % count;
            processed += 1;
            let size = state.sizes[index];
            if candidate_region(frame_size, size, search, anchor).is_none() {
                continue;
            }
            if state.variants[index].is_none() {
                let rgb = resize(&self.original, size)?;
                let edge_template = edges(&rgb)?;
                let bytes = (size.width as usize)
                    .saturating_mul(size.height as usize)
                    .saturating_mul(4);
                let variant = Variant {
                    rgb,
                    edges: edge_template,
                };
                if bytes <= CACHE_BYTES.saturating_sub(state.cache_bytes) {
                    state.cache_bytes += bytes;
                    state.variants[index] = Some(variant);
                } else if self.find_at_scale(
                    frame,
                    &frame_edges,
                    &variant,
                    threshold,
                    key,
                    impressions,
                    search,
                    anchor,
                )? {
                    return Ok(Some(true));
                } else {
                    continue;
                }
            }
            if self.find_at_scale(
                frame,
                &frame_edges,
                state.variants[index].as_ref().unwrap(),
                threshold,
                key,
                impressions,
                search,
                anchor,
            )? {
                return Ok(Some(true));
            }
        }
        // Discrete scales, limited candidates and per-frame time budget make a negative inconclusive.
        Ok(None)
    }

    fn find_at_scale(
        &self,
        frame: &Mat,
        frame_edges: &Mat,
        variant: &Variant,
        threshold: f64,
        key: &str,
        impressions: &ImpressionStore,
        search: ResolvedSearch,
        anchor: ImageAnchor,
    ) -> Result<bool, String> {
        if core::count_non_zero(&variant.edges).map_err(cv_error)? < 4 {
            return Ok(false);
        }
        let Some(area) = candidate_region(
            frame.size().map_err(cv_error)?,
            variant.rgb.size().map_err(cv_error)?,
            search,
            anchor,
        ) else {
            return Ok(false);
        };
        let edge_roi = Mat::roi(frame_edges, area).map_err(cv_error)?;
        let mut scores = Mat::default();
        imgproc::match_template(
            &edge_roi,
            &variant.edges,
            &mut scores,
            imgproc::TM_CCORR_NORMED,
            &core::no_array(),
        )
        .map_err(cv_error)?;
        // Finish a scale once started: otherwise an expensive frame can repeatedly
        // discard the same unverified candidates and refinement sizes.
        for _ in 0..CANDIDATES_PER_SCALE {
            let mut score = 0.0;
            let mut location = Point::default();
            core::min_max_loc(
                &scores,
                None,
                Some(&mut score),
                None,
                Some(&mut location),
                &core::no_array(),
            )
            .map_err(cv_error)?;
            if !score.is_finite() || score < SHAPE_THRESHOLD {
                break;
            }
            if let Some(found) = self.refine(
                frame,
                frame_edges,
                variant,
                Point::new(area.x + location.x, area.y + location.y),
                threshold,
                search,
                anchor,
            )? {
                impressions.observe_scale(key, found);
                return Ok(true);
            }
            // Suppress only this local neighbourhood; retain other full-screen candidates.
            let radius_x = (variant.rgb.cols() / 4).max(1);
            let radius_y = (variant.rgb.rows() / 4).max(1);
            let left = (location.x - radius_x).max(0);
            let top = (location.y - radius_y).max(0);
            let right = (location.x + radius_x + 1).min(scores.cols());
            let bottom = (location.y + radius_y + 1).min(scores.rows());
            imgproc::rectangle(
                &mut scores,
                Rect::new(left, top, right - left, bottom - top),
                Scalar::all(0.0),
                imgproc::FILLED,
                imgproc::LINE_8,
                0,
            )
            .map_err(cv_error)?;
        }
        Ok(false)
    }

    fn refine(
        &self,
        frame: &Mat,
        frame_edges: &Mat,
        variant: &Variant,
        location: Point,
        threshold: f64,
        search: ResolvedSearch,
        anchor: ImageAnchor,
    ) -> Result<Option<ScaleImpression>, String> {
        let refinement_anchor = if matches!(search, ResolvedSearch::Position { .. }) {
            anchor
        } else {
            ImageAnchor { x: 0.5, y: 0.5 }
        };
        let anchor_x = f64::from(location.x) + refinement_anchor.x * f64::from(variant.rgb.cols());
        let anchor_y = f64::from(location.y) + refinement_anchor.y * f64::from(variant.rgb.rows());
        for (index, factor) in [1.0, 0.97, 1.03, 0.945, 1.055].into_iter().enumerate() {
            let size = Size::new(
                (f64::from(variant.rgb.cols()) * factor).round().max(1.0) as i32,
                (f64::from(variant.rgb.rows()) * factor).round().max(1.0) as i32,
            );
            if size.width > frame.cols() || size.height > frame.rows() {
                continue;
            }
            let rgb = if index == 0 {
                variant.rgb.try_clone().map_err(cv_error)?
            } else {
                resize(&self.original, size)?
            };
            let edge_template = if index == 0 {
                variant.edges.try_clone().map_err(cv_error)?
            } else {
                edges(&rgb)?
            };
            if core::count_non_zero(&edge_template).map_err(cv_error)? < 4 {
                continue;
            }
            let position = Point::new(
                (anchor_x - refinement_anchor.x * f64::from(size.width)).round() as i32,
                (anchor_y - refinement_anchor.y * f64::from(size.height)).round() as i32,
            );
            if let Some(found) = verify_candidate(
                frame,
                frame_edges,
                &rgb,
                &edge_template,
                position,
                threshold,
                search,
                anchor,
            )? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }
}

fn verify_candidate(
    frame: &Mat,
    frame_edges: &Mat,
    template: &Mat,
    template_edges: &Mat,
    position: Point,
    threshold: f64,
    search: ResolvedSearch,
    anchor: ImageAnchor,
) -> Result<Option<ScaleImpression>, String> {
    let width = template.cols();
    let height = template.rows();
    let Some(allowed) = candidate_region(
        frame.size().map_err(cv_error)?,
        template.size().map_err(cv_error)?,
        search,
        anchor,
    ) else {
        return Ok(None);
    };
    let left = position.x.saturating_sub(2).max(allowed.x);
    let top = position.y.saturating_sub(2).max(allowed.y);
    let right = position
        .x
        .saturating_add(width)
        .saturating_add(2)
        .min(allowed.x + allowed.width);
    let bottom = position
        .y
        .saturating_add(height)
        .saturating_add(2)
        .min(allowed.y + allowed.height);
    if right - left < width || bottom - top < height {
        return Ok(None);
    }
    let area = Rect::new(left, top, right - left, bottom - top);
    let roi = Mat::roi(frame, area).map_err(cv_error)?;
    let mut scores = Mat::default();
    imgproc::match_template(
        &roi,
        template,
        &mut scores,
        imgproc::TM_SQDIFF,
        &core::no_array(),
    )
    .map_err(cv_error)?;
    let mut difference = 0.0;
    let mut location = Point::default();
    core::min_max_loc(
        &scores,
        Some(&mut difference),
        None,
        Some(&mut location),
        None,
        &core::no_array(),
    )
    .map_err(cv_error)?;
    let denominator = f64::from(width) * f64::from(height) * 3.0 * 255.0 * 255.0;
    if !difference.is_finite() || 1.0 - difference.max(0.0) / denominator < threshold {
        return Ok(None);
    }
    let x = left + location.x;
    let y = top + location.y;
    let edge_roi = Mat::roi(frame_edges, Rect::new(x, y, width, height)).map_err(cv_error)?;
    let mut shape = Mat::default();
    imgproc::match_template(
        &edge_roi,
        template_edges,
        &mut shape,
        imgproc::TM_CCORR_NORMED,
        &core::no_array(),
    )
    .map_err(cv_error)?;
    let shape_score = *shape.at_2d::<f32>(0, 0).map_err(cv_error)?;
    if !shape_score.is_finite() || f64::from(shape_score) < SHAPE_THRESHOLD {
        return Ok(None);
    }
    Ok(Some(ScaleImpression {
        x,
        y,
        width,
        height,
    }))
}

fn scale_sizes(template: Size, frame: Size) -> Vec<Size> {
    let max_scale = (f64::from(frame.width) / f64::from(template.width))
        .min(f64::from(frame.height) / f64::from(template.height));
    let min_scale = (1.0 / f64::from(template.width)).max(1.0 / f64::from(template.height));
    if max_scale < min_scale {
        return Vec::new();
    }
    let mut values = vec![max_scale, min_scale];
    for power in -150..=150 {
        let scale = SCALE_STEP.powi(power);
        if scale >= min_scale && scale <= max_scale {
            values.push(scale);
        }
    }
    values.sort_by(|a, b| a.ln().abs().total_cmp(&b.ln().abs()));
    let mut sizes = Vec::new();
    for scale in values {
        let size = Size::new(
            (f64::from(template.width) * scale).round().max(1.0) as i32,
            (f64::from(template.height) * scale).round().max(1.0) as i32,
        );
        if size.width <= frame.width && size.height <= frame.height && !sizes.contains(&size) {
            sizes.push(size);
        }
    }
    sizes
}
