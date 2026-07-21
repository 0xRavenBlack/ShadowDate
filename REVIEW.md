# Code Review: ShadowDate

**Project**: ShadowDate — Rust + GTK4 desktop calendar  
**Review date**: 2026-07-21  
**Status**: 15 tests pass, clippy clean  
**Files reviewed**: `src/main.rs`, `src/lib.rs`, `src/model.rs`, `src/io_ics.rs`, `src/calendar_view.rs`, `src/form_dialog.rs`, `src/i18n.rs`, `src/images.rs`, `resources/style.css`, `Cargo.toml`, `PKGBUILD`, `tests/ics.rs`

---

## BLOCKERS

### B1. Importing a modified RRULE for an existing event UID leaves orphaned occurrences

**File**: `src/io_ics.rs:152-167` + `src/model.rs:112-119`  
**Severity**: Data-loss potential

When a recurring event is imported, the RRULE is expanded into individual appointment
occurrences with UIDs `base_uid#0`, `base_uid#1`, etc. If the user edits the RRULE in
an external tool and re-imports the same `.ics` file (with the same base UID but
different RRULE), the old expanded occurrence UIDs may not overlap with the new ones.
Since `merge_store`/`insert` only replaces exact UID matches, orphaned old occurrences
remain in the store alongside the new ones.

**Example**: Original has COUNT=3 (UIDs `daily-1#0`, `daily-1#1`, `daily-1#2`). After
changing to COUNT=5 (UIDs `daily-1#0`..`daily-1#4`), re-importing replaces `#0..#2`
but `#3..#4` are also new, while the old ones were already replaced. However, if the
count is reduced (COUNT=2), old `#2` remains orphaned.

**Recommended fix**: Before merging imported events, remove all existing items whose
`series_uid` matches the base UID of each imported series.

---

## MAJOR

### M1. Untranslated hardcoded "All day" in time_label

**File**: `src/model.rs:79-82`

```rust
pub fn time_label(&self) -> String {
    if self.all_day {
        "All day".to_string()
    } else { ... }
}
```

This string is used in tooltips on grid day cells (`calendar_view.rs:469`) and in the
side-panel row meta, but it is always English. The i18n module already provides
`t("all_day_short")` with translations. The `time_label()` function should use the
localized version instead.

### M2. Unknown TZID silently falls back to floating local time

**File**: `src/io_ics.rs:80-93`

