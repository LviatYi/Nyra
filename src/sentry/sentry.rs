use crate::capture::selector::CaptureSelector;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentryTask {
    patrol: CaptureSelector,
    frequency_ms: u32,
    focus_on: FocusPoint,
    alarm_mode: AlarmMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusPoint {
    ContainsText(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlarmMode {
    PrintLog,
}

#[cfg(test)]
mod tests {
    use super::{AlarmMode, FocusPoint, SentryTask};
    use crate::capture::selector::CaptureSelector;

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
}
