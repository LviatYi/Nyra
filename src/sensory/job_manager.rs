use crate::job::JobConfig;
use bevy::prelude::*;
use std::time::Duration;

pub(super) struct ActiveJob {
    pub(super) current_index: usize,
    pub(super) preview_next_index: usize,
    pub(super) started_at: Duration,
    pub(super) ends_at: Duration,
}

impl ActiveJob {
    pub(super) fn elapsed_at(&self, now: Duration) -> Duration {
        now.saturating_sub(self.started_at)
    }

    pub(super) fn remaining_fraction_at(&self, now: Duration) -> f32 {
        let duration = self.ends_at.saturating_sub(self.started_at).as_secs_f32();
        if duration == 0.0 {
            return 0.0;
        }
        (1.0 - self.elapsed_at(now).as_secs_f32() / duration).clamp(0.0, 1.0)
    }
}

/// Read-only scheduling result consumed by presentation systems.
#[derive(Resource, Default)]
pub(super) struct ActiveJobState {
    pub(super) active_job: Option<ActiveJob>,
}

impl ActiveJobState {
    fn start(
        &mut self,
        index: usize,
        preview_next_index: usize,
        now: Duration,
        duration: Duration,
    ) {
        self.active_job = Some(ActiveJob {
            current_index: index,
            preview_next_index,
            started_at: now,
            ends_at: now.saturating_add(duration),
        });
    }
}

/// Runtime scheduling state for one configured job.
#[derive(Debug, Default)]
struct JobRuntimeState {
    /// `None` means the job has never completed and is immediately eligible.
    finished_at: Option<Duration>,
}

impl JobRuntimeState {
    fn eligible_at(&self, interval: Duration) -> Duration {
        self.finished_at
            .map(|finished_at| finished_at.saturating_add(interval))
            .unwrap_or(Duration::ZERO)
    }

    fn is_eligible_at(&self, now: Duration, interval: Duration) -> bool {
        now >= self.eligible_at(interval)
    }
}

/// Selects which configured job should be presented next.
///
/// Runtime deadlines and candidate selection deliberately live behind this type so
/// later conditions can extend `JobRuntimeState` and `select_next` without coupling
/// them to overlay rendering.
#[derive(Resource, Default)]
pub(super) struct JobManager {
    jobs: Vec<JobRuntimeState>,
}

impl JobManager {
    fn update_at(&mut self, config: &JobConfig, state: &mut ActiveJobState, now: Duration) -> bool {
        self.synchronize_job_count(config.0.tips.len());
        if config.is_empty() {
            return state.active_job.take().is_some();
        }

        let Some(active_job) = state.active_job.as_ref() else {
            let next_index = self.select_next(config, now, None).unwrap();
            self.start_job(config, state, next_index, now);
            return true;
        };
        if now < active_job.ends_at {
            return false;
        }

        let completed_index = active_job.current_index;
        // interval starts after presentation completes; the job's own showTime does
        // not consume its next interval.
        self.jobs[completed_index].finished_at = Some(now);
        let next_index = self
            .select_next(config, now, Some(completed_index))
            .unwrap();
        self.start_job(config, state, next_index, now);
        true
    }

    fn start_job(
        &self,
        config: &JobConfig,
        state: &mut ActiveJobState,
        index: usize,
        now: Duration,
    ) {
        let duration = Duration::from_secs(config.0.tips[index].show_time());
        let ends_at = now.saturating_add(duration);
        let preview_next_index = self
            .select_next(config, ends_at, Some(index))
            // A non-empty config always falls back to the selected job.
            .unwrap();
        state.start(index, preview_next_index, now, duration);
    }

    fn select_next(
        &self,
        config: &JobConfig,
        now: Duration,
        completed_index: Option<usize>,
    ) -> Option<usize> {
        config
            .0
            .tips
            .iter()
            .enumerate()
            // Do not repeat the current job while another job is eligible.
            .filter(|(index, _)| Some(*index) != completed_index)
            .filter(|(index, job)| {
                self.jobs[*index].is_eligible_at(now, Duration::from_secs(job.interval))
            })
            // Prefer the job that has been eligible for the longest time. A job
            // that has never completed is treated as eligible since startup, so
            // every initial job is selected before recurring jobs can starve it.
            // Interval and configuration order provide deterministic tie-breaks.
            .min_by_key(|(index, job)| {
                (
                    self.jobs[*index].eligible_at(Duration::from_secs(job.interval)),
                    job.interval,
                    *index,
                )
            })
            .map(|(index, _)| index)
            // Continuous display is preferred to waiting when every alternative is blocked.
            .or(completed_index.filter(|index| *index < config.0.tips.len()))
    }

    fn synchronize_job_count(&mut self, job_count: usize) {
        self.jobs.resize_with(job_count, JobRuntimeState::default);
    }
}

pub(super) fn process_jobs(
    time: Res<Time<Real>>,
    config: Res<JobConfig>,
    mut manager: ResMut<JobManager>,
    mut state: ResMut<ActiveJobState>,
) {
    let changed = manager.update_at(&config, state.bypass_change_detection(), time.elapsed());
    if changed {
        state.set_changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{Tip, Tips};

    #[test]
    fn schedules_by_earliest_eligibility() {
        assert_schedule(
            &[(1, 3), (10, 3), (15, 3)],
            &[
                (0, 0, 1),
                (3, 1, 2),
                (6, 2, 0),
                (9, 0, 0),
                (12, 0, 0),
                (15, 0, 1),
                (18, 1, 0),
            ],
        );
    }

    #[test]
    fn does_not_advance_before_show_time_finishes() {
        let mut scheduler = TestScheduler::new(&[(1, 3), (2, 3)]);
        assert!(scheduler.update_at(0));
        assert!(!scheduler.update_at(2));
        assert_eq!(scheduler.active_indexes(), (0, 1));
    }

    #[test]
    fn configuration_order_breaks_equal_interval_ties() {
        assert_schedule(&[(5, 1), (5, 1), (5, 1)], &[(0, 0, 1), (1, 1, 2)]);
    }

    fn assert_schedule(job_times: &[(u64, u64)], expected: &[(u64, usize, usize)]) {
        let mut scheduler = TestScheduler::new(job_times);
        for &(time, current_index, preview_next_index) in expected {
            assert!(scheduler.update_at(time));
            assert_eq!(
                scheduler.active_indexes(),
                (current_index, preview_next_index),
                "unexpected schedule at {time}s"
            );
        }
    }

    struct TestScheduler {
        config: JobConfig,
        manager: JobManager,
        state: ActiveJobState,
    }

    impl TestScheduler {
        fn new(job_times: &[(u64, u64)]) -> Self {
            let tips = job_times
                .iter()
                .enumerate()
                .map(|(index, &(interval, show_time))| Tip {
                    tip: format!("job {index}"),
                    interval,
                    show_time: Some(show_time),
                    color: None,
                })
                .collect();
            Self {
                config: JobConfig(Tips { tips }),
                manager: JobManager::default(),
                state: ActiveJobState::default(),
            }
        }

        fn update_at(&mut self, seconds: u64) -> bool {
            self.manager
                .update_at(&self.config, &mut self.state, Duration::from_secs(seconds))
        }

        fn active_indexes(&self) -> (usize, usize) {
            let active_job = self.state.active_job.as_ref().unwrap();
            (active_job.current_index, active_job.preview_next_index)
        }
    }
}
