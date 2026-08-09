# AGENTS.md

Guide for AI agents working on the **ShadowDate** app.

## Overview

A native **Rust + GTK4** desktop calendar for Linux (Wayland / Hyprland) with a gothic,
dark-pastel look. Month-view grid, appointment create/edit/delete form, multilingual
UI, **iCalendar (.ics)** import/export, and a background **reminder service**
(systemd-user daemon firing desktop notifications). Appointments are stored as a
single `.ics` file, which is also the on-disk format and the export format (so
save == write ics).

Previously known as "calendar"; the app was renamed to **ShadowDate** (binary
`shadowdate`, package id `0xravenblack.shadowdata`). The reminder daemon is the
binary `shadowdate-service`, installed as the systemd user unit
`shadowdate-service.service`.

## Layout

```
Cargo.toml              # [[bin]] shadowdate + [[bin]] shadowdate-service + [lib] shadowdate;
                        # deps: gtk4 (0.11), glib/gio (0.22), ical, chrono, chrono-tz, uuid,
                        # anyhow, resvg, serde (derive), toml
src/
  lib.rs                # pub mod model; pub mod ical_import; pub mod ical_export;
                        # pub mod rrule; pub mod store_io; pub mod i18n; pub mod paths; pub mod service
  main.rs               # app bootstrap, window, headerbar, file choosers, AppContext
  model.rs              # Appointment struct + in-memory Store (keyed by UID)
  ical_import.rs        # .ics parsing / import (tolerant), RRULE wiring, EXDATE/RDATE
  ical_export.rs        # .ics serialization / export, line folding, PRODID
  rrule.rs              # RRULE expansion engine (FREQ/INTERVAL/COUNT/UNTIL/BYDAY/…)
  store_io.rs           # atomic load/save/write_atomic, backup_corrupt, merge_store
  calendar_view.rs      # month grid (dots + keyboard nav), day list, background portrait
  form_dialog.rs        # create/edit/delete appointment dialog (620x520, non-resizable, fits the window); Cancel/Save live in the form (right-aligned), time uses a SpinButton grid, Delete asks for confirmation
  service_settings.rs   # ⚙️ reminders config dialog (lead time, all-day time, service start/stop/test)
  i18n.rs               # translations (EN/DE/FR/ES/ZH/JA/PL), date + weekday formatting (in the lib)
  images.rs             # embedded logo + portrait (include_bytes!), decoded to gdk::Texture
  paths.rs              # XDG data/config path helpers (data_path, config_path)
  service.rs            # ServiceConfig (toml), reminder_time, pending_reminders, notify, fired_key
  bin/
    shadowdate-service.rs # headless daemon: owns a D-Bus name, polls .ics + config, notifies
  main.rs / calendar_view.rs / form_dialog.rs / service_settings.rs use `shadowdate::i18n`
  and the rest of the lib crate via `shadowdate::*`
tests/
  ics.rs                # integration tests: ics round-trip, RRULE expansion, TZID,
                        # escaping, series delete (calendar_view has grid unit tests)
  service.rs            # unit tests for reminder_time / pending_reminders / config (no D-Bus)
resources/
  style.css             # dark pastel theme (loaded at runtime via CssProvider)
  svg/
    logo.svg              # vector app logo (embedded; shown at 30px, rasterized at 64px)
    face.svg              # vector background portrait, shown translucently behind the grid
  img/
    screenshot.jpg        # used by README only
PKGBUILD / .SRCINFO     # AUR package: the package is just the PKGBUILD — every
                        # source (desktop entry, icon, systemd unit, license) is
                        # downloaded at build time from the repo at the release
                        # tag via raw.githubusercontent.com
0xravenblack.shadowdata.desktop / shadowdate-service.service / LICENSE
                        # the only repo files the PKGBUILD harvests from the repo
                        # root (logo.svg comes from resources/svg/); they exist
                        # at the release tag, so no flat local copies are needed
```

## Build & run

- Build: `cargo build` (debug) or `cargo build --release` (builds both binaries)
- Run: `./target/release/shadowdate`
- Reminder daemon: `./target/release/shadowdate-service` (or `systemctl --user
  enable --now shadowdate-service` after installing the unit)
