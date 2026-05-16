use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{named_params, params_from_iter, Connection, ToSql};

use crate::path_map::{apply_first_path_map, PathMap};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRecord {
    pub id: String,
    pub rollout_path: Option<String>,
    pub cwd: Option<String>,
    pub source: Option<String>,
    pub model_provider: Option<String>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub has_user_event: bool,
    pub archived: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub created_at_ms: Option<i64>,
    pub updated_at_ms: Option<i64>,
    pub title: Option<String>,
    pub first_user_message: Option<String>,
}

pub struct StateDb {
    conn: Connection,
}

impl StateDb {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open SQLite database {}", path.display()))?;
        Ok(Self { conn })
    }

    pub fn read_threads(&self) -> Result<Vec<ThreadRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                id,
                rollout_path,
                cwd,
                source,
                model_provider,
                model,
                reasoning_effort,
                COALESCE(has_user_event, 0),
                COALESCE(archived, 0),
                CASE
                    WHEN typeof(created_at) = 'integer'
                    THEN strftime('%Y-%m-%dT%H:%M:%SZ', created_at, 'unixepoch')
                    ELSE created_at
                END,
                CASE
                    WHEN typeof(updated_at) = 'integer'
                    THEN strftime('%Y-%m-%dT%H:%M:%SZ', updated_at, 'unixepoch')
                    ELSE updated_at
                END,
                created_at_ms,
                updated_at_ms,
                title,
                first_user_message
            FROM threads
            ORDER BY COALESCE(updated_at_ms, created_at_ms, 0) DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ThreadRecord {
                id: row.get(0)?,
                rollout_path: row.get(1)?,
                cwd: row.get(2)?,
                source: row.get(3)?,
                model_provider: row.get(4)?,
                model: row.get(5)?,
                reasoning_effort: row.get(6)?,
                has_user_event: row.get::<_, i64>(7)? != 0,
                archived: row.get::<_, i64>(8)? != 0,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                created_at_ms: row.get(11)?,
                updated_at_ms: row.get(12)?,
                title: row.get(13)?,
                first_user_message: row.get(14)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read threads from state database")
    }

    pub fn update_provider(&mut self, from: &str, to: &str) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let changed = tx.execute(
            "UPDATE threads SET model_provider = :to WHERE model_provider = :from",
            named_params! { ":from": from, ":to": to },
        )?;
        tx.commit()?;
        Ok(changed)
    }

    pub fn update_selected_session_fields(
        &mut self,
        ids: &[String],
        provider: Option<&str>,
        cwd: Option<&str>,
    ) -> Result<usize> {
        if ids.is_empty() || (provider.is_none() && cwd.is_none()) {
            return Ok(0);
        }

        let tx = self.conn.transaction()?;
        let mut changed = 0;
        for id in ids {
            changed += tx.execute(
                r#"
                UPDATE threads
                SET
                    model_provider = COALESCE(:provider, model_provider),
                    cwd = COALESCE(:cwd, cwd)
                WHERE id = :id
                  AND (
                    (:provider IS NOT NULL AND COALESCE(model_provider, '') != :provider)
                    OR (:cwd IS NOT NULL AND COALESCE(cwd, '') != :cwd)
                  )
                "#,
                named_params! {
                    ":id": id,
                    ":provider": provider,
                    ":cwd": cwd,
                },
            )?;
        }
        tx.commit()?;
        Ok(changed)
    }

    pub fn update_session_titles(&mut self, renames: &[(String, String)]) -> Result<usize> {
        if renames.is_empty() {
            return Ok(0);
        }

        let tx = self.conn.transaction()?;
        let mut changed = 0;
        for (id, title) in renames {
            changed += tx.execute(
                r#"
                UPDATE threads
                SET title = :title
                WHERE id = :id
                  AND COALESCE(title, '') != :title
                "#,
                named_params! {
                    ":id": id,
                    ":title": title,
                },
            )?;
        }
        tx.commit()?;
        Ok(changed)
    }

    pub fn update_paths(&mut self, maps: &[PathMap]) -> Result<usize> {
        let threads = self.read_threads()?;
        let tx = self.conn.transaction()?;
        let mut changed = 0;

        for thread in threads {
            let new_rollout_path = thread
                .rollout_path
                .as_deref()
                .and_then(|value| apply_first_path_map(value, maps));
            let new_cwd = thread
                .cwd
                .as_deref()
                .and_then(|value| apply_first_path_map(value, maps));

            if new_rollout_path.is_none() && new_cwd.is_none() {
                continue;
            }

            tx.execute(
                r#"
                UPDATE threads
                SET
                    rollout_path = COALESCE(:rollout_path, rollout_path),
                    cwd = COALESCE(:cwd, cwd)
                WHERE id = :id
                "#,
                named_params! {
                    ":id": thread.id,
                    ":rollout_path": new_rollout_path,
                    ":cwd": new_cwd,
                },
            )?;
            changed += 1;
        }

        tx.commit()?;
        Ok(changed)
    }

    pub fn insert_repaired_thread(&mut self, thread: &ThreadRecord) -> Result<usize> {
        let columns = self.thread_columns()?;
        let values = repaired_thread_values(thread);
        let mut insert_columns = Vec::new();
        let mut insert_values = Vec::new();

        for (column, value) in values {
            if columns.contains(&column) {
                insert_columns.push(column);
                insert_values.push(value);
            }
        }

        let placeholders = (0..insert_columns.len())
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "INSERT INTO threads ({}) VALUES ({})",
            insert_columns.join(", "),
            placeholders
        );
        let params = insert_values
            .iter()
            .map(|value| value as &dyn ToSql)
            .collect::<Vec<_>>();

        self.conn
            .execute(&sql, params_from_iter(params))
            .context("failed to insert repaired thread row")
    }

    pub fn upsert_restored_thread(&mut self, thread: &ThreadRecord) -> Result<usize> {
        let existing: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM threads WHERE id = ?1",
            [&thread.id],
            |row| row.get(0),
        )?;
        if existing == 0 {
            return self.insert_repaired_thread(thread);
        }

        let tx = self.conn.transaction()?;
        let changed = tx.execute(
            r#"
            UPDATE threads
            SET
                rollout_path = :rollout_path,
                cwd = :cwd,
                source = :source,
                model_provider = :model_provider,
                title = :title,
                has_user_event = :has_user_event,
                archived = :archived,
                first_user_message = :first_user_message,
                model = :model,
                reasoning_effort = :reasoning_effort
            WHERE id = :id
            "#,
            named_params! {
                ":id": thread.id,
                ":rollout_path": thread.rollout_path,
                ":cwd": thread.cwd,
                ":source": thread.source,
                ":model_provider": thread.model_provider,
                ":title": thread.title,
                ":has_user_event": if thread.has_user_event { 1 } else { 0 },
                ":archived": if thread.archived { 1 } else { 0 },
                ":first_user_message": thread.first_user_message,
                ":model": thread.model,
                ":reasoning_effort": thread.reasoning_effort,
            },
        )?;
        tx.commit()?;
        Ok(changed)
    }

    pub fn upsert_thread(&mut self, thread: &ThreadRecord) -> Result<usize> {
        self.upsert_restored_thread(thread)
    }

    pub fn update_rollout_path(&mut self, id: &str, rollout_path: &str) -> Result<usize> {
        self.conn
            .execute(
                r#"
                UPDATE threads
                SET rollout_path = :rollout_path
                WHERE id = :id
                  AND COALESCE(rollout_path, '') != :rollout_path
                "#,
                named_params! {
                    ":id": id,
                    ":rollout_path": rollout_path,
                },
            )
            .context("failed to update rollout_path")
    }

    pub fn update_thread_timestamps(
        &mut self,
        ids: &[String],
        updated_at: i64,
        updated_at_ms: i64,
    ) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }

        let columns = self.thread_columns()?;
        let mut assignments = Vec::new();
        if columns.contains(&"updated_at".to_string()) {
            assignments.push("updated_at = :updated_at");
        }
        if columns.contains(&"updated_at_ms".to_string()) {
            assignments.push("updated_at_ms = :updated_at_ms");
        }
        if assignments.is_empty() {
            return Ok(0);
        }

        let sql = format!(
            "UPDATE threads SET {} WHERE id = :id",
            assignments.join(", ")
        );
        let tx = self.conn.transaction()?;
        let mut changed = 0;
        for id in ids {
            changed += tx
                .execute(
                    &sql,
                    named_params! {
                        ":id": id,
                        ":updated_at": updated_at,
                        ":updated_at_ms": updated_at_ms,
                    },
                )
                .context("failed to update thread timestamps")?;
        }
        tx.commit()?;
        Ok(changed)
    }

    pub fn repair_has_user_event(&mut self) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let changed = tx.execute(
            r#"
            UPDATE threads
            SET has_user_event = 1
            WHERE COALESCE(has_user_event, 0) = 0
              AND COALESCE(archived, 0) = 0
              AND source IN ('cli', 'vscode')
              AND (
                first_user_message IS NOT NULL
                OR title IS NOT NULL
              )
            "#,
            [],
        )?;
        tx.commit()?;
        Ok(changed)
    }

    pub fn set_archived(&mut self, ids: &[String], archived: bool) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }

        let tx = self.conn.transaction()?;
        let mut changed = 0;
        for id in ids {
            changed += tx.execute(
                r#"
                UPDATE threads
                SET archived = :archived
                WHERE id = :id
                  AND COALESCE(archived, 0) != :archived
                "#,
                named_params! {
                    ":id": id,
                    ":archived": if archived { 1 } else { 0 },
                },
            )?;
        }
        tx.commit()?;
        Ok(changed)
    }

    pub fn delete_threads(&mut self, ids: &[String]) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }

        let tx = self.conn.transaction()?;
        let mut changed = 0;
        for id in ids {
            changed += tx
                .execute("DELETE FROM threads WHERE id = ?1", [id])
                .context("failed to delete thread row")?;
        }
        tx.commit()?;
        Ok(changed)
    }

    pub fn integrity_check(&self) -> Result<String> {
        self.conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .context("failed to run SQLite integrity_check")
    }

    fn thread_columns(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("PRAGMA table_info(threads)")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to inspect threads columns")
    }
}

