use std::collections::{HashMap, HashSet};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::favorites;
use crate::profile::CodexProfile;
use crate::rollout::read_all_rollout_meta;
use crate::session_index::read_session_index;
use crate::state_db::StateDb;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionScope {
    Active,
    Archived,
    Favorite,
    #[default]
    All,
}

pub type ArchivedFilter = SessionScope;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionListFilter {
    pub project: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub source: Option<String>,
    pub archived: SessionScope,
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
    pub favorite: bool,
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
        .map(|meta| (meta, false))
        .chain(
            read_all_rollout_meta(&profile.archived_sessions_dir())?
                .into_iter()
                .map(|meta| (meta, true)),
        )
        .filter_map(|(meta, archived)| meta.id.clone().map(|id| (id, (meta, archived))))
        .collect::<HashMap<_, _>>();
    let index_by_id = read_session_index(&profile.session_index_path())?
        .into_iter()
        .map(|entry| (entry.id.clone(), entry))
        .collect::<HashMap<_, _>>();
    let favorite_ids = favorites::favorite_ids(profile)?;

    let thread_ids = threads
        .iter()
        .map(|thread| thread.id.clone())
        .collect::<HashSet<_>>();
    let mut sessions = threads
        .into_iter()
        .map(|thread| {
            let meta = metas_by_id.get(&thread.id);
            let index = index_by_id.get(&thread.id);
            let meta_archived = meta.is_some_and(|(_, archived)| *archived);
            let favorite = favorite_ids.contains(&thread.id);
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
                    .or_else(|| meta.and_then(|(m, _)| m.cwd.clone())),
                provider: thread
                    .model_provider
                    .clone()
                    .or_else(|| meta.and_then(|(m, _)| m.model_provider.clone())),
                model: thread.model.clone(),
                source: thread
                    .source
                    .clone()
                    .or_else(|| meta.and_then(|(m, _)| m.source.clone())),
                archived: thread.archived || meta_archived,
                favorite,
                updated_at: thread.updated_at.clone(),
                rollout_path: non_empty(thread.rollout_path.clone())
                    .or_else(|| meta.map(|(m, _)| m.path.display().to_string())),
                in_session_index: index.is_some(),
            }
        })
        .chain(
            metas_by_id
                .values()
                .filter(|(meta, _)| {
                    meta.id
                        .as_deref()
                        .is_some_and(|id| !thread_ids.contains(id))
                })
                .map(|(meta, archived)| {
                    let id = meta.id.clone().unwrap_or_default();
                    let index = index_by_id.get(&id);
                    SessionSummary {
                        favorite: favorite_ids.contains(&id),
                        id,
                        title: index.and_then(|entry| entry.thread_name.clone()),
                        first_user_message: None,
                        project: meta.cwd.clone(),
                        provider: meta.model_provider.clone(),
                        model: None,
                        source: meta.source.clone(),
                        archived: *archived,
                        updated_at: index.and_then(|entry| entry.updated_at.clone()),
                        rollout_path: Some(meta.path.display().to_string()),
                        in_session_index: index.is_some(),
                    }
                }),
        )
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
        SessionScope::Active if session.archived => return false,
        SessionScope::Archived if !session.archived => return false,
        SessionScope::Favorite if !session.favorite => return false,
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

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}
