use anyhow::Result;

use crate::helpers::spawn_app;

#[tokio::test]
async fn test_openapi_returns_200() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();

    // Act
    let url = format!("{}/openapi.json", &app.address);
    let response = client.get(url).send().await?;

    // Assert
    assert_eq!(response.status().as_u16(), 200);
    let content_type = response
        .headers()
        .get("content-type")
        .ok_or(anyhow::anyhow!("Missing Content-Type header"))?;
    assert_eq!(content_type, "application/json");

    Ok(())
}

#[tokio::test]
async fn test_index_returns_200() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();

    // Act
    let url = format!("{}/", &app.address);
    let response = client.get(url).send().await?;

    // Assert
    assert_eq!(response.status().as_u16(), 200);

    Ok(())
}
