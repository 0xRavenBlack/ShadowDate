//! Reading and writing the persistent `.ics` store on disk.
//!
//! Writes go through `write_atomic` (temp file + rename) so a concurrently
//! reading reminder daemon never observes a torn file, and corrupt input is
//! backed up before it could be overwritten.

use crate::ical_export::{store_to_ics, PRODID};
use crate::ical_import::parse_ics;
use crate::model::Store;
use anyhow::{Context, Result};
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};

/// Write `data` to `path` atomically: write to a temp file in the same
/// directory, then rename over the target. A concurrent reader (like the
/// background `shadowdate-service`) can never observe a partially-written file.
///
/// The temp name carries a unique pid + timestamp suffix, so concurrent writers
/// never clobber each other's in-flight file. A stale temp left by a crash
/// between write and rename is harmless (it is never the target) and the temp
/// is best-effort removed if the write or rename fails.
pub fn write_atomic(path: &Path, data: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    let tmp = temp_path(path);
    if let Err(e) = fs::write(&tmp, data) {
        fs::remove_file(&tmp).ok();
        return Err(e).with_context(|| format!("writing {}", tmp.display()));
    }
    if let Err(e) = fs::rename(&tmp, path) {
        fs::remove_file(&tmp).ok();
        return Err(e).with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()));
    }
    Ok(())
}

/// A unique temp-file name next to `path` (same directory, so the rename stays
/// on one filesystem and is atomic). Pid + timestamp guard against collisions
/// between concurrent writers of the same target.
fn temp_path(path: &Path) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    path.with_extension(format!("tmp.{}.{}", std::process::id(), nanos))
}

/// Load the persistent store from the default data file, if it exists.
///
/// A missing file yields an empty store; a file that cannot be read at all is
/// an error. Parse problems *inside* an existing file are reported as warnings
/// (so the caller can back the file up) rather than silently wiping the
/// calendar, and individual bad entries are skipped instead of failing the
/// whole load.
pub fn load_store(path: &Path) -> Result<(Store, Vec<String>)> {
    if !path.exists() {
        return Ok((Store::new(), Vec::new()));
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    Ok(parse_ics(&content))
}

/// Save the store to the default data file (also the export format). Written
/// atomically so a concurrently-reading reminder daemon never sees a torn file.
pub fn save_store(store: &Store, path: &Path) -> Result<()> {
    let data = store_to_ics(store, PRODID);
    write_atomic(path, &data)
}

/// Best-effort copy of a calendar file that could not be loaded, so the user's
/// data survives even when the app starts with a corrupt/unreadable file (and
/// the next save would otherwise overwrite it with an empty store). Returns the
/// backup path, or `None` if the file could not be read or copied.
pub fn backup_corrupt(path: &Path) -> Option<PathBuf> {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("warning: could not back up {}: {}", path.display(), e);
            return None;
        }
    };
    let ts = Local::now().format("%Y%m%d%H%M%S");
    let backup = path.with_extension(format!("ics.corrupt-{}.bak", ts));
    match fs::write(&backup, data) {
        Ok(()) => {
            eprintln!("backed up unreadable calendar to {}", backup.display());
            Some(backup)
        }
        Err(e) => {
            eprintln!("warning: could not write backup {}: {}", backup.display(), e);
            None
        }
    }
}

/// Merge another store into this one. For each series present in `other`
/// (identified by `series_uid`), first remove all existing occurrences of that
/// series from `base` so that a modified RRULE does not leave orphaned old
/// occurrences behind.
pub fn merge_store(base: &mut Store, other: Store) {
    let series_uids: Vec<String> = other
        .items()
        .iter()
        .map(|a| a.series_uid.clone())
        .collect();
    let mut seen = std::collections::HashSet::new();
    for uid in &series_uids {
        if seen.insert(uid.clone()) {
            base.remove_series(uid);
        }
    }
    for a in other.into_items() {
        base.insert(a);
    }
}
