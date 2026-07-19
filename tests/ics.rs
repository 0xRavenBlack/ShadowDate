use calendar::io_ics;
use calendar::model::{make_datetime, Appointment, Store};
use chrono::NaiveDate;

#[test]
fn roundtrip_ics() {
    let mut store = Store::new();
    let a = Appointment::with_uid(
        "test-uid-1".to_string(),
        "Dentist".to_string(),
        "Checkup".to_string(),
        "Clinic".to_string(),
        make_datetime(chrono::NaiveDate::from_ymd_opt(2026, 8, 5).unwrap(), 9, 30),
        make_datetime(chrono::NaiveDate::from_ymd_opt(2026, 8, 5).unwrap(), 10, 0),
        false,
    );
    store.insert(a);

    let ics = io_ics::store_to_ics(&store, "-//test//EN");
    assert!(ics.contains("BEGIN:VCALENDAR"));
    assert!(ics.contains("UID:test-uid-1"));
    assert!(ics.contains("SUMMARY:Dentist"));

    // parse it back
    let path = std::env::temp_dir().join("cal_test_roundtrip.ics");
    std::fs::write(&path, &ics).unwrap();
    let imported = io_ics::import_ics(&path).unwrap();
    assert_eq!(imported.items.len(), 1);
    let back = &imported.items[0];
    assert_eq!(back.uid, "test-uid-1");
    assert_eq!(back.title, "Dentist");
    assert_eq!(back.location, "Clinic");
    assert_eq!(back.start.format("%H:%M").to_string(), "09:30");
    std::fs::remove_file(&path).ok();
}

#[test]
fn load_nonexistent_is_empty() {
    let p = std::env::temp_dir().join("cal_does_not_exist_xyz.ics");
    let store = io_ics::load_store(&p);
    assert!(store.items.is_empty());
}

#[test]
fn allday_multiday_visible_on_each_day() {
    // iCalendar all-day event: 5..7 Aug (exclusive DTEND = 8 Aug).
    let ics = "\
BEGIN:VCALENDAR\r\n
VERSION:2.0\r\n
PRODID:-//test//EN\r\n
BEGIN:VEVENT\r\n
UID:multi-1\r\n
SUMMARY:Conference\r\n
DTSTART;VALUE=DATE:20260805\r\n
DTEND;VALUE=DATE:20260808\r\n
END:VEVENT\r\n
END:VCALENDAR\r\n";
    let path = std::env::temp_dir().join("cal_test_allday.ics");
    std::fs::write(&path, ics).unwrap();
    let store = io_ics::import_ics(&path).unwrap();
    assert_eq!(store.items.len(), 1);
    let a = &store.items[0];
    assert!(a.all_day);
    let d = |y, m, day| chrono::NaiveDate::from_ymd_opt(y, m, day).unwrap();
    // Visible on start, middle, and last covered day.
    assert_eq!(store.on_date(d(2026, 8, 5)).len(), 1);
    assert_eq!(store.on_date(d(2026, 8, 6)).len(), 1);
    assert_eq!(store.on_date(d(2026, 8, 7)).len(), 1);
    // Not visible the day before or the exclusive end day.
    assert_eq!(store.on_date(d(2026, 8, 4)).len(), 0);
    assert_eq!(store.on_date(d(2026, 8, 8)).len(), 0);
    std::fs::remove_file(&path).ok();
}

#[test]
fn allday_missing_dtend_defaults_to_one_day() {
    let ics = "\
BEGIN:VCALENDAR\r\n
VERSION:2.0\r\n
PRODID:-//test//EN\r\n
BEGIN:VEVENT\r\n
UID:single-1\r\n
SUMMARY:Holiday\r\n
DTSTART;VALUE=DATE:20260910\r\n
END:VEVENT\r\n
END:VCALENDAR\r\n";
    let path = std::env::temp_dir().join("cal_test_allday_single.ics");
    std::fs::write(&path, ics).unwrap();
    let store = io_ics::import_ics(&path).unwrap();
    let a = &store.items[0];
    assert!(a.all_day);
    let d = |y, m, day| chrono::NaiveDate::from_ymd_opt(y, m, day).unwrap();
    assert_eq!(store.on_date(d(2026, 9, 10)).len(), 1);
    assert_eq!(store.on_date(d(2026, 9, 11)).len(), 0);
    // Round-trips to an exclusive DTEND on the next day.
    let out = io_ics::store_to_ics(&store, "-//test//EN");
    assert!(out.contains("DTEND;VALUE=DATE:20260911"));
    std::fs::remove_file(&path).ok();
}

