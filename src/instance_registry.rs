use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use walkdir::WalkDir;

use crate::path_map::normalize_path_text;
use crate::wsl::{
    is_wsl_mounted_path, is_wsl_unc_path, normalize_architecture, validate_distribution,
    validate_linux_absolute_path, validate_user,
};

const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstanceRuntime {
    Native,
    Wsl {
        distribution: String,
        user: String,
        codex_home: String,
        host_path: String,
        architecture: String,
    },
}

impl Default for InstanceRuntime {
    fn default() -> Self {
        Self::Native
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstanceAvailability {
    Unknown,
    Available,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedInstance {
    pub id: i64,
    pub path: String,
    pub display_name: Option<String>,
    pub availability: InstanceAvailability,
    #[serde(default)]
    pub availability_error: Option<String>,
    #[serde(default)]
    pub runtime: InstanceRuntime,
    pub added_at_unix: i64,
    pub last_seen_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WslInstanceRegistration {
    pub distribution: String,
    pub user: String,
    pub codex_home: String,
    pub host_path: String,
    pub architecture: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InstanceScanReport {
    pub added: usize,
    pub reactivated: usize,
    pub ignored: usize,
    pub already_managed: usize,
    pub skipped: usize,
}

/// A reusable local-instance synchronization recipe.
///
/// Explicit session choices deliberately do not belong here: users choose
/// those for every run. Project selections are durable conditions, while config
/// paths are represented as TOML key segments so keys that contain dots are not
/// ambiguous.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceSyncPlan {
    pub id: i64,
    pub name: String,
    pub source_instance_id: i64,
    pub target_instance_ids: Vec<i64>,
    pub config_paths: Vec<Vec<String>>,
    #[serde(default)]
    pub project_selections: Vec<Option<String>>,
    pub created_at_unix: i64,
    pub updated_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceSyncPlanDraft {
    pub id: Option<i64>,
    pub name: String,
    pub source_instance_id: i64,
    pub target_instance_ids: Vec<i64>,
    pub config_paths: Vec<Vec<String>>,
    #[serde(default)]
    pub project_selections: Vec<Option<String>>,
}

pub fn validate_instance_sync_compatibility(
    source: &ManagedInstance,
    target: &ManagedInstance,
) -> Result<()> {
    match (&source.runtime, &target.runtime) {
        (InstanceRuntime::Native, InstanceRuntime::Native) => {
            if unresolved_wsl_path(&source.path) {
                bail!(
                    "managed instance {} is a WSL path without runtime metadata; discover or register it again",
                    source.id
                );
            }
            if unresolved_wsl_path(&target.path) {
                bail!(
                    "managed instance {} is a WSL path without runtime metadata; discover or register it again",
                    target.id
                );
            }
            Ok(())
        }
        (
            InstanceRuntime::Wsl {
                distribution: source_distribution,
                user: source_user,
                architecture: source_architecture,
                codex_home: source_codex_home,
                ..
            },
            InstanceRuntime::Wsl {
                distribution: target_distribution,
                user: target_user,
                architecture: target_architecture,
                codex_home: target_codex_home,
                ..
            },
        ) => {
            if !source_distribution.eq_ignore_ascii_case(target_distribution) {
                bail!("WSL instance sync requires the same distribution");
            }
            if source_user != target_user {
                bail!("WSL instance sync requires the same Linux user");
            }
            if normalize_architecture(source_architecture)?
                != normalize_architecture(target_architecture)?
            {
                bail!("WSL instance sync requires the same helper architecture");
            }
            if normalize_linux_codex_home(source_codex_home)
                == normalize_linux_codex_home(target_codex_home)
            {
                bail!("instance sync source cannot resolve to the same Codex home as a target");
            }
            Ok(())
        }
        _ => bail!("Windows and WSL instances cannot be synchronized with each other"),
    }
}

fn normalize_linux_codex_home(value: &str) -> String {
    let normalized = value.trim_end_matches('/');
    if normalized.is_empty() {
        "/".to_string()
    } else {
        normalized.to_string()
    }
}

#[derive(Debug)]
struct StoredManagedInstance {
    id: i64,
    path: String,
    display_name: Option<String>,
    added_at_unix: i64,
    last_seen_at_unix: i64,
    runtime_kind: String,
    distribution: Option<String>,
    linux_user: Option<String>,
    codex_home: Option<String>,
    host_path: Option<String>,
    architecture: Option<String>,
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
        let record_state = transaction
            .query_row(
                r#"
                SELECT deleted_at_unix, ignored_at_unix
                FROM managed_instances
                WHERE path = ?1
                LIMIT 1
                "#,
                [&instance_path],
                |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .optional()?;

        match record_state {
            Some((_, Some(_))) => {
                transaction.execute(
                    "UPDATE managed_instances SET last_seen_at_unix = ?1 WHERE path = ?2",
                    params![now, instance_path],
                )?;
                report.ignored += 1;
            }
            Some((Some(_), None)) => {
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
            Some((None, None)) => {
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

pub fn register_wsl_instance(
    database_path: &Path,
    registration: &WslInstanceRegistration,
) -> Result<ManagedInstance> {
    let registration = normalized_wsl_registration(registration)?;
    let mut connection = open_registry(database_path)?;
    let transaction = connection
        .transaction()
        .context("failed to start WSL instance registry transaction")?;
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let canonical_host_path =
        canonical_wsl_host_path(&registration.distribution, &registration.codex_home);

    let mut statement = transaction.prepare(
        r#"
        SELECT
            id, path, runtime_kind, distribution, linux_user, codex_home,
            ignored_at_unix
        FROM managed_instances
        "#,
    )?;
    let records = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<i64>>(6)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);

    let mut matching_records = records
        .into_iter()
        .filter(|record| {
            let (_, path, runtime_kind, distribution, linux_user, codex_home, _) = record;
            let runtime_matches = runtime_kind == "wsl"
                && distribution
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(&registration.distribution))
                && linux_user.as_deref() == Some(registration.user.as_str())
                && codex_home.as_deref() == Some(registration.codex_home.as_str());
            runtime_matches
                || legacy_wsl_path(path).is_some_and(|(distribution, codex_home)| {
                    distribution.eq_ignore_ascii_case(&registration.distribution)
                        && codex_home == registration.codex_home
                })
        })
        .collect::<Vec<_>>();
    if matching_records.iter().any(|record| record.6.is_some()) {
        bail!("this WSL Codex instance is permanently ignored");
    }
    matching_records.sort_by_key(|record| {
        if same_wsl_host_path(&record.1, &canonical_host_path) {
            0
        } else if record.2 == "wsl" {
            1
        } else {
            2
        }
    });
    let existing = matching_records.first().cloned();

    let instance_id = if let Some((id, _, _, _, _, _, _)) = existing {
        for duplicate in matching_records.iter().skip(1) {
            transaction.execute(
                "UPDATE managed_instances SET deleted_at_unix = ?1 WHERE id = ?2",
                params![now, duplicate.0],
            )?;
        }
        transaction.execute(
            r#"
            UPDATE managed_instances
            SET
                path = ?1,
                runtime_kind = 'wsl',
                distribution = ?2,
                linux_user = ?3,
                codex_home = ?4,
                host_path = ?5,
                architecture = ?6,
                deleted_at_unix = NULL,
                last_seen_at_unix = ?7
            WHERE id = ?8
            "#,
            params![
                canonical_host_path,
                registration.distribution,
                registration.user,
                registration.codex_home,
                registration.host_path,
                registration.architecture,
                now,
                id,
            ],
        )?;
        id
    } else {
        transaction.execute(
            r#"
            INSERT INTO managed_instances (
                path, display_name, added_at_unix, last_seen_at_unix,
                runtime_kind, distribution, linux_user, codex_home, host_path, architecture
            )
            VALUES (?1, NULL, ?2, ?2, 'wsl', ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                canonical_host_path,
                now,
                registration.distribution,
                registration.user,
                registration.codex_home,
                registration.host_path,
                registration.architecture,
            ],
        )?;
        transaction.last_insert_rowid()
    };

    transaction
        .commit()
        .context("failed to register WSL Codex instance")?;
    let connection = open_registry(database_path)?;
    read_instance(&connection, instance_id)
}

pub fn managed_instance(database_path: &Path, instance_id: i64) -> Result<ManagedInstance> {
    let connection = open_registry(database_path)?;
    read_instance(&connection, instance_id)
}

pub fn list_managed_instances(database_path: &Path) -> Result<Vec<ManagedInstance>> {
    let connection = open_registry(database_path)?;
    let mut statement = connection.prepare(
        r#"
        SELECT
            id, path, display_name, added_at_unix, last_seen_at_unix,
            runtime_kind, distribution, linux_user, codex_home, host_path, architecture
        FROM managed_instances
        WHERE deleted_at_unix IS NULL AND ignored_at_unix IS NULL
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
            runtime_kind: row.get(5)?,
            distribution: row.get(6)?,
            linux_user: row.get(7)?,
            codex_home: row.get(8)?,
            host_path: row.get(9)?,
            architecture: row.get(10)?,
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
        WHERE id = ?2 AND deleted_at_unix IS NULL AND ignored_at_unix IS NULL
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
        WHERE id = ?2 AND deleted_at_unix IS NULL AND ignored_at_unix IS NULL
        "#,
        params![deleted_at_unix, instance_id],
    )?;
    if changed == 0 {
        bail!("managed instance {instance_id} does not exist");
    }
    Ok(())
}

pub fn permanently_ignore_managed_instance(database_path: &Path, instance_id: i64) -> Result<()> {
    let connection = open_registry(database_path)?;
    let ignored_at_unix = OffsetDateTime::now_utc().unix_timestamp();
    let changed = connection.execute(
        r#"
        UPDATE managed_instances
        SET ignored_at_unix = ?1
        WHERE id = ?2 AND deleted_at_unix IS NULL AND ignored_at_unix IS NULL
        "#,
        params![ignored_at_unix, instance_id],
    )?;
    if changed == 0 {
        bail!("managed instance {instance_id} does not exist");
    }
    Ok(())
}

pub fn managed_instance_path(database_path: &Path, instance_id: i64) -> Result<PathBuf> {
    let connection = open_registry(database_path)?;
    let instance = read_instance(&connection, instance_id)?;
    if let InstanceRuntime::Wsl { host_path, .. } = instance.runtime {
        return Ok(PathBuf::from(host_path));
    }
    if unresolved_wsl_path(&instance.path) {
        bail!(
            "managed instance {instance_id} is a WSL path without runtime metadata; discover or register it again"
        );
    }
    let path = PathBuf::from(instance.path);
    if !path.is_dir() {
        bail!(
            "managed instance path is no longer available: {}",
            path.display()
        );
    }
    Ok(path)
}

pub fn list_instance_sync_plans(database_path: &Path) -> Result<Vec<InstanceSyncPlan>> {
    let connection = open_registry(database_path)?;
    let mut statement = connection.prepare(
        r#"
        SELECT
            id,
            name,
            source_instance_id,
            target_instance_ids_json,
            config_paths_json,
            project_selections_json,
            created_at_unix,
            updated_at_unix
        FROM instance_sync_plans
        ORDER BY updated_at_unix DESC, id DESC
        "#,
    )?;
    let rows = statement.query_map([], instance_sync_plan_from_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to list instance sync plans")
}

pub fn save_instance_sync_plan(
    database_path: &Path,
    draft: &InstanceSyncPlanDraft,
) -> Result<InstanceSyncPlan> {
    let name = draft.name.trim();
    if name.is_empty() {
        bail!("instance sync plan name cannot be empty");
    }

    let target_instance_ids =
        normalized_target_instance_ids(draft.source_instance_id, &draft.target_instance_ids)?;
    let config_paths = normalized_config_paths(&draft.config_paths)?;
    let project_selections = normalized_project_selections(&draft.project_selections);
    let connection = open_registry(database_path)?;
    ensure_sync_plan_instances_available(
        &connection,
        draft.source_instance_id,
        &target_instance_ids,
    )?;

    let target_instance_ids_json = serde_json::to_string(&target_instance_ids)
        .context("failed to serialize instance sync plan targets")?;
    let config_paths_json = serde_json::to_string(&config_paths)
        .context("failed to serialize instance sync plan config paths")?;
    let project_selections_json = serde_json::to_string(&project_selections)
        .context("failed to serialize instance sync plan project selections")?;
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let id = if let Some(id) = draft.id {
        let changed = connection.execute(
            r#"
            UPDATE instance_sync_plans
            SET
                name = ?1,
                source_instance_id = ?2,
                target_instance_ids_json = ?3,
                config_paths_json = ?4,
                project_selections_json = ?5,
                updated_at_unix = ?6
            WHERE id = ?7
            "#,
            params![
                name,
                draft.source_instance_id,
                target_instance_ids_json,
                config_paths_json,
                project_selections_json,
                now,
                id,
            ],
        )?;
        if changed == 0 {
            bail!("instance sync plan {id} does not exist");
        }
        id
    } else {
        connection.execute(
            r#"
            INSERT INTO instance_sync_plans (
                name,
                source_instance_id,
                target_instance_ids_json,
                config_paths_json,
                project_selections_json,
                created_at_unix,
                updated_at_unix
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
            "#,
            params![
                name,
                draft.source_instance_id,
                target_instance_ids_json,
                config_paths_json,
                project_selections_json,
                now,
            ],
        )?;
        connection.last_insert_rowid()
    };

    read_instance_sync_plan(&connection, id)
}

pub fn delete_instance_sync_plan(database_path: &Path, plan_id: i64) -> Result<()> {
    let connection = open_registry(database_path)?;
    let changed = connection.execute("DELETE FROM instance_sync_plans WHERE id = ?1", [plan_id])?;
    if changed == 0 {
        bail!("instance sync plan {plan_id} does not exist");
    }
    Ok(())
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
            deleted_at_unix INTEGER,
            ignored_at_unix INTEGER,
            runtime_kind TEXT NOT NULL DEFAULT 'native',
            distribution TEXT,
            linux_user TEXT,
            codex_home TEXT,
            host_path TEXT,
            architecture TEXT
        );
        CREATE TABLE IF NOT EXISTS instance_sync_plans (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            source_instance_id INTEGER NOT NULL,
            target_instance_ids_json TEXT NOT NULL,
            config_paths_json TEXT NOT NULL,
            project_selections_json TEXT NOT NULL DEFAULT '[]',
            created_at_unix INTEGER NOT NULL,
            updated_at_unix INTEGER NOT NULL
        );
        "#,
    )?;
    ensure_registry_column(
        &connection,
        "managed_instances",
        "deleted_at_unix",
        "ALTER TABLE managed_instances ADD COLUMN deleted_at_unix INTEGER",
    )?;
    ensure_registry_column(
        &connection,
        "managed_instances",
        "ignored_at_unix",
        "ALTER TABLE managed_instances ADD COLUMN ignored_at_unix INTEGER",
    )?;
    ensure_registry_column(
        &connection,
        "managed_instances",
        "runtime_kind",
        "ALTER TABLE managed_instances ADD COLUMN runtime_kind TEXT NOT NULL DEFAULT 'native'",
    )?;
    ensure_registry_column(
        &connection,
        "managed_instances",
        "distribution",
        "ALTER TABLE managed_instances ADD COLUMN distribution TEXT",
    )?;
    ensure_registry_column(
        &connection,
        "managed_instances",
        "linux_user",
        "ALTER TABLE managed_instances ADD COLUMN linux_user TEXT",
    )?;
    ensure_registry_column(
        &connection,
        "managed_instances",
        "codex_home",
        "ALTER TABLE managed_instances ADD COLUMN codex_home TEXT",
    )?;
    ensure_registry_column(
        &connection,
        "managed_instances",
        "host_path",
        "ALTER TABLE managed_instances ADD COLUMN host_path TEXT",
    )?;
    ensure_registry_column(
        &connection,
        "managed_instances",
        "architecture",
        "ALTER TABLE managed_instances ADD COLUMN architecture TEXT",
    )?;
    ensure_registry_column(
        &connection,
        "instance_sync_plans",
        "project_selections_json",
        "ALTER TABLE instance_sync_plans ADD COLUMN project_selections_json TEXT NOT NULL DEFAULT '[]'",
    )?;
    Ok(connection)
}

fn instance_sync_plan_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<InstanceSyncPlan> {
    let target_instance_ids_json = row.get::<_, String>(3)?;
    let config_paths_json = row.get::<_, String>(4)?;
    let project_selections_json = row.get::<_, String>(5)?;
    let target_instance_ids = serde_json::from_str(&target_instance_ids_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let config_paths = serde_json::from_str(&config_paths_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let project_selections = serde_json::from_str(&project_selections_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(InstanceSyncPlan {
        id: row.get(0)?,
        name: row.get(1)?,
        source_instance_id: row.get(2)?,
        target_instance_ids,
        config_paths,
        project_selections,
        created_at_unix: row.get(6)?,
        updated_at_unix: row.get(7)?,
    })
}

fn read_instance_sync_plan(connection: &Connection, id: i64) -> Result<InstanceSyncPlan> {
    connection
        .query_row(
            r#"
            SELECT
                id,
                name,
                source_instance_id,
                target_instance_ids_json,
                config_paths_json,
                project_selections_json,
                created_at_unix,
                updated_at_unix
            FROM instance_sync_plans
            WHERE id = ?1
            "#,
            [id],
            instance_sync_plan_from_row,
        )
        .optional()?
        .ok_or_else(|| anyhow::anyhow!("instance sync plan {id} does not exist"))
}

fn normalized_target_instance_ids(
    source_instance_id: i64,
    target_instance_ids: &[i64],
) -> Result<Vec<i64>> {
    if source_instance_id <= 0 {
        bail!("instance sync source must be a registered instance");
    }
    if target_instance_ids.is_empty() {
        bail!("instance sync plan must include at least one target instance");
    }

    let mut seen = std::collections::HashSet::new();
    let mut normalized = Vec::with_capacity(target_instance_ids.len());
    for target_id in target_instance_ids {
        if *target_id <= 0 {
            bail!("instance sync target must be a registered instance");
        }
        if *target_id == source_instance_id {
            bail!("instance sync source cannot also be a target");
        }
        if !seen.insert(*target_id) {
            bail!("instance sync targets cannot contain duplicates");
        }
        normalized.push(*target_id);
    }
    Ok(normalized)
}

fn normalized_config_paths(config_paths: &[Vec<String>]) -> Result<Vec<Vec<String>>> {
    let mut seen = std::collections::HashSet::new();
    let mut normalized = Vec::with_capacity(config_paths.len());
    for path in config_paths {
        if path.is_empty() || path.iter().any(|segment| segment.trim().is_empty()) {
            bail!("instance sync config paths cannot be empty");
        }
        let path = path
            .iter()
            .map(|segment| segment.trim().to_string())
            .collect::<Vec<_>>();
        let encoded = serde_json::to_string(&path)
            .context("failed to normalize instance sync config path")?;
        if seen.insert(encoded) {
            normalized.push(path);
        }
    }
    Ok(normalized)
}

fn normalized_project_selections(project_selections: &[Option<String>]) -> Vec<Option<String>> {
    let mut seen = std::collections::HashSet::new();
    let mut normalized = Vec::with_capacity(project_selections.len());
    for project in project_selections {
        let project = project
            .as_deref()
            .map(normalize_path_text)
            .filter(|project| !project.is_empty());
        if seen.insert(project.clone()) {
            normalized.push(project);
        }
    }
    normalized
}

fn ensure_sync_plan_instances_available(
    connection: &Connection,
    source_instance_id: i64,
    target_instance_ids: &[i64],
) -> Result<()> {
    let source = read_instance(connection, source_instance_id)?;
    ensure_sync_plan_instance_usable(&source)?;
    for target_instance_id in target_instance_ids {
        let target = read_instance(connection, *target_instance_id)?;
        ensure_sync_plan_instance_usable(&target)?;
        validate_instance_sync_compatibility(&source, &target)?;
    }
    Ok(())
}

fn ensure_sync_plan_instance_usable(instance: &ManagedInstance) -> Result<()> {
    if instance.availability == InstanceAvailability::Unavailable {
        bail!("managed instance {} is not available", instance.id);
    }
    if matches!(instance.runtime, InstanceRuntime::Native)
        && instance.availability != InstanceAvailability::Available
    {
        bail!("managed instance {} is not available", instance.id);
    }
    Ok(())
}

fn ensure_registry_column(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
    migration_statement: &str,
) -> Result<()> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let mut rows = statement.query([])?;
    let mut has_column = false;
    while let Some(row) = rows.next()? {
        let existing_column_name = row.get::<_, String>(1)?;
        if existing_column_name == column_name {
            has_column = true;
            break;
        }
    }
    drop(rows);
    drop(statement);

    if !has_column {
        connection
            .execute(migration_statement, [])
            .context("failed to migrate managed instance registry")?;
    }
    Ok(())
}

fn read_instance(connection: &Connection, instance_id: i64) -> Result<ManagedInstance> {
    let instance = connection
        .query_row(
            r#"
            SELECT
                id, path, display_name, added_at_unix, last_seen_at_unix,
                runtime_kind, distribution, linux_user, codex_home, host_path, architecture
            FROM managed_instances
            WHERE id = ?1 AND deleted_at_unix IS NULL AND ignored_at_unix IS NULL
            "#,
            [instance_id],
            |row| {
                Ok(StoredManagedInstance {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    display_name: row.get(2)?,
                    added_at_unix: row.get(3)?,
                    last_seen_at_unix: row.get(4)?,
                    runtime_kind: row.get(5)?,
                    distribution: row.get(6)?,
                    linux_user: row.get(7)?,
                    codex_home: row.get(8)?,
                    host_path: row.get(9)?,
                    architecture: row.get(10)?,
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
    let runtime = if instance.runtime_kind == "wsl" {
        match (
            instance.distribution,
            instance.linux_user,
            instance.codex_home,
            instance.host_path,
            instance.architecture,
        ) {
            (
                Some(distribution),
                Some(user),
                Some(codex_home),
                Some(host_path),
                Some(architecture),
            ) => InstanceRuntime::Wsl {
                distribution,
                user,
                codex_home,
                host_path,
                architecture,
            },
            _ => InstanceRuntime::Native,
        }
    } else {
        InstanceRuntime::Native
    };
    let unresolved_wsl =
        matches!(&runtime, InstanceRuntime::Native) && unresolved_wsl_path(&instance.path);
    let availability = match &runtime {
        InstanceRuntime::Native => {
            if !unresolved_wsl && instance_is_available(Path::new(&instance.path)) {
                InstanceAvailability::Available
            } else {
                InstanceAvailability::Unavailable
            }
        }
        InstanceRuntime::Wsl { .. } => InstanceAvailability::Unknown,
    };
    let path = match &runtime {
        InstanceRuntime::Native => display_path_text(Path::new(&instance.path)),
        InstanceRuntime::Wsl { host_path, .. } => host_path.clone(),
    };
    ManagedInstance {
        id: instance.id,
        path,
        display_name: instance.display_name,
        availability,
        availability_error: unresolved_wsl.then(|| {
            "WSL path is not bound to a distribution; discover or register it again".to_string()
        }),
        runtime,
        added_at_unix: instance.added_at_unix,
        last_seen_at_unix: instance.last_seen_at_unix,
    }
}

fn validate_wsl_registration(registration: &WslInstanceRegistration) -> Result<()> {
    validate_distribution(&registration.distribution)?;
    validate_user(&registration.user)?;
    validate_linux_absolute_path("Codex home", &registration.codex_home)?;
    normalize_architecture(&registration.architecture)?;
    let expected_host_path =
        canonical_wsl_host_path(&registration.distribution, &registration.codex_home);
    if !same_wsl_host_path(&registration.host_path, &expected_host_path) {
        bail!("WSL host path does not match the distribution and Codex home");
    }
    Ok(())
}

fn normalized_wsl_registration(
    registration: &WslInstanceRegistration,
) -> Result<WslInstanceRegistration> {
    validate_wsl_registration(registration)?;
    let mut normalized = registration.clone();
    normalized.architecture = normalize_architecture(&registration.architecture)?;
    Ok(normalized)
}

pub fn canonical_wsl_host_path(distribution: &str, codex_home: &str) -> String {
    let suffix = codex_home.trim_start_matches('/').replace('/', "\\");
    if suffix.is_empty() {
        format!(r"\\wsl.localhost\{distribution}")
    } else {
        format!(r"\\wsl.localhost\{distribution}\{suffix}")
    }
}

pub fn legacy_wsl_path(path: &str) -> Option<(String, String)> {
    let normalized = path.trim().replace('/', "\\");
    let normalized = normalized
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .unwrap_or(normalized);
    let rest = normalized
        .strip_prefix(r"\\wsl.localhost\")
        .or_else(|| normalized.strip_prefix(r"\\wsl$\"))?;
    let (distribution, suffix) = rest.split_once('\\').unwrap_or((rest, ""));
    if distribution.is_empty() {
        return None;
    }
    let codex_home = format!("/{}", suffix.replace('\\', "/").trim_start_matches('/'));
    Some((distribution.to_string(), codex_home))
}

fn same_wsl_host_path(left: &str, right: &str) -> bool {
    let normalize = |value: &str| {
        value
            .trim()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_ascii_lowercase()
    };
    normalize(left) == normalize(right)
}

fn unresolved_wsl_path(path: &str) -> bool {
    legacy_wsl_path(path).is_some() || is_wsl_mounted_path(path) || is_wsl_unc_path(path)
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
