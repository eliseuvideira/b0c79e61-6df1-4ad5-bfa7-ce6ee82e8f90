use anyhow::Result;
use integrations_api::{app::Application, config::Settings};

pub struct TestApp {
    pub address: String,
}

pub async fn spawn_app() -> Result<TestApp> {
    let configuration = {
        let mut configuration = Settings::build()?;
        configuration.application.host = "127.0.0.1".to_string();
        configuration.application.port = 0;
        configuration
    };

    let application = Application::build(configuration)
        .await
        .expect("Failed to build application.");
    let port = application.port();
    let address = format!("http://127.0.0.1:{}", port);
    let _ = tokio::spawn(application.run_until_stopped());

    Ok(TestApp { address })
}
