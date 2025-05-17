use crate::helpers::spawn_app;

#[tokio::test]
async fn health_check_works() {
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
