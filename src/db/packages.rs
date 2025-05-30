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
    if sqlx::query_as!(
        Package,
        r#"SELECT * FROM packages WHERE registry = $1 AND name = $2 FOR UPDATE;"#,
        package.registry,
        package.name,
    )
    .fetch_optional(&mut *conn)
    .instrument(instrument_query(Operation::Select, "packages"))
    .await?
    .is_some()
    {
        update_package(conn, package).await
    } else {
        insert_package(conn, package).await
    }
}
