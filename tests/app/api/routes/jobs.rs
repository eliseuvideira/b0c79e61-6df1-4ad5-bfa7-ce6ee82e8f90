use anyhow::{Context, Result};
use fake::{faker::name::en::Name, Fake};
use lapin::options::BasicGetOptions;
use reqwest::StatusCode;
use serde_json::json;

use crate::helpers::spawn_app;

#[tokio::test]
async fn test_create_job() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();
    let (registry, _) = app.registry_queue()?;

    // Act
    let url = format!("{}/jobs", app.address);
    let response = client
        .post(url)
        .json(&json!({
            "registry": registry,
            "package_name": "serde",
        }))
        .send()
        .await?;

    // Assert
    assert_eq!(response.status(), StatusCode::CREATED);

    Ok(())
}

#[tokio::test]
async fn test_create_job_inserts_into_db() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();
    let (registry, _) = app.registry_queue()?;
    let package_name: String = Name().fake();

    // Act
    let url = format!("{}/jobs", app.address);
    client
        .post(url)
        .json(&json!({
            "registry": registry,
            "package_name": package_name,
        }))
        .send()
        .await?;

    // Assert
    let rows = sqlx::query!("SELECT * FROM jobs;")
        .fetch_all(&app.db_pool)
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].registry, registry);
    assert_eq!(rows[0].package_name, package_name);

    Ok(())
}

#[tokio::test]
async fn test_create_job_returns_job_object() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();
    let (registry, _) = app.registry_queue()?;

    // Act
    let url = format!("{}/jobs", app.address);
    let response = client
        .post(url)
        .json(&json!({
            "registry": registry,
            "package_name": "serde",
        }))
        .send()
        .await?;

    // Assert
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.json::<serde_json::Value>().await?;
    assert_eq!(body["data"]["id"].is_string(), true);
    assert_eq!(body["data"]["registry"], *registry);
    assert_eq!(body["data"]["package_name"], "serde");

    Ok(())
}

#[tokio::test]
async fn test_create_job_publishes_to_rabbitmq() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();
    let (registry, queue_name) = app.registry_queue()?;

    // Act
    let url = format!("{}/jobs", app.address);
    client
        .post(url)
        .json(&json!({
            "registry": registry,
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
    let Some(delivery) = message else {
        return Err(anyhow::anyhow!("No message received"));
    };
    let payload = serde_json::from_slice::<serde_json::Value>(&delivery.data)?;
    assert_eq!(payload["package_name"], "serde");

    Ok(())
}

#[tokio::test]
async fn test_get_jobs_returns_200() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();

    // Act
    let url = format!("{}/jobs", app.address);
    let response = client.get(url).send().await?;

    // Assert
    assert_eq!(response.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
async fn test_get_jobs_returns_paginated_jobs() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();

    // Act
    let url = format!("{}/jobs", app.address);
    let response = client.get(url).send().await?;
    let response_body: serde_json::Value = response.json().await?;

    // Assert
    assert_eq!(response_body.get("data").is_some(), true);
    assert!(response_body["next_cursor"].is_null());

    Ok(())
}

#[tokio::test]
async fn test_get_jobs_returns_paginated_jobs_with_limit_for_asc_order() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();
    let (registry, _) = app.registry_queue()?;

    let jobs = app.mock_create_jobs(&client, &registry, 20).await?;
    let job_ids: Vec<String> = jobs.iter().map(|job| job.data.id.to_string()).collect();

    // Act
    let url = format!("{}/jobs?limit=10&order=asc", app.address);
    let response = client.get(url).send().await?;

    // Assert
    let body: serde_json::Value = response.json().await?;
    assert_eq!(body.get("data").is_some(), true);
    for i in 0..10 {
        assert_eq!(body["data"][i]["id"], job_ids[i]);
    }
    assert_eq!(body["next_cursor"], job_ids[9]);

    Ok(())
}

#[tokio::test]
async fn test_get_jobs_returns_paginated_jobs_with_limit_for_desc_order() -> Result<()> {
    // Arrange
    const LIMIT: usize = 10;
    const COUNT: usize = 15;
    let app = spawn_app().await?;
    let client = reqwest::Client::new();
    let (registry, _) = app.registry_queue()?;
    let jobs = app
        .mock_create_jobs(&client, &registry, COUNT as u64)
        .await?;
    let job_ids: Vec<String> = jobs.iter().map(|job| job.data.id.to_string()).collect();

    // Act
    let response = client
        .get(format!("{}/jobs?limit={}&order=desc", app.address, LIMIT))
        .send()
        .await?;

    // Assert
    let body: serde_json::Value = response.json().await?;
    assert_eq!(body.get("data").is_some(), true);
    for i in 0..LIMIT {
        assert_eq!(body["data"][i]["id"], job_ids[COUNT - i - 1]);
    }
    assert_eq!(body["next_cursor"], job_ids[COUNT - LIMIT]);

    Ok(())
}

#[tokio::test]
async fn test_get_jobs_paginates_properly_with_cursor_for_desc_order() -> Result<()> {
    // Arrange
    const LIMIT: usize = 10;
    const COUNT: usize = 50;
    let app = spawn_app().await?;
    let client = reqwest::Client::new();
    let (registry, _) = app.registry_queue()?;
    let jobs = app
        .mock_create_jobs(&client, &registry, COUNT as u64)
        .await?;
    let job_ids: Vec<String> = jobs.iter().map(|job| job.data.id.to_string()).collect();

    let pages = COUNT / LIMIT;
    let mut after = None;
    for page in 0..pages {
        // Act
        let url = match &after {
            Some(after) => format!(
                "{}/jobs?limit={}&order=desc&after={}",
                app.address, LIMIT, after
            ),
            None => format!("{}/jobs?limit={}&order=desc", app.address, LIMIT),
        };
        let response = client.get(url).send().await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = response.json().await?;
        if page < pages - 1 {
            assert_eq!(body["next_cursor"], job_ids[COUNT - (LIMIT * (page + 1))]);
        } else {
            assert!(body["next_cursor"].is_null());
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
async fn test_get_job_by_id_returns_200() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();
    let (registry, _) = app.registry_queue()?;
    let job = app.mock_create_job(&client, &registry).await?;

    // Act
    let response = client
        .get(format!("{}/jobs/{}", app.address, job.data.id))
        .send()
        .await?;

    // Assert
    assert_eq!(response.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
async fn test_get_job_by_id_returns_200_with_job_object() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();
    let (registry, _) = app.registry_queue()?;
    let job = app.mock_create_job(&client, &registry).await?;

    // Act
    let response = client
        .get(format!("{}/jobs/{}", app.address, job.data.id))
        .send()
        .await?;

    // Assert
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await?;
    assert_eq!(body["data"]["id"], job.data.id.to_string());

    Ok(())
}
