use crate::capture::tools::same_monitor;
use crate::job::entity::{ScreenPoint, ScreenRect};
use std::error::Error;
use std::ops::Mul;
use xcap::Monitor;

pub trait ImageCapture {
    fn capture(&self) -> Result<image::DynamicImage, Box<dyn Error>>;
}

impl ImageCapture for ScreenRect {
    fn capture(&self) -> Result<image::DynamicImage, Box<dyn Error>> {
        let ScreenRect {
            lt: ScreenPoint { x: lt_x, y: lt_y },
            rb: ScreenPoint { x: rb_x, y: rb_y },
        } = *self;
        {
            let top_left_monitor = Monitor::from_point(lt_x, lt_y)?;
            let bottom_right_monitor = Monitor::from_point(rb_x - 1, rb_y - 1)?;

            if !same_monitor(&top_left_monitor, &bottom_right_monitor)? {
                return Err(
                    "The selected region crosses multiple monitors, which is not supported."
                        .into(),
                );
            }

            let width = self.width();
            let height = self.height();

            if width.mul(height).eq(&0) {
                return Err("Region height and width must be positive.".into());
            }

            let relative_x = u32::try_from(lt_x - top_left_monitor.x()?)?;
            let relative_y = u32::try_from(lt_y - top_left_monitor.y()?)?;

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
    }
}