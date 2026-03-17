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
    use crate::measure::measure;
    use crate::perception::tesseract::TesseractPerceptor;
    use crate::sentry::sentry::SentryTask;
    use std::env;
    use std::error::Error;
    use std::fs;
    use std::path::Path;
    use tracing::level_filters::LevelFilter;

    pub fn bootstrap() -> Result<(), Box<dyn Error>> {
        tracing_subscriber::fmt()
            .with_ansi(true)
            .with_max_level(LevelFilter::INFO)
            .with_target(true)
            .try_init()
            .map_err(|error| format!("failed to initialize tracing: {error}"))?;

        crate::probe::clean_debug_image()?;

        Ok(())
    }

    pub async fn run() -> Result<(), Box<dyn Error>> {
        let task = parse_task(env::args().skip(1))?;
        let text_perceptor = measure("init_tesseract_perceptor", || {
            TesseractPerceptor::new_with_init()
        });
        task.run(&text_perceptor).await
    }

    fn parse_task<I>(args: I) -> Result<SentryTask, Box<dyn Error>>
    where
        I: Iterator<Item = String>,
    {
        let usage = concat!(
            "Usage: nyra '<SentryTask JSON>' | nyra <path-to-task.json>\n",
            "Example: nyra ",
            r#""{"patrol":{"Rect":{"x1":100,"y1":200,"x2":600,"y2":300}},"frequency_ms":500,"focus_on":{"ContainsText":"Hello"},"alarm_mode":"PrintLog"}""#,
        );

        let input = args.collect::<Vec<_>>().join(" ");
        if input.trim().is_empty() {
            return Err(usage.into());
        }

        let task_json = load_task_input(&input)?;

        serde_json::from_str::<SentryTask>(&task_json)
            .map_err(|error| format!("Invalid SentryTask JSON: {error}\n{usage}").into())
    }

    fn load_task_input(input: &str) -> Result<String, Box<dyn Error>> {
        let trimmed = input.trim();

        if looks_like_json(trimmed) {
            return Ok(trimmed.to_string());
        }

        let path = Path::new(trimmed);
        if path.is_file() {
            return fs::read_to_string(path)
                .map_err(|error| format!("Failed to read task file `{trimmed}`: {error}").into());
        }

        Ok(trimmed.to_string())
    }

    fn looks_like_json(input: &str) -> bool {
        matches!(input.chars().next(), Some('{') | Some('[') | Some('"'))
    }

    #[cfg(test)]
    mod tests {
        use super::parse_task;
        use crate::capture::selector::CaptureSelector;
        use crate::sentry::sentry::{AlarmMode, FocusPoint, SentryTask};
        use std::env;
        use std::fs;

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
                    Some(500),
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
                    .contains("Usage: nyra '<SentryTask JSON>' | nyra <path-to-task.json>")
            );
        }

        #[test]
        fn parses_sentry_task_from_json_file() {
            let path = env::temp_dir().join("nyra-parse-task-test.json");
            fs::write(
                &path,
                r#"{"patrol":{"Rect":{"x1":250,"y1":0,"x2":350,"y2":50}},"frequency_ms":500,"focus_on":{"ContainsText":"nyra"},"alarm_mode":"PrintLog"}"#,
            )
                .unwrap();

            let task = parse_task([path.display().to_string()].into_iter()).unwrap();

            assert_eq!(
                task,
                SentryTask::new(
                    CaptureSelector::Rect {
                        x1: 250,
                        y1: 0,
                        x2: 350,
                        y2: 50,
                    },
                    Some(500),
                    FocusPoint::ContainsText("nyra".to_string()),
                    AlarmMode::PrintLog,
                ),
            );

            fs::remove_file(path).unwrap();
        }

        #[test]
        fn defaults_missing_frequency_in_json_input() {
            let task = parse_task(
                [r#"{"patrol":{"Rect":{"x1":250,"y1":0,"x2":350,"y2":50}},"focus_on":{"ContainsText":"nyra"},"alarm_mode":"PrintLog"}"#]
                    .into_iter()
                    .map(str::to_string),
            )
                .unwrap();

            assert_eq!(
                task,
                SentryTask::new(
                    CaptureSelector::Rect {
                        x1: 250,
                        y1: 0,
                        x2: 350,
                        y2: 50,
                    },
                    Some(1000),
                    FocusPoint::ContainsText("nyra".to_string()),
                    AlarmMode::PrintLog,
                ),
            );
        }
    }
}

pub(crate) mod probe {
    use image::DynamicImage;
    use std::env;
    use std::error::Error;
    use std::path::PathBuf;

    fn debug_image_trace_path() -> PathBuf {
        env::temp_dir().join("nyra")
    }

    pub(super) fn save_debug_image(
        image: &DynamicImage,
        id: impl AsRef<str>,
    ) -> Result<(), Box<dyn Error>> {
        #[cfg(debug_assertions)]
        {
            let temp_dir = debug_image_trace_path();
            if !std::fs::exists(temp_dir.as_path())? {
                std::fs::create_dir(temp_dir.as_path())?;
            }

            let path = debug_image_trace_path().join(format!("nyra_debug_{}.png", id.as_ref()));
            image.save(&path)?;
            tracing::info!(target = "probe", path = %path.display(), "saved captured region");
        }

        Ok(())
    }

    pub(super) fn clean_debug_image() -> Result<(), Box<dyn Error>> {
        #[cfg(debug_assertions)]
        {
            let path = debug_image_trace_path();
            if path.is_dir() {
                std::fs::remove_dir_all(path.as_path())?;
            }

            tracing::info!(target = "probe", path = %path.display(), "cleaned debug image directory");
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
#[tokio::main(flavor = "multi_thread")]
async fn main() {
    if let Err(error) = app::bootstrap() {
        eprintln!("{error}");
        std::process::exit(2);
    }

    if let Err(error) = app::run().await {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
