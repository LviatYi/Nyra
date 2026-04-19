use std::time::{Duration, Instant};

pub mod entity;

#[derive(Debug, Clone)]
pub enum OCRJobStatus {
    Pending,
    Running,
    Complete(String, Instant),
}

#[derive(Debug, Clone)]
pub struct OCRJob {
    pub id: String,
    pub rect: ((i32, i32), (i32, i32)),
    pub start_at: Instant,
    pub frequency: Duration,
    pub state: OCRJobStatus,
}

pub struct OCRJobBuilder {
    id: String,
    rect: ((i32, i32), (i32, i32)),
    start_at: Option<Instant>,
    frequency: Option<Duration>,
}

impl From<OCRJobBuilder> for OCRJob {
    fn from(value: OCRJobBuilder) -> Self {
        OCRJob {
            id: value.id,
            rect: value.rect,
            start_at: value.start_at.unwrap_or_else(Instant::now),
            frequency: value.frequency.unwrap_or_else(|| Duration::from_secs(3)),
            state: OCRJobStatus::Pending,
        }
    }
}

impl OCRJobBuilder {
    pub fn new(id: String, rect: ((i32, i32), (i32, i32))) -> OCRJobBuilder {
        OCRJobBuilder {
            id,
            rect,
            start_at: None,
            frequency: None,
        }
    }

    pub fn with_frequency(mut self, frequency: Duration) -> OCRJobBuilder {
        self.frequency = Some(frequency);
        self
    }

    pub fn with_start_at(mut self, start_at: Instant) -> OCRJobBuilder {
        self.start_at = Some(start_at);
        self
    }
}
