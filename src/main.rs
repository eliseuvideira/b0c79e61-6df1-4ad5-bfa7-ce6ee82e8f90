use anyhow::Result;
use integrations_api::app::Application;
use integrations_api::config::Settings;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let configuration = Settings::build()?;
    let application = Application::build(configuration).await?;

    application.run_until_stopped().await?;

    Ok(())
}
