use anyhow::Result;
use fake::Fake;
use lapin::options::BasicGetOptions;
use reqwest::StatusCode;
use serde_json::json;

use crate::helpers::spawn_app;

#[tokio::test]
async fn test_create_job() -> Result<()> {
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
        .post(format!("{}/jobs", app.address))
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
async fn test_create_job_inserts_into_db() -> Result<()> {
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
        .post(format!("{}/jobs", app.address))
        .json(&json!({
            "registry_name": registry_name,
            "package_name": "serde",
        }))
        .send()
        .await?;

    // Assert
    let db_pool = &app.db_pool;
    let rows = sqlx::query!("SELECT * FROM jobs;")
        .fetch_all(db_pool)
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].registry_name, *registry_name);
    assert_eq!(rows[0].package_name, "serde");

    Ok(())
}

#[tokio::test]
async fn test_create_job_returns_job_object() -> Result<()> {
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
        .post(format!("{}/jobs", app.address))
        .json(&json!({
            "registry_name": registry_name,
            "package_name": "serde",
        }))
        .send()
        .await?;

    // Assert
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = response.json::<serde_json::Value>().await?;
    assert_eq!(body["data"]["id"].is_string(), true);
    assert_eq!(body["data"]["registry_name"], *registry_name);
    assert_eq!(body["data"]["package_name"], "serde");

    Ok(())
}

#[tokio::test]
async fn test_create_job_publishes_to_rabbitmq() -> Result<()> {
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
        .post(format!("{}/jobs", app.address))
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

#[tokio::test]
async fn test_get_jobs_returns_200() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();

    // Act
    let response = client.get(format!("{}/jobs", app.address)).send().await?;

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
    let response = client.get(format!("{}/jobs", app.address)).send().await?;
    let body: serde_json::Value = response.json().await?;

    // Assert
    assert_eq!(body.get("data").is_some(), true);
    assert!(body["next_cursor"].is_null());

    Ok(())
}

#[tokio::test]
async fn test_get_jobs_returns_paginated_jobs_with_limit_for_asc_order() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();

    let (registry_name, _) = app
        .integration_queues
        .iter()
        .next()
        .expect("No registry name");

    let mut job_ids = Vec::new();

    for _ in 0..5 {
        let package_name: String = fake::faker::name::en::Name().fake();
        let response = client
            .post(format!("{}/jobs", app.address))
            .json(&json!({
                "registry_name": registry_name,
                "package_name": package_name,
            }))
            .send()
            .await?;

        let body: serde_json::Value = response.json().await?;
        job_ids.push(
            body["data"]["id"]
                .as_str()
                .ok_or(anyhow::anyhow!("Job ID is not a string"))?
                .to_string(),
        );
    }

    // Act
    let response = client
        .get(format!("{}/jobs?limit=4&order=asc", app.address))
        .send()
        .await?;

    // Assert
    let body: serde_json::Value = response.json().await?;
    assert_eq!(body.get("data").is_some(), true);
    assert_eq!(body["data"][0]["id"], job_ids[0]);
    assert_eq!(body["data"][1]["id"], job_ids[1]);
    assert_eq!(body["data"][2]["id"], job_ids[2]);
    assert_eq!(body["data"][3]["id"], job_ids[3]);
    assert_eq!(body["next_cursor"], job_ids[4]);

    Ok(())
}

