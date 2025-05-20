use anyhow::Result;
use reqwest::StatusCode;
use serde_json::json;

use crate::helpers::spawn_app;

#[tokio::test]
async fn test_create_scrapper_job() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();

    // Act
    let response = client
        .post(format!("{}/scrapper-jobs", app.address))
        .json(&json!({
            "registry_name": "crates.io",
            "package_name": "serde",
        }))
        .send()
        .await?;

    // Assert
    assert_eq!(response.status(), StatusCode::CREATED);

    Ok(())
}

#[tokio::test]
async fn test_create_scrapper_inserts_into_db() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();

    // Act
    client
        .post(format!("{}/scrapper-jobs", app.address))
        .json(&json!({
            "registry_name": "crates.io",
            "package_name": "serde",
        }))
        .send()
        .await?;

    // Assert
    let db_pool = &app.db_pool;
    let rows = sqlx::query!("SELECT * FROM scrapper_jobs;")
        .fetch_all(db_pool)
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].registry_name, "crates.io");
    assert_eq!(rows[0].package_name, "serde");

    Ok(())
}

#[tokio::test]
async fn test_create_scrapper_job_returns_scrapper_job_object() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();

    // Act
    let response = client
        .post(format!("{}/scrapper-jobs", app.address))
        .json(&json!({
            "registry_name": "crates.io",
            "package_name": "serde",
        }))
        .send()
        .await?;

    // Assert
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = response.json::<serde_json::Value>().await?;
    assert_eq!(body["id"].is_string(), true);
    assert_eq!(body["registry_name"], "crates.io");
    assert_eq!(body["package_name"], "serde");

    Ok(())
}
