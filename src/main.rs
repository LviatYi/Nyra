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
    use crate::capture::selector::ImageCapture;
    use crate::measure::measure;
    use crate::perception::tesseract::TesseractPerceptor;
    use crate::perception::text_perceptor::TextPerceptor;
    use crate::sentry::sentry::{AlarmMode, FocusPoint, SentryTask};
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
        let task = parse_task(env::args().skip(1))?;
        let image = task.patrol.capture()?;
        save_debug_image(&image)?;

        let text_perceptor = measure("init_tesseract_perceptor", || {
            TesseractPerceptor::new_with_init()
        });

        let recognized = measure("text_recognize", || text_perceptor.recognize(&image))?;
        let matched = matches_focus(&recognized, &task.focus_on);

        if matched {
            emit_alarm(&task.alarm_mode, &recognized);
            println!("success");
            Ok(0)
        } else {
            println!("failed. original text: {}", recognized);
            Ok(1)
        }
    }

    fn parse_task<I>(args: I) -> Result<SentryTask, Box<dyn Error>>
    where
        I: Iterator<Item = String>,
    {
        let usage = concat!(
            "Usage: nyra '<SentryTask JSON>'\n",
            "Example: nyra ",
            r#""{"patrol":{"Rect":{"x1":100,"y1":200,"x2":600,"y2":300}},"frequency_ms":500,"focus_on":{"ContainsText":"Hello"},"alarm_mode":"PrintLog"}""#,
        );

        let input = args.collect::<Vec<_>>().join(" ");
        if input.trim().is_empty() {
            return Err(usage.into());
        }

        serde_json::from_str::<SentryTask>(&input)
            .map_err(|error| format!("Invalid SentryTask JSON: {error}\n{usage}").into())
    }

    fn matches_focus(recognized: &str, focus_on: &FocusPoint) -> bool {
        match focus_on {
            FocusPoint::ContainsText(expected) => recognized.contains(expected),
        }
    }

    fn emit_alarm(alarm_mode: &AlarmMode, recognized: &str) {
        match alarm_mode {
            AlarmMode::PrintLog => println!("matched text: {}", recognized),
        }
    }

    fn save_debug_image(image: &image::DynamicImage) -> Result<(), Box<dyn Error>> {
        let path = env::temp_dir().join("nyra-captured-region.png");
        image.save(&path)?;
        tracing::info!(target = "capture_debug_image", path = %path.display(), "saved captured region");
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::{matches_focus, parse_task};
        use crate::capture::selector::CaptureSelector;
        use crate::sentry::sentry::{AlarmMode, FocusPoint, SentryTask};

        #[test]
        fn parses_sentry_task_from_json_input() {
            let task = parse_task(
                [r#"{"patrol":{"Rect":{"x1":10,"y1":20,"x2":30,"y2":40}},"frequency_ms":500,"focus_on":{"ContainsText":"hello world"},"alarm_mode":"PrintLog"}"#]
                    .into_iter()
                    .map(str::to_string),
            )
            .unwrap();

            assert_eq!(
                task,
                SentryTask::new(
                    CaptureSelector::Rect {
                        x1: 10,
                        y1: 20,
                        x2: 30,
                        y2: 40,
                    },
                    500,
                    FocusPoint::ContainsText("hello world".to_string()),
                    AlarmMode::PrintLog,
                ),
            );
        }

        #[test]
        fn rejects_empty_task_input() {
            let error = parse_task(std::iter::empty::<String>()).unwrap_err();

            assert!(
                error
                    .to_string()
                    .contains("Usage: nyra '<SentryTask JSON>'")
            );
        }

        #[test]
        fn matches_contains_text_focus() {
            assert!(matches_focus(
                "system alert triggered",
                &FocusPoint::ContainsText("alert".to_string()),
            ));
            assert!(!matches_focus(
                "system healthy",
                &FocusPoint::ContainsText("alert".to_string()),
            ));
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
