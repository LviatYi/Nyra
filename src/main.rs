mod capture;
pub mod measure;
pub mod perception;
pub mod sentry;

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("This CLI currently supports Windows only.");
    std::process::exit(1);
}

#[cfg(target_os = "windows")]
mod app {
    use crate::capture::selector::{CaptureSelector, ImageCapture};
    use crate::measure::measure;
    use crate::perception::tesseract::TesseractPerceptor;
    use crate::perception::text_perceptor::TextPerceptor;
    use std::env;
    use std::error::Error;
    use tracing::level_filters::LevelFilter;

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
        let image = CaptureSelector::from_rect(args.x1, args.y1, args.x2, args.y2).capture()?;
        save_debug_image(&image)?;

        let text_perceptor = measure("init_tesseract_perceptor", || {
            TesseractPerceptor::new_with_init()
        });

        let recognized = measure("text_recognize", || text_perceptor.recognize(&image))?;

        let matched = recognized.contains(&args.text);

        if matched {
            println!("success");
            Ok(0)
        } else {
            println!("failed. original text: {}", recognized);
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
            I: Iterator<Item = String>,
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

    fn save_debug_image(image: &image::DynamicImage) -> Result<(), Box<dyn Error>> {
        let path = env::temp_dir().join("nyra-captured-region.png");
        image.save(&path)?;
        tracing::info!(target = "capture_debug_image", path = %path.display(), "saved captured region");
        Ok(())
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
