use crate::model::{Appointment, Store};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime, TimeDelta, TimeZone, Weekday};
use chrono_tz::Tz;
use ical::parser::ical::component::IcalCalendar;
use ical::property::Property;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Hard safety caps so a malformed/giant RRULE can never hang the importer.
const MAX_OCCURRENCES: usize = 4000;
const MAX_EXPAND_YEARS: i32 = 20;

/// Parse an .ics file into a Store. Recurring events (RRULE) are expanded into
/// individual occurrence appointments so the existing grid/list rendering works
/// without change. Each occurrence keeps the base event's UID in `series_uid`.
pub fn import_ics(path: &Path) -> Result<Store> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let reader = ical::IcalParser::new(content.as_bytes());
    let mut store = Store::new();
    for cal in reader {
        let cal: IcalCalendar = cal.map_err(|e| anyhow!("ics parse error: {}", e))?;
        for event in cal.events {
            for appt in event_to_appointments(&event.properties)? {
                store.insert(appt);
            }
        }
    }
    Ok(store)
}

fn get_prop<'a>(props: &'a [Property], name: &str) -> Option<&'a Property> {
    props.iter().find(|p| p.name.eq_ignore_ascii_case(name))
}

fn prop_value(props: &[Property], name: &str) -> Option<String> {
    get_prop(props, name).and_then(|p| p.value.clone())
}

/// Look up a parameter value (e.g. TZID) on a property. The `ical` crate stores
/// params as `Vec<(key, Vec<value>)>` with the key uppercased.
fn prop_param(prop: &Property, key: &str) -> Option<String> {
    prop.params.as_ref().and_then(|ps| {
        ps.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .and_then(|(_, v)| v.first().cloned())
    })
}

/// iCalendar datetimes may be:
/// - a DATE only (`VALUE=DATE` / 8 digits) -> start of that day, local
/// - UTC (trailing `Z`)
/// - local, optionally tagged with a `TZID` timezone parameter
fn parse_ical_datetime(prop: &Property) -> Result<DateTime<Local>> {
    let raw = prop
        .value
        .as_deref()
        .ok_or_else(|| anyhow!("missing datetime value"))?
        .trim();
    parse_datetime_raw(raw, prop_param(prop, "TZID").as_deref())
}

fn parse_datetime_raw(raw: &str, tzid: Option<&str>) -> Result<DateTime<Local>> {
    if raw.len() == 8 && raw.chars().all(|c| c.is_ascii_digit()) {
        // DATE only -> start of that day, local
        let date = NaiveDate::parse_from_str(raw, "%Y%m%d")
            .map_err(|e| anyhow!("bad date {}: {}", raw, e))?;
        let ndt = NaiveDateTime::new(date, chrono::NaiveTime::from_hms_opt(0, 0, 0)
            .expect("midnight is always valid"));
        return Ok(local_from_naive(ndt));
    }
    if let Some(utc) = raw.strip_suffix('Z') {
        let ndt = parse_naive_dt(utc)?;
        return Ok(DateTime::<chrono::Utc>::from_naive_utc_and_offset(ndt, chrono::Utc)
            .with_timezone(&Local));
    }
    // Local date-time, possibly with an explicit TZID timezone.
    let ndt = parse_naive_dt(raw)?;
    if let Some(tzid) = tzid {
        if let Ok(tz) = tzid.parse::<Tz>() {
            if let Some(dt) = tz.from_local_datetime(&ndt).single() {
                return Ok(dt.with_timezone(&Local));
            }
            // Ambiguous/non-existent (DST) -> fall back to the offset before/after.
            if let Some(dt) = tz.from_local_datetime(&ndt).earliest() {
                return Ok(dt.with_timezone(&Local));
            }
            if let Some(dt) = tz.from_local_datetime(&ndt).latest() {
                return Ok(dt.with_timezone(&Local));
            }
        }
        eprintln!(
            "warning: unknown or unresolvable TZID '{}', treating '{}' as floating local time",
            tzid, raw
        );
    }
    Ok(local_from_naive(ndt))
}

fn parse_naive_dt(raw: &str) -> Result<NaiveDateTime> {
    NaiveDateTime::parse_from_str(raw, "%Y%m%dT%H%M%S")
        .or_else(|_| NaiveDateTime::parse_from_str(raw, "%Y%m%dT%H%M%S%f"))
        .map_err(|e| anyhow!("bad datetime {}: {}", raw, e))
}

