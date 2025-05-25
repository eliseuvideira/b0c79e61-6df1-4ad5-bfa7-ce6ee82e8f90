use anyhow::Result;
use prometheus::{
    register_counter_vec_with_registry, register_gauge_vec_with_registry,
    register_histogram_vec_with_registry, Counter, CounterVec, Gauge, GaugeVec, Histogram,
    HistogramVec, Registry,
};

pub struct Metrics {
    pub registry: Registry,

    http_requests_pending: GaugeVec,
    http_requests_total: CounterVec,
    http_requests_duration_seconds: HistogramVec,
}

impl Metrics {
    pub fn build() -> Result<Self> {
        let registry = Registry::default();

        let http_requests_pending = register_gauge_vec_with_registry!(
            "http_requests_pending",
            "Total number of HTTP requests in progress",
            &["method", "endpoint"],
            &registry
        )?;
        let http_requests_total = register_counter_vec_with_registry!(
            "http_requests_total",
            "Total number of HTTP requests",
            &["method", "endpoint", "status"],
            &registry
        )?;
        let http_requests_duration_seconds = register_histogram_vec_with_registry!(
            "http_requests_duration_seconds",
            "Duration of HTTP requests in seconds",
            &["method", "endpoint", "status"],
            &registry
        )?;

        Ok(Self {
            registry,
            http_requests_total,
            http_requests_pending,
            http_requests_duration_seconds,
        })
    }

    pub fn http_requests_pending(&self, method: &str, endpoint: &str) -> Gauge {
        self.http_requests_pending
            .with_label_values(&[method, endpoint])
    }

    pub fn http_requests_total(&self, method: &str, endpoint: &str, status: &str) -> Counter {
        self.http_requests_total
            .with_label_values(&[method, endpoint, status])
    }

    pub fn http_requests_duration_seconds(
        &self,
        method: &str,
        endpoint: &str,
        status: &str,
    ) -> Histogram {
        self.http_requests_duration_seconds
            .with_label_values(&[method, endpoint, status])
    }
}
