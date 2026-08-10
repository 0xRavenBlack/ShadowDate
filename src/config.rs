//! App configuration shared by the `shadowdate` GUI and the `shadowdate-service`
//! notification daemon.
//!
//! A single TOML file (`$XDG_CONFIG_HOME/shadowdate/service.toml`) holds both the
//! reminder settings the daemon consumes and the appearance settings the GUI
//! consumes. The daemon watches the file and picks up changes without a restart;
//! the app's ⚙️ dialog edits it.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Maximum reminder lead time. A lead of more than a day warns absurdly early.
/// Single source of truth shared by the settings dialog spin range and the
/// config sanitizer, so the UI and the file-backed contract can't drift.
pub const MAX_LEAD_MIN: u32 = 24 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceConfig {
    #[serde(default)]
    pub reminders: Reminders,
    #[serde(default)]
    pub appearance: Appearance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminders {
    /// Minutes before a timed event's start at which its reminder fires.
    pub lead_min: u32,
    /// Wall-clock time at which all-day events are reminded (on their start date).
    pub all_day_hour: u32,
    pub all_day_minute: u32,
}

impl Default for Reminders {
    fn default() -> Self {
        Self {
            lead_min: 10,
            all_day_hour: 9,
            all_day_minute: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Appearance {
    /// Whether the translucent background portrait is shown behind the grid.
    pub show_portrait: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self { show_portrait: true }
    }
}

impl ServiceConfig {
    /// Load the config from `path`. A missing or unreadable file yields the
    /// defaults; an unparseable file yields the defaults plus a warning.
    pub fn load(path: &Path) -> ServiceConfig {
        if path.as_os_str().is_empty() || !path.exists() {
            return ServiceConfig::default();
        }
        match fs::read_to_string(path) {
            Ok(content) => match toml::from_str::<ServiceConfig>(&content) {
                // Sanitize hand-edited values so the daemon can never panic on an
                // out-of-range wall-clock time (see `sanitize_config`).
                Ok(cfg) => sanitize_config(cfg),
                Err(e) => {
                    eprintln!(
                        "warning: invalid service config {} ({}); using defaults",
                        path.display(),
                        e
                    );
                    ServiceConfig::default()
                }
            },
            Err(e) => {
                eprintln!(
                    "warning: reading service config {}: {}; using defaults",
                    path.display(),
                    e
                );
                ServiceConfig::default()
            }
        }
    }

    /// Write the config to `path` (atomically, like the `.ics` store).
    pub fn save(&self, path: &Path) -> Result<()> {
        let data = toml::to_string(self).context("serializing service config")?;
        crate::store_io::write_atomic(path, &data)
    }
}

/// Clamp reminder config values to valid ranges. The settings dialog already
/// constrains its inputs, but the config file is user-editable and the daemon
/// reads it directly: a bad hour/minute would otherwise panic `make_datetime`
/// (release builds use `panic = "abort"`, so the unit would crash-loop).
fn sanitize_config(mut cfg: ServiceConfig) -> ServiceConfig {
    cfg.reminders.all_day_hour = cfg.reminders.all_day_hour.min(23);
    cfg.reminders.all_day_minute = cfg.reminders.all_day_minute.min(59);
    // A lead of more than a day warns absurdly early; cap it to `MAX_LEAD_MIN`.
    cfg.reminders.lead_min = cfg.reminders.lead_min.min(MAX_LEAD_MIN);
    cfg
}
