//! Serialize a `Store` to an `.ics` file.

use crate::model::Store;
use anyhow::{Context, Result};
use chrono::Local;
use std::fs;
use std::path::Path;

/// PRODID written into exported `.ics` files (also used when saving the store).
pub const PRODID: &str = "-//ravenblack//calendar//EN";

/// Serialize a Store to an .ics string.
pub fn store_to_ics(store: &Store, prodid: &str) -> String {
    let mut out = String::new();
    out.push_str("BEGIN:VCALENDAR\r\n");
    out.push_str("VERSION:2.0\r\n");
    out.push_str(&format!("PRODID:{}\r\n", prodid));
    out.push_str("CALSCALE:GREGORIAN\r\n");
    for a in store.items() {
        out.push_str("BEGIN:VEVENT\r\n");
        out.push_str(&fold_line(&format!("UID:{}", a.uid)));
        // Expanded occurrences of a recurring series share a `series_uid` (the
        // base event's UID) that differs from their own UID. Without an explicit
        // round-trip marker, a save→load cycle would flatten them back into
        // independent single events and break series-wide edit/delete.
        if a.series_uid != a.uid {
            out.push_str(&fold_line(&format!(
                "X-SHADOWDATE-SERIES-UID:{}",
                a.series_uid
            )));
        }
        out.push_str(&fold_line(&format!("SUMMARY:{}", escape_text(&a.title))));
        if !a.description.is_empty() {
            out.push_str(&fold_line(&format!(
                "DESCRIPTION:{}",
                escape_text(&a.description)
            )));
        }
        if !a.location.is_empty() {
            out.push_str(&fold_line(&format!(
                "LOCATION:{}",
                escape_text(&a.location)
            )));
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

/// Fold a content line to at most 75 octets per RFC 5545 §3.1: each chunk is
/// followed by CRLF and a single leading space on the continuation. Long
/// SUMMARY/DESCRIPTION/LOCATION values would otherwise be emitted as
/// non-compliant 1000+ octet lines that strict consumers may reject.
///
/// The fold point is kept strictly mid-word (never adjacent to a space): the
/// `ical` parser trims trailing whitespace from every physical line and eats
/// the continuation's leading space, so a boundary next to a space would lose
/// it on re-import. Cutting inside a word keeps both sides non-whitespace and
/// the round-trip byte-identical.
fn fold_line(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    let mut width = 75;
    while rest.len() > width {
        let mut cut = width;
        while cut > 0
            && (cut >= rest.len()
                || rest[..cut].ends_with(' ')
                || rest[cut..].starts_with(' '))
        {
            cut -= 1;
        }
        if cut == 0 {
            // Pathological input (e.g. an over-long run of spaces): no mid-word
            // spot exists, so split at the width anyway.
            cut = width;
        }
        // Never split a UTF-8 sequence.
        while cut > 0 && !rest.is_char_boundary(cut) {
            cut -= 1;
        }
        if cut == 0 {
            cut = 1; // a single codepoint, well below the 75-octet limit
        }
        out.push_str(&rest[..cut]);
        out.push_str("\r\n ");
        rest = &rest[cut..];
        width = 74;
    }
    out.push_str(rest);
    out.push_str("\r\n");
    out
}

/// Export a store to a file.
pub fn export_ics(store: &Store, path: &Path, prodid: &str) -> Result<()> {
    let data = store_to_ics(store, prodid);
    fs::write(path, data).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}
