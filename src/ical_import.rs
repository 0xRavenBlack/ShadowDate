//! Parse `.ics` content into a `Store`.
//!
//! Tolerant by design: a malformed calendar or event is skipped with a warning
//! instead of failing the whole import, so a single bad line can never wipe the
//! rest of the calendar. Only a file that cannot be read at all yields an error.
//! Recurring events (RRULE) are expanded into individual occurrence
//! appointments (`crate::rrule`) so the existing grid/list rendering and the
//! reminder daemon work without change.

use crate::model::{local_from_naive, Appointment, Store};
use crate::rrule::{expand_recurrence, MAX_OCCURRENCES};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeDelta, TimeZone};
use chrono_tz::Tz;
use ical::parser::ical::component::IcalCalendar;
use ical::property::Property;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Parse an .ics file into a Store. Recurring events (RRULE) are expanded into
/// individual occurrence appointments so the existing grid/list rendering works
/// without change. Each occurrence keeps the base event's UID in `series_uid`.
///
/// Parsing is tolerant: a malformed calendar or event is skipped with a warning
/// instead of failing the whole import, so a single bad line can never wipe the
/// rest of the calendar. Only a file that cannot be read at all yields an error.
pub fn import_ics(path: &Path) -> Result<Store> {
    let (store, warnings) = import_ics_with_warnings(path)?;
    if !warnings.is_empty() {
        eprintln!(
            "warning: importing {}: {} entries skipped",
            path.display(),
            warnings.len()
        );
    }
    Ok(store)
}

/// Like [`import_ics`], but also returns a human-readable warning for each
/// calendar/event that had to be skipped. Callers can surface the warnings and
/// back up a partially-corrupt file before the next save overwrites it.
pub fn import_ics_with_warnings(path: &Path) -> Result<(Store, Vec<String>)> {
    // Read as bytes and strip a UTF-8 BOM: some editors/exports prepend one,
    // and the `ical` lexer would otherwise choke on the invisible U+FEFF before
    // "BEGIN:VCALENDAR", making the whole calendar look empty.
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    let content = String::from_utf8_lossy(bytes);
    Ok(parse_ics(&content))
}

/// Parse .ics content into a Store, skipping anything malformed and collecting
/// warnings. Never fails: structural problems are reported, not fatal.
pub(crate) fn parse_ics(content: &str) -> (Store, Vec<String>) {
    let mut store = Store::new();
    let mut warnings = Vec::new();
    for cal in ical::IcalParser::new(content.as_bytes()) {
        let cal: IcalCalendar = match cal {
            Ok(c) => c,
            Err(e) => {
                warnings.push(format!("skipping an unreadable calendar: {}", e));
                continue;
            }
        };
        for event in cal.events {
            for appt in event_to_appointments(&event.properties, &mut warnings) {
                store.insert(appt);
            }
        }
    }
    (store, warnings)
}

fn get_prop<'a>(props: &'a [Property], name: &str) -> Option<&'a Property> {
    props.iter().find(|p| p.name.eq_ignore_ascii_case(name))
}

/// Look up a parameter value (e.g. TZID) on a property. The `ical` crate stores
/// params as `Vec<(key, Vec<value>)>` with the key uppercased.
fn prop_value(props: &[Property], name: &str) -> Option<String> {
    get_prop(props, name)
        .and_then(|p| p.value.clone())
        .map(|v| strip_crlf(&v))
}

