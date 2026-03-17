const MEASURE_TRACING_TITLE: &'static str = "MEASURE";

pub fn measure<R>(label: &str, f: impl FnOnce() -> R) -> R {
    let start = std::time::Instant::now();
    let result = f();
    let elapsed = start.elapsed();

    tracing::info!(target = label, elapsed_sec = elapsed.as_secs_f64(), "{}", MEASURE_TRACING_TITLE);

    result
}
