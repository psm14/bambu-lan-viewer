use crate::config::PrinterConfig;
use anyhow::Context;
use serde::Deserialize;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterCreateRequest {
    pub name: String,
    pub host: String,
    pub serial: String,
    pub access_code: String,
    pub rtsp_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterUpdateRequest {
    pub name: Option<String>,
    pub host: Option<String>,
    pub serial: Option<String>,
    pub access_code: Option<String>,
    pub rtsp_url: Option<String>,
}

pub async fn init(database_url: &str) -> anyhow::Result<SqlitePool> {
    ensure_parent_dir(database_url)?;
    let pool = SqlitePool::connect(database_url).await?;
    sqlx::query("PRAGMA journal_mode = WAL;")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA foreign_keys = ON;")
        .execute(&pool)
        .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS printers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            host TEXT NOT NULL,
            serial TEXT NOT NULL UNIQUE,
            access_code TEXT NOT NULL,
            rtsp_url TEXT
        )
        "#,
    )
    .execute(&pool)
    .await?;
    Ok(pool)
}

pub async fn list_printers(pool: &SqlitePool) -> anyhow::Result<Vec<PrinterConfig>> {
    let rows = sqlx::query(
        r#"
        SELECT id, name, host, serial, access_code, rtsp_url
        FROM printers
        ORDER BY name COLLATE NOCASE, id
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_printer).collect())
}

pub async fn get_printer(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<PrinterConfig>> {
    let row = sqlx::query(
        r#"
        SELECT id, name, host, serial, access_code, rtsp_url
        FROM printers
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_printer))
}

pub async fn create_printer(
    pool: &SqlitePool,
    payload: PrinterCreateRequest,
) -> anyhow::Result<PrinterConfig> {
    let name = payload.name.trim().to_string();
    let host = payload.host.trim().to_string();
    let serial = payload.serial.trim().to_string();
    let access_code = payload.access_code.trim().to_string();
    let rtsp_url = normalize_optional(payload.rtsp_url);

    validate_printer_fields(&name, &host, &serial, &access_code)?;
    let result = sqlx::query(
        r#"
        INSERT INTO printers (name, host, serial, access_code, rtsp_url)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(name)
    .bind(host)
    .bind(serial)
    .bind(access_code)
    .bind(rtsp_url)
    .execute(pool)
    .await
    .context("insert printer")?;
    let id = result.last_insert_rowid();
    get_printer(pool, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("printer insert failed"))
}

pub async fn update_printer(
    pool: &SqlitePool,
    id: i64,
    payload: PrinterUpdateRequest,
) -> anyhow::Result<Option<PrinterConfig>> {
    let existing = get_printer(pool, id).await?;
    let Some(existing) = existing else {
        return Ok(None);
    };

    let name = payload.name.unwrap_or(existing.name).trim().to_string();
    let host = payload.host.unwrap_or(existing.host).trim().to_string();
    let serial = payload.serial.unwrap_or(existing.serial).trim().to_string();
    let access_code = payload
        .access_code
        .unwrap_or(existing.access_code)
        .trim()
        .to_string();
    let rtsp_url = match payload.rtsp_url {
        Some(value) => normalize_optional(Some(value)),
        None => existing.rtsp_url,
    };

    validate_printer_fields(&name, &host, &serial, &access_code)?;

    sqlx::query(
        r#"
        UPDATE printers
        SET name = ?, host = ?, serial = ?, access_code = ?, rtsp_url = ?
        WHERE id = ?
        "#,
    )
    .bind(&name)
    .bind(&host)
    .bind(&serial)
    .bind(&access_code)
    .bind(&rtsp_url)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(Some(PrinterConfig {
        id,
        name,
        host,
        serial,
        access_code,
        rtsp_url,
    }))
}

pub async fn delete_printer(pool: &SqlitePool, id: i64) -> anyhow::Result<bool> {
    let result = sqlx::query("DELETE FROM printers WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

fn validate_printer_fields(
    name: &str,
    host: &str,
    serial: &str,
    access_code: &str,
) -> anyhow::Result<()> {
    if name.trim().is_empty() {
        return Err(anyhow::anyhow!("printer name is required"));
    }
    if host.trim().is_empty() {
        return Err(anyhow::anyhow!("printer host is required"));
    }
    if serial.trim().is_empty() {
        return Err(anyhow::anyhow!("printer serial is required"));
    }
    if access_code.trim().is_empty() {
        return Err(anyhow::anyhow!("printer access code is required"));
    }
    Ok(())
}

fn row_to_printer(row: SqliteRow) -> PrinterConfig {
    PrinterConfig {
        id: row.get("id"),
        name: row.get("name"),
        host: row.get("host"),
        serial: row.get("serial"),
        access_code: row.get("access_code"),
        rtsp_url: row.get("rtsp_url"),
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    let trimmed = value?.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn ensure_parent_dir(database_url: &str) -> anyhow::Result<()> {
    let Some(path) = sqlite_path_from_url(database_url) else {
        return Ok(());
    };
    if path == Path::new(":memory:") {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create db dir {:?}", parent))?;
        }
    }
    if !path.exists() {
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .with_context(|| format!("create db file {:?}", path))?;
    }
    Ok(())
}

fn sqlite_path_from_url(database_url: &str) -> Option<PathBuf> {
    database_url
        .strip_prefix("sqlite://")
        .or_else(|| database_url.strip_prefix("sqlite:"))
        .map(|path| PathBuf::from(strip_query(path)))
}

fn strip_query(value: &str) -> &str {
    value.split('?').next().unwrap_or(value)
}
