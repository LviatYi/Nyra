use crate::capture::selector::ImageCapture;
use crate::perception::perception_service::service::{
    MAX_WORKER_COUNT, PerceptionError, PerceptionRequest, PerceptionResponse, PerceptionResult,
};
use crate::perception::tesseract::TesseractPerceptor;
use crate::perception::text_perceptor::TextPerceptor;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Waker};

pub mod service {
    pub(crate) const INITIAL_WORKER_COUNT: usize = 1;
    pub(crate) const MAX_WORKER_COUNT: usize = 8;

    use crate::job::entity::ScreenRect;
    use crate::perception::perception_service::{
        PerceptionFuture, PoolState, WorkerPool, build_workers, complete_future, execute_request,
    };
    use std::error::Error;
    use std::sync::{Arc, Condvar, Mutex};
    use std::thread;
    use std::time::Instant;

    #[derive(Debug, Clone)]
    pub struct PerceptionRequest {
        pub request_id: u64,
        pub region: ScreenRect,
        pub submitted_at: Instant,
    }

    #[derive(Debug, Clone)]
    pub struct PerceptionResult {
        pub request_id: u64,
        pub region: ScreenRect,
        pub summary: String,
        pub text: String,
        pub latency_ms: u128,
    }

    #[derive(Debug, Clone)]
    pub struct PerceptionError {
        pub request_id: u64,
        pub message: String,
    }

    impl Error for PerceptionError {}

    impl std::fmt::Display for PerceptionError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "request {} failed: {}", self.request_id, self.message)
        }
    }

    pub type PerceptionResponse = Result<PerceptionResult, PerceptionError>;

    /// Perception Service
    #[derive(Clone)]
    pub struct PerceptionService {
        pool: Arc<WorkerPool>,
    }

    impl Default for PerceptionService {
        fn default() -> Self {
            Self::with_initial_worker_count(INITIAL_WORKER_COUNT)
        }
    }

    impl PerceptionService {
        fn with_initial_worker_count(initial_worker_count: usize) -> Self {
            Self::build(initial_worker_count)
        }

        fn build(initial_worker_count: usize) -> Self {
            let initial_worker_count = initial_worker_count.clamp(1, MAX_WORKER_COUNT);
            let pool = Arc::new(WorkerPool {
                state: Mutex::new(PoolState {
                    idle_workers: build_workers(0, initial_worker_count),
                    working_workers: 0,
                    initializing_workers: 0,
                    next_worker_id: initial_worker_count,
                }),
                worker_available: Condvar::new(),
            });

            Self { pool }
        }

        pub fn sync_analyze(&self, request: PerceptionRequest) -> PerceptionResponse {
            execute_request(Arc::clone(&self.pool), request)
        }

        pub async fn analyze(&self, request: PerceptionRequest) -> PerceptionResponse {
            self.analyze_in_thread(request).await
        }

        fn analyze_in_thread(&self, request: PerceptionRequest) -> PerceptionFuture {
            let request_id = request.request_id;
            let future = PerceptionFuture::new();
            let pool = Arc::clone(&self.pool);
            let future_state = Arc::clone(&future.state);
            let spawn_result = thread::Builder::new()
                .name(format!("NYRA perception-job-{request_id}"))
                .spawn(move || {
                    let result = execute_request(pool, request);
                    complete_future(future_state, result);
                });

            if let Err(error) = spawn_result {
                complete_future(
                    Arc::clone(&future.state),
                    Err(PerceptionError {
                        request_id,
                        message: format!("failed to spawn perception job: {error}"),
                    }),
                );
            }

            future
        }

        // pub fn pool_snapshot(&self) -> (usize, usize) {
        //     let state = self.pool.state.lock().expect("perception pool poisoned");
        //     (state.idle_workers.len(), state.working_workers)
        // }
    }
}

pub struct PerceptionWorker {
    pub id: usize,
    pub perceptor: TesseractPerceptor,
    pub status: WorkerStatus,
}

pub enum WorkerStatus {
    Idle,
    Working(u64),
}

struct PoolState {
    idle_workers: Vec<PerceptionWorker>,
    working_workers: usize,
    initializing_workers: usize,
    next_worker_id: usize,
}

impl PoolState {
    fn total_workers(&self) -> usize {
        self.idle_workers.len() + self.working_workers + self.initializing_workers
    }
}

