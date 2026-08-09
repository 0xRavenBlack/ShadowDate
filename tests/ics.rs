use calendar::ical_export::store_to_ics;
use calendar::ical_import::{import_ics, import_ics_with_warnings};
use calendar::model::{make_datetime, Appointment, Store};
use calendar::store_io::{backup_corrupt, load_store, save_store};
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

    let ics = store_to_ics(&store, "-//test//EN");
    assert!(ics.contains("BEGIN:VCALENDAR"));
    assert!(ics.contains("UID:test-uid-1"));
    assert!(ics.contains("SUMMARY:Dentist"));

    // parse it back
    let path = std::env::temp_dir().join("cal_test_roundtrip.ics");
    std::fs::write(&path, &ics).unwrap();
    let imported = import_ics(&path).unwrap();
    assert_eq!(imported.items().len(), 1);
    let back = &imported.items()[0];
    assert_eq!(back.uid, "test-uid-1");
    assert_eq!(back.title, "Dentist");
    assert_eq!(back.location, "Clinic");
    assert_eq!(back.start.format("%H:%M").to_string(), "09:30");
    std::fs::remove_file(&path).ok();
}

#[test]
fn load_nonexistent_is_empty() {
    let p = std::env::temp_dir().join("cal_does_not_exist_xyz.ics");
    let (store, warnings) = load_store(&p).unwrap();
    assert!(store.items().is_empty());
    assert!(warnings.is_empty());
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
    let store = import_ics(&path).unwrap();
    assert_eq!(store.items().len(), 1);
    let a = &store.items()[0];
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
    let store = import_ics(&path).unwrap();
    let a = &store.items()[0];
    assert!(a.all_day);
    let d = |y, m, day| chrono::NaiveDate::from_ymd_opt(y, m, day).unwrap();
    assert_eq!(store.on_date(d(2026, 9, 10)).len(), 1);
    assert_eq!(store.on_date(d(2026, 9, 11)).len(), 0);
    // Round-trips to an exclusive DTEND on the next day.
    let out = store_to_ics(&store, "-//test//EN");
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
    save_store(&store, &path).unwrap();
    let loaded = import_ics(&path).unwrap();
    let a = &loaded.items()[0];
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
    let store = import_ics(&path).unwrap();
    assert_eq!(store.items().len(), 3, "daily COUNT=3 should yield 3 occurrences");
    assert_eq!(store.on_date(d(2026, 8, 5)).len(), 1);
    assert_eq!(store.on_date(d(2026, 8, 6)).len(), 1);
    assert_eq!(store.on_date(d(2026, 8, 7)).len(), 1);
    assert_eq!(store.on_date(d(2026, 8, 8)).len(), 0);
    // All occurrences share the series uid and color.
    assert!(store.items().iter().all(|a| a.series_uid == "daily-1"));
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
    let store = import_ics(&path).unwrap();
    // 2 weeks * 3 days = 6 occurrences.
    assert_eq!(store.items().len(), 6);
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
    let store = import_ics(&path).unwrap();
    assert_eq!(store.items().len(), 3);
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
    let store = import_ics(&path).unwrap();
    assert_eq!(store.items().len(), 2);
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
    let store = import_ics(&path).unwrap();
    assert_eq!(store.items().len(), 3);
    assert_eq!(store.on_date(d(2026, 8, 7)).len(), 1);
    assert_eq!(store.on_date(d(2026, 8, 8)).len(), 0);
    std::fs::remove_file(&path).ok();
}

