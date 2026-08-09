use shadowdate::model::{make_datetime, Appointment, NewAppointment, Store};
use shadowdate::service::{pending_reminders, prune_fired, reminder_time, ServiceConfig};
use chrono::{DateTime, Local, NaiveDate, TimeDelta};
use std::collections::HashSet;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn timed(uid: &str, start: (u32, u32), dur_h: i64, all_day: bool) -> Appointment {
    let uid = uid.to_string();
    Appointment::build(NewAppointment {
        series_uid: uid.clone(),
        uid,
        title: "Event".to_string(),
        description: String::new(),
        location: String::new(),
        start: make_datetime(d(2026, 8, 5), start.0, start.1),
        end: make_datetime(d(2026, 8, 5), start.0 + dur_h as u32, start.1),
        all_day,
    })
}

fn cfg(lead: u32) -> ServiceConfig {
    ServiceConfig {
        reminders: shadowdate::service::Reminders {
            lead_min: lead,
            all_day_hour: 9,
            all_day_minute: 0,
        },
    }
}

#[test]
fn reminder_time_timed_event_uses_lead() {
    let appt = timed("t1", (9, 30), 1, false);
    let rt = reminder_time(&appt, &cfg(10));
    assert_eq!(rt, appt.start - TimeDelta::minutes(10));
    assert_eq!(rt.format("%H:%M").to_string(), "09:20");
}

#[test]
fn reminder_time_allday_uses_morning_time() {
    let appt = timed("a1", (0, 0), 0, true);
    let rt = reminder_time(&appt, &cfg(10));
    assert_eq!(rt.date_naive(), appt.date());
    assert_eq!(rt.format("%H:%M").to_string(), "09:00");
}

#[test]
fn reminder_time_zero_lead_fires_at_start() {
    let appt = timed("t2", (14, 0), 1, false);
    assert_eq!(reminder_time(&appt, &cfg(0)), appt.start);
}

#[test]
fn pending_reminders_dedupes_across_ticks() {
    let appt = timed("t3", (10, 0), 1, false); // reminder 09:50
    let mut store = Store::new();
    store.insert(appt);
    let now = make_datetime(d(2026, 8, 5), 9, 50);
    let mut fired = HashSet::new();

    let first = pending_reminders(&store, &cfg(10), now, &fired);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].1.uid, "t3");
    // 09:50 is past the 09:50 reminder instant, so it must be due.
    let now2 = now + TimeDelta::seconds(5);
    let second = pending_reminders(&store, &cfg(10), now2, &fired);
    assert_eq!(second.len(), 1);
    // Simulate a successful send: the key goes into the fired set.
    let key = second[0].0.clone();
    fired.insert(key);
    let third = pending_reminders(&store, &cfg(10), now2, &fired);
    assert!(third.is_empty(), "fired reminder must not fire again");
}

#[test]
fn pending_reminders_skips_ended_events() {
    let appt = timed("t4", (8, 0), 1, false); // ends 09:00
    let mut store = Store::new();
    store.insert(appt);
    // Reminder 09:50 was never fired, but the event ended at 09:00.
    let now = make_datetime(d(2026, 8, 5), 10, 0);
    let fired = HashSet::new();
    assert!(pending_reminders(&store, &cfg(10), now, &fired).is_empty());
}

#[test]
fn pending_reminders_skips_future_reminders() {
    let appt = timed("t5", (12, 0), 1, false); // reminder 11:50
    let mut store = Store::new();
    store.insert(appt);
    let now = make_datetime(d(2026, 8, 5), 11, 0);
    let fired = HashSet::new();
    assert!(pending_reminders(&store, &cfg(10), now, &fired).is_empty());
}

#[test]
fn pending_reminders_allday_fires_once_on_start_date() {
    // All-day event covering 5..8 Aug (exclusive end = 8 Aug), like the ics
    // importer produces for a multi-day event.
    let appt = Appointment::build(NewAppointment {
        uid: "ad1".to_string(),
        series_uid: "ad1".to_string(),
        title: "Conference".to_string(),
        description: String::new(),
        location: String::new(),
        start: make_datetime(d(2026, 8, 5), 0, 0),
        end: make_datetime(d(2026, 8, 8), 0, 0),
        all_day: true,
    });
    let mut store = Store::new();
    store.insert(appt);
    let fired = HashSet::new();

    // On the start date at 09:00 it is due exactly once.
    let start_9am = make_datetime(d(2026, 8, 5), 9, 0);
    let pending = pending_reminders(&store, &cfg(10), start_9am, &fired);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].1.uid, "ad1");

    // On a middle day it must NOT be due again.
    let mid = make_datetime(d(2026, 8, 6), 9, 0);
    assert!(pending_reminders(&store, &cfg(10), mid, &fired).is_empty());
    // Before the start date it must not be due.
    let before = make_datetime(d(2026, 8, 4), 9, 0);
    assert!(pending_reminders(&store, &cfg(10), before, &fired).is_empty());
}

