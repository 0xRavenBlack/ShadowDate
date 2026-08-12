use std::ffi::OsString;
use std::path::PathBuf;

/// Read an env var, treating a set-but-empty value as unset. Per the XDG base
/// directory spec, an empty `XDG_DATA_HOME`/`XDG_CONFIG_HOME` means "not set";
/// treating it as a real directory would redirect the store/config into the
/// current working directory (`./calendar/calendar.ics`).
fn non_empty_env(key: &str) -> Option<OsString> {
    std::env::var_os(key).filter(|v| !v.is_empty())
}

/// XDG data directory (`$XDG_DATA_HOME` or `~/.local/share`).
pub fn dirs_data() -> Option<PathBuf> {
    non_empty_env("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            non_empty_env("HOME").map(|h| {
                let mut p = PathBuf::from(h);
                p.push(".local");
                p.push("share");
                p
            })
        })
}

/// XDG config directory (`$XDG_CONFIG_HOME` or `~/.config`).
pub fn dirs_config() -> Option<PathBuf> {
    non_empty_env("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            non_empty_env("HOME").map(|h| {
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

/// Single-instance lock file: `$XDG_RUNTIME_DIR/shadowdate.lock` (fallback:
/// the data dir, then the temp dir). Held with an advisory `flock` for the app's
/// whole lifetime; kernel locks die with the process, so no stale-lock cleanup
/// is ever needed.
pub fn lock_path() -> PathBuf {
    if let Some(dir) = non_empty_env("XDG_RUNTIME_DIR") {
        let mut p = PathBuf::from(dir);
        p.push("shadowdate.lock");
        return p;
    }
    let mut p = dirs_data().unwrap_or_else(std::env::temp_dir);
    p.push("shadowdate.lock");
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
