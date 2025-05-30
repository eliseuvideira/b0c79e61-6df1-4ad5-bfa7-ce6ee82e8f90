use anyhow::Result;
use sqlx::PgConnection;
use tracing::{instrument, Instrument};

use crate::{
    models::package::Package,
    telemetry::{instrument_query, Operation},
};

#[instrument(name = "insert_package", skip(conn))]
pub async fn insert_package(conn: &mut PgConnection, package: Package) -> Result<Package> {
    let package = sqlx::query_as!(
        Package,
        r#"INSERT INTO packages (id, registry, name, version, downloads) VALUES ($1, $2, $3, $4, $5) RETURNING *;"#,
        package.id,
        package.registry,
        package.name,
        package.version,
        package.downloads,
    )
    .fetch_one(&mut *conn)
    .instrument(instrument_query(Operation::Insert, "packages"))
    .await?;

    Ok(package)
}

#[instrument(name = "update_package", skip(conn))]
pub async fn update_package(conn: &mut PgConnection, package: Package) -> Result<Package> {
    let package = sqlx::query_as!(
        Package,
        r#"UPDATE packages SET registry = $1, name = $2, version = $3, downloads = $4 WHERE id = $5 RETURNING *;"#,
        package.registry,
        package.name,
        package.version,
        package.downloads,
        package.id,
    )
    .fetch_one(&mut *conn)
    .instrument(instrument_query(Operation::Update, "packages"))
    .await?;

    Ok(package)
}

#[instrument(name = "upsert_package", skip(conn))]
pub async fn upsert_package(conn: &mut PgConnection, package: Package) -> Result<Package> {
    let package = sqlx::query_as!(
        Package,
        r#"INSERT INTO packages (id, registry, name, version, downloads) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (registry, name) DO UPDATE SET version = $4, downloads = $5 RETURNING *;"#,
        package.id,
        package.registry,
        package.name,
        package.version,
        package.downloads,
    )
    .fetch_one(&mut *conn)
    .instrument(instrument_query(Operation::Insert, "packages"))
    .await?;

    Ok(package)
}
