use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appointment {
    pub uid: String,
    pub title: String,
    pub description: String,
    pub location: String,
    pub start: DateTime<Local>,
    pub end: DateTime<Local>,
    pub all_day: bool,
    pub color_index: usize,
}

impl Appointment {
    pub fn with_uid(
        uid: String,
        title: String,
        description: String,
        location: String,
        start: DateTime<Local>,
        end: DateTime<Local>,
        all_day: bool,
    ) -> Self {
        let color_index = color_for_uid(&uid);
        Self {
            uid,
            title,
            description,
            location,
            start,
            end,
            all_day,
            color_index,
        }
    }

    pub fn date(&self) -> NaiveDate {
        self.start.date_naive()
    }

    pub fn time_label(&self) -> String {
        if self.all_day {
            "All day".to_string()
        } else {
            format!(
                "{} – {}",
                self.start.format("%H:%M"),
                self.end.format("%H:%M")
            )
        }
    }
}

fn color_for_uid(uid: &str) -> usize {
    let mut h: u64 = 0;
    for b in uid.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as u64);
    }
    (h % 6) as usize
}

/// In-memory store keyed by UID, plus a stable ordering.
#[derive(Debug, Clone, Default)]
pub struct Store {
    pub items: Vec<Appointment>,
    index: HashMap<String, usize>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, appt: Appointment) {
        if let Some(pos) = self.index.get(&appt.uid) {
            self.items[*pos] = appt;
        } else {
            self.index.insert(appt.uid.clone(), self.items.len());
            self.items.push(appt);
        }
    }

    pub fn remove(&mut self, uid: &str) {
        if let Some(pos) = self.index.remove(uid) {
            self.items.remove(pos);
            // rebuild index
            self.index.clear();
            for (i, a) in self.items.iter().enumerate() {
                self.index.insert(a.uid.clone(), i);
            }
        }
    }

    pub fn get(&self, uid: &str) -> Option<&Appointment> {
        self.index.get(uid).map(|&i| &self.items[i])
    }

    pub fn on_date(&self, date: NaiveDate) -> Vec<&Appointment> {
        let mut v: Vec<&Appointment> = self
            .items
            .iter()
            .filter(|a| {
                let sd = a.start.date_naive();
                let ed = a.end.date_naive();
                if a.all_day {
                    // iCalendar all-day DTEND is exclusive (start of the day after).
                    date >= sd && date < ed
                } else {
                    // Timed events: inclusive of both the start and end day.
                    date >= sd && date <= ed
                }
            })
            .collect();
        v.sort_by_key(|a| a.start);
        v
    }
}

/// Helper to build a local DateTime from date + optional time components.
///
/// During a DST transition a local time can be ambiguous or non-existent, so
/// `from_local_datetime(...).single()` may return `None`. In that case we fall
/// back to interpreting the naive value as a UTC timestamp; this is acceptable
/// for this app (times near a DST boundary may shift by an hour). `hour`/`min`
/// are assumed valid (callers validate ranges before calling), so the
/// `from_hms_opt(...).unwrap()` will not panic in normal use.
pub fn make_datetime(date: NaiveDate, hour: u32, min: u32) -> DateTime<Local> {
    let ndt = NaiveDateTime::new(date, chrono::NaiveTime::from_hms_opt(hour, min, 0).unwrap());
    Local
        .from_local_datetime(&ndt)
        .single()
        .unwrap_or_else(|| Local.timestamp_opt(ndt.and_utc().timestamp(), 0).unwrap())
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// Format a date for display.
pub fn format_date(d: NaiveDate) -> String {
    d.format("%A, %B %-d, %Y").to_string()
}
