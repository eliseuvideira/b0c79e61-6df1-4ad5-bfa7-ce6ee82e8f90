use anyhow::Result;
use reqwest::StatusCode;

use crate::helpers::spawn_app;

#[tokio::test]
async fn metrics_endpoint_returns_200() -> Result<()> {
    // Arrange
    let app = spawn_app().await.unwrap();
    let client = reqwest::Client::new();

    // Act
    let response = client
        .get(format!("{}/metrics", app.address))
        .send()
        .await?;

    // Assert
    assert_eq!(response.status(), StatusCode::OK);

    Ok(())
}