fn repaired_thread_values(thread: &ThreadRecord) -> Vec<(String, rusqlite::types::Value)> {
    use rusqlite::types::Value;

    vec![
        ("id".to_string(), Value::Text(thread.id.clone())),
        (
            "rollout_path".to_string(),
            Value::Text(thread.rollout_path.clone().unwrap_or_default()),
        ),
        ("created_at".to_string(), Value::Integer(0)),
        ("updated_at".to_string(), Value::Integer(0)),
        (
            "source".to_string(),
            Value::Text(thread.source.clone().unwrap_or_else(|| "cli".to_string())),
        ),
        (
            "model_provider".to_string(),
            Value::Text(thread.model_provider.clone().unwrap_or_default()),
        ),
        (
            "cwd".to_string(),
            Value::Text(thread.cwd.clone().unwrap_or_default()),
        ),
        (
            "title".to_string(),
            Value::Text(thread.title.clone().unwrap_or_default()),
        ),
        (
            "sandbox_policy".to_string(),
            Value::Text("workspace-write".to_string()),
        ),
        (
            "approval_mode".to_string(),
            Value::Text("on-request".to_string()),
        ),
        ("tokens_used".to_string(), Value::Integer(0)),
        (
            "has_user_event".to_string(),
            Value::Integer(if thread.has_user_event { 1 } else { 0 }),
        ),
        (
            "archived".to_string(),
            Value::Integer(if thread.archived { 1 } else { 0 }),
        ),
        (
            "first_user_message".to_string(),
            Value::Text(thread.first_user_message.clone().unwrap_or_default()),
        ),
        (
            "model".to_string(),
            thread.model.clone().map(Value::Text).unwrap_or(Value::Null),
        ),
        (
            "reasoning_effort".to_string(),
            thread
                .reasoning_effort
                .clone()
                .map(Value::Text)
                .unwrap_or(Value::Null),
        ),
        (
            "created_at_ms".to_string(),
            thread
                .created_at_ms
                .map(Value::Integer)
                .unwrap_or(Value::Null),
        ),
        (
            "updated_at_ms".to_string(),
            thread
                .updated_at_ms
                .map(Value::Integer)
                .unwrap_or(Value::Null),
        ),
    ]
}
