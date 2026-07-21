# AGENTS.md

Guide for AI agents working on the **ShadowDate** app.

## Overview

A native **Rust + GTK4** desktop calendar for Linux (Wayland / Hyprland) with a gothic,
dark-pastel look. Month-view grid, appointment create/edit/delete form, multilingual
UI, and **iCalendar (.ics)** import/export. Appointments are stored as a single `.ics`
file, which is also the on-disk format and the export format (so save == write ics).

Previously known as "calendar"; the app was renamed to **ShadowDate** (binary
`shadowdate`, package id `0xravenblack.shadowdata`).

## Layout

```
Cargo.toml              # [[bin]] shadowdate + [lib] calendar; deps: gtk4 (0.11), ical, chrono, chrono-tz, uuid, anyhow
src/
  lib.rs                # pub mod model; pub mod io_ics;  (library target for tests)
  main.rs               # app bootstrap, window, headerbar, file choosers, responsive signal
  model.rs              # Appointment struct + in-memory Store (keyed by UID)
  io_ics.rs             # parse/serialize .ics, import/export, load/save, merge
  calendar_view.rs      # month grid, nav, day list, responsive two-pane, background portrait
  form_dialog.rs        # create/edit/delete appointment dialog (620x520, non-resizable, fits the window); Cancel/Save live in the form (right-aligned), time uses a SpinButton grid, Delete asks for confirmation
  i18n.rs               # translations (EN/DE/FR/ES/ZH/JA/PL), date + weekday formatting
  images.rs             # embedded logo + portrait (include_bytes!), decoded to gdk::Texture
  calendar_view.rs / form_dialog.rs / i18n.rs / images.rs use `calendar::model` (the lib crate)
tests/
  ics.rs                # integration tests: ics round-trip, RRULE expansion, TZID,
                        # escaping, series delete (calendar_view has grid unit tests)
resources/
  style.css             # dark pastel theme (loaded at runtime via CssProvider)
  0xravenblack.shadowdata.desktop  # desktop entry (also installed by PKGBUILD)
  img/
    Logo.png              # 128x128 app logo (embedded; shown at 30px, scaled to 64px texture)
    portrait_face.png     # 1024x1024, shown translucently behind the calendar grid
    screenshot.jpg        # used by README only
PKGBUILD / .SRCINFO     # AUR package: clones the GitHub repo, builds, installs
```

## Build & run

- Build: `cargo build` (debug) or `cargo build --release`
- Run: `./target/release/shadowdate`
- Test: `cargo test` (ics round-trip + load tests)
- Lint/typecheck: `cargo clippy` (clean; no warnings expected)
- AUR build: `makepkg` (clones `https://github.com/0xRavenBlack/ShadowDate.git`)

## Key architecture decisions

- **Window**: `ApplicationWindow`, decorated, **non-resizable, non-maximizable**,
  fixed at **1024×560**. App ID = `0xravenblack.shadowdata` (also used as the icon
  name via `gtk::Window::set_default_icon_name` and the desktop `Icon=`/window class).
  Floating on Hyprland is enforced by `windowrule` in `~/.config/hypr/hyprland.conf`:
  `windowrule = float, class:(0xravenblack.shadowdata)` and
  `windowrule = size 1024 560, class:(0xravenblack.shadowdata)`.
- **Close button**: default title-buttons hidden (`set_show_title_buttons(false)`);
  a textual **"Exit"** button (`.exit-button` dark red CSS) closes the window.
- **Branding**: the `Logo.png` is embedded (`include_bytes!`) and shown as a 30px
  rounded icon plus a "ShadowDate" title in the headerbar's left side (`.brand-box`).
- **Headerbar controls** (always visible, even when small): leftmost = brand
  (logo + "ShadowDate"); then `‹ Today ›` nav box; right = `+ New`, `Import`,
  `Export`, `Exit`. All labels are localized via `i18n::t`.
- **Responsive layout**: `CalendarView::apply_responsive(width)` switches the
  content `Box` from horizontal (grid + side list) to vertical when width < 680px.
  Driven by `connect_notify_local` on the window's `default-width` property
  (main-thread only, because the view is held in `Rc<RefCell<Option<CalendarView>>>`
  and is not `Send`).
- **Background portrait**: the calendar content is wrapped in a `gtk::Overlay`; the
  `portrait_face.png` (embedded) sits behind as a translucent backdrop
  (`.bg-portrait`, `opacity: 0.30`), aligned to the start (left), full height, uniform
  width (aspect ratio preserved). Day cells are semi-transparent (`rgba(...)`) so the
  portrait shows through.
- **Month grid cells**: the `Grid` is `column_homogeneous` / `row_homogeneous` with a
  fixed cell height (64px) and chip labels capped via `set_max_width_chars` +
  ellipsize, so day cells never resize with appointment title length or count. Up to 3
  chips are shown per cell plus a "+N more" label; a **hover tooltip** on the cell
  lists every appointment's time, title, location, and description in full. The grid
  scrolled window does not propagate natural size (so it fills the fixed window).
- **Data model**: `Appointment { uid, series_uid, title, description, location,
  start, end, all_day, color_index }` with `chrono::DateTime<Local>`. `Store` is
  keyed by UID (`HashMap<uid, index>` + `Vec`). `series_uid` is the base event's UID
  for recurring occurrences (equal to `uid` for single events); color index is
  derived from `series_uid` so all occurrences of a series share a pastel class
  (`c0..c5`). `remove` uses `swap_remove` (re-indexes only the moved item);
  `remove_series(series_uid)` deletes every appointment in a series.
