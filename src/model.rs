use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appointment {
    pub uid: String,
    /// UID of the series this appointment belongs to. For single (non-recurring)
    /// appointments this equals `uid`; for occurrences expanded from an `RRULE`
    /// it is the base event's UID, so the whole series can be edited/deleted
    /// together. The color is derived from `series_uid` so all occurrences of a
    /// recurring event share the same pastel class.
    pub series_uid: String,
    pub title: String,
    pub description: String,
    pub location: String,
    pub start: DateTime<Local>,
    pub end: DateTime<Local>,
    pub all_day: bool,
    pub color_index: usize,
}

/// Fields needed to construct an `Appointment`. Used by `Appointment::build`
/// to avoid an excessively long argument list.
pub struct NewAppointment {
    pub uid: String,
    pub series_uid: String,
    pub title: String,
    pub description: String,
    pub location: String,
    pub start: DateTime<Local>,
    pub end: DateTime<Local>,
    pub all_day: bool,
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
        Self::with_uid_series(NewAppointment {
            series_uid: uid.clone(),
            uid,
            title,
            description,
            location,
            start,
            end,
            all_day,
        })
    }

    /// Build an appointment that belongs to a recurring series. `series_uid`
    /// must be the base event's UID so the whole series can be edited/deleted
    /// together; `uid` should be unique per occurrence.
    pub fn with_uid_series(n: NewAppointment) -> Self {
        let color_index = color_for_uid(&n.series_uid);
        Self {
            uid: n.uid,
            series_uid: n.series_uid,
            title: n.title,
            description: n.description,
            location: n.location,
            start: n.start,
            end: n.end,
            all_day: n.all_day,
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
            // swap_remove keeps the Vec compact in O(1); only the item that was
            // moved into `pos` needs its index entry corrected.
            let swapped = self.items.swap_remove(pos);
            if let Some(moved) = self.items.get(pos) {
                self.index.insert(moved.uid.clone(), pos);
            }
            let _ = swapped;
        }
    }

    /// Remove every appointment that belongs to the given series (matched by
    /// `series_uid`), including single appointments whose `series_uid == uid`.
    pub fn remove_series(&mut self, series_uid: &str) {
        let keep: Vec<Appointment> = self
            .items
            .drain(..)
            .filter(|a| a.series_uid != series_uid)
            .collect();
        self.items = keep;
        self.index.clear();
        for (i, a) in self.items.iter().enumerate() {
            self.index.insert(a.uid.clone(), i);
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
