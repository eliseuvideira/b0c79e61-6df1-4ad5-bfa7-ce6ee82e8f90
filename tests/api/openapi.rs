use crate::helpers::spawn_app;

#[tokio::test]
async fn test_openapi_returns_200() {
    // Arrange
    let app = spawn_app().await.expect("Failed to spawn app.");
    let client = reqwest::Client::new();

    // Act
    let response = client
        .get(format!("{}/openapi", &app.address))
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn test_index_returns_200() {
    // Arrange
    let app = spawn_app().await.expect("Failed to spawn app.");
    let client = reqwest::Client::new();

    // Act
    let response = client
        .get(&app.address)
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert_eq!(response.status().as_u16(), 200);
}