#[test]
fn pending_reminders_sorted_by_start() {
    // Both events are still running at 10:30 and both reminders are due.
    let mut store = Store::new();
    store.insert(Appointment::build(NewAppointment {
        uid: "later".to_string(),
        series_uid: "later".to_string(),
        title: "Event".to_string(),
        description: String::new(),
        location: String::new(),
        start: make_datetime(d(2026, 8, 5), 10, 0),
        end: make_datetime(d(2026, 8, 5), 11, 0),
        all_day: false,
    }));
    store.insert(Appointment::build(NewAppointment {
        uid: "early".to_string(),
        series_uid: "early".to_string(),
        title: "Event".to_string(),
        description: String::new(),
        location: String::new(),
        start: make_datetime(d(2026, 8, 5), 9, 0),
        end: make_datetime(d(2026, 8, 5), 12, 0),
        all_day: false,
    }));
    let now = make_datetime(d(2026, 8, 5), 10, 30);
    let fired = HashSet::new();
    let pending = pending_reminders(&store, &cfg(0), now, &fired);
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].1.uid, "early");
    assert_eq!(pending[1].1.uid, "later");
}

#[test]
fn edited_event_refires_with_new_reminder_time() {
    let appt = timed("same-uid", (10, 0), 1, false);
    let mut store = Store::new();
    store.insert(appt);
    let fired = HashSet::new();
    let now = make_datetime(d(2026, 8, 5), 10, 0);
    let first = pending_reminders(&store, &cfg(10), now, &fired);
    let key = first[0].0.clone();
    let mut fired = HashSet::new();
    fired.insert(key);

    // The user edits the appointment to a later start; same UID, new time.
    let edited = Appointment::build(NewAppointment {
        uid: "same-uid".to_string(),
        series_uid: "same-uid".to_string(),
        title: "Event".to_string(),
        description: String::new(),
        location: String::new(),
        start: make_datetime(d(2026, 8, 5), 11, 0),
        end: make_datetime(d(2026, 8, 5), 12, 0),
        all_day: false,
    });
    let mut store = Store::new();
    store.insert(edited.clone());
    let now = make_datetime(d(2026, 8, 5), 10, 50);
    let pending = pending_reminders(&store, &cfg(10), now, &fired);
    assert_eq!(pending.len(), 1, "edited time must produce a fresh reminder");
    let expected_key = format!(
        "same-uid@{}",
        shadowdate::service::reminder_time(&edited, &cfg(10)).timestamp()
    );
    assert_eq!(pending[0].0, expected_key);
}

#[test]
fn prune_fired_removes_stale_keys() {
    let mut fired = HashSet::new();
    let now: DateTime<Local> = Local::now();
    let old = now - TimeDelta::hours(7);
    let fresh = now - TimeDelta::minutes(1);
    fired.insert(format!("a@{}", old.timestamp()));
    fired.insert(format!("b@{}", fresh.timestamp()));
    prune_fired(&mut fired, now);
    assert!(!fired.contains(&format!("a@{}", old.timestamp())));
    assert!(fired.contains(&format!("b@{}", fresh.timestamp())));
}

#[test]
fn config_defaults_on_missing_file() {
    let p = std::env::temp_dir().join("shadowdate_service_missing.toml");
    std::fs::remove_file(&p).ok();
    let cfg = ServiceConfig::load(&p);
    assert_eq!(cfg.reminders.lead_min, 10);
    assert_eq!(cfg.reminders.all_day_hour, 9);
    assert_eq!(cfg.reminders.all_day_minute, 0);
}

#[test]
fn config_defaults_on_invalid_content() {
    let p = std::env::temp_dir().join("shadowdate_service_bad.toml");
    std::fs::write(&p, "not = [valid").unwrap();
    let cfg = ServiceConfig::load(&p);
    assert_eq!(cfg.reminders.lead_min, 10);
    std::fs::remove_file(&p).ok();
}

#[test]
fn config_clamps_out_of_range_values() {
    // A hand-edited config with absurd hours/minutes must not panic the daemon
    // when scheduling; values are clamped to valid ranges instead.
    let p = std::env::temp_dir().join("shadowdate_service_clamp.toml");
    std::fs::write(
        &p,
        "[reminders]\nlead_min = 5000\nall_day_hour = 99\nall_day_minute = 99\n",
    )
    .unwrap();
    let cfg = ServiceConfig::load(&p);
    assert_eq!(
        cfg.reminders.lead_min,
        shadowdate::service::MAX_LEAD_MIN,
        "lead must clamp to the shared MAX_LEAD_MIN contract"
    );
    assert_eq!(cfg.reminders.all_day_hour, 23);
    assert_eq!(cfg.reminders.all_day_minute, 59);
    std::fs::remove_file(&p).ok();
}

#[test]
fn config_save_load_roundtrip() {
    let p = std::env::temp_dir().join("shadowdate_service_rt.toml");
    let cfg = ServiceConfig {
        reminders: shadowdate::service::Reminders {
            lead_min: 30,
            all_day_hour: 8,
            all_day_minute: 45,
        },
    };
    cfg.save(&p).unwrap();
    let back = ServiceConfig::load(&p);
    assert_eq!(back.reminders.lead_min, 30);
    assert_eq!(back.reminders.all_day_hour, 8);
    assert_eq!(back.reminders.all_day_minute, 45);
    std::fs::remove_file(&p).ok();
}
