use anyhow::{Context, Result};
use http::StatusCode;
use serde_json::json;

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

#[tokio::test]
async fn test_get_package_by_id_returns_200() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();
    let (registry, _) = app.registry_queue()?;
    let package = app.mock_create_package(&registry).await?;

    // Act
    let url = format!("{}/packages/{}", app.address, package.id);
    let response = client.get(url).send().await?;

    // Assert
    assert_eq!(response.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
async fn test_get_package_by_id_returns_200_with_package_object() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();
    let (registry, _) = app.registry_queue()?;
    let package = app.mock_create_package(&registry).await?;

    // Act
    let url = format!("{}/packages/{}", app.address, package.id);
    let response = client.get(url).send().await?;

    // Assert
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.json::<serde_json::Value>().await?;
    assert_eq!(body["data"]["id"], package.id.to_string());
    assert_eq!(body["data"]["registry"], package.registry);
    assert_eq!(body["data"]["name"], package.name);
    assert_eq!(body["data"]["version"], package.version);
    assert_eq!(body["data"]["downloads"], package.downloads);

    Ok(())
}

#[tokio::test]
async fn test_get_package_by_id_returns_404_if_package_does_not_exist() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();
    let non_existent_id = "00000000-0000-0000-0000-000000000000";

    // Act
    let url = format!("{}/packages/{}", app.address, non_existent_id);
    let response = client.get(url).send().await?;

    // Assert
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response.json::<serde_json::Value>().await?;
    assert_eq!(
        body["message"],
        format!("Package with id {} not found", non_existent_id)
    );

    Ok(())
}

#[tokio::test]
async fn test_get_package_by_id_returns_400_if_id_is_invalid() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();
    let invalid_id = "invalid-uuid";

    // Act
    let url = format!("{}/packages/{}", app.address, invalid_id);
    let response = client.get(url).send().await?;

    // Assert
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.json::<serde_json::Value>().await?;
    assert_eq!(body["message"], "Invalid package ID");

    Ok(())
}
