use std::fmt::{Display, Formatter};
use std::future::Future;

const MEASURE_TRACING_TITLE: &str = "MEASURE";
const TAB_WIDTH: usize = 4;

pub struct MeasureReturn<R> {
    pub result: R,
    pub report: Report,
}

impl<R> MeasureReturn<R> {
    pub fn map<RO>(self, f: impl FnOnce(R) -> RO) -> MeasureReturn<RO> {
        MeasureReturn {
            result: f(self.result),
            report: self.report,
        }
    }

    pub fn try_map<RO, E>(
        self,
        f: impl FnOnce(R) -> Result<RO, E>,
    ) -> Result<MeasureReturn<RO>, E> {
        Ok(MeasureReturn {
            result: f(self.result)?,
            report: self.report,
        })
    }

    pub fn inspect(self, f: impl FnOnce(&R)) -> Self {
        f(&self.result);
        self
    }

    pub fn measure<RO>(self, label: impl AsRef<str>, f: impl FnOnce() -> RO) -> MeasureReturn<RO> {
        measure_with_report(label, f, self.report)
    }

    pub fn measure_with<RO>(
        self,
        label: impl AsRef<str>,
        f: impl FnOnce(R) -> RO,
    ) -> MeasureReturn<RO> {
        measure_with_report(label, || f(self.result), self.report)
    }

    pub fn try_measure<RO, E>(
        self,
        label: impl AsRef<str>,
        f: impl FnOnce(R) -> Result<RO, E>,
    ) -> Result<MeasureReturn<RO>, E> {
        try_measure_with_report(label, || f(self.result), self.report)
    }

    pub async fn measure_async<RO, Fut>(
        self,
        label: impl AsRef<str>,
        f: impl FnOnce() -> Fut,
    ) -> MeasureReturn<RO>
    where
        Fut: Future<Output = RO>,
    {
        measure_async_with_report(label, f(), self.report).await
    }

    pub async fn measure_with_async<RO, Fut>(
        self,
        label: impl AsRef<str>,
        f: impl FnOnce(R) -> Fut,
    ) -> MeasureReturn<RO>
    where
        Fut: Future<Output = RO>,
    {
        measure_async_with_report(label, f(self.result), self.report).await
    }

    pub async fn try_measure_async<RO, E, Fut>(
        self,
        label: impl AsRef<str>,
        f: impl FnOnce(R) -> Fut,
    ) -> Result<MeasureReturn<RO>, E>
    where
        Fut: Future<Output = Result<RO, E>>,
    {
        try_measure_async_with_report(label, f(self.result), self.report).await
    }

    pub fn into_inner(self) -> R {
        self.result
    }

    pub fn into_report(self) -> Report {
        self.report
    }

    pub fn into_parts(self) -> (R, Report) {
        (self.result, self.report)
    }
}

impl<R> AsRef<R> for MeasureReturn<R> {
    fn as_ref(&self) -> &R {
        &self.result
    }
}

#[derive(Debug, Default)]
pub struct Report {
    pub items: Vec<ReportItem>,
}

#[derive(Debug)]
pub struct ReportItem {
    pub label: String,
    pub elapsed_sec: f64,
}

impl Report {
    pub fn append(&mut self, label: impl AsRef<str>, elapsed_sec: f64) {
        self.items.push(ReportItem {
            label: label.as_ref().to_string(),
            elapsed_sec,
        });
    }

    pub fn print_by_tracing(&self) {
        tracing::info!(report = %self, "{}", MEASURE_TRACING_TITLE);
    }
}

impl Display for Report {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let max_tab_stop = self
            .items
            .iter()
            .map(|item| item.label.len())
            .max()
            .unwrap_or(0)
            / TAB_WIDTH
            + 1;

        write!(
            f,
            "{}",
            self.items
                .iter()
                .map(|item| {
                    let label_tab_stop = item.label.len() / TAB_WIDTH;
                    let tab_count = max_tab_stop.saturating_sub(label_tab_stop).max(1);
                    format!(
                        "{}{}{} sec",
                        item.label,
                        "\t".repeat(tab_count),
                        item.elapsed_sec
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

pub fn measure<R>(label: impl AsRef<str>, f: impl FnOnce() -> R) -> MeasureReturn<R> {
    measure_with_report(label, f, Report::default())
}

pub fn measure_with_report<R>(
    label: impl AsRef<str>,
    f: impl FnOnce() -> R,
    mut report: Report,
) -> MeasureReturn<R> {
    let label = label.as_ref();

    let start = std::time::Instant::now();
    let result = f();
    let elapsed_sec = start.elapsed().as_secs_f64();

    tracing::trace!(
        target = label,
        elapsed_sec = elapsed_sec,
        "{}",
        MEASURE_TRACING_TITLE
    );

    report.append(label, elapsed_sec);

    MeasureReturn { result, report }
}

pub fn try_measure<R, E>(
    label: impl AsRef<str>,
    f: impl FnOnce() -> Result<R, E>,
) -> Result<MeasureReturn<R>, E> {
    try_measure_with_report(label, f, Report::default())
}

pub fn try_measure_with_report<R, E>(
    label: impl AsRef<str>,
    f: impl FnOnce() -> Result<R, E>,
    report: Report,
) -> Result<MeasureReturn<R>, E> {
    let measured = measure_with_report(label, f, report);
    Ok(MeasureReturn {
        result: measured.result?,
        report: measured.report,
    })
}

pub async fn measure_async<R, Fut>(label: impl AsRef<str>, future: Fut) -> MeasureReturn<R>
where
    Fut: Future<Output = R>,
{
    measure_async_with_report(label, future, Report::default()).await
}

pub async fn measure_async_with_report<R, Fut>(
    label: impl AsRef<str>,
    future: Fut,
    mut report: Report,
) -> MeasureReturn<R>
where
    Fut: Future<Output = R>,
{
    let label = label.as_ref();

    let start = std::time::Instant::now();
    let result = future.await;
    let elapsed_sec = start.elapsed().as_secs_f64();

    tracing::trace!(
        target = label,
        elapsed_sec = elapsed_sec,
        "{}",
        MEASURE_TRACING_TITLE
    );

    report.append(label, elapsed_sec);

    MeasureReturn { result, report }
}

pub async fn try_measure_async<R, E, Fut>(
    label: impl AsRef<str>,
    future: Fut,
) -> Result<MeasureReturn<R>, E>
where
    Fut: Future<Output = Result<R, E>>,
{
    try_measure_async_with_report(label, future, Report::default()).await
}

pub async fn try_measure_async_with_report<R, E, Fut>(
    label: impl AsRef<str>,
    future: Fut,
    report: Report,
) -> Result<MeasureReturn<R>, E>
where
    Fut: Future<Output = Result<R, E>>,
{
    let measured = measure_async_with_report(label, future, report).await;
    Ok(MeasureReturn {
        result: measured.result?,
        report: measured.report,
    })
}
