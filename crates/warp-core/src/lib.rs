//! W.A.R.P. core: everything that isn't UI.
//!
//! - [`order`]: the load-order solver
//! - [`mp`]: multiplayer share codes and list comparison
//! - [`knowledge`]: tiers, roles and relations, merged from user, community and heuristics
//! - [`library`]: the user's mods, sets and profiles, tying it all together

pub mod import_v1;
pub mod kaedrin;
pub mod knowledge;
pub mod library;
pub mod model;
pub mod mp;
pub mod order;
pub mod steam;
pub mod store;
pub mod taxonomy;

use std::path::PathBuf;

use knowledge::KnowledgeBase;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("{0}")]
    Format(String),
    #[error("{0}")]
    Invalid(String),
    #[error("share code: {0}")]
    ShareCode(String),
    #[error("Steam: {0}")]
    Steam(String),
    #[error("workbook: {0}")]
    Excel(String),
}

/// The community knowledge base this build ships with.
const BUILTIN_KB: &str = include_str!("../../../knowledge/mods.json");

pub fn builtin_kb() -> KnowledgeBase {
    KnowledgeBase::parse(BUILTIN_KB).expect("built-in knowledge/mods.json is valid")
}

/// Where WARP keeps per-user data. `WARP_HOME` overrides it (handy for testing).
pub fn data_dir() -> PathBuf {
    std::env::var_os("WARP_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("WARP"))
}

pub fn default_db_path() -> PathBuf {
    data_dir().join("warp.db")
}