#[test]
fn rrule_weekly_count_still_applies_exdate_and_rdate() {
    // COUNT ends Weekly/Monthly/Yearly generation early. The EXDATE/RDATE
    // post-processing must still run after that (a regression: the early return
    // used to skip it, so the excluded date reappeared and the extra date was
    // lost).
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n\
BEGIN:VEVENT\r\nUID:wk-ex\r\nSUMMARY:Wk\r\n\
DTSTART;VALUE=DATE:20260805\r\nDTEND;VALUE=DATE:20260806\r\n\
RRULE:FREQ=WEEKLY;COUNT=3;BYDAY=MO,WE,FR\r\n\
EXDATE:20260807\r\nRDATE:20260818\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let path = write_ics("cal_test_rrule_weekly_exdate.ics", ics);
    let store = import_ics(&path).unwrap();
    // COUNT=3 picks 5, 7, 10; EXDATE drops 7; RDATE adds 18 -> 5, 10, 18.
    assert_eq!(store.items().len(), 3);
    assert_eq!(store.on_date(d(2026, 8, 5)).len(), 1);
    assert_eq!(store.on_date(d(2026, 8, 7)).len(), 0, "EXDATE date must be excluded");
    assert_eq!(store.on_date(d(2026, 8, 10)).len(), 1);
    assert_eq!(store.on_date(d(2026, 8, 18)).len(), 1, "RDATE date must be appended");
    std::fs::remove_file(&path).ok();
}

#[test]
fn rrule_monthly_count_still_applies_exdate() {
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n\
BEGIN:VEVENT\r\nUID:mo-ex\r\nSUMMARY:Mo\r\n\
DTSTART;VALUE=DATE:20260115\r\nDTEND;VALUE=DATE:20260116\r\n\
RRULE:FREQ=MONTHLY;COUNT=3;BYMONTHDAY=15\r\n\
EXDATE:20260215\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let path = write_ics("cal_test_rrule_monthly_exdate.ics", ics);
    let store = import_ics(&path).unwrap();
    // COUNT=3 picks 15 Jan/Feb/Mar; EXDATE drops 15 Feb -> Jan, Mar.
    assert_eq!(store.items().len(), 2);
    assert_eq!(store.on_date(d(2026, 1, 15)).len(), 1);
    assert_eq!(store.on_date(d(2026, 2, 15)).len(), 0, "EXDATE date must be excluded");
    assert_eq!(store.on_date(d(2026, 3, 15)).len(), 1);
    std::fs::remove_file(&path).ok();
}

#[test]
fn rrule_yearly_count_still_applies_rdate() {
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n\
BEGIN:VEVENT\r\nUID:yr-ex\r\nSUMMARY:Yr\r\n\
DTSTART;VALUE=DATE:20260301\r\nDTEND;VALUE=DATE:20260302\r\n\
RRULE:FREQ=YEARLY;COUNT=1\r\n\
RDATE:20270601\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let path = write_ics("cal_test_rrule_yearly_rdate.ics", ics);
    let store = import_ics(&path).unwrap();
    // COUNT=1 stops after 2026-03-01; RDATE appends 2027-06-01.
    assert_eq!(store.items().len(), 2);
    assert_eq!(store.on_date(d(2026, 3, 1)).len(), 1);
    assert_eq!(store.on_date(d(2027, 6, 1)).len(), 1, "RDATE date must be appended");
    std::fs::remove_file(&path).ok();
}

#[test]
fn rrule_weekly_until_keeps_remaining_byday_in_boundary_week() {
    // BYDAY lists are not necessarily chronological (FR before MO here). UNTIL
    // lands on MO of the following week, so the scan must not stop at the first
    // candidate past UNTIL (FR), or that Monday would be lost.
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n\
BEGIN:VEVENT\r\nUID:wk-boundary\r\nSUMMARY:Wk\r\n\
DTSTART;VALUE=DATE:20260805\r\nDTEND;VALUE=DATE:20260806\r\n\
RRULE:FREQ=WEEKLY;BYDAY=FR,MO;UNTIL=20260810\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let path = write_ics("cal_test_rrule_until_boundary.ics", ics);
    let store = import_ics(&path).unwrap();
    assert_eq!(store.items().len(), 2);
    assert_eq!(store.on_date(d(2026, 8, 7)).len(), 1);  // FR before UNTIL
    assert_eq!(store.on_date(d(2026, 8, 10)).len(), 1); // MO on UNTIL
    assert_eq!(store.on_date(d(2026, 8, 14)).len(), 0); // FR after UNTIL
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
    let store = import_ics(&path).unwrap();
    assert_eq!(store.items().len(), 1);
    let a = &store.items()[0];
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
    let mut store = import_ics(&path).unwrap();
    assert_eq!(store.items().len(), 3);
    store.remove_series("series-1");
    assert!(store.items().is_empty());
    std::fs::remove_file(&path).ok();
}

