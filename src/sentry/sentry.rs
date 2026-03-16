use crate::capture::selector::{CaptureSelector, ImageCapture};
use crate::perception::text_perceptor::TextPerceptor;
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentryTask {
    pub patrol: CaptureSelector,
    pub frequency_ms: u32,
    pub focus_on: FocusPoint,
    pub alarm_mode: AlarmMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusPoint {
    ContainsText(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlarmMode {
    PrintLog,
}

impl AlarmMode {
    fn emit(&self, sentry_run_output: &SentryRunOutput) {
        match self {
            AlarmMode::PrintLog => {
                tracing::info!(target = "sentry_alarm", result = %sentry_run_output);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentryRunOutput {
    pub matched: bool,

    pub recognized_text: String,
}

impl Display for SentryRunOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SentryRunOutput {{ matched: {}, recognized_text: '{}' }}",
            self.matched, self.recognized_text
        )
    }
}

impl SentryTask {
    pub fn new(
        patrol: CaptureSelector,
        frequency_ms: u32,
        focus_on: FocusPoint,
        alarm_mode: AlarmMode,
    ) -> Self {
        Self {
            patrol,
            frequency_ms,
            focus_on,
            alarm_mode,
        }
    }

    pub fn evaluate<P>(&self, text_perceptor: &P) -> Result<SentryRunOutput, Box<dyn Error>>
    where
        P: TextPerceptor,
    {
        let image = self.patrol.capture()?;

        save_debug_image(&image)?;

        self.run_with_image(text_perceptor, &image)
    }

    fn run_with_image<P>(
        &self,
        text_perceptor: &P,
        image: &DynamicImage,
    ) -> Result<SentryRunOutput, Box<dyn Error>>
    where
        P: TextPerceptor,
    {
        let recognized_text = text_perceptor.recognize(image)?;
        let matched = matches_focus(&recognized_text, &self.focus_on);

        let output = SentryRunOutput {
            recognized_text: recognized_text.clone(),
            matched,
        };

        if matched {
            self.alarm_mode.emit(&output);
        }

        Ok(output)
    }
}

fn matches_focus(recognized: &str, focus_on: &FocusPoint) -> bool {
    match focus_on {
        FocusPoint::ContainsText(expected) => recognized.contains(expected),
    }
}

fn save_debug_image(image: &DynamicImage) -> Result<(), Box<dyn Error>> {
    #[cfg(debug_assertions)]
    {
        let path = debug_image_path();
        image.save(&path)?;
        tracing::info!(target = "capture_debug_image", path = %path.display(), "saved captured region");
    }

    Ok(())
}

fn debug_image_path() -> PathBuf {
    env::temp_dir().join("nyra-captured-region.png")
}

#[cfg(test)]
mod tests {
    use super::{AlarmMode, FocusPoint, SentryRunOutput, SentryTask};
    use crate::capture::selector::CaptureSelector;
    use crate::perception::text_perceptor::TextPerceptor;
    use image::DynamicImage;
    use std::error::Error;

    fn create_test_sentry() -> SentryTask {
        SentryTask {
            patrol: CaptureSelector::Rect {
                x1: 10,
                y1: 20,
                x2: 30,
                y2: 40,
            },
            frequency_ms: 500,
            focus_on: FocusPoint::ContainsText("alert".to_string()),
            alarm_mode: AlarmMode::PrintLog,
        }
    }

    #[test]
    fn serializes_sentry_task_to_json() {
        let task = create_test_sentry();

        let actual = serde_json::to_string(&task).unwrap();
        let expected = r#"{"patrol":{"Rect":{"x1":10,"y1":20,"x2":30,"y2":40}},"frequency_ms":500,"focus_on":{"ContainsText":"alert"},"alarm_mode":"PrintLog"}"#;

        assert_eq!(actual, expected);
    }

    #[test]
    fn deserializes_sentry_task_from_json() {
        let json = r#"{"patrol":{"Rect":{"x1":10,"y1":20,"x2":30,"y2":40}},"frequency_ms":500,"focus_on":{"ContainsText":"alert"},"alarm_mode":"PrintLog"}"#;

        let actual: SentryTask = serde_json::from_str(json).unwrap();
        let expected = create_test_sentry();

        assert_eq!(actual, expected);
    }

    struct FakeTextPerceptor {
        recognized_text: String,
    }

    impl TextPerceptor for FakeTextPerceptor {
        fn recognize(&self, _grey_image: &DynamicImage) -> Result<String, Box<dyn Error>> {
            Ok(self.recognized_text.clone())
        }
    }

    #[test]
    fn run_matches_focus_text() {
        let task = create_test_sentry();
        let perceptor = FakeTextPerceptor {
            recognized_text: "system alert triggered".to_string(),
        };
        let image = DynamicImage::new_luma8(1, 1);

        let actual = task.run_with_image(&perceptor, &image).unwrap();

        assert_eq!(
            actual,
            SentryRunOutput {
                recognized_text: "system alert triggered".to_string(),
                matched: true,
            }
        );
    }

    #[test]
    fn run_reports_unmatched_text() {
        let task = create_test_sentry();
        let perceptor = FakeTextPerceptor {
            recognized_text: "system healthy".to_string(),
        };
        let image = DynamicImage::new_luma8(1, 1);

        let actual = task.run_with_image(&perceptor, &image).unwrap();

        assert_eq!(
            actual,
            SentryRunOutput {
                recognized_text: "system healthy".to_string(),
                matched: false,
            }
        );
    }
}