- **iCalendar**: uses the `ical` crate. Supports UTC (`...Z`), local, `TZID`-tagged
  (resolved via `chrono-tz`), and `VALUE=DATE` (all-day) datetimes. Unknown TZID
  values emit a warning and fall back to floating local time. `RRULE` recurrences
  (FREQ DAILY/WEEKLY/MONTHLY/YEARLY with INTERVAL, COUNT, UNTIL, BYDAY, BYMONTHDAY,
  BYMONTH, WKST) are **expanded at import** into individual occurrence appointments
  that share the base `series_uid` (capped at 4000 occurrences / 20 years). `EXDATE`
  dates are excluded from the expanded set; `RDATE` dates are appended. Export writes
  `DTSTART`/`DTEND` as UTC for timed events. Text values are escaped symmetrically
  (`\`, `;`, `,`, `\r`, `\n` → `\\`, `\;`, `\,`, `\r`, `\n`) on write and unescaped
  on read. Import merges into the store by UID (`merge_store`); for recurring events,
  existing occurrences of the same series are removed first to prevent orphaned entries
  when the RRULE is modified. Persistence path: `$XDG_DATA_HOME/calendar/calendar.ics`
  (falls back to `$HOME/.local/share/calendar/calendar.ics`, then
  `std::env::temp_dir()`). The Export dialog defaults to `shadowdate.ics`. Editing or
  deleting an occurrence acts on the **whole series** (`series_uid`); editing replaces
  the series with the single submitted (now non-recurring) appointment.
- **GTK4 dialogs are async**: `Dialog::run()` does not exist in gtk4 0.11; use
  `run_async` / `connect_response`. The appointment form delivers its result via a
  `  Box<dyn Fn(Option<Appointment>)>` callback (never blocks). On validation error the
  form stays open so the user can correct input. The form uses a `Grid` for the Start/
  End time (`SpinButton`s with "Hours"/"Minutes" column headers), Cancel + Save are a
  right-aligned button group inside the form (Save = `.suggested-action`), and **Delete**
  opens a `MessageDialog` confirmation before removing the appointment. All-day events
  get a non-color cue (`◆` on chips, a dashed left border + localized "All day" tag on
  rows). Empty days in the side list show a "+ Add appointment" CTA (uses `on_new`).

- **i18n**: `src/i18n.rs` detects the language from `LC_ALL`/`LC_MESSAGES`/`LANG`
  (cached once via `OnceLock`) and provides `t(key)` plus helpers `more_label`,
  `weekday_abbrevs`, `format_month_year`, and `format_date` (each with per-locale
  ordering). Note: time entry uses `SpinButton`s, so `must_be_number` was removed.
  Supported: English, German, French, Spanish, Chinese, Japanese, Polish. The embedded
  `ContentFit` is unavailable on the current gtk4 feature set, so sizing uses
  `set_keep_aspect_ratio`/`set_can_shrink`.

## Conventions

- `Box` for trait objects must be written as `std::boxed::Box` (the `gtk::Box` widget
  type is imported via `gtk::prelude::*` and shadows the std `Box`).
- Sharing the view across GTK signal callbacks uses `Rc<RefCell<Option<...>>>` (not
  `Arc<Mutex<...>>`) because the view is not `Send`/`Sync` and all access is on the
  main thread (`connect_notify_local`, `run_async`).
- The `calendar` crate is both a **bin** and a **lib**; `main.rs` uses `calendar::*`
  while `calendar_view.rs` / `form_dialog.rs` / `i18n.rs` / `images.rs` use
  `calendar::model`. The binary name is `shadowdate`; do not rename the lib crate
  without updating those paths.
- Embedded images: add new art via `include_bytes!` in `images.rs` and decode through
  a `Pixbuf` memory stream → `gdk::Texture` (the gdk4 `from_bytes` helper is not
  available on this version). Keep source PNGs small (Logo 128×128, portrait 1024×1024):
  `texture_from` scales the decoded pixbuf down to a display cap (logo 64px, portrait
  512px) before uploading to a GPU texture so full-resolution sources never allocate
  multi-megabyte textures. Re-encode assets with ImageMagick (`convert in.png -resize
  NxN out.png`) rather than committing huge PNGs.
- Keep CSS classes consistent with `resources/style.css` (pastel accents: lavender
  `#b39ddb`, mint `#a0e7c0`, peach `#f6c79b`, pink `#f4a3c0`, sky `#a7c7e7`, lilac
  `#c7b6e8` on charcoal `#1b1b26`).
- Localize user-facing strings through `i18n::t` / the i18n helpers — do not hardcode
  English text in UI widgets.

## Common tasks

- Add a field to appointments: update `Appointment` in `model.rs`, the form in
  `form_dialog.rs`, ics mapping in `io_ics.rs`, and the row/chip rendering in
  `calendar_view.rs`.
- Change theme: edit `resources/style.css` (loaded at startup in `load_css`).
- Add a language: add a column to every `match` in `i18n.rs` (and the two lookup
  tables in `format_date`/`format_month_year`), then extend `Lang` + `lang_index`.
- Change the background portrait / logo: replace the PNGs in `resources/img/` and
  rebuild (they are embedded; no runtime path needed).
- Change window size/float behavior: edit `main.rs` (`set_default_size`) and the
  Hyprland `windowrule`s in `~/.config/hypr/hyprland.conf`, then `hyprctl reload`.
- Release / package: bump `pkgver` in `PKGBUILD`, regenerate `.SRCINFO`
  (`makepkg --printsrcinfo > .SRCINFO`), commit, and tag the matching GitHub release.
