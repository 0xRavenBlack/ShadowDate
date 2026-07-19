# AGENTS.md

Guide for AI agents working on the **ShadowDate** app.

## Overview

A native **Rust + GTK4** desktop calendar for Linux (Wayland / Hyprland) with a gothic,
dark-pastel look. Month-view grid, appointment create/edit/delete form, multilingual
UI, and **iCalendar (.ics)** import/export. Appointments are stored as a single `.ics`
file, which is also the on-disk format and the export format (so save == write ics).

Previously known as "calendar"; the app was renamed to **ShadowDate** (binary
`shadowdate`, package id `com.ravenblack.shadowdate`).

## Layout

```
Cargo.toml              # [[bin]] shadowdate + [lib] calendar; deps: gtk4 (0.11), ical, chrono, chrono-tz, uuid, anyhow
src/
  lib.rs                # pub mod model; pub mod io_ics;  (library target for tests)
  main.rs               # app bootstrap, window, headerbar, file choosers, responsive poll
  model.rs              # Appointment struct + in-memory Store (keyed by UID)
  io_ics.rs             # parse/serialize .ics, import/export, load/save, merge
  calendar_view.rs      # month grid, nav, day list, responsive two-pane, background portrait
  form_dialog.rs        # create/edit/delete appointment dialog (620x520, non-resizable, fits the window)
  i18n.rs               # translations (EN/DE/FR/ES/ZH/JA/PL), date + weekday formatting
  images.rs             # embedded logo + portrait (include_bytes!), decoded to gdk::Texture
  calendar_view.rs / form_dialog.rs / i18n.rs / images.rs use `calendar::model` (the lib crate)
tests/
  ics.rs                # integration tests: ics round-trip, RRULE expansion, TZID,
                        # escaping, series delete (calendar_view has grid unit tests)
resources/
  style.css             # dark pastel theme (loaded at runtime via CssProvider)
  com.ravenblack.shadowdate.desktop  # desktop entry (also installed by PKGBUILD)
  img/
    Logo.png            # 2048x2048 app logo, embedded as the icon/branding
    portrait_face.png   # 2048x2048, shown translucently behind the calendar grid
    screenshot.jpg      # used by README only
PKGBUILD / .SRCINFO     # AUR package: clones the GitHub repo, builds, installs
```

## Build & run

- Build: `cargo build` (debug) or `cargo build --release`
- Run: `./target/release/shadowdate`
- Test: `cargo test` (ics round-trip + load tests)
- Lint/typecheck: `cargo clippy` (clean; no warnings expected)
- AUR build: `makepkg` (clones `https://github.com/0xRavenBlack/com.ravenblack.shadowdate.git`)

## Key architecture decisions

- **Window**: `ApplicationWindow`, decorated, **non-resizable, non-maximizable**,
  fixed at **1024×560**. App ID = `com.ravenblack.shadowdate` (also used as the icon
  name via `gtk::Window::set_default_icon_name` and the desktop `Icon=`/window class).
  Floating on Hyprland is enforced by `windowrule` in `~/.config/hypr/hyprland.conf`:
  `windowrule = float, class:(com.ravenblack.shadowdate)` and
  `windowrule = size 1024 560, class:(com.ravenblack.shadowdate)`.
- **Close button**: default title-buttons hidden (`set_show_title_buttons(false)`);
  a textual **"Exit"** button (`.exit-button` dark red CSS) closes the window.
- **Branding**: the `Logo.png` is embedded (`include_bytes!`) and shown as a 30px
  rounded icon plus a "ShadowDate" title in the headerbar's left side (`.brand-box`).
- **Headerbar controls** (always visible, even when small): leftmost = brand
  (logo + "ShadowDate"); then `‹ Today ›` nav box; right = `+ New`, `Import`,
  `Export`, `Exit`. All labels are localized via `i18n::t`.
- **Responsive layout**: `CalendarView::apply_responsive(width)` switches the
  content `Box` from horizontal (grid + side list) to vertical when width < 680px.
  Driven by a `gtk::glib::timeout_add_local` poll (main-thread only, because the
  view is held in `Rc<RefCell<Option<CalendarView>>>` and is not `Send`).
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
  (resolved via `chrono-tz`), and `VALUE=DATE` (all-day) datetimes. `RRULE`
  recurrences (FREQ DAILY/WEEKLY/MONTHLY/YEARLY with INTERVAL, COUNT, UNTIL, BYDAY,
  BYMONTHDAY, BYMONTH) are **expanded at import** into individual occurrence
  appointments that share the base `series_uid` (capped at 4000 occurrences / 20
  years). Export writes `DTSTART`/`DTEND` as UTC for timed events. Text values are
  escaped symmetrically (`\`, `;`, `,`, `\n` → `\\`, `\;`, `\,`, `\\n`; `\r` dropped)
  on write and unescaped on read. Import merges into the store by UID
  (`merge_store`). Persistence path: `$XDG_DATA_HOME/calendar/calendar.ics` (falls
  back to `$HOME/.local/share/calendar/calendar.ics`, then `std::env::temp_dir()`).
  The Export dialog defaults to `shadowdate.ics`. Editing or deleting an occurrence
  acts on the **whole series** (`series_uid`); editing replaces the series with the
  single submitted (now non-recurring) appointment.
- **GTK4 dialogs are async**: `Dialog::run()` does not exist in gtk4 0.11; use
  `run_async` / `connect_response`. The appointment form delivers its result via a
  `Box<dyn Fn(Option<Appointment>)>` callback (never blocks). On validation error the
  form stays open so the user can correct input.
- **i18n**: `src/i18n.rs` detects the language from `LC_ALL`/`LC_MESSAGES`/`LANG`
  (cached once via `OnceLock`) and provides `t(key)` plus helpers `must_be_number`,
  `more_label`, `weekday_abbrevs`, `format_month_year`, and `format_date` (each with
  per-locale ordering). Supported: English, German, French, Spanish, Chinese,
  Japanese, Polish. The embedded `ContentFit` is unavailable on the current gtk4
  feature set, so sizing uses `set_keep_aspect_ratio`/`set_can_shrink`.

## Conventions

- `Box` for trait objects must be written as `std::boxed::Box` (the `gtk::Box` widget
  type is imported via `gtk::prelude::*` and shadows the std `Box`).
- Sharing the view across GTK signal callbacks uses `Rc<RefCell<Option<...>>>` (not
  `Arc<Mutex<...>>`) because the view is not `Send`/`Sync` and all access is on the
  main thread (`timeout_add_local`, `run_async`).
- The `calendar` crate is both a **bin** and a **lib**; `main.rs` uses `calendar::*`
  while `calendar_view.rs` / `form_dialog.rs` / `i18n.rs` / `images.rs` use
  `calendar::model`. The binary name is `shadowdate`; do not rename the lib crate
  without updating those paths.
- Embedded images: add new art via `include_bytes!` in `images.rs` and decode through
  a `Pixbuf` memory stream → `gdk::Texture` (the gdk4 `from_bytes` helper is not
  available on this version).
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
