use anyhow::Result;
use sqlx::PgConnection;
use tracing::{instrument, Instrument};
use uuid::Uuid;

use crate::{
    models::package::Package,
    telemetry::{instrument_query, Operation},
};

use super::Order;

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

#[instrument(name = "get_packages", skip(conn))]
pub async fn get_packages(
    conn: &mut PgConnection,
    limit: u64,
    after: Option<Uuid>,
    order: Order,
) -> Result<Vec<Package>> {
    match after {
        Some(after) => get_packages_with_limit_after(conn, limit, after, order).await,
        None => get_packages_with_limit(conn, limit, order).await,
    }
}

async fn get_packages_with_limit(
    conn: &mut PgConnection,
    limit: u64,
    order: Order,
) -> Result<Vec<Package>> {
    match order {
        Order::Asc => get_packages_with_limit_asc(conn, limit).await,
        Order::Desc => get_packages_with_limit_desc(conn, limit).await,
    }
}

async fn get_packages_with_limit_asc(conn: &mut PgConnection, limit: u64) -> Result<Vec<Package>> {
    let packages = sqlx::query_as!(
        Package,
        "SELECT * FROM packages ORDER BY id ASC LIMIT $1;",
        limit as i64
    )
    .fetch_all(&mut *conn)
    .instrument(instrument_query(Operation::Select, "packages"))
    .await?;

    Ok(packages)
}

async fn get_packages_with_limit_desc(conn: &mut PgConnection, limit: u64) -> Result<Vec<Package>> {
    let packages = sqlx::query_as!(
        Package,
        "SELECT * FROM packages ORDER BY id DESC LIMIT $1;",
        limit as i64
    )
    .fetch_all(&mut *conn)
    .instrument(instrument_query(Operation::Select, "packages"))
    .await?;

    Ok(packages)
}

async fn get_packages_with_limit_after(
    conn: &mut PgConnection,
    limit: u64,
    after: Uuid,
    order: Order,
) -> Result<Vec<Package>> {
    match order {
        Order::Asc => get_packages_with_limit_after_asc(conn, limit, after).await,
        Order::Desc => get_packages_with_limit_after_desc(conn, limit, after).await,
    }
}

async fn get_packages_with_limit_after_asc(
    conn: &mut PgConnection,
    limit: u64,
    after: Uuid,
) -> Result<Vec<Package>> {
    let packages = sqlx::query_as!(
        Package,
        "SELECT * FROM packages WHERE id > $1 ORDER BY id ASC LIMIT $2;",
        after,
        limit as i64
    )
    .fetch_all(&mut *conn)
    .instrument(instrument_query(Operation::Select, "packages"))
    .await?;

    Ok(packages)
}

async fn get_packages_with_limit_after_desc(
    conn: &mut PgConnection,
    limit: u64,
    after: Uuid,
) -> Result<Vec<Package>> {
    let packages = sqlx::query_as!(
        Package,
        "SELECT * FROM packages WHERE id < $1 ORDER BY id DESC LIMIT $2;",
        after,
        limit as i64
    )
    .fetch_all(&mut *conn)
    .instrument(instrument_query(Operation::Select, "packages"))
    .await?;

    Ok(packages)
}

#[instrument(name = "get_package_by_id", skip(conn))]
pub async fn get_package_by_id(conn: &mut PgConnection, id: Uuid) -> Result<Option<Package>> {
    let package = sqlx::query_as!(
        Package,
        "SELECT * FROM packages WHERE id = $1;",
        id
    )
    .fetch_optional(&mut *conn)
    .instrument(instrument_query(Operation::Select, "packages"))
    .await?;

    Ok(package)
}