#[tokio::test]
async fn test_get_jobs_returns_paginated_jobs_with_limit_for_desc_order() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();

    let (registry_name, _) = app
        .integration_queues
        .iter()
        .next()
        .expect("No registry name");

    let mut job_ids = Vec::new();

    for _ in 0..5 {
        let package_name: String = fake::faker::name::en::Name().fake();
        let response = client
            .post(format!("{}/jobs", app.address))
            .json(&json!({
                "registry_name": registry_name,
                "package_name": package_name,
            }))
            .send()
            .await?;

        let body: serde_json::Value = response.json().await?;
        job_ids.push(
            body["data"]["id"]
                .as_str()
                .ok_or(anyhow::anyhow!("Job ID is not a string"))?
                .to_string(),
        );
    }

    // Act
    let response = client
        .get(format!("{}/jobs?limit=4&order=desc", app.address))
        .send()
        .await?;

    // Assert
    let body: serde_json::Value = response.json().await?;
    assert_eq!(body.get("data").is_some(), true);
    assert_eq!(body["data"][0]["id"], job_ids[4]);
    assert_eq!(body["data"][1]["id"], job_ids[3]);
    assert_eq!(body["data"][2]["id"], job_ids[2]);
    assert_eq!(body["data"][3]["id"], job_ids[1]);
    assert_eq!(body["next_cursor"], job_ids[0]);

    Ok(())
}

#[tokio::test]
async fn test_get_jobs_paginates_properly() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();

    let (registry_name, _) = app
        .integration_queues
        .iter()
        .next()
        .expect("No registry name");

    let mut job_ids = Vec::new();

    for _ in 0..10 {
        let package_name: String = fake::faker::name::en::Name().fake();
        let response = client
            .post(format!("{}/jobs", app.address))
            .json(&json!({
                "registry_name": registry_name,
                "package_name": package_name,
            }))
            .send()
            .await?;

        let body: serde_json::Value = response.json().await?;
        job_ids.push(
            body["data"]["id"]
                .as_str()
                .ok_or(anyhow::anyhow!("Job ID is not a string"))?
                .to_string(),
        );
    }

    // Act
    let response = client
        .get(format!("{}/jobs?limit=5&order=desc", app.address))
        .send()
        .await?;

    // Assert
    let body: serde_json::Value = response.json().await?;
    assert_eq!(body.get("data").is_some(), true);
    assert_eq!(body["next_cursor"], job_ids[4]);

    // Act
    let response = client
        .get(format!(
            "{}/jobs?limit=5&order=desc&cursor={}",
            app.address, job_ids[4]
        ))
        .send()
        .await?;
    let body: serde_json::Value = response.json().await?;
    assert_eq!(body.get("data").is_some(), true);
    assert_eq!(body["data"][0]["id"], job_ids[4]);
    assert_eq!(body["data"][1]["id"], job_ids[3]);
    assert_eq!(body["data"][2]["id"], job_ids[2]);
    assert_eq!(body["data"][3]["id"], job_ids[1]);
    assert_eq!(body["data"][4]["id"], job_ids[0]);
    assert!(body["next_cursor"].is_null());

    Ok(())
}

#[tokio::test]
async fn test_get_job_by_id_returns_200() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let client = reqwest::Client::new();

    let (registry_name, _) = app
        .integration_queues
        .iter()
        .next()
        .expect("No registry name");

    let package_name: String = fake::faker::name::en::Name().fake();
    let response = client
        .post(format!("{}/jobs", app.address))
        .json(&json!({
            "registry_name": registry_name,
            "package_name": package_name,
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;
    let job_id = body["data"]["id"].as_str().unwrap();

    // Act
    let response = client
        .get(format!("{}/jobs/{}", app.address, job_id))
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

    let (registry_name, _) = app
        .integration_queues
        .iter()
        .next()
        .expect("No registry name");

    let package_name: String = fake::faker::name::en::Name().fake();
    let response = client
        .post(format!("{}/jobs", app.address))
        .json(&json!({
            "registry_name": registry_name,
            "package_name": package_name,
        }))
        .send()
        .await?;
    let body: serde_json::Value = response.json().await?;
    let job_id = body["data"]["id"].as_str().unwrap();

    // Act
    let response = client
        .get(format!("{}/jobs/{}", app.address, job_id))
        .send()
        .await?;

    // Assert
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await?;
    assert_eq!(body["data"]["id"], job_id);

    Ok(())
}
