//! Reminder scheduling + notification, shared by the `shadowdate` app and the
//! `shadowdate-service` notification daemon.
//!
//! The on-disk `.ics` store is fully RRULE-expanded (every occurrence is its own
//! `VEVENT`), so the daemon schedules reminders straight off the imported
//! `Store` with no recurrence logic. Timed events are reminded `lead_min`
//! minutes before `start`; all-day events are reminded once, at
//! `all_day_hour:all_day_minute` on their start date. The settings themselves
//! (including the app's appearance preferences) live in [`crate::config`].
//!
//! `pending_reminders` is a pure function (no I/O) so the dedupe / due-window
//! rules are unit-testable without a D-Bus daemon.

use crate::config::ServiceConfig;
use crate::model::{Appointment, Store};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Local, TimeDelta};
use gio::prelude::*;
use std::collections::HashSet;

/// Application id, also the themed icon name and the D-Bus notification
/// `desktop-entry` hint.
pub const APP_ID: &str = "0xravenblack.shadowdata";

/// Session-bus well-known name that guards against a second service instance.
pub const SERVICE_NAME: &str = "org.ravenblack.ShadowDate.Service";

/// Freedesktop notification protocol bus.
const NOTIFY_IFACE: &str = "org.freedesktop.Notifications";
const NOTIFY_PATH: &str = "/org/freedesktop/Notifications";

/// When an appointment's reminder should fire. Timed events are reminded
/// `lead_min` minutes early; all-day events at the configured morning time on
/// the start date.
pub fn reminder_time(appt: &Appointment, cfg: &ServiceConfig) -> DateTime<Local> {
    if appt.all_day {
        crate::model::make_datetime(appt.date(), cfg.reminders.all_day_hour, cfg.reminders.all_day_minute)
    } else {
        appt.start - TimeDelta::minutes(cfg.reminders.lead_min as i64)
    }
}

/// Dedupe key for a fired reminder. Includes the reminder instant so editing an
/// appointment (same UID, new time) produces a fresh notification.
fn fired_key(appt: &Appointment, rt: &DateTime<Local>) -> String {
    format!("{}@{}", appt.uid, rt.timestamp())
}

/// Appointments whose reminder is due `now` and has not fired yet.
///
/// A reminder is due when its `reminder_time` is in the past but the event has
/// not ended (so a reminder missed while the daemon slept still shows up for an
/// ongoing event). All-day events only fire on their start date, so a multi-day
/// all-day event is announced exactly once. Results are ordered by start time.
pub fn pending_reminders<'a>(
    store: &'a Store,
    cfg: &ServiceConfig,
    now: DateTime<Local>,
    fired: &HashSet<String>,
) -> Vec<(String, &'a Appointment)> {
    let mut pending: Vec<(String, &'a Appointment)> = Vec::new();
    for appt in store.items() {
        if appt.end <= now {
            continue;
        }
        if appt.all_day && appt.date() != now.date_naive() {
            continue;
        }
        let rt = reminder_time(appt, cfg);
        if rt > now {
            continue;
        }
        let key = fired_key(appt, &rt);
        if !fired.contains(&key) {
            pending.push((key, appt));
        }
    }
    pending.sort_by_key(|(_, a)| a.start);
    pending
}

/// Drop fired keys whose appointment is no longer due.
///
/// A reminder is "due" from its `reminder_time` until the event ends (for
/// all-day events, until the end of the start date). The dedupe key must live
/// exactly that long or a still-running event would re-fire — an all-day event
/// stays due up to ~24 h, far past any fixed time-based retention. A key is
/// therefore kept only while its event is still due (same rules as
/// [`pending_reminders`]); the moment the event ends or is deleted the key is
/// pruned, so the set stays bounded by the number of live events. Events
/// re-added under the same UID while still due simply fire again, which is the
/// correct "this is a fresh reminder" behaviour.
pub fn prune_fired(fired: &mut HashSet<String>, store: &Store, now: DateTime<Local>) {
    fired.retain(|k| match k.rfind('@') {
        Some(i) => match store.get(&k[..i]) {
            Some(appt) => is_due(appt, now),
            None => false,
        },
        None => true,
    });
}

/// Whether an appointment's reminder can still be due, i.e. the event has not
/// ended and (for all-day events) today is still the start date.
fn is_due(appt: &Appointment, now: DateTime<Local>) -> bool {
    appt.end > now && (!appt.all_day || appt.date() == now.date_naive())
}

/// Connect a proxy to the session notification daemon.
pub fn notification_proxy() -> Result<gio::DBusProxy> {
    let conn = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>)
        .map_err(|e| anyhow!("connecting to session bus: {}", e))?;
    gio::DBusProxy::new_sync(
        &conn,
        gio::DBusProxyFlags::NONE,
        None,
        Some(NOTIFY_IFACE),
        NOTIFY_PATH,
        NOTIFY_IFACE,
        None::<&gio::Cancellable>,
    )
    .map_err(|e| anyhow!("creating notification proxy: {}", e))
}

/// Build the `Notify` method-call argument tuple `(susssasa{sv}i)`.
///
/// The tuple is assembled with `Variant::tuple_from_iter` so each child keeps
/// its own type — in particular the hints `a{sv}` must stay `a{sv}`. A plain
/// Rust tuple passed through `.to_variant()` would box the hints into a `v`,
/// which notification daemons reject with `InvalidArgs`.
fn notify_args(summary: &str, body: &str, icon: &str) -> glib::Variant {
    let hints = glib::VariantDict::new(None);
    hints.insert_value("urgency", &glib::Variant::from(1u32));
    hints.insert_value("category", &glib::Variant::from("reminder"));
    hints.insert_value("desktop-entry", &glib::Variant::from(APP_ID));
    glib::Variant::tuple_from_iter([
        "ShadowDate".to_variant(),       // app_name
        0u32.to_variant(),               // replaces_id: never replace
        icon.to_variant(),               // app_icon
        summary.to_variant(),            // summary
        body.to_variant(),               // body
        Vec::<&str>::new().to_variant(), // actions (none)
        hints.end(),                     // hints: a{sv}
        (-1i32).to_variant(),            // expire_timeout: server default
    ])
}

/// Send a desktop notification through the freedesktop Notification Protocol.
/// Returns the daemon-assigned notification id.
pub fn notify(
    proxy: &gio::DBusProxy,
    summary: &str,
    body: &str,
    icon: &str,
) -> Result<u32> {
    let args = notify_args(summary, body, icon);
    let reply = proxy.call_sync(
        "Notify",
        Some(&args),
        gio::DBusCallFlags::NONE,
        -1,
        None::<&gio::Cancellable>,
    )?;
    let (id,) = reply.get::<(u32,)>().unwrap_or((0,));
    Ok(id)
}

/// Body text for an appointment reminder, localized through `i18n`.
pub fn reminder_body(appt: &Appointment) -> String {
    let all_day = crate::i18n::t("all_day");
    if appt.all_day {
        if appt.location.is_empty() {
            all_day.to_string()
        } else {
            format!("{} · {}", all_day, appt.location)
        }
    } else {
        let times = crate::i18n::time_range(&appt.start, &appt.end);
        if appt.location.is_empty() {
            times
        } else {
            format!("{} · {}", times, appt.location)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_args_have_expected_signature() {
        let args = notify_args("s", "b", "i");
        assert_eq!(args.type_().as_str(), "(susssasa{sv}i)");
    }
}
