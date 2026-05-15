//! Library entry point for the Codex session manager.
//!
//! The binary is intentionally thin. Most behavior lives in modules here so the
//! migration rules can be tested without invoking a command line process.

pub mod backup;
pub mod backup_store;
pub mod cli;
pub mod compact;
pub mod db_repair;
pub mod favorites;
pub mod migrate;
pub mod path_map;
pub mod profile;
pub mod restore;
pub mod rollout;
pub mod safety;
pub mod scan;
pub mod session_index;
pub mod session_list;
pub mod session_ops;
pub mod settings;
pub mod state_db;
pub mod trash;
pub mod validate;