/// Build a local DateTime, falling back across DST gaps.
fn local_from_naive(ndt: NaiveDateTime) -> DateTime<Local> {
    Local
        .from_local_datetime(&ndt)
        .single()
        .unwrap_or_else(|| Local.timestamp_opt(ndt.and_utc().timestamp(), 0).unwrap())
}

fn event_to_appointments(props: &[Property]) -> Result<Vec<Appointment>> {
    let uid = match prop_value(props, "UID") {
        Some(u) => u,
        None => return Ok(Vec::new()),
    };
    let title = unescape_text(&prop_value(props, "SUMMARY").unwrap_or_default());
    let description = unescape_text(
        &prop_value(props, "DESCRIPTION").unwrap_or_default(),
    );
    let location = unescape_text(&prop_value(props, "LOCATION").unwrap_or_default());
    let start_prop = match get_prop(props, "DTSTART") {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };
    let start_raw = start_prop
        .value
        .clone()
        .ok_or_else(|| anyhow!("DTSTART without value"))?;
    let start = parse_ical_datetime(start_prop)?;
    let all_day = start_raw.trim().len() == 8;

    let end = match get_prop(props, "DTEND") {
        Some(e) => parse_ical_datetime(e)?,
        None if all_day => start + TimeDelta::days(1),
        None => start + TimeDelta::hours(1),
    };

    // Common metadata shared by every occurrence of the series.
    let mk = |occ_uid: String, s: DateTime<Local>, e: DateTime<Local>| {
        Appointment::with_uid_series(crate::model::NewAppointment {
            uid: occ_uid,
            series_uid: uid.clone(),
            title: title.clone(),
            description: description.clone(),
            location: location.clone(),
            start: s,
            end: e,
            all_day,
        })
    };

    let rrule = prop_value(props, "RRULE");
    match rrule {
        Some(rrule) if !rrule.trim().is_empty() => {
            let exclude: HashSet<NaiveDate> = props
                .iter()
                .filter(|p| p.name.eq_ignore_ascii_case("EXDATE"))
                .flat_map(parse_date_list)
                .collect();
            let extra: Vec<NaiveDate> = props
                .iter()
                .filter(|p| p.name.eq_ignore_ascii_case("RDATE"))
                .flat_map(parse_date_list)
                .collect();
            let occurrences = expand_recurrence(start, end, all_day, &rrule, &exclude, &extra);
            if occurrences.is_empty() {
                // Unsupported rule: keep just the base occurrence.
                Ok(vec![mk(uid.clone(), start, end)])
            } else {
                Ok(occurrences
                    .into_iter()
                    .enumerate()
                    .map(|(i, (s, e))| mk(format!("{}#{}", uid, i), s, e))
                    .collect())
            }
        }
        _ => Ok(vec![mk(uid.clone(), start, end)]),
    }
}

