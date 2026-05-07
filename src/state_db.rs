use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{named_params, Connection};

use crate::path_map::{apply_first_path_map, PathMap};

#[derive(Debug, Clone, PartialEq, Eq)]
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

    pub fn update_selected_session_updated_at(
        &mut self,
        ids: &[String],
        updated_at: &str,
        updated_at_ms: i64,
    ) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }

        let tx = self.conn.transaction()?;
        let mut changed = 0;
        for id in ids {
            changed += tx.execute(
                r#"
                UPDATE threads
                SET
                    updated_at = :updated_at,
                    updated_at_ms = :updated_at_ms
                WHERE id = :id
                  AND (
                    COALESCE(CAST(updated_at AS TEXT), '') != :updated_at
                    OR COALESCE(updated_at_ms, -1) != :updated_at_ms
                  )
                "#,
                named_params! {
                    ":id": id,
                    ":updated_at": updated_at,
                    ":updated_at_ms": updated_at_ms,
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

    pub fn integrity_check(&self) -> Result<String> {
        self.conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .context("failed to run SQLite integrity_check")
    }
}
