use anyhow::{bail, Result};

/// A one-way path rewrite rule.
///
/// The manager uses explicit path maps instead of guessing every possible
/// Windows/WSL conversion. That keeps migrations auditable: a path changes only
/// when it matches a rule the operator provided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathMap {
    from_raw: String,
    to_raw: String,
    from_norm: String,
    to_norm: String,
}

impl PathMap {
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Result<Self> {
        let from_raw = from.into();
        let to_raw = to.into();

        if from_raw.trim().is_empty() {
            bail!("path map source cannot be empty");
        }
        if to_raw.trim().is_empty() {
            bail!("path map destination cannot be empty");
        }

        Ok(Self {
            from_norm: normalize_path_text(&from_raw),
            to_norm: normalize_path_text(&to_raw),
            from_raw,
            to_raw,
        })
    }

    pub fn parse(spec: &str) -> Result<Self> {
        let Some((from, to)) = spec.split_once('=') else {
            bail!("path map must use FROM=TO syntax: {spec}");
        };
        Self::new(from, to)
    }

    pub fn apply(&self, value: &str) -> Option<String> {
        let value_norm = normalize_path_text(value);

        if value_norm == self.from_norm {
            return Some(self.to_norm.clone());
        }

        let prefix = format!("{}/", self.from_norm.trim_end_matches('/'));
        if let Some(suffix) = value_norm.strip_prefix(&prefix) {
            return Some(format!("{}/{}", self.to_norm.trim_end_matches('/'), suffix));
        }

        None
    }

    pub fn from(&self) -> &str {
        &self.from_raw
    }

    pub fn to(&self) -> &str {
        &self.to_raw
    }
}

/// Normalizes path-like text enough for prefix matching.
///
/// This function is deliberately conservative: it standardizes separators and
/// duplicate slashes, but it does not resolve symlinks or touch the filesystem.
pub fn normalize_path_text(value: &str) -> String {
    let mut text = value.trim().replace('\\', "/");

    while text.contains("//") {
        text = text.replace("//", "/");
    }

    // Preserve Windows drive casing as lowercase so E:\code and e:\code match.
    if text.len() >= 2 && text.as_bytes()[1] == b':' {
        let mut chars = text.chars();
        let drive = chars.next().unwrap().to_ascii_lowercase();
        let rest: String = chars.collect();
        text = format!("{drive}{rest}");
    }

    text.trim_end_matches('/').to_string()
}

pub fn apply_first_path_map(value: &str, maps: &[PathMap]) -> Option<String> {
    maps.iter().find_map(|map| map.apply(value))
}
