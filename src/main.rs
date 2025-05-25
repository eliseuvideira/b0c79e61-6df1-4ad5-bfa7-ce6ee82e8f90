use anyhow::Result;
use integrations_api::{
    app::Application,
    config::Config,
    telemetry::{init_subscribers, Metrics},
};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let _guard = init_subscribers()?;

    let metrics = Metrics::build()?;
    let configuration = Config::build()?;
    let application = Application::build(configuration, metrics).await?;

    application.run_until_stopped().await?;

    Ok(())
}
