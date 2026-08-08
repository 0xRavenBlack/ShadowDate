use std::path::PathBuf;

/// XDG data directory (`$XDG_DATA_HOME` or `~/.local/share`).
pub fn dirs_data() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| {
                let mut p = PathBuf::from(h);
                p.push(".local");
                p.push("share");
                p
            })
        })
}

/// XDG config directory (`$XDG_CONFIG_HOME` or `~/.config`).
pub fn dirs_config() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| {
                let mut p = PathBuf::from(h);
                p.push(".config");
                p
            })
        })
}

/// Persistent appointment store: `$XDG_DATA_HOME/calendar/calendar.ics`
/// (fallback: `~/.local/share/calendar/calendar.ics`, then the temp dir).
pub fn data_path() -> PathBuf {
    let mut p = dirs_data().unwrap_or_else(std::env::temp_dir);
    p.push("calendar");
    p.push("calendar.ics");
    p
}

/// Service config file: `$XDG_CONFIG_HOME/shadowdate/service.toml`
/// (fallback: `~/.config/shadowdate/service.toml`). Returns an empty path when
/// neither XDG_CONFIG_HOME nor HOME is set, meaning "no config file".
pub fn config_path() -> PathBuf {
    match dirs_config() {
        Some(mut p) => {
            p.push("shadowdate");
            p.push("service.toml");
            p
        }
        None => PathBuf::new(),
    }
}
