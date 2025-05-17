use uuid::Uuid;

use crate::helpers::spawn_app;

#[tokio::test]
async fn not_found_returns_404() {
    // Arrange
    let app = spawn_app().await.expect("Failed to spawn app.");
    let client = reqwest::Client::new();
    let random_path = Uuid::new_v4().to_string();
    let url = format!("{}/{}", &app.address, random_path);

    // Act
    let response = client
        .get(url)
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert_eq!(response.status().as_u16(), 404);
}

#[tokio::test]
async fn not_found_returns_json_error() {
    // Arrange
    let app = spawn_app().await.expect("Failed to spawn app");
    let client = reqwest::Client::new();
    let random_path = Uuid::new_v4();
    let url = format!("{}/{}", &app.address, random_path.to_string());

    // Act
    let response = client
        .get(url)
        .send()
        .await
        .expect("Failed to execute request");

    // Assert
    let body = response
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse response");
    assert_eq!(body["message"], "Not Found");
}