/// Expand a recurrence rule into (start, end) pairs. Returns an empty vec when
/// the rule is unsupported or yields nothing. Covers the common cases:
/// FREQ=DAILY|WEEKLY|MONTHLY|YEARLY with INTERVAL, COUNT, UNTIL, BYDAY,
/// BYMONTHDAY and BYMONTH. Dates in `exclude` (from EXDATE) are removed from
/// the result; dates in `extra` (from RDATE) are appended.
fn expand_recurrence(
    start: DateTime<Local>,
    end: DateTime<Local>,
    all_day: bool,
    rrule: &str,
    exclude: &HashSet<NaiveDate>,
    extra: &[NaiveDate],
) -> Vec<(DateTime<Local>, DateTime<Local>)> {
    let rule = match RRule::parse(rrule) {
        Some(r) => r,
        None => return Vec::new(),
    };
    let freq = match rule.freq {
        Some(f) => f,
        None => return Vec::new(),
    };

    // Duration carried by each occurrence.
    let duration = if all_day {
        TimeDelta::days((end.date_naive() - start.date_naive()).num_days())
    } else {
        end - start
    };

    let base_date = start.date_naive();
    let base_time = start.time();
    let interval = rule.interval.max(1);
    let hard_stop = base_date + TimeDelta::days((MAX_EXPAND_YEARS as i64) * 366);

    let mut dates: Vec<NaiveDate> = Vec::new();
    let mut emitted = 0usize;
    let mut count_ok = |d: NaiveDate| -> bool {
        if d > hard_stop {
            return false;
        }
        if let Some(u) = rule.until {
            // UNTIL is inclusive of the recurrence instant; compare against the
            // occurrence start so all-day (exclusive-end) events are bounded by
            // their start date, matching RFC 5545 semantics.
            let occ_start = occ_start_datetime(d, base_time, all_day);
            if occ_start > u {
                return false;
            }
        }
        dates.push(d);
        emitted += 1;
        if let Some(c) = rule.count {
            if emitted >= c {
                return false;
            }
        }
        emitted < MAX_OCCURRENCES
    };

    match freq {
        Freq::Daily => {
            let mut d = base_date;
            while count_ok(d) {
                d += TimeDelta::days(interval as i64);
            }
        }
        Freq::Weekly => {
            let bydays = if rule.byday.is_empty() {
                vec![base_date.weekday()]
            } else {
                rule.byday.iter().map(|(wd, _)| *wd).collect()
            };
            let mut week = base_date;
            while week <= hard_stop {
                for wd in &bydays {
                    let cand = date_of_weekday_in_week(week, *wd, rule.wkst);
                    if cand >= base_date && !count_ok(cand) {
                        return finish(dates, base_date, base_time, duration, all_day);
                    }
                }
                week += TimeDelta::weeks(interval as i64);
            }
        }
        Freq::Monthly => {
            let mut year = base_date.year();
            let mut month = base_date.month();
            while NaiveDate::from_ymd_opt(year, month, 1)
                .expect("year/month in expansion loop should be valid")
                <= hard_stop {
                let days: Vec<NaiveDate> = if !rule.bymonthday.is_empty() {
                    rule.bymonthday
                        .iter()
                        .filter_map(|&md| month_day_to_date(year, month, md))
                        .collect()
                } else if !rule.byday.is_empty() {
                    rule.byday
                        .iter()
                        .filter_map(|(wd, pos)| nth_weekday_in_month(year, month, *wd, *pos))
                        .collect()
                } else {
                    month_day_to_date(year, month, base_date.day() as i32)
                        .into_iter()
                        .collect()
                };
                for d in days {
                    if d >= base_date && !count_ok(d) {
                        return finish(dates, base_date, base_time, duration, all_day);
                    }
                }
                // advance month by interval
                let total = year as i64 * 12 + (month as i64 - 1) + interval as i64;
                year = (total / 12) as i32;
                month = (total % 12) as u32 + 1;
            }
        }
        Freq::Yearly => {
            let mut year = base_date.year();
            while NaiveDate::from_ymd_opt(year, 1, 1)
                .expect("year in expansion loop should be valid")
                <= hard_stop {
                let months: Vec<u32> = if rule.bymonth.is_empty() {
                    vec![base_date.month()]
                } else {
                    rule.bymonth.clone()
                };
                for m in months {
                    let day: Vec<NaiveDate> = if !rule.bymonthday.is_empty() {
                        rule.bymonthday
                            .iter()
                            .filter_map(|&md| month_day_to_date(year, m, md))
                            .collect()
                    } else if !rule.byday.is_empty() {
                        rule.byday
                            .iter()
                            .filter_map(|(wd, pos)| nth_weekday_in_month(year, m, *wd, *pos))
                            .collect()
                    } else {
                        month_day_to_date(year, m, base_date.day() as i32)
                            .into_iter()
                            .collect()
                    };
                    for d in day {
                        if d >= base_date && !count_ok(d) {
                            return finish(dates, base_date, base_time, duration, all_day);
                        }
                    }
                }
                year += interval as i32;
            }
        }
    }

    // Apply EXDATE: remove excluded dates from the expanded set.
    dates.retain(|d| !exclude.contains(d));
    // Apply RDATE: append extra dates that are >= base_date and not already present.
    for &d in extra {
        if d >= base_date && !dates.contains(&d) {
            dates.push(d);
        }
    }
    dates.sort();
    dates.dedup();

    finish(dates, base_date, base_time, duration, all_day)
}

