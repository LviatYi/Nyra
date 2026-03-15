use std::error::Error;
use xcap::Monitor;
use crate::capture::tools::same_monitor;

pub trait ImageCapture {
    fn capture(&self) -> Result<image::DynamicImage, Box<dyn Error>>;
}

pub struct AreaSelector {
    r#type: SelectorType,
}

impl AreaSelector {
    pub fn from_rect(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        AreaSelector {
            r#type: SelectorType::Rect {
                x1,
                y1,
                x2,
                y2,
            }
        }
    }
}

impl ImageCapture for AreaSelector {
    fn capture(&self) -> Result<image::DynamicImage, Box<dyn Error>> {
        match self.r#type {
            SelectorType::Rect { x1, y1, x2, y2 } => {
                let left = x1.min(x2);
                let top = y1.min(y2);
                let right = x1.max(x2);
                let bottom = y1.max(y2);

                let width = u32::try_from(right - left)
                    .map_err(|_| "Region width must be positive.")?
                    .max(1);
                let height = u32::try_from(bottom - top)
                    .map_err(|_| "Region height must be positive.")?
                    .max(1);

                let top_left_monitor = Monitor::from_point(left, top)?;
                let bottom_right_monitor = Monitor::from_point(right - 1, bottom - 1)?;

                if !same_monitor(&top_left_monitor, &bottom_right_monitor)? {
                    return Err("The selected region crosses multiple monitors, which is not supported.".into());
                }

                let relative_x = u32::try_from(left - top_left_monitor.x()?)?;
                let relative_y = u32::try_from(top - top_left_monitor.y()?)?;

                let max_width = top_left_monitor.width()?.saturating_sub(relative_x);
                let max_height = top_left_monitor.height()?.saturating_sub(relative_y);
                if width > max_width || height > max_height {
                    return Err("The selected region exceeds the monitor bounds.".into());
                }

                let capture = top_left_monitor.capture_region(relative_x, relative_y, width, height)?;
                let result = image::DynamicImage::ImageRgba8(capture);

                Ok(result)
            }
        }
    }
}

pub enum SelectorType {
    Rect {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
    }
}