pub mod health;
pub mod jobs;
pub mod openapi;

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use http::StatusCode;
    use uuid::Uuid;

    use crate::helpers::spawn_app;

    #[tokio::test]
    async fn test_unknown_route_returns_404() -> Result<()> {
        // Arrange
        let app = spawn_app().await?;
        let client = reqwest::Client::new();
        let random_path = Uuid::new_v4().to_string();

        // Act
        let url = format!("{}/{}", &app.address, random_path);
        let response = client.get(url).send().await?;

        // Assert
        assert_eq!(response.status().as_u16(), 404);
        assert_eq!(Some(0), response.content_length());

        Ok(())
    }

    #[tokio::test]
    async fn test_health_check_returns_204_with_no_content() {
        // Arrange
        let app = spawn_app().await.expect("Failed to spawn app");
        let client = reqwest::Client::new();

        // Act
        let response = client
            .get(format!("{}/health", &app.address))
            .send()
            .await
            .expect("Failed to execute request.");

        // Assert
        assert_eq!(response.status().as_u16(), 204);
        assert_eq!(Some(0), response.content_length());
    }

    #[tokio::test]
    async fn test_metrics_returns_200() -> Result<()> {
        // Arrange
        let app = spawn_app().await?;
        let client = reqwest::Client::new();

        // Act
        let url = format!("{}/metrics", app.address);
        let response = client.get(url).send().await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);

        Ok(())
    }
}