fn finish(
    dates: Vec<NaiveDate>,
    _base_date: NaiveDate,
    base_time: chrono::NaiveTime,
    duration: TimeDelta,
    all_day: bool,
) -> Vec<(DateTime<Local>, DateTime<Local>)> {
    dates
        .into_iter()
        .map(|d| {
            let s = occ_start_datetime(d, base_time, all_day);
            let e = occ_end_datetime(d, base_time, duration, all_day);
            (s, e)
        })
        .collect()
}

fn occ_start_datetime(d: NaiveDate, t: chrono::NaiveTime, all_day: bool) -> DateTime<Local> {
    if all_day {
        local_from_naive(NaiveDateTime::new(d, chrono::NaiveTime::from_hms_opt(0, 0, 0)
            .expect("midnight is always valid")))
    } else {
        local_from_naive(NaiveDateTime::new(d, t))
    }
}

fn occ_end_datetime(
    d: NaiveDate,
    t: chrono::NaiveTime,
    duration: TimeDelta,
    all_day: bool,
) -> DateTime<Local> {
    if all_day {
        // All-day end is exclusive (start of the day after the last day).
        local_from_naive(
            NaiveDateTime::new(d + duration, chrono::NaiveTime::from_hms_opt(0, 0, 0)
                .expect("midnight is always valid")),
        )
    } else {
        occ_start_datetime(d, t, false) + duration
    }
}

/// The date of the given weekday within the week that contains `anchor`,
/// where the week starts on `week_start` (per WKST).
fn date_of_weekday_in_week(anchor: NaiveDate, wd: Weekday, week_start: Weekday) -> NaiveDate {
    let base = week_start.num_days_from_sunday() as i64;
    let anchor_offset = anchor.weekday().num_days_from_sunday() as i64 - base;
    let wd_offset = wd.num_days_from_sunday() as i64 - base;
    anchor - TimeDelta::days(anchor_offset) + TimeDelta::days(wd_offset)
}

/// Convert a (possibly negative) month-day to a concrete date, or None if invalid
/// (e.g. Feb 30, or -1 on a 28-day Feb).
fn month_day_to_date(year: i32, month: u32, md: i32) -> Option<NaiveDate> {
    let day = if md > 0 {
        md
    } else {
        // Negative counts from the end of the month.
        let last = NaiveDate::from_ymd_opt(year, month + if month == 12 { 0 } else { 1 }, 1)?
            - TimeDelta::days(1);
        last.day() as i32 + 1 + md
    };
    NaiveDate::from_ymd_opt(year, month, day as u32)
}

