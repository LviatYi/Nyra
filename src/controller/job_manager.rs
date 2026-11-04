use crate::{
    job::{Job, JobConfig, Jobs},
    perception::{ImageCondition, ImpressionStore},
    reaction::ReactionRunner,
    reaction::sdk_types::UiCondition,
};
use bevy::prelude::*;
use std::{
    path::Path,
    sync::Mutex,
    time::{Duration, Instant},
};

const INITIATIVE_ROTATION: Duration = Duration::from_secs(5);

enum InitiativeCondition {
    Image {
        matcher: Mutex<ImageCondition>,
        interval: Duration,
        next_poll: Instant,
    },
    Delay {
        ready_at: Instant,
    },
}

struct InitiativeRuntime {
    index: usize,
    condition: InitiativeCondition,
    matched: bool,
    last_error: Option<String>,
}

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
pub(crate) struct JobManager {
    jobs: Vec<JobRuntimeState>,
    initiative: Vec<InitiativeRuntime>,
    next_poll: usize,
    rotation_at: Option<Duration>,
    suspended_interval: Option<(usize, Duration)>,
    sleeping: bool,
}

impl JobManager {
    fn set_sleeping(&mut self, sleeping: bool) {
        if self.sleeping == sleeping {
            return;
        }
        self.sleeping = sleeping;
        if !sleeping {
            let now = Instant::now();
            for item in &mut self.initiative {
                if let InitiativeCondition::Image { next_poll, .. } = &mut item.condition {
                    item.matched = false;
                    *next_poll = now;
                }
            }
        }
    }
    pub(crate) fn prepare(
        config: &Jobs,
        base_dir: &Path,
        impressions: &ImpressionStore,
    ) -> Result<Self, String> {
        let mut manager = Self::default();
        let now = Instant::now();
        for (index, job) in config.jobs.iter().enumerate() {
            let Job::Initiative { condition, .. } = job else {
                continue;
            };
            let condition = match condition {
                UiCondition::Delay(delay) => InitiativeCondition::Delay {
                    ready_at: now
                        .checked_add(Duration::from_millis(delay.duration_ms))
                        .ok_or_else(|| format!("jobs[{index}].condition duration is too large"))?,
                },
                UiCondition::Image(image) => {
                    let interval = Duration::from_millis(image.poll_interval_ms());
                    if now.checked_add(interval).is_none() {
                        return Err(format!(
                            "jobs[{index}].condition poll interval is too large"
                        ));
                    }
                    let matcher =
                        ImageCondition::prepare(image.clone(), base_dir, impressions.clone())
                            .map_err(|error| format!("jobs[{index}].condition: {error}"))?;
                    InitiativeCondition::Image {
                        matcher: Mutex::new(matcher),
                        interval,
                        next_poll: now,
                    }
                }
            };
            manager.initiative.push(InitiativeRuntime {
                index,
                condition,
                matched: false,
                last_error: None,
            });
        }
        Ok(manager)
    }

    fn poll_initiatives(&mut self) {
        let now = Instant::now();
        for item in &mut self.initiative {
            if let InitiativeCondition::Delay { ready_at } = &item.condition {
                item.matched = now >= *ready_at;
            }
        }
        let count = self.initiative.len();
        for step in 0..count {
            let index = (self.next_poll + step) % count;
            let item = &mut self.initiative[index];
            let InitiativeCondition::Image {
                matcher,
                interval,
                next_poll,
            } = &mut item.condition
            else {
                continue;
            };
            if now < *next_poll {
                continue;
            }
            let result = matcher
                .lock()
                .map_err(|_| "Image condition state is unavailable".to_string())
                .and_then(|image| image.matches());
            let completed_at = Instant::now();
            *next_poll = completed_at.checked_add(*interval).unwrap_or(completed_at);
            match result {
                Ok(value) => {
                    item.matched = value == Some(true);
                    item.last_error = None;
                }
                Err(error) => {
                    item.matched = false;
                    if item.last_error.as_deref() != Some(error.as_str()) {
                        eprintln!("[Initiative job {}] {error}", item.index);
                        item.last_error = Some(error);
                    }
                }
            }
            self.next_poll = (index + 1) % count;
            break;
        }
    }

    fn eval_update_at(
        &mut self,
        config: &JobConfig,
        state: &ActiveJobState,
        now: Duration,
    ) -> JobUpdate {
        self.synchronize_job_count(config.0.jobs.len());
        if config.is_empty() {
            self.suspended_interval = None;
            self.rotation_at = None;
            return if state.active_job.is_some() {
                JobUpdate::Replace(None)
            } else {
                JobUpdate::Unchanged
            };
        }

        let eligible = self
            .initiative
            .iter()
            .filter(|item| {
                item.matched
                    && matches!(config.0.jobs.get(item.index), Some(Job::Initiative { .. }))
            })
            .map(|item| item.index)
            .collect::<Vec<_>>();
        if !eligible.is_empty() {
            let current = state
                .current_index()
                .filter(|index| matches!(config.0.jobs.get(*index), Some(Job::Initiative { .. })));
            if let Some(index) = current {
                if eligible.contains(&index)
                    && (eligible.len() == 1
                        || self.rotation_at.is_some_and(|deadline| now < deadline))
                {
                    return JobUpdate::Unchanged;
                }
            } else if let Some(active) = state.active_job.as_ref() {
                if matches!(
                    config.0.jobs.get(active.current_index),
                    Some(Job::Interval { .. })
                ) {
                    let remaining = active.ends_at.unwrap_or(now).saturating_sub(now);
                    if remaining.is_zero() {
                        self.jobs[active.current_index].finished_at = Some(now);
                    } else {
                        self.suspended_interval = Some((active.current_index, remaining));
                    }
                }
            }
            let next = current
                .and_then(|index| {
                    eligible
                        .iter()
                        .copied()
                        .find(|candidate| *candidate > index)
                })
                .unwrap_or(eligible[0]);
            self.rotation_at = Some(now.saturating_add(INITIATIVE_ROTATION));
            return JobUpdate::Replace(Some(self.create_job(config, next, now)));
        }

        if state
            .current_index()
            .is_some_and(|index| matches!(config.0.jobs.get(index), Some(Job::Initiative { .. })))
        {
            self.rotation_at = None;
            if let Some((index, remaining)) = self.suspended_interval.take() {
                if matches!(config.0.jobs.get(index), Some(Job::Interval { .. })) {
                    return JobUpdate::Replace(Some(ActiveJob {
                        current_index: index,
                        started_at: now,
                        ends_at: Some(now.saturating_add(remaining)),
                    }));
                }
            }
            let next = self
                .select_next(config, now, None)
                .map(|index| self.create_job(config, index, now));
            return JobUpdate::Replace(next);
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
            Job::Initiative { .. } => None,
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
            .or(completed_index
                .filter(|index| matches!(config.0.jobs.get(*index), Some(Job::Interval { .. }))))
    }

    fn synchronize_job_count(&mut self, job_count: usize) {
        self.jobs.resize_with(job_count, JobRuntimeState::default);
    }
}

pub(super) fn update_job_sleep(runner: Res<ReactionRunner>, mut manager: ResMut<JobManager>) {
    let sleeping = runner.is_busy();
    if manager.sleeping != sleeping {
        manager.set_sleeping(sleeping);
    }
}

pub(super) fn jobs_are_awake(manager: Res<JobManager>) -> bool {
    !manager.sleeping
}

pub(super) fn poll_initiatives(mut manager: ResMut<JobManager>) {
    manager.poll_initiatives();
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