fn write_ics(name: &str, body: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).unwrap();
    path
}

#[test]
fn text_escaping_roundtrips() {
    // Special characters in SUMMARY/DESCRIPTION/LOCATION must survive a
    // save -> load cycle: backslash, semicolon, comma, and newlines.
    let mut store = Store::new();
    store.insert(Appointment::with_uid(
        "escape-1".to_string(),
        "a;b,c\\d".to_string(),
        "line1\nline2;x,y\\z".to_string(),
        "Rome, Italy;near\\colosseum".to_string(),
        make_datetime(d(2026, 8, 5), 9, 30),
        make_datetime(d(2026, 8, 5), 10, 0),
        false,
    ));
    let path = std::env::temp_dir().join("cal_test_escape.ics");
    io_ics::save_store(&store, &path).unwrap();
    let loaded = io_ics::import_ics(&path).unwrap();
    let a = &loaded.items[0];
    assert_eq!(a.title, "a;b,c\\d");
    assert_eq!(a.description, "line1\nline2;x,y\\z");
    assert_eq!(a.location, "Rome, Italy;near\\colosseum");
    std::fs::remove_file(&path).ok();
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

#[test]
fn rrule_daily_count_expands() {
    // Daily for 3 days starting 2026-08-05.
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n\
BEGIN:VEVENT\r\nUID:daily-1\r\nSUMMARY:Standup\r\n\
DTSTART;VALUE=DATE:20260805\r\nDTEND;VALUE=DATE:20260806\r\n\
RRULE:FREQ=DAILY;COUNT=3\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let path = write_ics("cal_test_rrule_daily.ics", ics);
    let store = io_ics::import_ics(&path).unwrap();
    assert_eq!(store.items.len(), 3, "daily COUNT=3 should yield 3 occurrences");
    assert_eq!(store.on_date(d(2026, 8, 5)).len(), 1);
    assert_eq!(store.on_date(d(2026, 8, 6)).len(), 1);
    assert_eq!(store.on_date(d(2026, 8, 7)).len(), 1);
    assert_eq!(store.on_date(d(2026, 8, 8)).len(), 0);
    // All occurrences share the series uid and color.
    assert!(store.items.iter().all(|a| a.series_uid == "daily-1"));
    std::fs::remove_file(&path).ok();
}

#[test]
fn rrule_weekly_byday_expands() {
    // Weekly on Mon/Wed/Fri for 2 weeks starting Wed 2026-08-05.
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n\
BEGIN:VEVENT\r\nUID:weekly-1\r\nSUMMARY:Class\r\n\
DTSTART;VALUE=DATE:20260805\r\nDTEND;VALUE=DATE:20260806\r\n\
RRULE:FREQ=WEEKLY;COUNT=6;BYDAY=MO,WE,FR\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let path = write_ics("cal_test_rrule_weekly.ics", ics);
    let store = io_ics::import_ics(&path).unwrap();
    // 2 weeks * 3 days = 6 occurrences.
    assert_eq!(store.items.len(), 6);
    // First week: Wed 5, Fri 7. Second: Mon 10, Wed 12, Fri 14.
    assert_eq!(store.on_date(d(2026, 8, 5)).len(), 1);
    assert_eq!(store.on_date(d(2026, 8, 7)).len(), 1);
    assert_eq!(store.on_date(d(2026, 8, 10)).len(), 1);
    assert_eq!(store.on_date(d(2026, 8, 14)).len(), 1);
    // No occurrence on Thu 6 or Sun 9.
    assert_eq!(store.on_date(d(2026, 8, 6)).len(), 0);
    assert_eq!(store.on_date(d(2026, 8, 9)).len(), 0);
    std::fs::remove_file(&path).ok();
}

#[test]
fn rrule_monthly_bymonthday_expands() {
    // Monthly on the 15th, 3 occurrences from 2026-01-15.
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n\
BEGIN:VEVENT\r\nUID:monthly-1\r\nSUMMARY:Pay\r\n\
DTSTART;VALUE=DATE:20260115\r\nDTEND;VALUE=DATE:20260116\r\n\
RRULE:FREQ=MONTHLY;COUNT=3;BYMONTHDAY=15\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let path = write_ics("cal_test_rrule_monthly.ics", ics);
    let store = io_ics::import_ics(&path).unwrap();
    assert_eq!(store.items.len(), 3);
    assert_eq!(store.on_date(d(2026, 1, 15)).len(), 1);
    assert_eq!(store.on_date(d(2026, 2, 15)).len(), 1);
    assert_eq!(store.on_date(d(2026, 3, 15)).len(), 1);
    assert_eq!(store.on_date(d(2026, 4, 15)).len(), 0);
    std::fs::remove_file(&path).ok();
}

#[test]
fn rrule_yearly_expands() {
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n\
BEGIN:VEVENT\r\nUID:yearly-1\r\nSUMMARY:Birthday\r\n\
DTSTART;VALUE=DATE:20260301\r\nDTEND;VALUE=DATE:20260302\r\n\
RRULE:FREQ=YEARLY;COUNT=2\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let path = write_ics("cal_test_rrule_yearly.ics", ics);
    let store = io_ics::import_ics(&path).unwrap();
    assert_eq!(store.items.len(), 2);
    assert_eq!(store.on_date(d(2026, 3, 1)).len(), 1);
    assert_eq!(store.on_date(d(2027, 3, 1)).len(), 1);
    std::fs::remove_file(&path).ok();
}

#[test]
fn rrule_until_stops() {
    // Daily until 2026-08-07 (inclusive) starting 2026-08-05 -> 3 days.
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n\
BEGIN:VEVENT\r\nUID:until-1\r\nSUMMARY:Thing\r\n\
DTSTART;VALUE=DATE:20260805\r\nDTEND;VALUE=DATE:20260806\r\n\
RRULE:FREQ=DAILY;UNTIL=20260807\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let path = write_ics("cal_test_rrule_until.ics", ics);
    let store = io_ics::import_ics(&path).unwrap();
    assert_eq!(store.items.len(), 3);
    assert_eq!(store.on_date(d(2026, 8, 7)).len(), 1);
    assert_eq!(store.on_date(d(2026, 8, 8)).len(), 0);
    std::fs::remove_file(&path).ok();
}

#[test]
fn tzid_is_honored_on_import() {
    // Event at 09:00 America/New_York on 2026-08-05. Imported in a local zone
    // that differs should still resolve to that wall-clock time in NY.
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n\
BEGIN:VEVENT\r\nUID:tz-1\r\nSUMMARY:TZ\r\n\
DTSTART;TZID=America/New_York:20260805T090000\r\n\
DTEND;TZID=America/New_York:20260805T100000\r\n\
END:VEVENT\r\nEND:VCALENDAR\r\n";
    let path = write_ics("cal_test_tzid.ics", ics);
    let store = io_ics::import_ics(&path).unwrap();
    assert_eq!(store.items.len(), 1);
    let a = &store.items[0];
    // The stored local datetime must equal the NY wall time converted to local.
    // Verify the hour-of-day in the original timezone is 09:00.
    let as_ny = a.start.with_timezone(&chrono_tz::America::New_York);
    assert_eq!(as_ny.format("%H:%M").to_string(), "09:00");
    std::fs::remove_file(&path).ok();
}

#[test]
fn remove_series_deletes_all_occurrences() {
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n\
BEGIN:VEVENT\r\nUID:series-1\r\nSUMMARY:Rep\r\n\
DTSTART;VALUE=DATE:20260805\r\nDTEND;VALUE=DATE:20260806\r\n\
RRULE:FREQ=DAILY;COUNT=3\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let path = write_ics("cal_test_series_del.ics", ics);
    let mut store = io_ics::import_ics(&path).unwrap();
    assert_eq!(store.items.len(), 3);
    store.remove_series("series-1");
    assert!(store.items.is_empty());
    std::fs::remove_file(&path).ok();
}