struct WorkerPool {
    state: Mutex<PoolState>,
    worker_available: Condvar,
}

struct PerceptionFutureState {
    result: Option<PerceptionResponse>,
    waker: Option<Waker>,
}

pub struct PerceptionFuture {
    state: Arc<Mutex<PerceptionFutureState>>,
}

impl PerceptionFuture {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(PerceptionFutureState {
                result: None,
                waker: None,
            })),
        }
    }
}

impl Future for PerceptionFuture {
    type Output = PerceptionResponse;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.lock().expect("perception future poisoned");
        if let Some(result) = state.result.take() {
            Poll::Ready(result)
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

struct WorkerLease {
    pool: Arc<WorkerPool>,
    worker: Option<PerceptionWorker>,
}

impl WorkerLease {
    fn new(pool: Arc<WorkerPool>, worker: PerceptionWorker) -> Self {
        Self {
            pool,
            worker: Some(worker),
        }
    }

    fn perceptor(&self) -> &TesseractPerceptor {
        &self
            .worker
            .as_ref()
            .expect("worker lease missing worker")
            .perceptor
    }
}

impl Drop for WorkerLease {
    fn drop(&mut self) {
        if let Some(mut worker) = self.worker.take() {
            worker.status = WorkerStatus::Idle;
            let mut state = self.pool.state.lock().expect("perception pool poisoned");
            state.working_workers = state.working_workers.saturating_sub(1);
            state.idle_workers.push(worker);
            self.pool.worker_available.notify_all();
        }
    }
}

fn build_workers(start_id: usize, worker_count: usize) -> Vec<PerceptionWorker> {
    (start_id..start_id + worker_count)
        .map(build_worker)
        .collect()
}

fn build_worker(id: usize) -> PerceptionWorker {
    PerceptionWorker {
        id,
        perceptor: TesseractPerceptor::new_with_init(),
        status: WorkerStatus::Idle,
    }
}

fn complete_future(state: Arc<Mutex<PerceptionFutureState>>, response: PerceptionResponse) {
    let waker = {
        let mut state = state.lock().expect("perception future poisoned");
        state.result = Some(response);
        state.waker.take()
    };

    if let Some(waker) = waker {
        waker.wake();
    }
}

fn execute_request(pool: Arc<WorkerPool>, request: PerceptionRequest) -> PerceptionResponse {
    let mut worker = checkout_worker(pool, request.request_id);
    analyze_with_worker(&mut worker, request)
}

fn analyze_with_worker(worker: &mut WorkerLease, request: PerceptionRequest) -> PerceptionResponse {
    let region = request.region;
    let capture = region.capture().map_err(|error| PerceptionError {
        request_id: request.request_id,
        message: format!("capture failed: {error}"),
    })?;
    let text = worker
        .perceptor()
        .recognize(&capture)
        .map_err(|error| PerceptionError {
            request_id: request.request_id,
            message: format!("ocr failed: {error}"),
        })?;

    Ok(PerceptionResult {
        request_id: request.request_id,
        region,
        summary: format!("OCR: {text}"),
        text,
        latency_ms: request.submitted_at.elapsed().as_millis(),
    })
}

fn checkout_worker(pool: Arc<WorkerPool>, request_id: u64) -> WorkerLease {
    let mut state = pool.state.lock().expect("perception pool poisoned");
    loop {
        if let Some(mut worker) = state.idle_workers.pop() {
            worker.status = WorkerStatus::Working(request_id);
            state.working_workers += 1;
            drop(state);
            return WorkerLease::new(pool, worker);
        }

        if state.total_workers() < MAX_WORKER_COUNT {
            let worker_id = state.next_worker_id;
            state.next_worker_id += 1;
            state.initializing_workers += 1;
            drop(state);

            let mut worker = build_worker(worker_id);
            worker.status = WorkerStatus::Working(request_id);

            let mut state = pool.state.lock().expect("perception pool poisoned");
            state.initializing_workers = state.initializing_workers.saturating_sub(1);
            state.working_workers += 1;
            drop(state);

            return WorkerLease {
                worker: Some(worker),
                pool,
            };
        }

        state = pool
            .worker_available
            .wait(state)
            .expect("perception pool poisoned");
    }
}