- Test: `cargo test` (ics round-trip + load tests + service scheduling tests)
- Lint/typecheck: `cargo clippy --all-targets` (clean; no warnings expected)
- AUR build: `makepkg` — the AUR package is **just the PKGBUILD** (plus the
  generated `.SRCINFO`). It downloads the prebuilt `shadowdate-<pkgver>-x86_64-linux`
  and `shadowdate-service-<pkgver>-x86_64-linux` binaries from the GitHub release
  and harvests the desktop entry, icon, systemd unit, and license from the repo
  at the release tag via raw.githubusercontent.com; no compilation, no local
  source files, no install script. NOTE: do not run `makepkg`
  inside the repo itself (its `src/` dir collides with the tracked source — and
  `src/` is NOT gitignored, so a stray build dir shows up in `git status`); copy
  the repo or set a separate `BUILDDIR`.

## Key architecture decisions

- **Window**: `ApplicationWindow`, decorated, **non-resizable, non-maximizable**,
  fixed at **1024×560**. App ID = `0xravenblack.shadowdata` (also used as the icon
  name via `gtk::Window::set_default_icon_name` and the desktop `Icon=`/window class).
  Floating on Hyprland is enforced by `windowrule` in `~/.config/hypr/hyprland.conf`:
  `windowrule = float, class:(0xravenblack.shadowdata)` and
  `windowrule = size 1024 560, class:(0xravenblack.shadowdata)`.
- **Close button**: default title-buttons hidden (`set_show_title_buttons(false)`);
  a textual **"Exit"** button (`.exit-button` dark red CSS) closes the window.
- **Single instance**: `main()` first runs a `/proc` pre-check
  (`bail_if_already_running`) that quits a second `shadowdate` process before GTK
  starts — it scans `/proc/*/comm` for a `shadowdate` process owned by the same
  effective UID, skipping zombies. This is the app's **only** single-instance
  guard: `APP_ID` starts with a digit, so it is not a valid GApplication id and
  the `gtk::Application`'s session-bus registration never engages (a second
  process would otherwise build a second window). The `shadowdate-service`
  daemon guards itself separately via its D-Bus name (`SERVICE_NAME`,
  `DO_NOT_QUEUE`).
- **Branding**: the `logo.svg` is embedded (`include_bytes!`, rasterized with `resvg`)
  and shown as a 30px rounded icon plus a "ShadowDate" title in the headerbar's left
  side (`.brand-box`).
- **Headerbar controls** (always visible, even when small): leftmost = brand
  (logo + "ShadowDate"); then `‹ Today ›` nav box; right = `+ New`, `Import`,
  `Export`, `Exit`. All labels are localized via `i18n::t`.
- **Fixed window**: the window is `set_resizable(false)` and fixed at 1024×560,
  so there is no responsive/stacking code path; the two-pane layout
  (grid + side list) is always horizontal.
- **Background portrait**: the calendar content is wrapped in a `gtk::Overlay`; the
  `face.svg` (embedded, rasterized by `resvg`) sits behind as a translucent backdrop
  (`.bg-portrait`, `opacity: 0.30`), aligned to the start (left), full height, uniform
  width (aspect ratio preserved). Day cells are semi-transparent (`rgba(...)`) so the
  portrait shows through.
- **Month grid cells**: the `Grid` is `column_homogeneous` / `row_homogeneous`
  and always renders a solid 6×7 frame: days from the previous/next month fill
  the first/last rows and are dimmed (`.day-cell.other-month`), so the grid never
  looks ragged. Each cell keeps a fixed footprint — the day number is pinned to
  the top (so numbers align across every row) with up to 5 small colored dots
  below (one per appointment; `●` timed, `○` all-day, colored by `c0..c5`). Days
  with more than 5 appointments show a compact `+N` count. Cells never resize
  with appointment count or title length; a **hover tooltip** on the cell lists
  every appointment's time, title, location, and description in full. The grid
  scroller is focusable and the grid scrolls only if the window were ever shrunk
  below natural size.
