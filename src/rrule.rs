//! RRULE expansion engine.
//!
//! Recurrence rules (`FREQ=DAILY|WEEKLY|MONTHLY|YEARLY` with `INTERVAL`,
//! `COUNT`, `UNTIL`, `BYDAY`, `BYMONTHDAY`, `BYMONTH`, `WKST`) are expanded at
//! import time into individual occurrence dates so the calendar view and the
//! reminder daemon can work off a plain list. Capped by `MAX_OCCURRENCES` /
//! `MAX_EXPAND_YEARS` so a malformed or giant rule can never hang the importer.

use crate::ical_import::parse_ical_datetime;
use crate::model::local_from_naive;
use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime, TimeDelta, Weekday};
use ical::property::Property;
use std::collections::HashSet;

/// Hard safety caps so a malformed/giant RRULE can never hang the importer.
const MAX_OCCURRENCES: usize = 4000;
const MAX_EXPAND_YEARS: i32 = 20;

/// Expand a recurrence rule into (start, end) pairs. Returns an empty vec when
/// the rule is unsupported or yields nothing. Covers the common cases:
/// FREQ=DAILY|WEEKLY|MONTHLY|YEARLY with INTERVAL, COUNT, UNTIL, BYDAY,
/// BYMONTHDAY and BYMONTH. Dates in `exclude` (from EXDATE) are removed from
/// the result; dates in `extra` (from RDATE) are appended.
pub(crate) fn expand_recurrence(
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

    // Register `d` as an occurrence when it is inside the rule's window, and
    // report whether generation may stop: true once COUNT is reached or the
    // MAX_OCCURRENCES safety cap is hit. Dates rejected by UNTIL/hard_stop are
    // dropped *without* ending the scan — BYDAY/BYMONTHDAY lists are not
    // guaranteed to be in chronological order, so one candidate past a boundary
    // does not mean the remaining candidates in this week/month are past it
    // too. The loops bound the scan with `hard_stop`, so the lookahead is small.
    let mut emit = |d: NaiveDate| -> bool {
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
                return true;
            }
        }
        emitted >= MAX_OCCURRENCES
    };

    match freq {
        Freq::Daily => {
            let mut d = base_date;
            while d <= hard_stop && !emit(d) {
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
            'outer: while week <= hard_stop {
                for wd in &bydays {
                    let cand = date_of_weekday_in_week(week, *wd, rule.wkst);
                    if cand >= base_date && emit(cand) {
                        break 'outer;
                    }
                }
                week += TimeDelta::weeks(interval as i64);
            }
        }
        Freq::Monthly => {
            let mut year = base_date.year();
            let mut month = base_date.month();
            'outer: while NaiveDate::from_ymd_opt(year, month, 1)
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
                    if d >= base_date && emit(d) {
                        break 'outer;
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
            'outer: while NaiveDate::from_ymd_opt(year, 1, 1)
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
                        if d >= base_date && emit(d) {
                            break 'outer;
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
        // Negative counts from the end of the month. The trick: get the first
        // day of the *next* month and subtract 1. December must wrap to January
        // of the following year (month 13 is invalid for chrono, and month 12
        // minus one day would be November 30 — which made -1 in December
        // resolve to the 30th instead of the 31st).
        let (next_year, next_month) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        let last = NaiveDate::from_ymd_opt(next_year, next_month, 1)? - TimeDelta::days(1);
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
