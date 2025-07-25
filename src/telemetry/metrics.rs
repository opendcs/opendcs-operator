use kube::ResourceExt;
use opentelemetry::trace::TraceId;
use prometheus_client::{
    encoding::EncodeLabelSet,
    metrics::{counter::Counter, exemplar::HistogramWithExemplars, family::Family},
    registry::{Registry, Unit},
};
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::time::Instant;

#[derive(Clone)]
pub struct Metrics<T: Clone + ResourceExt> {
    pub reconcile: ReconcileMetrics<T>,
    pub registry: Arc<Registry>,
}

impl<T: Clone + ResourceExt> Metrics<T> {
    fn named(controller_name: &str) -> Self {
        let mut registry = Registry::with_prefix(format!("${controller_name}ctrl_reconcile"));
        let reconcile = ReconcileMetrics::default().register(&mut registry);
        Self {
            registry: Arc::new(registry),
            reconcile,
        }
    }
}

impl<T: Clone + ResourceExt> Default for Metrics<T> {
    fn default() -> Self {
        Metrics::named("ctrl_reconcile")
    }
}

#[derive(Clone, Hash, PartialEq, Eq, EncodeLabelSet, Debug, Default)]
pub struct TraceLabel {
    pub trace_id: String,
}
impl TryFrom<&TraceId> for TraceLabel {
    type Error = anyhow::Error;

    fn try_from(id: &TraceId) -> Result<TraceLabel, Self::Error> {
        if std::matches!(id, &TraceId::INVALID) {
            anyhow::bail!("invalid trace id")
        } else {
            let trace_id = id.to_string();
            Ok(Self { trace_id })
        }
    }
}

#[derive(Clone)]
pub struct ReconcileMetrics<T: Clone + ResourceExt> {
    pub runs: Counter,
    pub failures: Family<ErrorLabels, Counter>,
    pub duration: HistogramWithExemplars<TraceLabel>,
    phantom: PhantomData<T>,
}

impl<T: Clone + ResourceExt> Default for ReconcileMetrics<T> {
    fn default() -> Self {
        Self {
            runs: Counter::default(),
            failures: Family::<ErrorLabels, Counter>::default(),
            duration: HistogramWithExemplars::new(
                [0.01, 0.1, 0.25, 0.5, 1., 5., 15., 60.].into_iter(),
            ),
            phantom: PhantomData,
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct ErrorLabels {
    pub instance: String,
    pub error: String,
}

pub trait MetricLabel {
    fn metric_label(&self) -> String;
}

impl MetricLabel for anyhow::Error {
    fn metric_label(&self) -> String {
        format!("{self:?}").to_lowercase()
    }
}

impl<T: Clone + ResourceExt> ReconcileMetrics<T> {
    /// Register API metrics to start tracking them.
    pub fn register(self, r: &mut Registry) -> Self {
        r.register_with_unit(
            "duration",
            "reconcile duration",
            Unit::Seconds,
            self.duration.clone(),
        );
        r.register("failures", "reconciliation errors", self.failures.clone());
        r.register("runs", "reconciliations", self.runs.clone());
        self
    }

    pub fn set_failure(&self, obj: &T, e: &anyhow::Error) {
        self.failures
            .get_or_create(&ErrorLabels {
                instance: obj.name_any(),
                error: e.metric_label(),
            })
            .inc();
    }

    pub fn count_and_measure(&self, trace_id: &TraceId) -> ReconcileMeasurer {
        self.runs.inc();
        ReconcileMeasurer {
            start: Instant::now(),
            labels: trace_id.try_into().ok(),
            metric: self.duration.clone(),
        }
    }
}

/// Smart function duration measurer
///
/// Relies on Drop to calculate duration and register the observation in the histogram
pub struct ReconcileMeasurer {
    start: Instant,
    labels: Option<TraceLabel>,
    metric: HistogramWithExemplars<TraceLabel>,
}

impl Drop for ReconcileMeasurer {
    fn drop(&mut self) {
        #[allow(clippy::cast_precision_loss)]
        let duration = self.start.elapsed().as_millis() as f64 / 1000.0;
        let labels = self.labels.take();
        self.metric.observe(duration, labels);
    }
}
