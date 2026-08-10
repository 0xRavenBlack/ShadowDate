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

/// Named fields needed to construct an `Appointment`, so callers never thread
/// seven positional arguments. `series_uid` is the base event's UID for
/// recurring occurrences (equal to `uid` for single events).
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
    /// Build an appointment from its named fields. `series_uid` must be the
    /// base event's UID so the whole series can be edited/deleted together;
    /// `uid` should be unique per occurrence (equal to `series_uid` for single
    /// events). The color is derived from `series_uid`.
    pub fn build(n: NewAppointment) -> Self {
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
}

fn color_for_uid(uid: &str) -> usize {
    let mut h: u64 = 0;
    for b in uid.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as u64);
    }
    (h % 6) as usize
}

/// In-memory store keyed by UID, plus a stable ordering.
///
/// `items` is private: it must only change through `insert`/`remove`/
/// `remove_series` so the `index` map stays in sync. Use [`Store::items`] for
/// read-only access.
#[derive(Debug, Clone, Default)]
pub struct Store {
    items: Vec<Appointment>,
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
            self.items.swap_remove(pos);
            if let Some(moved) = self.items.get(pos) {
                self.index.insert(moved.uid.clone(), pos);
            }
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

    /// Appointments that fall on `date`, sorted by start time, as borrowed
    /// references. Rendering the month grid calls this once per cell; returning
    /// references (rather than clones) keeps a 4000-event store cheap to redraw.
    pub fn on_date(&self, date: NaiveDate) -> Vec<&Appointment> {
        let mut v: Vec<&Appointment> = self
            .items
            .iter()
            .filter(|a| {
                let sd = a.date();
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

    /// All appointments in insertion order. Read-only: mutations must go
    /// through `insert`/`remove`/`remove_series` so the UID index stays valid.
    pub fn items(&self) -> &[Appointment] {
        &self.items
    }

    /// Consume the store, returning its appointments in insertion order.
    pub fn into_items(self) -> Vec<Appointment> {
        self.items
    }
}

/// Build a local DateTime from a naive value, falling back across DST
/// transitions where the local time is ambiguous or non-existent.
///
/// `from_local_datetime(...).single()` returns `None` in that case; we fall
/// back to interpreting the naive value as a UTC timestamp, which is acceptable
/// for this app (times near a DST boundary may shift by an hour).
pub fn local_from_naive(ndt: NaiveDateTime) -> DateTime<Local> {
    Local
        .from_local_datetime(&ndt)
        .single()
        .unwrap_or_else(|| Local.timestamp_opt(ndt.and_utc().timestamp(), 0).unwrap())
}

/// Helper to build a local DateTime from date + optional time components.
/// `hour`/`min` are clamped instead of panicking so a hand-edited service
/// config or form input can never crash the daemon/app.
pub fn make_datetime(date: NaiveDate, hour: u32, min: u32) -> DateTime<Local> {
    let ndt = NaiveDateTime::new(
        date,
        chrono::NaiveTime::from_hms_opt(hour.min(23), min.min(59), 0).unwrap(),
    );
    local_from_naive(ndt)
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}
