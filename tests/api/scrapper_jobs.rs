use anyhow::Result;
use lapin::options::BasicGetOptions;
use reqwest::StatusCode;
use serde_json::json;

use crate::helpers::spawn_app;

#[tokio::test]
async fn test_create_scrapper_job() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();

    let (registry_name, _) = app
        .integration_queues
        .iter()
        .next()
        .expect("No registry name");

    // Act
    let response = client
        .post(format!("{}/scrapper-jobs", app.address))
        .json(&json!({
            "registry_name": registry_name,
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
    let (registry_name, _) = app
        .integration_queues
        .iter()
        .next()
        .expect("No registry name");

    // Act
    client
        .post(format!("{}/scrapper-jobs", app.address))
        .json(&json!({
            "registry_name": registry_name,
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
    assert_eq!(rows[0].registry_name, *registry_name);
    assert_eq!(rows[0].package_name, "serde");

    Ok(())
}

#[tokio::test]
async fn test_create_scrapper_job_returns_scrapper_job_object() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();
    let (registry_name, _) = app
        .integration_queues
        .iter()
        .next()
        .expect("No registry name");

    // Act
    let response = client
        .post(format!("{}/scrapper-jobs", app.address))
        .json(&json!({
            "registry_name": registry_name,
            "package_name": "serde",
        }))
        .send()
        .await?;

    // Assert
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = response.json::<serde_json::Value>().await?;
    assert_eq!(body["id"].is_string(), true);
    assert_eq!(body["registry_name"], *registry_name);
    assert_eq!(body["package_name"], "serde");

    Ok(())
}

#[tokio::test]
async fn test_create_scrapper_job_publishes_to_rabbitmq() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();

    let (registry_name, queue_name) = app
        .integration_queues
        .iter()
        .next()
        .expect("No registry name");

    // Act
    client
        .post(format!("{}/scrapper-jobs", app.address))
        .json(&json!({
            "registry_name": registry_name,
            "package_name": "serde",
        }))
        .send()
        .await?;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Assert
    let channel = &app.channel;
    let message = channel
        .basic_get(&queue_name, BasicGetOptions::default())
        .await?;
    assert_eq!(message.is_some(), true);

    if let Some(delivery) = message {
        let payload = serde_json::from_slice::<serde_json::Value>(&delivery.data)?;
        assert_eq!(payload["package_name"], "serde");
    }

    Ok(())
}
