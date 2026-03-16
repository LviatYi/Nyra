use crate::capture::selector::{CaptureSelector, ImageCapture};
use crate::perception::text_perceptor::TextPerceptor;
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::{self, MissedTickBehavior};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentryTask {
    pub patrol: CaptureSelector,
    #[serde(
        default = "default_frequency_ms",
        skip_serializing_if = "is_default_frequency_ms"
    )]
    pub frequency_ms: Option<i64>,
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
                tracing::info!(target = "sentry_alarm", result = %sentry_run_output, "SUCCESS");
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
        frequency_ms: Option<i64>,
        focus_on: FocusPoint,
        alarm_mode: AlarmMode,
    ) -> Self {
        Self {
            patrol,
            frequency_ms: frequency_ms.or_else(default_frequency_ms),
            focus_on,
            alarm_mode,
        }
    }

    pub async fn run<P>(&self, text_perceptor: &P) -> Result<(), Box<dyn Error>>
    where
        P: TextPerceptor,
    {
        if let Some(duration) = self.interval_duration()? {
            let mut interval = time::interval(duration);
            interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

            loop {
                interval.tick().await;
                let output = self.evaluate(text_perceptor)?;
                tracing::trace!(target = "sentry_run", result = %output);
            }
        } else {
            let output = self.evaluate(text_perceptor)?;
            tracing::trace!(target = "sentry_run", result = %output);
            Ok(())
        }
    }

    fn evaluate<P>(&self, text_perceptor: &P) -> Result<SentryRunOutput, Box<dyn Error>>
    where
        P: TextPerceptor,
    {
        let image = self.patrol.capture()?;

        save_debug_image(&image)?;

        self.evaluate_by_image(text_perceptor, &image)
    }

    fn evaluate_by_image<P>(
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

    fn interval_duration(&self) -> Result<Option<Duration>, Box<dyn Error>> {
        let Some(frequency_ms) = self.frequency_ms else {
            return Ok(Some(Duration::from_millis(1000)));
        };

        if frequency_ms <= 0 {
            return Ok(None);
        }

        Ok(Some(Duration::from_millis(frequency_ms as u64)))
    }
}

fn default_frequency_ms() -> Option<i64> {
    Some(1000)
}

fn is_default_frequency_ms(value: &Option<i64>) -> bool {
    *value == default_frequency_ms()
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
    use std::time::Duration;

    fn create_test_sentry() -> SentryTask {
        SentryTask {
            patrol: CaptureSelector::Rect {
                x1: 10,
                y1: 20,
                x2: 30,
                y2: 40,
            },
            frequency_ms: Some(500),
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

        let actual = task.evaluate_by_image(&perceptor, &image).unwrap();

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

        let actual = task.evaluate_by_image(&perceptor, &image).unwrap();

        assert_eq!(
            actual,
            SentryRunOutput {
                recognized_text: "system healthy".to_string(),
                matched: false,
            }
        );
    }

    #[test]
    fn converts_frequency_to_interval_duration() {
        let task = create_test_sentry();

        let duration = task.interval_duration().unwrap();

        assert_eq!(duration, Some(Duration::from_millis(500)));
    }

    #[test]
    fn rejects_zero_frequency() {
        let task = SentryTask::new(
            CaptureSelector::Rect {
                x1: 10,
                y1: 20,
                x2: 30,
                y2: 40,
            },
            Some(0),
            FocusPoint::ContainsText("alert".to_string()),
            AlarmMode::PrintLog,
        );

        let duration = task.interval_duration().unwrap();

        assert_eq!(duration, None);
    }

    #[test]
    fn defaults_missing_frequency_to_one_second_interval() {
        let task = SentryTask::new(
            CaptureSelector::Rect {
                x1: 10,
                y1: 20,
                x2: 30,
                y2: 40,
            },
            None,
            FocusPoint::ContainsText("alert".to_string()),
            AlarmMode::PrintLog,
        );

        let duration = task.interval_duration().unwrap();

        assert_eq!(duration, Some(Duration::from_millis(1000)));
    }

    #[test]
    fn treats_negative_frequency_as_single_run() {
        let task = SentryTask::new(
            CaptureSelector::Rect {
                x1: 10,
                y1: 20,
                x2: 30,
                y2: 40,
            },
            Some(-1),
            FocusPoint::ContainsText("alert".to_string()),
            AlarmMode::PrintLog,
        );

        let duration = task.interval_duration().unwrap();

        assert_eq!(duration, None);
    }

    #[test]
    fn omits_default_frequency_from_json() {
        let task = SentryTask::new(
            CaptureSelector::Rect {
                x1: 10,
                y1: 20,
                x2: 30,
                y2: 40,
            },
            None,
            FocusPoint::ContainsText("alert".to_string()),
            AlarmMode::PrintLog,
        );

        let actual = serde_json::to_string(&task).unwrap();

        assert_eq!(
            actual,
            r#"{"patrol":{"Rect":{"x1":10,"y1":20,"x2":30,"y2":40}},"focus_on":{"ContainsText":"alert"},"alarm_mode":"PrintLog"}"#
        );
    }
}
