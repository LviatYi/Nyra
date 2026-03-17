use crate::perception::tesseract::TesseractPerceptorPool;
use crate::sentry::sentry::SentryTask;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Arc;
use tokio::task::JoinSet;

pub struct Nyra {
    sentries: Vec<SentryTask>,
    text_perceptors: Arc<TesseractPerceptorPool>,
}

impl Default for Nyra {
    fn default() -> Self {
        Self {
            sentries: Vec::new(),
            text_perceptors: Arc::new(TesseractPerceptorPool::new()),
        }
    }
}

impl Nyra {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_sentries_from_json(
        &mut self,
        defs: impl AsRef<str>,
    ) -> Result<(), Box<dyn Error>> {
        let mut defs = serde_json::from_str::<NyraJobs>(defs.as_ref())?;
        self.sentries.append(&mut defs.sentries);
        Ok(())
    }

    pub async fn deploy(&self) -> Result<(), Box<dyn Error>> {
        if self.sentries.is_empty() {
            return Err("No sentry tasks loaded.".into());
        }

        let mut tasks = JoinSet::new();
        for sentry in self.sentries.iter().cloned() {
            let pool = Arc::clone(&self.text_perceptors);
            tasks.spawn(async move {
                sentry
                    .run(pool.as_ref())
                    .await
                    .map_err(|error| format!("sentry `{}` failed: {error}", sentry.id))
            });
        }

        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tasks.abort_all();
                    return Err(error.into());
                }
                Err(error) => {
                    tasks.abort_all();
                    return Err(format!("failed to join sentry task: {error}").into());
                }
            }
        }

        Ok(())
    }

    pub fn sentries(&self) -> &[SentryTask] {
        &self.sentries
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NyraJobs {
    pub sentries: Vec<SentryTask>,
}

#[cfg(test)]
mod tests {
    use super::Nyra;
    use crate::capture::selector::CaptureSelector;
    use crate::sentry::sentry::{AlarmMode, FocusPoint, SentryTask};

    #[test]
    fn appends_sentries_loaded_from_json() {
        let mut nyra = Nyra::new();
        nyra.load_sentries_from_json(
            r#"{"sentries":[{"id":"a","patrol":{"Rect":{"x1":1,"y1":2,"x2":3,"y2":4}},"focus_on":{"ContainsText":"first"},"alarm_mode":"PrintLog"}]}"#,
        )
        .unwrap();
        nyra.load_sentries_from_json(
            r#"{"sentries":[{"id":"b","patrol":{"Rect":{"x1":5,"y1":6,"x2":7,"y2":8}},"focus_on":{"ContainsText":"second"},"alarm_mode":"PrintLog"}]}"#,
        )
        .unwrap();

        assert_eq!(
            nyra.sentries(),
            &[
                SentryTask::new_with_custom_id(
                    "a",
                    CaptureSelector::Rect {
                        x1: 1,
                        y1: 2,
                        x2: 3,
                        y2: 4,
                    },
                    None,
                    FocusPoint::ContainsText("first".to_string()),
                    AlarmMode::PrintLog,
                ),
                SentryTask::new_with_custom_id(
                    "b",
                    CaptureSelector::Rect {
                        x1: 5,
                        y1: 6,
                        x2: 7,
                        y2: 8,
                    },
                    None,
                    FocusPoint::ContainsText("second".to_string()),
                    AlarmMode::PrintLog,
                ),
            ]
        );
    }
}
