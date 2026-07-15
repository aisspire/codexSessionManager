use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use walkdir::WalkDir;

const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedInstance {
    pub id: i64,
    pub path: String,
    pub display_name: Option<String>,
    pub available: bool,
    pub added_at_unix: i64,
    pub last_seen_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InstanceScanReport {
    pub added: usize,
    pub reactivated: usize,
    pub already_managed: usize,
    pub skipped: usize,
}

#[derive(Debug)]
struct StoredManagedInstance {
    id: i64,
    path: String,
    display_name: Option<String>,
    added_at_unix: i64,
    last_seen_at_unix: i64,
}

pub fn scan_and_register(database_path: &Path, parent_path: &Path) -> Result<InstanceScanReport> {
    if !parent_path.is_dir() {
        bail!(
            "instance scan path is not a directory: {}",
            parent_path.display()
        );
    }

    let parent_path = fs::canonicalize(parent_path).with_context(|| {
        format!(
            "failed to resolve instance scan directory {}",
            parent_path.display()
        )
    })?;
    let mut connection = open_registry(database_path)?;
    let transaction = connection
        .transaction()
        .context("failed to start managed instance registry transaction")?;
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let mut report = InstanceScanReport::default();

    for entry in WalkDir::new(&parent_path).follow_links(false) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                report.skipped += 1;
                continue;
            }
        };
        if !entry.file_type().is_file() || entry.file_name() != OsStr::new(CONFIG_FILE_NAME) {
            continue;
        }

        let Some(instance_path) = entry.path().parent() else {
            report.skipped += 1;
            continue;
        };
        let instance_path = match fs::canonicalize(instance_path) {
            Ok(path) => path,
            Err(_) => {
                report.skipped += 1;
                continue;
            }
        };
        let instance_path = instance_path.to_string_lossy().into_owned();
        let deleted_at = transaction
            .query_row(
                "SELECT deleted_at_unix FROM managed_instances WHERE path = ?1 LIMIT 1",
                [&instance_path],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?;

        match deleted_at {
            Some(Some(_)) => {
                transaction.execute(
                    r#"
                    UPDATE managed_instances
                    SET deleted_at_unix = NULL, last_seen_at_unix = ?1
                    WHERE path = ?2
                    "#,
                    params![now, instance_path],
                )?;
                report.reactivated += 1;
            }
            Some(None) => {
                transaction.execute(
                    "UPDATE managed_instances SET last_seen_at_unix = ?1 WHERE path = ?2",
                    params![now, instance_path],
                )?;
                report.already_managed += 1;
            }
            None => {
                transaction.execute(
                    r#"
                    INSERT INTO managed_instances (path, display_name, added_at_unix, last_seen_at_unix)
                    VALUES (?1, NULL, ?2, ?2)
                    "#,
                    params![instance_path, now],
                )?;
                report.added += 1;
            }
        }
    }

    transaction
        .commit()
        .context("failed to save managed instance scan results")?;
    Ok(report)
}

pub fn list_managed_instances(database_path: &Path) -> Result<Vec<ManagedInstance>> {
    let connection = open_registry(database_path)?;
    let mut statement = connection.prepare(
        r#"
        SELECT id, path, display_name, added_at_unix, last_seen_at_unix
        FROM managed_instances
        WHERE deleted_at_unix IS NULL
        ORDER BY
            CASE WHEN COALESCE(TRIM(display_name), '') = '' THEN path ELSE display_name END COLLATE NOCASE,
            path COLLATE NOCASE
        "#,
    )?;
    let rows = statement.query_map([], |row| {
        Ok(StoredManagedInstance {
            id: row.get(0)?,
            path: row.get(1)?,
            display_name: row.get(2)?,
            added_at_unix: row.get(3)?,
            last_seen_at_unix: row.get(4)?,
        })
    })?;
    let instances = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(instances
        .into_iter()
        .map(managed_instance_from_stored)
        .collect())
}

pub fn rename_managed_instance(
    database_path: &Path,
    instance_id: i64,
    display_name: &str,
) -> Result<ManagedInstance> {
    let display_name = display_name.trim();
    if display_name.is_empty() {
        bail!("managed instance display name cannot be empty");
    }

    let connection = open_registry(database_path)?;
    let changed = connection.execute(
        r#"
        UPDATE managed_instances
        SET display_name = ?1
        WHERE id = ?2 AND deleted_at_unix IS NULL
        "#,
        params![display_name, instance_id],
    )?;
    if changed == 0 {
        bail!("managed instance {instance_id} does not exist");
    }

    read_instance(&connection, instance_id)
}