/// Nth weekday of a month. `pos` is 1-based (1 = first, 2 = second, ...);
/// negative counts from the end (-1 = last). `None` means "every" (used for
/// weekly-style BYDAY in a monthly context -> first match).
fn nth_weekday_in_month(year: i32, month: u32, wd: Weekday, pos: Option<i32>) -> Option<NaiveDate> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let first_wd = date_of_weekday_in_week(first, wd, Weekday::Mon);
    // First occurrence may be in the previous month; shift forward.
    let first_occ = if first_wd.month() == month {
        first_wd
    } else {
        first_wd + TimeDelta::weeks(1)
    };
    match pos {
        Some(p) if p > 0 => Some(first_occ + TimeDelta::weeks((p - 1) as i64)),
        Some(p) if p < 0 => {
            // Last (or p-th from last) occurrence.
            let mut last = first_occ;
            loop {
                let next = last + TimeDelta::weeks(1);
                if next.month() != month {
                    break;
                }
                last = next;
            }
            let total = ((last - first_occ).num_days() / 7) as i32 + 1;
            let idx = (total + p) as i64;
            if idx < 0 {
                None
            } else {
                Some(first_occ + TimeDelta::weeks(idx))
            }
        }
        _ => Some(first_occ),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

struct RRule {
    freq: Option<Freq>,
    interval: u32,
    count: Option<usize>,
    until: Option<DateTime<Local>>,
    wkst: Weekday,
    byday: Vec<(Weekday, Option<i32>)>,
    bymonthday: Vec<i32>,
    bymonth: Vec<u32>,
}

impl RRule {
    fn parse(s: &str) -> Option<RRule> {
        let mut r = RRule {
            freq: None,
            interval: 1,
            count: None,
            until: None,
            wkst: Weekday::Mon,
            byday: Vec::new(),
            bymonthday: Vec::new(),
            bymonth: Vec::new(),
        };
        for part in s.split(';') {
            let mut kv = part.splitn(2, '=');
            let key = kv.next()?.trim().to_ascii_uppercase();
            let val = kv.next()?.trim();
            match key.as_str() {
                "FREQ" => {
                    r.freq = match val.to_ascii_uppercase().as_str() {
                        "DAILY" => Some(Freq::Daily),
                        "WEEKLY" => Some(Freq::Weekly),
                        "MONTHLY" => Some(Freq::Monthly),
                        "YEARLY" => Some(Freq::Yearly),
                        _ => None,
                    }
                }
                "INTERVAL" => r.interval = val.parse().unwrap_or(1),
                "COUNT" => r.count = val.parse().ok(),
                "UNTIL" => {
                    // UNTIL is a datetime (possibly UTC with Z) or a DATE.
                    let prop = Property {
                        name: "UNTIL".into(),
                        params: None,
                        value: Some(val.to_string()),
                    };
                    r.until = parse_ical_datetime(&prop).ok();
                }
                "BYDAY" => {
                    for tok in val.split(',') {
                        if let Some((wd, pos)) = parse_byday(tok.trim()) {
                            r.byday.push((wd, pos));
                        }
                    }
                }
                "BYMONTHDAY" => {
                    for tok in val.split(',') {
                        if let Ok(d) = tok.trim().parse::<i32>() {
                            r.bymonthday.push(d);
                        }
                    }
                }
                "BYMONTH" => {
                    for tok in val.split(',') {
                        if let Ok(m) = tok.trim().parse::<u32>() {
                            r.bymonth.push(m);
                        }
                    }
                }
                "WKST" => {
                    if let Some((wd, _)) = parse_byday(val) {
                        r.wkst = wd;
                    }
                }
                _ => {}
            }
        }
        r.freq?;
        Some(r)
    }
}

/// Parse a BYDAY token like "MO", "-1MO", "2TU" into (weekday, optional position).
fn parse_byday(tok: &str) -> Option<(Weekday, Option<i32>)> {
    let tok = tok.trim();
    let (digits, rest) = split_digits(tok);
    let (pos, wd_str) = if rest.is_empty() {
        // No weekday suffix — entire token is the weekday abbreviation.
        (None, tok)
    } else if digits.is_empty() {
        // Just a weekday, no numeric prefix.
        (None, rest)
    } else {
        let num = digits.parse::<i32>().unwrap_or(1);
        // A leading '-' negates the position; '+' is ignored.
        let sign = if tok.starts_with('-') { -1 } else { 1 };
        (Some(sign * num), rest)
    };
    let weekday = match wd_str.to_ascii_uppercase().as_str() {
        "MO" => Weekday::Mon,
        "TU" => Weekday::Tue,
        "WE" => Weekday::Wed,
        "TH" => Weekday::Thu,
        "FR" => Weekday::Fri,
        "SA" => Weekday::Sat,
        "SU" => Weekday::Sun,
        _ => return None,
    };
    Some((weekday, pos))
}

/// Split a string into a leading digit prefix and the remaining suffix.
/// E.g. "2TU" -> ("2", "TU"), "-1MO" -> ("1", "MO"), "MO" -> ("", "MO").
fn split_digits(s: &str) -> (&str, &str) {
    let start = s.strip_prefix(['+', '-']).unwrap_or(s);
    let digit_end = start
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(start.len());
    let digits = &start[..digit_end];
    // The suffix starts after the optional sign + digits.
    let suffix_offset = if s.starts_with(['+', '-']) { 1 } else { 0 } + digit_end;
    (digits, &s[suffix_offset..])
}

/// Serialize a Store to an .ics string.
pub fn store_to_ics(store: &Store, prodid: &str) -> String {
    let mut out = String::new();
    out.push_str("BEGIN:VCALENDAR\r\n");
    out.push_str("VERSION:2.0\r\n");
    out.push_str(&format!("PRODID:{}\r\n", prodid));
    out.push_str("CALSCALE:GREGORIAN\r\n");
    for a in &store.items {
        out.push_str("BEGIN:VEVENT\r\n");
        out.push_str(&format!("UID:{}\r\n", a.uid));
        out.push_str(&format!("SUMMARY:{}\r\n", escape_text(&a.title)));
        if !a.description.is_empty() {
            out.push_str(&format!("DESCRIPTION:{}\r\n", escape_text(&a.description)));
        }
        if !a.location.is_empty() {
            out.push_str(&format!("LOCATION:{}\r\n", escape_text(&a.location)));
        }
        if a.all_day {
            out.push_str(&format!("DTSTART;VALUE=DATE:{}\r\n", a.start.format("%Y%m%d")));
            out.push_str(&format!("DTEND;VALUE=DATE:{}\r\n", a.end.format("%Y%m%d")));
        } else {
            out.push_str(&format!(
                "DTSTART:{}\r\n",
                a.start.with_timezone(&chrono::Utc).format("%Y%m%dT%H%M%SZ")
            ));
            out.push_str(&format!(
                "DTEND:{}\r\n",
                a.end.with_timezone(&chrono::Utc).format("%Y%m%dT%H%M%SZ")
            ));
        }
        out.push_str(&format!(
            "DTSTAMP:{}\r\n",
            Local::now()
                .with_timezone(&chrono::Utc)
                .format("%Y%m%dT%H%M%SZ")
        ));
        out.push_str("END:VEVENT\r\n");
    }
    out.push_str("END:VCALENDAR\r\n");
    out
}

/// Escape text for an iCalendar value. Order matters: backslash first so the
/// escapes we introduce are not themselves re-escaped. Carriage returns are
/// escaped as `\r` per RFC 5545 Section 3.3.11.
fn escape_text(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

/// Reverse of `escape_text`. Undo the specific escapes in the opposite order
/// they were introduced so a literal `\\;` is decoded to `\;` not `;`.
fn unescape_text(s: &str) -> String {
    s.replace("\\\\", "\u{0}") // placeholder to protect already-escaped backslashes
        .replace("\\r", "\r")
        .replace("\\n", "\n")
        .replace("\\;", ";")
        .replace("\\,", ",")
        .replace('\u{0}', "\\")
}

/// Parse a comma-separated list of DATE or DATE-TIME values (used by EXDATE
/// and RDATE) into a list of `NaiveDate`s. DATE values (8 digits) are parsed
/// directly; DATE-TIME values use only the date portion.
fn parse_date_list(prop: &Property) -> Vec<NaiveDate> {
    let val = match &prop.value {
        Some(v) => v,
        None => return Vec::new(),
    };
    val.split(',')
        .filter_map(|tok| {
            let tok = tok.trim();
            if tok.is_empty() {
                return None;
            }
            if tok.len() == 8 && tok.chars().all(|c| c.is_ascii_digit()) {
                return NaiveDate::parse_from_str(tok, "%Y%m%d").ok();
            }
            // DATE-TIME: parse and extract the date portion.
            let prop = Property {
                name: "DT".into(),
                params: None,
                value: Some(tok.to_string()),
            };
            parse_ical_datetime(&prop)
                .ok()
                .map(|dt| dt.date_naive())
        })
        .collect()
}

/// Export a store to a file.
pub fn export_ics(store: &Store, path: &Path, prodid: &str) -> Result<()> {
    let data = store_to_ics(store, prodid);
    fs::write(path, data).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Load the persistent store from the default data file, if it exists.
pub fn load_store(path: &Path) -> Store {
    if path.exists() {
        import_ics(path).unwrap_or_default()
    } else {
        Store::new()
    }
}

/// Save the store to the default data file (also the export format).
pub fn save_store(store: &Store, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    export_ics(store, path, "-//ravenblack//calendar//EN")
}

/// Merge another store into this one. For each series present in `other`
/// (identified by `series_uid`), first remove all existing occurrences of that
/// series from `base` so that a modified RRULE does not leave orphaned old
/// occurrences behind.
pub fn merge_store(base: &mut Store, other: Store) {
    let series_uids: Vec<String> = other
        .items
        .iter()
        .map(|a| a.series_uid.clone())
        .collect();
    let mut seen = std::collections::HashSet::new();
    for uid in &series_uids {
        if seen.insert(uid.clone()) {
            base.remove_series(uid);
        }
    }
    for a in other.items {
        base.insert(a);
    }
}
