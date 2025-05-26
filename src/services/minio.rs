use anyhow::Result;
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{error::SdkError, operation::head_bucket::HeadBucketError, Client};
use tracing::instrument;

use crate::config::MinioConfig;

#[instrument(name = "create_client", skip_all)]
pub async fn create_client(settings: &MinioConfig) -> Result<Client> {
    let credentials = settings.credentials();

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

#[instrument(name = "list_buckets", skip_all)]
pub async fn list_buckets(client: &Client) -> Result<Vec<String>> {
    let response = client.list_buckets().send().await?;
    let buckets = response.buckets().to_owned();
    let bucket_names = buckets
        .into_iter()
        .filter_map(|bucket| bucket.name().map(|name| name.to_owned()))
        .collect();
    Ok(bucket_names)
}

#[instrument(name = "create_bucket", skip_all, fields(bucket_name = %bucket_name))]
pub async fn create_bucket(client: &Client, bucket_name: &str) -> Result<()> {
    client.create_bucket().bucket(bucket_name).send().await?;

    Ok(())
}

#[instrument(name = "ensure_bucket", skip_all, fields(bucket_name = %bucket_name))]
pub async fn ensure_bucket(client: &Client, bucket_name: &str) -> Result<()> {
    match client.head_bucket().bucket(bucket_name).send().await {
        Ok(_) => Ok(()),
        Err(SdkError::ServiceError(service_error))
            if matches!(service_error.err(), HeadBucketError::NotFound(_)) =>
        {
            create_bucket(client, bucket_name).await
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use anyhow::Result;
    use claims::assert_ok;
    use uuid::Uuid;

    fn setup_tests() -> Result<Config> {
        dotenvy::dotenv().ok();

        let config = Config::build()?;

        Ok(config)
    }

    #[tokio::test]
    async fn test_list_buckets() -> Result<()> {
        // Arrange
        let config = setup_tests()?;
        let client = create_client(&config.minio).await?;

        // Act
        let buckets = list_buckets(&client).await;

        // Assert
        assert_ok!(buckets);

        Ok(())
    }

    #[tokio::test]
    async fn test_create_bucket() -> Result<()> {
        // Arrange
        let config = setup_tests()?;
        let client = create_client(&config.minio).await?;
        let bucket_name = Uuid::new_v4().to_string();

        // Act
        create_bucket(&client, &bucket_name).await?;

        // Assert
        let buckets = list_buckets(&client).await?;
        assert!(buckets.contains(&bucket_name));

        Ok(())
    }

    #[tokio::test]
    async fn test_ensure_bucket_creates_bucket() -> Result<()> {
        // Arrange
        let config = setup_tests()?;
        let client = create_client(&config.minio).await?;
        let bucket_name = Uuid::new_v4().to_string();

        // Act
        ensure_bucket(&client, &bucket_name).await?;

        // Assert
        let buckets = list_buckets(&client).await?;
        assert!(buckets.contains(&bucket_name));

        Ok(())
    }

    #[tokio::test]
    async fn test_ensure_bucket_does_not_create_bucket_if_it_already_exists() -> Result<()> {
        // Arrange
        let config = setup_tests()?;
        let client = create_client(&config.minio).await?;
        let bucket_name = Uuid::new_v4().to_string();
        create_bucket(&client, &bucket_name).await?;

        // Act
        let result = ensure_bucket(&client, &bucket_name).await;

        // Assert
        assert_ok!(result);

        Ok(())
    }

    #[tokio::test]
    async fn test_ensure_bucket_is_safe_to_call_multiple_times() -> Result<()> {
        // Arrange
        let config = setup_tests()?;
        let client = create_client(&config.minio).await?;
        let bucket_name = Uuid::new_v4().to_string();
        create_bucket(&client, &bucket_name).await?;

        // Act
        ensure_bucket(&client, &bucket_name).await?;

        // Assert
        let buckets = list_buckets(&client).await?;
        assert!(buckets.contains(&bucket_name));

        Ok(())
    }
}