#[test]
fn bad_event_is_skipped_but_rest_is_kept() {
    // A single malformed VEVENT must not fail the whole import: the valid
    // event survives and the import reports a warning.
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n\
BEGIN:VEVENT\r\nUID:good-1\r\nSUMMARY:Fine\r\n\
DTSTART:20260805T090000\r\nDTEND:20260805T100000\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:bad-1\r\nSUMMARY:Broken\r\n\
DTSTART:not-a-date\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let path = write_ics("cal_test_bad_event.ics", ics);
    let (store, warnings) = import_ics_with_warnings(&path).unwrap();
    assert_eq!(store.items().len(), 1);
    assert_eq!(store.items()[0].uid, "good-1");
    assert!(!warnings.is_empty());
    std::fs::remove_file(&path).ok();
}

#[test]
fn load_store_tolerant_of_partial_corruption() {
    // Same resilience through the persistent-store loader: a broken entry is
    // skipped and reported, never wiping the rest of the calendar.
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n\
BEGIN:VEVENT\r\nUID:ok-1\r\nSUMMARY:Fine\r\n\
DTSTART;VALUE=DATE:20260805\r\nDTEND;VALUE=DATE:20260806\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:broken\r\nDTSTART:garbage\r\nEND:VEVENT\r\n\
END:VCALENDAR\r\n";
    let path = write_ics("cal_test_load_tolerant.ics", ics);
    let (store, warnings) = load_store(&path).unwrap();
    assert_eq!(store.items().len(), 1);
    assert_eq!(store.items()[0].uid, "ok-1");
    assert!(!warnings.is_empty());
    std::fs::remove_file(&path).ok();
}

#[test]
fn backup_corrupt_preserves_unreadable_file() {
    // The app must never silently start empty over a corrupt file: it backs the
    // bytes up first so the data survives the next save.
    let p = std::env::temp_dir().join("cal_backup_test.ics");
    std::fs::write(&p, b"some \xff\xfe binary-ish garbage").unwrap();
    let backup = backup_corrupt(&p).unwrap();
    assert!(backup.exists());
    assert_ne!(backup, p);
    std::fs::remove_file(&p).ok();
    std::fs::remove_file(&backup).ok();
}

#[test]
fn long_text_survives_line_folding_roundtrip() {
    // Values longer than 75 octets are folded on export (RFC 5545 §3.1); they
    // must come back byte-identical on import, not gain stray newlines.
    let long_title = "A suspiciously lengthy meeting title that deliberately exceeds the seventy-five octet line limit for iCalendar content lines, on purpose".to_string();
    assert!(long_title.len() > 75);
    let mut store = Store::new();
    store.insert(Appointment::with_uid(
        "fold-1".to_string(),
        long_title.clone(),
        format!("Description with a very long body that also keeps going far past the fold threshold to exercise the continuation path thoroughly: {}", "y".repeat(120)),
        "Location with an extremely long address that crosses the seventy-five octet boundary as well, stretching well beyond it".to_string(),
        make_datetime(d(2026, 8, 5), 9, 30),
        make_datetime(d(2026, 8, 5), 10, 0),
        false,
    ));
    let path = std::env::temp_dir().join("cal_test_fold.ics");
    save_store(&store, &path).unwrap();
    // Physical lines must not exceed 75 octets + CRLF.
    let raw = std::fs::read_to_string(&path).unwrap();
    for line in raw.split("\r\n") {
        assert!(line.len() <= 75, "unfolded line too long: {:?}", line);
    }
    let loaded = import_ics(&path).unwrap();
    let a = &loaded.items()[0];
    assert_eq!(a.title, long_title);
    assert!(a.description.starts_with("Description with a very long body"));
    assert!(a.location.starts_with("Location with an extremely long address"));
    std::fs::remove_file(&path).ok();
}

