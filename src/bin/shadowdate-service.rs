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

use shadowdate::ical_import;
use shadowdate::model::Store;
use shadowdate::paths;
use shadowdate::service::{self, ServiceConfig, APP_ID, SERVICE_NAME};
use chrono::Local;
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
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
    let proxy = gio::DBusProxy::new_sync(
        conn,
        gio::DBusProxyFlags::NONE,
        None,
        Some("org.freedesktop.Notifications"),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
        None::<&gio::Cancellable>,
    )?;
    let state = Rc::new(RefCell::new(Daemon {
        proxy,
        store: Store::new(),
        fired: HashSet::new(),
        ics_mtime: None,
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
    service::prune_fired(&mut d.fired, now);
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
/// trigger stale reminders.
fn reload_store(d: &mut Daemon) {
    let path = paths::data_path();
    let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
    if mtime == d.ics_mtime {
        return;
    }
    d.ics_mtime = mtime;
    if mtime.is_none() {
        return;
    }
    match ical_import::import_ics_with_warnings(&path) {
        Ok((store, warnings)) if warnings.is_empty() => d.store = store,
        Ok((_, warnings)) => eprintln!(
            "warning: reloading {} (keeping previous store): {} entries skipped",
            path.display(),
            warnings.len()
        ),
        Err(e) => eprintln!(
            "warning: reloading {} (keeping previous store): {}",
            path.display(),
            e
        ),
    }
}
