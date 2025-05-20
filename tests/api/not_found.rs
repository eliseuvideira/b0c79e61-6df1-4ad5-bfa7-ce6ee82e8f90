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
    assert_eq!(Some(0), response.content_length());
}