#[test]
fn series_uid_survives_save_load() {
    // Expanded occurrences share a series_uid distinct from their own UID. That
    // grouping must survive a save -> load cycle so series-wide edit/delete
    // still acts on the whole series afterwards.
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n\
BEGIN:VEVENT\r\nUID:series-rt\r\nSUMMARY:Daily\r\n\
DTSTART;VALUE=DATE:20260805\r\nDTEND;VALUE=DATE:20260806\r\n\
RRULE:FREQ=DAILY;COUNT=3\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let path = write_ics("cal_test_series_rt.ics", ics);
    let store = import_ics(&path).unwrap();
    assert_eq!(store.items().len(), 3);
    assert!(store.items().iter().all(|a| a.series_uid == "series-rt"));
    save_store(&store, &path).unwrap();
    let reloaded = import_ics(&path).unwrap();
    assert_eq!(reloaded.items().len(), 3);
    // Every occurrence still belongs to the same series, not independent events.
    let uids: Vec<&str> = reloaded.items().iter().map(|a| a.uid.as_str()).collect();
    assert!(uids.iter().any(|u| u.contains("#")), "occurrences should have derived UIDs");
    assert!(reloaded.items().iter().all(|a| a.series_uid == "series-rt"));
    // And deleting the series removes all three.
    let mut s = reloaded;
    s.remove_series("series-rt");
    assert!(s.items().is_empty());
    std::fs::remove_file(&path).ok();
}

#[test]
fn utf8_bom_is_stripped_on_import() {
    // Some editors prepend a UTF-8 BOM; the lexer would otherwise see garbage
    // before BEGIN:VCALENDAR and drop the whole calendar.
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n\
BEGIN:VEVENT\r\nUID:bom-1\r\nSUMMARY:Bom\r\n\
DTSTART;VALUE=DATE:20260805\r\nDTEND;VALUE=DATE:20260806\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let path = std::env::temp_dir().join("cal_test_bom.ics");
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(ics.as_bytes());
    std::fs::write(&path, &bytes).unwrap();
    let (store, warnings) = import_ics_with_warnings(&path).unwrap();
    assert_eq!(store.items().len(), 1);
    assert_eq!(store.items()[0].uid, "bom-1");
    assert!(warnings.is_empty());
    std::fs::remove_file(&path).ok();
}

#[test]
fn negative_bymonthday_in_december() {
    // BYMONTHDAY=-1 in December must resolve to Dec 31, not Dec 30 (the old
    // code subtracted one day from the first of December instead of wrapping
    // to the first of January).
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n\
BEGIN:VEVENT\r\nUID:dec-last\r\nSUMMARY:NewYearsEve\r\n\
DTSTART;VALUE=DATE:20261201\r\nDTEND;VALUE=DATE:20261202\r\n\
RRULE:FREQ=MONTHLY;COUNT=1;BYMONTH=12;BYMONTHDAY=-1\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let path = write_ics("cal_test_dec_last.ics", ics);
    let store = import_ics(&path).unwrap();
    assert_eq!(store.items().len(), 1);
    assert_eq!(store.on_date(d(2026, 12, 31)).len(), 1);
    assert_eq!(store.on_date(d(2026, 12, 30)).len(), 0);
    std::fs::remove_file(&path).ok();
}

#[test]
fn make_datetime_clamps_out_of_range_components() {
    // A hand-edited service config with hour=99/min=99 must not panic the
    // daemon; the values are clamped instead.
    let dt = make_datetime(d(2026, 8, 5), 99, 99);
    assert_eq!(dt.format("%H:%M").to_string(), "23:59");
}

