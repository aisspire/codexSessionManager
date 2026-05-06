use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::profile::CodexProfile;
use crate::rollout::read_all_rollout_meta;
use crate::session_index::read_session_index;
use crate::state_db::StateDb;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArchivedFilter {
    Active,
    Archived,
    #[default]
    All,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionListFilter {
    pub project: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub source: Option<String>,
    pub archived: ArchivedFilter,
    pub search: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub title: Option<String>,
    pub first_user_message: Option<String>,
    pub project: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub source: Option<String>,
    pub archived: bool,
    pub updated_at: Option<String>,
    pub rollout_path: Option<String>,
    pub in_session_index: bool,
}

pub fn list_sessions(
    profile: &CodexProfile,
    filter: &SessionListFilter,
) -> Result<Vec<SessionSummary>> {
    let db = StateDb::open(&profile.state_db_path())?;
    let threads = db.read_threads()?;
    let metas_by_id = read_all_rollout_meta(&profile.sessions_dir())?
        .into_iter()
        .filter_map(|meta| meta.id.clone().map(|id| (id, meta)))
        .collect::<HashMap<_, _>>();
    let index_by_id = read_session_index(&profile.session_index_path())?
        .into_iter()
        .map(|entry| (entry.id.clone(), entry))
        .collect::<HashMap<_, _>>();

    let mut sessions = threads
        .into_iter()
        .map(|thread| {
            let meta = metas_by_id.get(&thread.id);
            let index = index_by_id.get(&thread.id);
            SessionSummary {
                id: thread.id.clone(),
                title: index
                    .and_then(|entry| entry.thread_name.clone())
                    .or(thread.title.clone())
                    .or(thread.first_user_message.clone()),
                first_user_message: thread.first_user_message.clone(),
                project: thread
                    .cwd
                    .clone()
                    .or_else(|| meta.and_then(|m| m.cwd.clone())),
                provider: thread
                    .model_provider
                    .clone()
                    .or_else(|| meta.and_then(|m| m.model_provider.clone())),
                model: thread.model.clone(),
                source: thread
                    .source
                    .clone()
                    .or_else(|| meta.and_then(|m| m.source.clone())),
                archived: thread.archived,
                updated_at: thread.updated_at.clone(),
                rollout_path: thread.rollout_path.clone(),
                in_session_index: index.is_some(),
            }
        })
        .filter(|session| matches_filter(session, filter))
        .collect::<Vec<_>>();

    sessions.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then(left.id.cmp(&right.id))
    });
    Ok(sessions)
}

fn matches_filter(session: &SessionSummary, filter: &SessionListFilter) -> bool {
    match filter.archived {
        ArchivedFilter::Active if session.archived => return false,
        ArchivedFilter::Archived if !session.archived => return false,
        _ => {}
    }

    if !matches_optional(session.project.as_deref(), filter.project.as_deref()) {
        return false;
    }
    if !matches_optional(session.provider.as_deref(), filter.provider.as_deref()) {
        return false;
    }
    if !matches_optional(session.model.as_deref(), filter.model.as_deref()) {
        return false;
    }
    if !matches_optional(session.source.as_deref(), filter.source.as_deref()) {
        return false;
    }
    if let Some(search) = filter.search.as_deref().filter(|value| !value.is_empty()) {
        let search = search.to_ascii_lowercase();
        let haystack = [
            Some(session.id.as_str()),
            session.title.as_deref(),
            session.first_user_message.as_deref(),
            session.project.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
        if !haystack.contains(&search) {
            return false;
        }
    }

    true
}

fn matches_optional(actual: Option<&str>, expected: Option<&str>) -> bool {
    let Some(expected) = expected.filter(|value| !value.is_empty()) else {
        return true;
    };
    actual == Some(expected)
}