- **Keyboard navigation**: the grid scroller is focusable and holds the
  `EventControllerKey`s: Arrow keys move the selection by ±1 day / ±7 days
  (crossing month boundaries navigates the view), and Enter/Space opens the
  new-appointment form for the selected day. After every rebuild the scroller
  re-grabs focus so arrow navigation keeps working; the selected day is marked
  by the `.selected` border.
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
- **Reminder service**: `src/service.rs` holds the pure scheduling logic
  (`ServiceConfig` + serde/toml, `reminder_time`, `pending_reminders`,
  `prune_fired`, `fired_key`) and the D-Bus plumbing (`notification_proxy`,
  `notify` via the freedesktop Notification Protocol on the session bus).
  `pending_reminders` is a **pure function** (no I/O) so the dedupe / due-window
  rules are unit-tested in `tests/service.rs` without a daemon. The store is
  fully RRULE-expanded, so scheduling is straight off `Store`. Rules: timed
  events are reminded `lead_min` minutes before `start`, but **never after the
  event ends** (a reminder missed while the daemon slept still fires for an
  ongoing event); all-day events fire **once**, at `all_day_hour:all_day_minute`
  on their **start date only** (multi-day all-day events are announced exactly
  once); results are sorted by start. Dedupe keys are `uid@<reminder_ts>`, so
  editing an event (same UID, new time) re-fires; keys are pruned after 6h.
  The daemon (`src/bin/shadowdate-service.rs`) is headless (no GTK widgets), owns
  the session name `org.ravenblack.ShadowDate.Service` with `DO_NOT_QUEUE` (a
  second instance exits), and **polls** the `.ics` file + config every 1s via
  `SystemTime` mtimes — an mtime change reloads the store/config, keeping the
  last good one on parse errors. The app saves the `.ics` **atomically**
  (`write_atomic`: temp file + rename) so the daemon never reads a torn file.
  Config lives at `$XDG_CONFIG_HOME/shadowdate/service.toml` (`paths::config_path`).
  The app's ⚙️ **Settings** dialog (`service_settings.rs`) edits the config, tests
  notifications, and starts/stops the systemd user unit `shadowdate-service`
  (`systemctl --user enable/disable --now`), detecting running state via
  D-Bus `NameHasOwner`.
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
- The `shadowdate` crate is both a **bin** and a **lib**; `main.rs` uses `shadowdate::*`
  while `calendar_view.rs` / `form_dialog.rs` / `i18n.rs` / `images.rs` use
  `shadowdate::model`. The package and lib crate are both named `shadowdate`
  (lib imports are `shadowdate::...`); the `calendar_view` module/file keeps its name.
  `i18n`, `paths`, and `service` live **in the lib**
  (not in `main.rs`) so the headless `shadowdate-service` bin can use them without
  pulling in GTK.
- Embedded images: add new art via `include_bytes!` in `images.rs` and rasterize the
  SVG with `resvg` (bundles `usvg` + `tiny-skia`, pure Rust, no system deps) into a
  `tiny_skia::Pixmap`, then upload it as a `gdk::MemoryTexture` (the gdk4 `from_bytes`
  helper is not available on this version). `texture_from` rasterizes at a display cap
  (logo 64px, portrait 512px) so the GPU texture never grows multi-megabyte. Note: the
  gdk-pixbuf SVG loader is NOT installed on this system, so `Pixbuf::from_stream`
  cannot load the SVGs. Keep SVG sources reasonably small; re-encode with
  `rsvg-convert`/ImageMagick rather than committing huge files.
- Keep CSS classes consistent with `resources/style.css` (pastel accents: lavender
  `#b39ddb`, mint `#a0e7c0`, peach `#f6c79b`, pink `#f4a3c0`, sky `#a7c7e7`, lilac
  `#c7b6e8` on charcoal `#1b1b26`).
- Localize user-facing strings through `i18n::t` / the i18n helpers — do not hardcode
  English text in UI widgets.

## Common tasks

- Add a field to appointments: update `Appointment` in `model.rs`, the form in
  `form_dialog.rs`, ics mapping in `ical_import.rs` / `ical_export.rs`, and the
  row/chip rendering in `calendar_view.rs`.
- Change theme: edit `resources/style.css` (loaded at startup in `load_css`).
- Add a language: add a column to every `match` in `i18n.rs` (and the two lookup
  tables in `format_date`/`format_month_year`), then extend `Lang` + `lang_index`.
- Change the background portrait / logo: replace the SVGs in `resources/svg/` and
  rebuild (they are embedded; no runtime path needed).
- Change window size/float behavior: edit `main.rs` (`set_default_size`) and the
  Hyprland `windowrule`s in `~/.config/hypr/hyprland.conf`, then `hyprctl reload`.
- Release / package: publish a GitHub release **tagged `v<pkgver>`** with the
  `shadowdate-<pkgver>-x86_64-linux` and `shadowdate-service-<pkgver>-x86_64-linux`
  binaries uploaded as release assets (the AUR package installs these directly).
  Then bump `pkgver` in `PKGBUILD`, update the binaries' `sha256sums`
  (`makepkg -g` regenerates them), rebuild `.SRCINFO`
  (`makepkg --printsrcinfo > .SRCINFO`), and commit.
  **`pkgver` in `PKGBUILD` is the single source of truth for the version number**:
  always take the version from there and mirror it into `Cargo.toml` (`[package]
  version`), keeping the two in sync.
- Add a reminder setting: update `Reminders` in `service.rs` (serde fields), the
  UI in `service_settings.rs`, and the `tests/service.rs` config round-trip test.
