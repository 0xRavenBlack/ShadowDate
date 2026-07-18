use crate::model::{Appointment, Store};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone};
use ical::parser::ical::component::IcalCalendar;
use ical::property::Property;
use std::fs;
use std::path::Path;

/// Parse an .ics file into a Store.
pub fn import_ics(path: &Path) -> Result<Store> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let reader = ical::IcalParser::new(content.as_bytes());
    let mut store = Store::new();
    for cal in reader {
        let cal: IcalCalendar = cal.map_err(|e| anyhow!("ics parse error: {}", e))?;
        for event in cal.events {
            if let Some(appt) = event_to_appointment(&event.properties)? {
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

/// iCalendar datetimes may be UTC (trailing Z) or local (with optional TZID param).
fn parse_ical_datetime(raw: &str) -> Result<DateTime<Local>> {
    let raw = raw.trim();
    if raw.len() == 8 && raw.chars().all(|c| c.is_ascii_digit()) {
        // DATE only -> start of that day, local
        let date = NaiveDate::parse_from_str(raw, "%Y%m%d")
            .map_err(|e| anyhow!("bad date {}: {}", raw, e))?;
        let ndt = NaiveDateTime::new(date, chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        return Ok(Local.from_local_datetime(&ndt).single().unwrap());
    }
    if let Some(utc) = raw.strip_suffix('Z') {
        let ndt = NaiveDateTime::parse_from_str(utc, "%Y%m%dT%H%M%S")
            .or_else(|_| NaiveDateTime::parse_from_str(utc, "%Y%m%dT%H%M%S%f"))
            .map_err(|e| anyhow!("bad utc datetime {}: {}", raw, e))?;
        return Ok(DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            ndt,
            chrono::Utc,
        )
        .with_timezone(&Local));
    }
    // Local date-time
    let ndt = NaiveDateTime::parse_from_str(raw, "%Y%m%dT%H%M%S")
        .or_else(|_| NaiveDateTime::parse_from_str(raw, "%Y%m%dT%H%M%S%f"))
        .map_err(|e| anyhow!("bad local datetime {}: {}", raw, e))?;
    Ok(Local
        .from_local_datetime(&ndt)
        .single()
        .unwrap_or_else(|| Local.timestamp_opt(ndt.and_utc().timestamp(), 0).unwrap()))
}

fn event_to_appointment(props: &[Property]) -> Result<Option<Appointment>> {
    let uid = match prop_value(props, "UID") {
        Some(u) => u,
        None => return Ok(None),
    };
    let title = prop_value(props, "SUMMARY").unwrap_or_default();
    let description = prop_value(props, "DESCRIPTION")
        .unwrap_or_default()
        .replace("\\n", "\n")
        .replace("\\,", ",");
    let location = prop_value(props, "LOCATION").unwrap_or_default();
    let start_raw = match prop_value(props, "DTSTART") {
        Some(s) => s,
        None => return Ok(None),
    };
    let start = parse_ical_datetime(&start_raw)?;
    let all_day = start_raw.trim().len() == 8;

    let end = match prop_value(props, "DTEND") {
        Some(e) => parse_ical_datetime(&e)?,
        None if all_day => start + chrono::Duration::days(1),
        None => start + chrono::Duration::hours(1),
    };

    Ok(Some(Appointment::with_uid(
        uid,
        title,
        description,
        location,
        start,
        end,
        all_day,
    )))
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

fn escape_text(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
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

/// Merge another store into this one (imported items replace same UID).
pub fn merge_store(base: &mut Store, other: Store) {
    for a in other.items {
        base.insert(a);
    }
}
