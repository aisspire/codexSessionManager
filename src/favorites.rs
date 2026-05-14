use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::profile::CodexProfile;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FavoritesFile {
    pub version: u32,
    pub favorites: Vec<FavoriteEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FavoriteEntry {
    pub session_id: String,
    pub starred_at_unix: i64,
}

impl Default for FavoritesFile {
    fn default() -> Self {
        Self {
            version: 1,
            favorites: Vec::new(),
        }
    }
}

pub fn favorites_path(profile: &CodexProfile) -> PathBuf {
    profile
        .codex_home
        .join("codex-session-manager")
        .join("favorites.json")
}

pub fn read_favorites(profile: &CodexProfile) -> Result<FavoritesFile> {
    let path = favorites_path(profile);
    if !path.exists() {
        return Ok(FavoritesFile::default());
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read favorites file {}", path.display()))?;
    let favorites = serde_json::from_str::<FavoritesFile>(&content)
        .with_context(|| format!("failed to parse favorites file {}", path.display()))?;
    Ok(normalize(favorites))
}

pub fn favorite_ids(profile: &CodexProfile) -> Result<HashSet<String>> {
    Ok(read_favorites(profile)?
        .favorites
        .into_iter()
        .map(|entry| entry.session_id)
        .collect())
}

pub fn set_favorite(
    profile: &CodexProfile,
    session_id: &str,
    favorite: bool,
) -> Result<FavoritesFile> {
    let session_id = session_id.trim();
    let mut favorites = read_favorites(profile)?;
    favorites
        .favorites
        .retain(|entry| entry.session_id != session_id);

    if favorite && !session_id.is_empty() {
        favorites.favorites.push(FavoriteEntry {
            session_id: session_id.to_string(),
            starred_at_unix: OffsetDateTime::now_utc().unix_timestamp(),
        });
    }

    favorites = normalize(favorites);
    write_favorites(profile, &favorites)?;
    Ok(favorites)
}

pub fn toggle_favorite(profile: &CodexProfile, session_id: &str) -> Result<FavoritesFile> {
    let ids = favorite_ids(profile)?;
    set_favorite(profile, session_id, !ids.contains(session_id))
}

fn write_favorites(profile: &CodexProfile, favorites: &FavoritesFile) -> Result<()> {
    let path = favorites_path(profile);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("failed to create favorites directory {}", parent.display())
        })?;
    }
    let content =
        serde_json::to_string_pretty(favorites).context("failed to serialize favorites")?;
    fs::write(&path, content)
        .with_context(|| format!("failed to write favorites file {}", path.display()))
}

fn normalize(file: FavoritesFile) -> FavoritesFile {
    let mut first_positions = HashMap::<String, usize>::new();
    let mut favorites = Vec::<FavoriteEntry>::new();

    for entry in file
        .favorites
        .into_iter()
        .filter(|entry| !entry.session_id.trim().is_empty())
    {
        if first_positions.contains_key(&entry.session_id) {
            continue;
        }
        first_positions.insert(entry.session_id.clone(), favorites.len());
        favorites.push(entry);
    }

    FavoritesFile {
        version: if file.version == 0 { 1 } else { file.version },
        favorites,
    }
}
