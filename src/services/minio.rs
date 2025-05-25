use anyhow::Result;
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{config::Credentials, Client};
use tracing::instrument;

use crate::config::MinioSettings;

#[instrument(name = "create_client", skip_all)]
pub async fn create_client(settings: &MinioSettings) -> Result<Client> {
    let credentials = Credentials::new(
        settings.username.clone(),
        settings.password.clone(),
        None,
        None,
        "minio0",
    );

    let config = aws_config::defaults(BehaviorVersion::latest())
        .credentials_provider(credentials)
        .region(Region::new("us-east-1"))
        .load()
        .await;

    let s3_config = aws_sdk_s3::config::Builder::from(&config)
        .endpoint_url(&settings.url)
        .build();

    Ok(Client::from_conf(s3_config))
}
