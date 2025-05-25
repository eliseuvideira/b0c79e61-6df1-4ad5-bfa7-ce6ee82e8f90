use anyhow::Result;
use integrations_api::{
    app::Application,
    config::Config,
    telemetry::{init_metrics, init_subscribers},
};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let _guard = init_subscribers()?;
    let metrics_handle = init_metrics();

    let configuration = Config::build()?;
    let application = Application::build(configuration, metrics_handle).await?;

    application.run_until_stopped().await?;

    Ok(())
}
