use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

pub fn init_metrics() -> PrometheusHandle {
    PrometheusBuilder::new().install_recorder().unwrap()
}
