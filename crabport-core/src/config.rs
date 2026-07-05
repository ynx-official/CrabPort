//! Application configuration (`config.toml`).
//!
//! A single process-wide `CrabPortConfig` is exposed via the [`CONFIG`]
//! `LazyLock` — load-on-first-access, mutate through [`update`], and persist
//! to `{data_dir}/crabport/config.toml`.
//!
//! Why a `LazyLock` instead of a GPUI global? The settings window needs to
//! read/write config from contexts that may not have a `cx` handy (e.g. the
//! terminal pane reading its font size), and we want the same handle to be
//! reachable from `crabport-core` without introducing a circular dependency
//! on `gpui`. A `parking_lot::RwLock`-guarded `Arc` matches the
//! `Send + Sync` requirements of a static.
//!
//! # File layout
//!
//! ```text
//! {data_dir}/crabport/
//!   crabport.db       — SQLite database (hosts, credentials, ...)
//!   .key              — AES-256 encryption key
//!   config.toml       — this module's persisted config
//! ```

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Sub-config structs
// ---------------------------------------------------------------------------

/// User-configurable appearance settings. Stored under `[appearance]` in
/// `config.toml`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppearanceConfig {
    /// Currently-active UI language code, e.g. "en" or "zh-CN". Mirrors
    /// the value passed to `rust_i18n::set_locale` in the binary crate.
    #[serde(default = "default_locale")]
    pub locale: String,
}

fn default_locale() -> String {
    "en".to_string()
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            locale: default_locale(),
        }
    }
}

/// Top-level config root, serialized to `config.toml` and reachable via the
/// [`CONFIG`] `LazyLock`.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CrabPortConfig {
    #[serde(default)]
    pub appearance: AppearanceConfig,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ConfigError {
    Io(String),
    Parse(String),
    Serialize(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "IO: {e}"),
            ConfigError::Parse(e) => write!(f, "Parse: {e}"),
            ConfigError::Serialize(e) => write!(f, "Serialize: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<toml::de::Error> for ConfigError {
    fn from(e: toml::de::Error) -> Self {
        ConfigError::Parse(e.to_string())
    }
}

impl From<toml::ser::Error> for ConfigError {
    fn from(e: toml::ser::Error) -> Self {
        ConfigError::Serialize(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// LazyLock global
// ---------------------------------------------------------------------------

/// Process-wide configuration handle. Initialized on first access from the
/// on-disk `config.toml` (or defaults if the file does not exist yet).
pub static CONFIG: LazyLock<Arc<RwLock<CrabPortConfig>>> =
    LazyLock::new(|| Arc::new(RwLock::new(load().unwrap_or_default())));

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Path to the `config.toml` file inside the CrabPort data directory.
/// Re-uses the same `dirs::data_dir()` root as the SQLite store so config and
/// credentials live next to each other.
pub fn config_path() -> Result<PathBuf, ConfigError> {
    let base =
        dirs::data_dir().ok_or_else(|| ConfigError::Io("cannot determine data dir".into()))?;
    Ok(base.join("crabport").join("config.toml"))
}

// ---------------------------------------------------------------------------
// Load / save
// ---------------------------------------------------------------------------

/// Read `config.toml` from disk and deserialize it. Returns `Ok(defaults)`
/// when the file does not exist yet (fresh install) so callers don't have to
/// distinguish "missing" from "present".
pub fn load() -> Result<CrabPortConfig, ConfigError> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(CrabPortConfig::default());
    }
    let text = fs::read_to_string(&path).map_err(|e| ConfigError::Io(e.to_string()))?;
    let cfg: CrabPortConfig = toml::from_str(&text)?;
    Ok(cfg)
}

/// Serialize and atomically write `cfg` to `config.toml`. Creates the parent
/// directory if needed. Atomicity is provided by writing to a `.tmp` file and
/// renaming — a crash mid-write won't corrupt the existing config.
pub fn save(cfg: &CrabPortConfig) -> Result<(), ConfigError> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| ConfigError::Io(e.to_string()))?;
    }
    let text = toml::to_string_pretty(cfg)?;
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, text).map_err(|e| ConfigError::Io(e.to_string()))?;
    fs::rename(&tmp, &path).map_err(|e| ConfigError::Io(e.to_string()))?;
    Ok(())
}

/// Mutate the live config inside the [`CONFIG`] lock, then persist it to
/// disk. Use this from the UI: the closure sees a `&mut CrabPortConfig`, and
/// the lock is held only for the duration of the mutation.
///
/// Returns the *post-mutation* snapshot so callers can react to the new
/// values (e.g. apply the new locale).
pub fn update<R>(f: impl FnOnce(&mut CrabPortConfig) -> R) -> Result<R, ConfigError> {
    let mut guard = CONFIG.write();
    let ret = f(&mut guard);
    save(&guard)?;
    Ok(ret)
}

/// Convenience: take a read lock and clone the current config snapshot.
pub fn snapshot() -> CrabPortConfig {
    CONFIG.read().clone()
}
