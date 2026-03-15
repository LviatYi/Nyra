pub mod apperceiver;
pub mod measure;
mod selector;

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("This CLI currently supports Windows only.");
    std::process::exit(1);
}

#[cfg(target_os = "windows")]
mod app {
    use crate::apperceiver::tesseract::TesseractApperceiver;
    use crate::apperceiver::text_apperceiver::TextApperceiver;
    use crate::measure::measure;
    use image::imageops::FilterType;
    use std::env;
    use std::error::Error;
    use tracing::level_filters::LevelFilter;
    use xcap::Monitor;

    pub fn bootstrap() -> Result<(), Box<dyn Error>> {
        tracing_subscriber::fmt()
            .with_max_level(LevelFilter::INFO)
            .with_target(false)
            .try_init()
            .map_err(|error| format!("failed to initialize tracing: {error}"))?;

        Ok(())
    }

    pub fn run() -> Result<i32, Box<dyn Error>> {
        let args = CliArgs::parse(env::args().skip(1))?;
        let image = capture_region(args.x1, args.y1, args.x2, args.y2)?;
        save_debug_image(&image)?;

        let text_apperceiver = measure("init_tesseract_apperceiver", || TesseractApperceiver::new_with_init());

        let recognized = measure("text_recognize", || text_apperceiver.recognize(&image))?;

        let matched = recognized.contains(&args.text);

        if matched {
            println!("success");
            Ok(0)
        } else {
            println!("failed");
            Ok(1)
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    struct CliArgs {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        text: String,
    }

    impl CliArgs {
        fn parse<I>(mut args: I) -> Result<Self, Box<dyn Error>>
        where
            I: Iterator<Item=String>,
        {
            let usage =
                "Usage: nyra <x1> <y1> <x2> <y2> <text>\nExample: nyra 100 200 600 300 Hello";

            let x1 = parse_i32(args.next(), "x1", usage)?;
            let y1 = parse_i32(args.next(), "y1", usage)?;
            let x2 = parse_i32(args.next(), "x2", usage)?;
            let y2 = parse_i32(args.next(), "y2", usage)?;
            let text = args.collect::<Vec<_>>().join(" ");

            if text.is_empty() {
                return Err(usage.into());
            }

            Ok(Self {
                x1,
                y1,
                x2,
                y2,
                text,
            })
        }
    }

    fn parse_i32(value: Option<String>, name: &str, usage: &str) -> Result<i32, Box<dyn Error>> {
        let value = value.ok_or_else(|| format!("Missing argument `{name}`.\n{usage}"))?;
        value
            .parse::<i32>()
            .map_err(|_| format!("Invalid integer for `{name}`: {value}\n{usage}").into())
    }

    fn capture_region(
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
    ) -> Result<image::GrayImage, Box<dyn Error>> {
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
        let grayscale = image::DynamicImage::ImageRgba8(capture).to_luma8();

        Ok(image::imageops::resize(
            &grayscale,
            grayscale.width().saturating_mul(2),
            grayscale.height().saturating_mul(2),
            FilterType::CatmullRom,
        ))
    }

    fn save_debug_image(image: &image::GrayImage) -> Result<(), Box<dyn Error>> {
        let path = env::temp_dir().join("nyra-captured-region.png");
        image.save(&path)?;
        tracing::info!(target = "capture_debug_image", path = %path.display(), "saved captured region");
        Ok(())
    }

    fn same_monitor(left: &Monitor, right: &Monitor) -> Result<bool, Box<dyn Error>> {
        Ok(left.x()? == right.x()?
            && left.y()? == right.y()?
            && left.width()? == right.width()?
            && left.height()? == right.height()?)
    }

    #[cfg(test)]
    mod tests {
        use super::CliArgs;

        #[test]
        fn parses_text_with_spaces() {
            let args = CliArgs::parse(
                ["10", "20", "30", "40", "hello", "world"]
                    .into_iter()
                    .map(str::to_string),
            )
                .unwrap();

            assert_eq!(
                args,
                CliArgs {
                    x1: 10,
                    y1: 20,
                    x2: 30,
                    y2: 40,
                    text: "hello world".to_string(),
                }
            );
        }
    }
}

#[cfg(target_os = "windows")]
fn main() {
    match app::bootstrap().and_then(|_| app::run()) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}