/// Remove literal CR/LF from a raw property value. The `ical` crate's line
/// unfolding turns a fold continuation (`CRLF `) into a literal `\n` inside the
/// value, and valid files never contain raw newlines (real ones are escaped as
/// `\n`), so any CR/LF here is a fold artifact or non-compliant input and is
/// dropped before unescaping.
fn strip_crlf(s: &str) -> String {
    s.chars().filter(|c| *c != '\r' && *c != '\n').collect()
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
pub(crate) fn parse_ical_datetime(prop: &Property) -> Result<DateTime<Local>> {
    let raw = prop
        .value
        .as_deref()
        .ok_or_else(|| anyhow!("missing datetime value"))?;
    let raw = strip_crlf(raw);
    parse_datetime_raw(raw.trim(), prop_param(prop, "TZID").as_deref())
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

/// Convert one VEVENT's properties into its appointment(s), appending a
/// human-readable warning to `warnings` for anything that had to be skipped.
/// Malformed events are skipped entirely (a warning); an RRULE that cannot be
/// expanded keeps the base occurrence (a warning), so a bad rule never silently
/// collapses the series or fails the rest of the import.
fn event_to_appointments(props: &[Property], warnings: &mut Vec<String>) -> Vec<Appointment> {
    let uid = match prop_value(props, "UID") {
        Some(u) => u,
        None => {
            warnings.push("skipping an invalid event: VEVENT without UID".to_string());
            return Vec::new();
        }
    };
    // Re-imported expanded occurrences carry the base event's UID in our
    // private X-SHADOWDATE-SERIES-UID marker (see `store_to_ics`); without it,
    // each occurrence would collapse back into an independent single event.
    let series_uid = prop_value(props, "X-SHADOWDATE-SERIES-UID")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| uid.clone());
    let title = unescape_text(&prop_value(props, "SUMMARY").unwrap_or_default());
    let description = unescape_text(
        &prop_value(props, "DESCRIPTION").unwrap_or_default(),
    );
    let location = unescape_text(&prop_value(props, "LOCATION").unwrap_or_default());
    let start_prop = match get_prop(props, "DTSTART") {
        Some(s) => s,
        None => {
            warnings.push(format!("skipping invalid event '{}': missing DTSTART", uid));
            return Vec::new();
        }
    };
    let start_raw = start_prop
        .value
        .clone()
        .unwrap_or_default();
    let start = match parse_ical_datetime(start_prop) {
        Ok(s) => s,
        Err(e) => {
            warnings.push(format!("skipping invalid event '{}': {}", uid, e));
            return Vec::new();
        }
    };
    let all_day = start_raw.trim().len() == 8;

    let end = match get_prop(props, "DTEND") {
        Some(e) => match parse_ical_datetime(e) {
            Ok(e) => e,
            Err(e) => {
                warnings.push(format!("skipping invalid event '{}': {}", uid, e));
                return Vec::new();
            }
        },
        None if all_day => start + TimeDelta::days(1),
        None => start + TimeDelta::hours(1),
    };

    // Common metadata shared by every occurrence of the series.
    let mk = |occ_uid: String, s: DateTime<Local>, e: DateTime<Local>| {
        Appointment::build(crate::model::NewAppointment {
            uid: occ_uid,
            series_uid: series_uid.clone(),
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
            // EXDATE/RDATE lists are capped at the same occurrence bound as the
            // base expansion so a hostile file cannot grow the store past
            // MAX_OCCURRENCES (or blow up memory parsing a giant list). `.take`
            // is lazy: a single property with millions of commas is never fully
            // parsed.
            let exclude: HashSet<NaiveDate> = props
                .iter()
                .filter(|p| p.name.eq_ignore_ascii_case("EXDATE"))
                .flat_map(parse_date_list)
                .take(MAX_OCCURRENCES)
                .collect();
            let extra: Vec<NaiveDate> = props
                .iter()
                .filter(|p| p.name.eq_ignore_ascii_case("RDATE"))
                .flat_map(parse_date_list)
                .take(MAX_OCCURRENCES)
                .collect();
            match expand_recurrence(start, end, all_day, &rrule, &exclude, &extra) {
                Ok(occurrences) => occurrences
                    .into_iter()
                    .enumerate()
                    .map(|(i, (s, e))| mk(format!("{}#{}", uid, i), s, e))
                    .collect(),
                Err(reason) => {
                    warnings.push(format!(
                        "RRULE '{}' on event '{}' is unsupported ({}); keeping a single occurrence",
                        rrule, uid, reason
                    ));
                    vec![mk(uid, start, end)]
                }
            }
        }
        _ => vec![mk(uid, start, end)],
    }
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
/// directly; DATE-TIME values use only the date portion. Capped at
/// `MAX_OCCURRENCES` entries so a single hostile property with millions of
/// commas is never fully materialized.
fn parse_date_list(prop: &Property) -> Vec<NaiveDate> {
    let val = match &prop.value {
        Some(v) => v,
        None => return Vec::new(),
    };
    val.split(',')
        .take(MAX_OCCURRENCES)
        .filter_map(|tok| {
            let tok = tok.trim();
            if tok.is_empty() {
                return None;
            }
            if tok.len() == 8 && tok.chars().all(|c| c.is_ascii_digit()) {
                return NaiveDate::parse_from_str(tok, "%Y%m%d").ok();
            }
            // DATE-TIME: parse and extract the date portion. DATE-TIME values
            // carry the timezone on the *property* (the common `EXDATE;TZID=...`
            // form), so preserve the original params instead of treating the
            // value as floating local time.
            let prop = Property {
                name: "DT".into(),
                params: prop.params.clone(),
                value: Some(tok.to_string()),
            };
            parse_ical_datetime(&prop)
                .ok()
                .map(|dt| dt.date_naive())
        })
        .collect()
}
