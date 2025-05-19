use anyhow::Result;
use integrations_api::{app::Application, config::Settings};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle, PrometheusRecorder};

pub struct TestApp {
    pub address: String,
}

pub fn init_metrics() -> (PrometheusHandle, PrometheusRecorder) {
    let builder = PrometheusBuilder::new();
    let recorder = builder.build_recorder();
    let handle = recorder.handle();

    (handle, recorder)
}

pub async fn spawn_app() -> Result<TestApp> {
    let configuration = {
        let mut configuration = Settings::build()?;
        configuration.application.host = "127.0.0.1".to_string();
        configuration.application.port = 0;
        configuration
    };

    let (metrics_handle, _) = init_metrics();

    let application = Application::build(configuration, metrics_handle)
        .await
        .expect("Failed to build application.");
    let port = application.port();
    let address = format!("http://127.0.0.1:{}", port);
    let _ = tokio::spawn(application.run_until_stopped());

    Ok(TestApp { address })
}
