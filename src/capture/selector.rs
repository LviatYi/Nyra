use crate::capture::tools::same_monitor;
use image::GenericImage;
use serde::{Deserialize, Serialize};
use std::error::Error;
use xcap::Monitor;

pub trait ImageCapture {
    fn capture(&self) -> Result<image::DynamicImage, Box<dyn Error>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureSelector {
    FullScreen,
    Rect { x1: i32, y1: i32, x2: i32, y2: i32 },
}

impl CaptureSelector {
    pub fn from_rect(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        CaptureSelector::Rect { x1, y1, x2, y2 }
    }
}

impl ImageCapture for CaptureSelector {
    fn capture(&self) -> Result<image::DynamicImage, Box<dyn Error>> {
        match self {
            CaptureSelector::Rect { x1, y1, x2, y2 } => {
                let left = *x1.min(x2);
                let top = *y1.min(y2);
                let right = *x1.max(x2);
                let bottom = *y1.max(y2);

                let width = u32::try_from(right - left)
                    .map_err(|_| "Region width must be positive.")?
                    .max(1);
                let height = u32::try_from(bottom - top)
                    .map_err(|_| "Region height must be positive.")?
                    .max(1);

                let top_left_monitor = Monitor::from_point(left, top)?;
                let bottom_right_monitor = Monitor::from_point(right - 1, bottom - 1)?;

                if !same_monitor(&top_left_monitor, &bottom_right_monitor)? {
                    return Err(
                        "The selected region crosses multiple monitors, which is not supported."
                            .into(),
                    );
                }

                let relative_x = u32::try_from(left - top_left_monitor.x()?)?;
                let relative_y = u32::try_from(top - top_left_monitor.y()?)?;

                let max_width = top_left_monitor.width()?.saturating_sub(relative_x);
                let max_height = top_left_monitor.height()?.saturating_sub(relative_y);
                if width > max_width || height > max_height {
                    return Err("The selected region exceeds the monitor bounds.".into());
                }

                let capture =
                    top_left_monitor.capture_region(relative_x, relative_y, width, height)?;
                let result = image::DynamicImage::ImageRgba8(capture);

                Ok(result)
            }
            CaptureSelector::FullScreen => Monitor::all()?
                .into_iter()
                .map(|monitor| monitor.capture_image())
                .try_fold(None, |acc: Option<image::DynamicImage>, capture_result| {
                    let capture = capture_result?;
                    let image = image::DynamicImage::ImageRgba8(capture);
                    Ok::<Option<image::DynamicImage>, Box<dyn Error>>(Some(match acc {
                        Some(prev_image) => {
                            let prev_width = prev_image.width();
                            let prev_height = prev_image.height();
                            let new_width = image.width();
                            let new_height = image.height();
                            let mut combined_image = image::DynamicImage::new_rgba8(
                                prev_width + new_width,
                                prev_height.max(new_height),
                            );
                            combined_image.copy_from(&prev_image, 0, 0)?;
                            combined_image.copy_from(&image, prev_width, 0)?;
                            combined_image
                        }
                        None => image,
                    }))
                })?
                .ok_or("No monitors found to capture.".into()),
        }
    }
}
