use calendar::io_ics;
use calendar::model::{make_datetime, Appointment, Store};
use chrono::Local;

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

#[allow(dead_code)]
fn _local() -> chrono::DateTime<Local> {
    Local::now()
}