When an imported event has a `TZID` parameter that cannot be resolved (unknown
timezone name, or `chrono-tz` doesn't support it), the datetime is silently treated
as floating local time. This means an event scheduled at 14:00 `America/New_York` on
a system that doesn't resolve that TZID would appear at 14:00 in the user's local
timezone, which is incorrect. Consider emitting at least a warning (or storing the
failure for display).

### M3. Duplicate CSS rules override intended styling

**File**: `resources/style.css`
- `.day-cell` defined at lines 111 and again at 393  
- `.appt-chip` defined at lines 156 and again at 399

The second block overrides padding and font-size of the first unconditionally (no
media query or conditional selector). The comment on line 393 says "keep small and
legible on narrow windows", suggesting these were intended to be responsive overrides
but are always active. Effective result:
- `.day-cell` padding is 3px (not 6px)
- `.appt-chip` font-size is 10px (not 11px) and padding is 1px 4px (not 2px 6px)

Either merge the definitions or (if responsive behavior is desired) apply the
overrides programmatically via GTK style classes when the window narrows.

### M4. Poll-based 150ms timer for responsive layout is wasteful

**File**: `src/main.rs:199-205`

```rust
gtk::glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
    let w = win.width();
    if let Some(v) = vref.borrow().as_ref() {
        v.apply_responsive(w);
    }
    gtk::glib::ControlFlow::Continue
});
```

A perpetual 150ms timer polling window width is wasteful for battery life and CPU.
GTK4 provides `notify::default-width` signals or `size-allocate` on the content area.
For example, connect to the window's or the overlay's `size_allocate` signal instead.

### M5. `escape_text` silently drops carriage return characters

**File**: `src/io_ics.rs:574-580`

```rust
s.replace('\\', "\\\\")
    .replace(';', "\\;")
    .replace(',', "\\,")
    .replace('\n', "\\n")
    .replace('\r', "")
```

According to RFC 5545 Section 3.3.11, literal carriage returns in property values
should be escaped as `\r` (or encoded via `\n` for line breaks). Dropping `\r`
silently alters the original data. If an event description contains `\r\n` (common in
Windows-originated text), the sequence becomes only `\n`, and after unescape it's a
plain newline — which is usually acceptable, but a round-trip through the app would
lose the distinction.

### M6. `form_dialog.rs:191` — `.unwrap()` on button root can panic

**File**: `src/form_dialog.rs:191`

```rust
let dialog = b.root().and_downcast::<Dialog>().unwrap();
```

If the delete button is somehow detached from the widget tree when clicked (during
rapid dialog dismissal, for example), `root()` returns `None`, and `unwrap()` panics.
This is a latent crash.

### M7. No EXDATE/RDATE handling

**File**: `src/io_ics.rs` — `event_to_appointments`

The importer does not process `EXDATE` (exception dates) or `RDATE` (recurrence
dates). If a recurring event has `EXDATE` entries to skip certain dates, those dates
would still appear in the calendar.

---

## MINOR

### N1. Numerous unchecked `unwrap()` calls on date/time construction

**Files**: `src/calendar_view.rs:295,305,404,406,408`, `src/io_ics.rs:69,108,254,283,338,353`,
`src/model.rs:182,186`

There are ~15 calls to `.unwrap()` on `from_ymd_opt()` and `from_hms_opt()` results.
Most are safe (values come from validated internal state), but a single corrupt state
(month 0, hour 27) would crash the application rather than displaying an error.

### N2. Full grid + list re-renders on every click

**File**: `src/calendar_view.rs:326-372`

Every day-cell click or row click triggers `refresh_all()`, which fully clears and
repopulates the entire month grid and the day appointment list. For 31 cells each
containing labels and gesture controllers, this is noticeable allocation churn.
Consider incremental updates (e.g., just toggle the selected class on old/new day
cells, and only re-render the list).

### N3. Unnecessary `render_day()` call in row click handler

**File**: `src/calendar_view.rs:371`

```rust
render_day(&lb, &dl, &st, &sto, &on_edit, &on_new);
```

After calling `on_edit(&appt)` (which opens an async dialog), the code immediately
calls `render_day()` again. Since the dialog hasn't closed yet, the store hasn't
changed, so this re-render is a no-op (the list looks identical). Remove this
redundant call.

### N4. `wire_nav()` clones many widget handles redundantly

**File**: `src/calendar_view.rs:146-217`

Each button closure clones the entire set of `Rc<T>` references and widget handles
(`grid`, `list_box`, `month_label`, etc.). This is 7-8 clones per button, for 4
buttons. The `Rc` clones are cheap (just ref-count bumps), but the widget clones are
unnecessary — they could be captured by the `self` reference in a method. Consider
using a `Rc<CalendarView>` pattern or connecting handlers after construction.

### N5. Large `match`-based translation table

**File**: `src/i18n.rs:42-228`

The `t()` function uses a single `match` statement with ~25 arms, each containing
7-tuple literals. This is O(n) on every translation call and generates substantial
code. A `HashMap<&'static str, [&'static str; 7]>` or a macro-based approach would
be more maintainable and slightly more efficient. The `???` fallback gives no
visibility into missing keys in development.

### N6. Window title "Shadow Date" vs app name "ShadowDate"

**File**: `src/main.rs:66`

```rust
.title("Shadow Date")
```

The window title has a space ("Shadow Date"), while the binary, package, repo, and
AGENTS.md consistently use "ShadowDate" (no space). This is inconsistent branding.

### N7. No `WKST` support in RRULE expansion

**File**: `src/io_ics.rs:234-249`

RFC 5545 allows `WKST` (week start day) to override the default Monday start for
weekly rules. The code always assumes Monday. For calendars that use Sunday as the
week start, weekly occurrences could be off by one day.

### N8. `chrono::Duration` usage (deprecated type in chrono ≥ 0.4.35)

**Files**: `src/io_ics.rs`, `src/model.rs`, `src/form_dialog.rs`

`chrono::Duration` was renamed to `chrono::TimeDelta` in chrono 0.4.35. The project
is currently pinned via `Cargo.lock` to 0.4.45, where `Duration` is a deprecated
alias. Consider migrating to `TimeDelta`.

### N9. `parse_byday()` prefix stripping logic is fragile

**File**: `src/io_ics.rs:500-515`

The `parse_byday` function has two code paths for extracting a numeric prefix from
a BYDAY token, but the first branch (for `+`/`-` prefixed values) calls
`tok.starts_with('-')` after already stripping the prefix via
`strip_prefix(['+', '-'])`. This works but is opaque. The second branch duplicates
digit-stripping logic. Refactoring into a single helper would reduce the chance of
bugs.

### N10. `DTSTAMP` regenerated on every export

**File**: `src/io_ics.rs:559-564`

`DTSTAMP` is computed as `Local::now()` on every `store_to_ics` call. RFC 5545
says DTSTAMP should be "the date and time that the instance of the iCalendar object
was created". In practice this is minor, but storing the original creation timestamp
and only updating DTSTAMP on modification would be more standards-compliant.

### N11. `expand_recurrence()` cap logic allows one extra occurrence past COUNT

**File**: `src/io_ics.rs:219-224`

```rust
if let Some(c) = rule.count {
    if emitted >= c {
        return false;  // stops after emitting c items
    }
}
```

When `emitted >= count` returns false, `count_ok` returns false, which causes the
callers to return `finish()`. However, when `emitted` reaches `count` exactly, the
line `emitted < MAX_OCCURRENCES` on line 224 returns false (since emitted == count,
but normally count < MAX_OCCURRENCES). Actually this check at line 224 would also
return false, but only because the `count` check returned false first. So the
behavior is correct but the flow is confusing: line 224 (`emitted < MAX_OCCURRENCES`)
is only reached when `count` is `None` or `emitted < count`. The overlap between
the two limits is confusing.

### N12. Test file `tests/ics.rs` uses `from_ymd_opt(...).unwrap()` in helper `d()`

**File**: `tests/ics.rs:131-133`

```rust
fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}
```

Test-only, and always called with valid dates. Not a runtime concern, but if a test
date is wrong, the panic message won't be helpful.

### N13. `PKGBUILD:12` — `gtk4` listed in both `depends` and `makedepends`

**File**: `PKGBUILD:11-12`

```bash
depends=('gtk4' 'glib2')
makedepends=('git' 'cargo' 'gtk4')
```

Since `gtk4` is already a runtime dependency, it doesn't need to be listed in
`makedepends` (it will be pulled as a transitive make dependency automatically).

---

## Summary

| Category | Count |
|----------|-------|
| BLOCKER  | 1     |
| MAJOR    | 7     |
| MINOR    | 13    |
| **Total** | **21** |

The codebase is well-structured, idiomatic Rust with good test coverage (12
integration tests + 3 unit tests, all passing). Clippy is clean. The main
architectural concerns are around ICS import correctness (orphaned RRULE
occurrences, missing EXDATE support, silent TZID fallback) and the unbounded
poll-based responsive check.
