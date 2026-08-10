//! `shadowdate-service` — background reminder daemon.
//!
//! Watches the calendar's `.ics` store and the service config, and fires
//! desktop notifications through the freedesktop Notification Protocol on the
//! session bus. Runs headless (no GTK widgets, no window): all it needs is a
//! session D-Bus, so it behaves like any other login-session daemon.
//!
//! Single instance: it owns the well-known name `org.ravenblack.ShadowDate.Service`
//! with `DO_NOT_QUEUE`; a second invocation sees the name is taken, logs a note,
//! and exits immediately.
//!
//! Scheduling policy lives in `shadowdate::service` (pure functions, unit-tested);
//! this binary only wires them to the clock, the file system, and D-Bus.

use shadowdate::config::ServiceConfig;
use shadowdate::ical_import;
use shadowdate::model::Store;
use shadowdate::paths;
use shadowdate::service::{self, APP_ID, SERVICE_NAME};
use chrono::Local;
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::time::SystemTime;

/// How often to poll for file changes and due reminders.
const TICK_SECONDS: u32 = 1;

fn main() -> glib::ExitCode {
    let conn = match gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("shadowdate-service: no session bus: {}", e);
            return glib::ExitCode::FAILURE;
        }
    };

    let main_loop = glib::MainLoop::new(None, false);
    let acquired = Rc::new(std::cell::Cell::new(false));

    gio::bus_own_name_on_connection(
        &conn,
        SERVICE_NAME,
        gio::BusNameOwnerFlags::DO_NOT_QUEUE,
        {
            let acquired = acquired.clone();
            move |conn, _name| {
                acquired.set(true);
                if let Err(e) = setup(&conn) {
                    eprintln!("shadowdate-service: {}", e);
                    std::process::exit(1);
                }
            }
        },
        {
            let main_loop = main_loop.clone();
            let acquired = acquired.clone();
            move |_conn, _name| {
                if !acquired.get() {
                    eprintln!("shadowdate-service: another instance is already running");
                } else {
                    eprintln!("shadowdate-service: lost session-bus name; exiting");
                }
                main_loop.quit();
            }
        },
    );

    main_loop.run();
    glib::ExitCode::SUCCESS
}

fn setup(conn: &gio::DBusConnection) -> anyhow::Result<()> {
    let proxy = service::notification_proxy_on(conn)?;
    let state = Rc::new(RefCell::new(Daemon {
        proxy,
        store: Store::new(),
        fired: HashSet::new(),
        ics_mtime: None,
        ics_warned_mtime: None,
        config: ServiceConfig::default(),
        cfg_mtime: None,
    }));
    glib::timeout_add_seconds_local(TICK_SECONDS, move || {
        tick(&state);
        glib::ControlFlow::Continue
    });
    Ok(())
}

struct Daemon {
    proxy: gio::DBusProxy,
    store: Store,
    fired: HashSet<String>,
    ics_mtime: Option<SystemTime>,
    ics_warned_mtime: Option<SystemTime>,
    config: ServiceConfig,
    cfg_mtime: Option<SystemTime>,
}

fn tick(state: &Rc<RefCell<Daemon>>) {
    let mut d = state.borrow_mut();
    reload_config(&mut d);
    reload_store(&mut d);
    let now = Local::now();
    // Collect owned (key, summary, body) triples first so the immutable borrow
    // of the store (held by the pending references) never overlaps the mutable
    // `fired` insert below.
    let due: Vec<(String, String, String)> = service::pending_reminders(&d.store, &d.config, now, &d.fired)
        .into_iter()
        .map(|(key, appt)| (key, appt.title.clone(), service::reminder_body(appt)))
        .collect();
    for (key, summary, body) in due {
        match service::notify(&d.proxy, &summary, &body, APP_ID) {
            Ok(_) => d.fired.insert(key),
            Err(e) => {
                eprintln!("warning: sending reminder notification: {}", e);
                // Don't mark as fired: retry on the next tick.
                false
            }
        };
    }
    // The fired set and the store are disjoint fields; reborrow through a plain
    // `&mut` reference (not the `RefMut`) so the borrow checker can split them.
    let d = &mut *d;
    service::prune_fired(&mut d.fired, &d.store, now);
}

/// Reload the config whenever its file changes. A parse failure keeps the last
/// good config; `ServiceConfig::load` already warns and returns defaults.
fn reload_config(d: &mut Daemon) {
    let path = paths::config_path();
    if path.as_os_str().is_empty() {
        return;
    }
    let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
    if mtime == d.cfg_mtime {
        return;
    }
    d.cfg_mtime = mtime;
    d.config = ServiceConfig::load(&path);
}

/// Reload the store whenever the `.ics` file changes. A torn/parse-error file
/// keeps the last good store (atomic app saves make this rare); a deleted file
/// keeps the last known store so a transient remove/recreate cycle doesn't
/// trigger stale reminders. The change marker (`ics_mtime`) is only advanced
/// when a reload fully succeeds, so a bad file is retried on every tick instead
/// of permanently diverging the daemon from the on-disk truth. The warning is
/// printed once per offending mtime, not once per tick.
fn reload_store(d: &mut Daemon) {
    let path = paths::data_path();
    let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
    if mtime == d.ics_mtime {
        return;
    }
    if mtime.is_none() {
        d.ics_mtime = None;
        d.ics_warned_mtime = None;
        return;
    }
    match ical_import::import_ics_with_warnings(&path) {
        Ok((store, warnings)) if warnings.is_empty() => {
            d.store = store;
            d.ics_mtime = mtime;
            d.ics_warned_mtime = None;
        }
        Ok((_, warnings)) => warn_reload(&mut *d, &path, mtime, &format!("{} entries skipped", warnings.len())),
        Err(e) => warn_reload(&mut *d, &path, mtime, &e.to_string()),
    }
}

/// Log a reload failure once per offending mtime, leaving `ics_mtime` untouched
/// so the reload is retried on the next tick.
fn warn_reload(d: &mut Daemon, path: &Path, mtime: Option<SystemTime>, detail: &str) {
    if d.ics_warned_mtime == mtime {
        return;
    }
    eprintln!(
        "warning: reloading {} (keeping previous store, will retry): {}",
        path.display(),
        detail
    );
    d.ics_warned_mtime = mtime;
}
