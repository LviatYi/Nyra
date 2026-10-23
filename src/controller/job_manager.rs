use crate::job::{Job, JobConfig};
use bevy::prelude::*;
use std::time::Duration;

pub(crate) struct ActiveJob {
    pub(crate) current_index: usize,
    pub(crate) started_at: Duration,
    ends_at: Option<Duration>,
}

impl ActiveJob {
    pub(crate) fn countdown_at(&self, now: Duration) -> Option<(Duration, Duration)> {
        let ends_at = self.ends_at?;
        Some((
            ends_at.saturating_sub(now),
            ends_at.saturating_sub(self.started_at),
        ))
    }
}

/// Read-only scheduling result consumed by presentation systems.
#[derive(Resource, Default)]
pub(crate) struct ActiveJobState {
    pub(crate) active_job: Option<ActiveJob>,
}

impl ActiveJobState {
    pub(crate) fn current_index(&self) -> Option<usize> {
        self.active_job.as_ref().map(|job| job.current_index)
    }
}

/// A replacement is committed only when the scheduling result changes.
enum JobUpdate {
    Unchanged,
    Replace(Option<ActiveJob>),
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
    fn eval_update_at(
        &mut self,
        config: &JobConfig,
        state: &ActiveJobState,
        now: Duration,
    ) -> JobUpdate {
        self.synchronize_job_count(config.0.jobs.len());
        if config.is_empty() {
            return if state.active_job.is_some() {
                JobUpdate::Replace(None)
            } else {
                JobUpdate::Unchanged
            };
        }

        let Some(active_job) = state.active_job.as_ref() else {
            let next = self
                .select_next(config, now, None)
                .map(|index| self.create_job(config, index, now));
            return if next.is_some() {
                JobUpdate::Replace(next)
            } else {
                JobUpdate::Unchanged
            };
        };
        if active_job.ends_at.is_some_and(|ends_at| now < ends_at) {
            return JobUpdate::Unchanged;
        }

        let completed_index = active_job.current_index;
        // interval starts after presentation completes; the job's own showTime does
        // not consume its next interval.
        self.jobs[completed_index].finished_at = Some(now);
        let next = self
            .select_next(config, now, Some(completed_index))
            .map(|index| self.create_job(config, index, now));
        JobUpdate::Replace(next)
    }

    fn create_job(&self, config: &JobConfig, index: usize, now: Duration) -> ActiveJob {
        let ends_at = match &config.0.jobs[index] {
            Job::Interval { show_time, .. } => Some(now.saturating_add(Duration::from_secs(
                show_time.unwrap_or(crate::settings_default_values::DEFAULT_TIP_SHOW_TIME),
            ))),
        };
        ActiveJob {
            current_index: index,
            started_at: now,
            ends_at,
        }
    }

    fn select_next(
        &self,
        config: &JobConfig,
        now: Duration,
        completed_index: Option<usize>,
    ) -> Option<usize> {
        config
            .0
            .jobs
            .iter()
            .enumerate()
            // Do not repeat the current job while another job is eligible.
            .filter_map(|(index, job)| match job {
                Job::Interval { interval, .. } if Some(index) != completed_index => {
                    Some((index, *interval))
                }
                _ => None,
            })
            .filter(|(index, interval)| {
                self.jobs[*index].is_eligible_at(now, Duration::from_secs(*interval))
            })
            // Prefer the job that has been eligible for the longest time. A job
            // that has never completed is treated as eligible since startup, so
            // every initial job is selected before recurring jobs can starve it.
            // Interval and configuration order provide deterministic tie-breaks.
            .min_by_key(|(index, interval)| {
                (
                    self.jobs[*index].eligible_at(Duration::from_secs(*interval)),
                    *interval,
                    *index,
                )
            })
            .map(|(index, _)| index)
            // Continuous display is preferred to waiting when every alternative is blocked.
            .or(completed_index.filter(|index| *index < config.0.jobs.len()))
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
    let update = manager.eval_update_at(&config, &state, time.elapsed());
    if let JobUpdate::Replace(next) = update {
        state.active_job = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{Job, Jobs, Tip};

    #[test]
    fn schedules_by_earliest_eligibility() {
        assert_schedule(
            &[(1, 3), (10, 3), (15, 3)],
            &[(0, 0), (3, 1), (6, 2), (9, 0), (12, 0), (15, 0), (18, 1)],
        );
    }

    #[test]
    fn does_not_advance_before_show_time_finishes() {
        let mut scheduler = TestScheduler::new(&[(1, 3), (2, 3)]);
        assert!(scheduler.update_at(0));
        assert!(!scheduler.update_at(2));
        assert_eq!(scheduler.active_index(), 0);
    }

    #[test]
    fn configuration_order_breaks_equal_interval_ties() {
        assert_schedule(&[(5, 1), (5, 1), (5, 1)], &[(0, 0), (1, 1)]);
    }

    #[test]
    fn marks_state_changed_only_when_committing_a_scheduling_update() {
        #[derive(Resource, Default)]
        struct ObservedChanges(Vec<bool>);

        fn observe(state: Res<ActiveJobState>, mut changes: ResMut<ObservedChanges>) {
            changes.0.push(state.is_changed());
        }

        // Exercise both repeating the current tip and switching to another tip.
        for job_times in [vec![(1, 3)], vec![(1, 3), (2, 3)]] {
            let scheduler = TestScheduler::new(&job_times);
            let mut world = World::new();
            world.insert_resource(scheduler.config);
            world.insert_resource(scheduler.manager);
            world.insert_resource(scheduler.state);
            world.init_resource::<Time<Real>>();
            world.init_resource::<ObservedChanges>();
            let mut schedule = Schedule::default();
            schedule.add_systems((process_jobs, observe).chain());

            for elapsed in [0, 1, 2, 1] {
                world
                    .resource_mut::<Time<Real>>()
                    .advance_by(Duration::from_secs(elapsed));
                schedule.run(&mut world);
            }

            world.resource_mut::<JobConfig>().0.jobs.clear();
            schedule.run(&mut world);
            schedule.run(&mut world);

            assert_eq!(
                world.resource::<ObservedChanges>().0,
                [true, false, true, false, true, false],
            );
            assert!(world.resource::<ActiveJobState>().active_job.is_none());
        }
    }

    fn assert_schedule(job_times: &[(u64, u64)], expected: &[(u64, usize)]) {
        let mut scheduler = TestScheduler::new(job_times);
        for &(time, current_index) in expected {
            assert!(scheduler.update_at(time));
            assert_eq!(
                scheduler.active_index(),
                current_index,
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
            let jobs = job_times
                .iter()
                .enumerate()
                .map(|(index, &(interval, show_time))| Job::Interval {
                    tip: Tip {
                        text: format!("job {index}"),
                        color: None,
                        icon: None,
                        reaction: None,
                    },
                    interval,
                    show_time: Some(show_time),
                })
                .collect();
            Self {
                config: JobConfig(Jobs { jobs }),
                manager: JobManager::default(),
                state: ActiveJobState::default(),
            }
        }

        fn update_at(&mut self, seconds: u64) -> bool {
            match self.manager.eval_update_at(
                &self.config,
                &self.state,
                Duration::from_secs(seconds),
            ) {
                JobUpdate::Unchanged => false,
                JobUpdate::Replace(next) => {
                    self.state.active_job = next;
                    true
                }
            }
        }

        fn active_index(&self) -> usize {
            self.state.active_job.as_ref().unwrap().current_index
        }
    }
}