pub fn soft_delete_managed_instance(database_path: &Path, instance_id: i64) -> Result<()> {
    let connection = open_registry(database_path)?;
    let deleted_at_unix = OffsetDateTime::now_utc().unix_timestamp();
    let changed = connection.execute(
        r#"
        UPDATE managed_instances
        SET deleted_at_unix = ?1
        WHERE id = ?2 AND deleted_at_unix IS NULL
        "#,
        params![deleted_at_unix, instance_id],
    )?;
    if changed == 0 {
        bail!("managed instance {instance_id} does not exist");
    }
    Ok(())
}

pub fn managed_instance_path(database_path: &Path, instance_id: i64) -> Result<PathBuf> {
    let connection = open_registry(database_path)?;
    let path = connection
        .query_row(
            "SELECT path FROM managed_instances WHERE id = ?1 AND deleted_at_unix IS NULL",
            [instance_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| anyhow::anyhow!("managed instance {instance_id} does not exist"))?;
    let path = PathBuf::from(path);
    if !path.is_dir() {
        bail!(
            "managed instance path is no longer available: {}",
            path.display()
        );
    }
    Ok(path)
}

fn open_registry(database_path: &Path) -> Result<Connection> {
    if let Some(parent) = database_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create managed instance registry directory {}",
                parent.display()
            )
        })?;
    }
    let connection = Connection::open(database_path).with_context(|| {
        format!(
            "failed to open managed instance registry {}",
            database_path.display()
        )
    })?;
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS managed_instances (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            display_name TEXT,
            added_at_unix INTEGER NOT NULL,
            last_seen_at_unix INTEGER NOT NULL,
            deleted_at_unix INTEGER
        );
        "#,
    )?;
    ensure_deleted_at_column(&connection)?;
    Ok(connection)
}

fn ensure_deleted_at_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(managed_instances)")?;
    let mut rows = statement.query([])?;
    let mut has_deleted_at_column = false;
    while let Some(row) = rows.next()? {
        let column_name = row.get::<_, String>(1)?;
        if column_name == "deleted_at_unix" {
            has_deleted_at_column = true;
            break;
        }
    }
    drop(rows);
    drop(statement);

    if !has_deleted_at_column {
        connection
            .execute(
                "ALTER TABLE managed_instances ADD COLUMN deleted_at_unix INTEGER",
                [],
            )
            .context("failed to migrate managed instance registry for logical deletion")?;
    }
    Ok(())
}

fn read_instance(connection: &Connection, instance_id: i64) -> Result<ManagedInstance> {
    let instance = connection
        .query_row(
            r#"
            SELECT id, path, display_name, added_at_unix, last_seen_at_unix
            FROM managed_instances
            WHERE id = ?1 AND deleted_at_unix IS NULL
            "#,
            [instance_id],
            |row| {
                Ok(StoredManagedInstance {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    display_name: row.get(2)?,
                    added_at_unix: row.get(3)?,
                    last_seen_at_unix: row.get(4)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| anyhow::anyhow!("managed instance {instance_id} does not exist"))?;
    Ok(managed_instance_from_stored(instance))
}

fn instance_is_available(path: &Path) -> bool {
    path.is_dir() && path.join(CONFIG_FILE_NAME).is_file()
}

fn managed_instance_from_stored(instance: StoredManagedInstance) -> ManagedInstance {
    ManagedInstance {
        id: instance.id,
        path: display_path_text(Path::new(&instance.path)),
        display_name: instance.display_name,
        available: instance_is_available(Path::new(&instance.path)),
        added_at_unix: instance.added_at_unix,
        last_seen_at_unix: instance.last_seen_at_unix,
    }
}

fn display_path_text(path: &Path) -> String {
    let path = path.to_string_lossy().into_owned();
    #[cfg(windows)]
    {
        if let Some(unc_path) = path.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{unc_path}");
        }
        if let Some(normal_path) = path.strip_prefix(r"\\?\") {
            return normal_path.to_string();
        }
    }
    path
}
