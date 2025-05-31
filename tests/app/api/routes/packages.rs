use anyhow::{Context, Result};
use http::StatusCode;

use crate::helpers::spawn_app;

#[tokio::test]
async fn test_get_packages_returns_200() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();

    // Act
    let url = format!("{}/packages", app.address);
    let response = client.get(url).send().await?;

    // Assert
    assert_eq!(response.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
async fn test_get_packages_returns_paginated_packages() -> Result<()> {
    const LIMIT: u64 = 10;
    const COUNT: u64 = 100;
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();
    let (registry_queue, _) = app.registry_queue()?;
    let packages = app.mock_create_packages(&registry_queue, COUNT).await?;

    let pages = COUNT / LIMIT;
    let mut after: Option<String> = None;
    for page in 0..pages {
        // Act
        let url = match after {
            Some(after) => format!(
                "{}/packages?limit={}&order=asc&after={}",
                app.address, LIMIT, after
            ),
            None => format!("{}/packages?limit={}&order=asc", app.address, LIMIT),
        };
        let response = client.get(url).send().await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);

        let body: serde_json::Value = response.json().await?;
        assert!(body["data"].is_array());
        assert!(body["next_cursor"].is_string() || body["next_cursor"].is_null());

        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), LIMIT as usize);

        for i in 0..LIMIT as usize {
            assert_eq!(
                data[i]["id"].as_str().unwrap(),
                packages[page as usize * LIMIT as usize + i].id.to_string()
            );
        }

        let next_cursor = body
            .get("next_cursor")
            .context("next_cursor is not present")?;
        after = next_cursor.as_str().map(|id| id.to_string());
    }
    assert!(after.is_none());

    Ok(())
}
