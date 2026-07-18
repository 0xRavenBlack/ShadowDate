# AGENTS.md

Guide for AI agents working on the **calendar** app.

## Overview

A native **Rust + GTK4** desktop calendar for Linux (Wayland / Hyprland). Dark pastel
theme, month-view grid, appointment create/edit form, and **iCalendar (.ics)**
import/export. Appointments are stored as a single `.ics` file, which is also the
on-disk format and the export format (so save == write ics).

## Layout

```
Cargo.toml              # deps: gtk4 (0.11), ical (0.11), chrono, uuid, home
src/
  lib.rs                # pub mod model; pub mod io_ics;  (library target for tests)
  main.rs               # app bootstrap, window, headerbar, file choosers, responsive poll
  model.rs              # Appointment struct + in-memory Store (keyed by UID)
  io_ics.rs             # parse/serialize .ics, import/export, load/save, merge
  calendar_view.rs      # month grid, nav, day list, responsive two-pane
  form_dialog.rs        # create/edit appointment dialog (scrollable)
  calendar_view.rs / form_dialog.rs use `calendar::model` (the lib crate)
tests/
  ics.rs                # integration tests for ics round-trip + load behavior
resources/
  style.css             # dark pastel theme (loaded at runtime via CssProvider)
```

## Build & run

- Build: `cargo build` (debug) or `cargo build --release`
- Run: `./target/release/calendar`
- Test: `cargo test` (ics round-trip + load tests)
- Lint/typecheck: `cargo build` (no separate clippy/target configured; add if desired)

## Key architecture decisions

- **Window**: `ApplicationWindow`, decorated, **non-resizable, non-maximizable**,
  fixed at **1024×560**. App ID = `com.ravenblack.calendar` (GTK requires a dotted ID).
  Floating on Hyprland is enforced by `windowrule` in `~/.config/hypr/hyprland.conf`:
  `windowrule = float, class:(com.ravenblack.calendar)` and
  `windowrule = size 1024 560, class:(com.ravenblack.calendar)`.
- **Close button**: default title-buttons hidden (`set_show_title_buttons(false)`);
  a textual **"Exit"** button (`.exit-button` dark red CSS) closes the window.
- **Headerbar controls** (always visible, even when small): left = `‹ Today ›`
  nav; right = `+ New`, `Import`, `Export`, `Exit`.
- **Responsive layout**: `CalendarView::apply_responsive(width)` switches the
  content `Box` from horizontal (grid + side list) to vertical when width < 680px.
  Driven by a `gtk::glib::timeout_add_local` poll (main-thread only, because the
  view is held in `Arc<Mutex<Option<CalendarView>>>` and is not `Send`).
- **Data model**: `Appointment { uid, title, description, location, start, end,
  all_day, color_index }` with `chrono::DateTime<Local>`. `Store` is keyed by UID
  (`HashMap<uid, index>` + `Vec`). Color index is derived from the UID hash (stable
  per appointment, 6 pastel classes `c0..c5`).
- **iCalendar**: uses the `ical` crate. Supports UTC (`...Z`), local, and `VALUE=DATE`
  (all-day) datetimes. Export writes `DTSTART`/`DTEND` as UTC for timed events.
  Import merges into the store by UID (`merge_store`). Persistence path:
  `$XDG_DATA_HOME/calendar/calendar.ics` (falls back to `~/.local/share/calendar/calendar.ics`).
- **GTK4 dialogs are async**: `Dialog::run()` does not exist in gtk4 0.11; use
  `run_async` / `connect_response`. The appointment form delivers its result via a
  `Box<dyn Fn(Option<Appointment>)>` callback (never blocks).

## Conventions

- `Box` for trait objects must be written as `std::boxed::Box` (the `gtk::Box` widget
  type is imported via `gtk::prelude::*` and shadows the std `Box`).
- Sharing the view across GTK signal callbacks requires thread-safe types
  (`Arc<Mutex<...>>`); `Rc<RefCell<...>>` only works for single-threaded, non-`Send`
  closures (e.g. `run_async`/`timeout_add_local`).
- The `calendar` crate is both a **bin** and a **lib**; `main.rs` uses `calendar::*`
  while `calendar_view.rs` / `form_dialog.rs` use `calendar::model`.
- Keep CSS classes consistent with `resources/style.css` (pastel accents: lavender
  `#b39ddb`, mint `#a0e7c0`, peach `#f6c79b`, pink `#f4a3c0`, sky `#a7c7e7`, lilac
  `#c7b6e8` on charcoal `#1b1b26`).

## Common tasks

- Add a field to appointments: update `Appointment` in `model.rs`, the form in
  `form_dialog.rs`, ics mapping in `io_ics.rs`, and the row/chip rendering in
  `calendar_view.rs`.
- Change theme: edit `resources/style.css` (loaded at startup in `load_css`).
- Change window size/float behavior: edit `main.rs` (`set_default_size`) and the
  Hyprland `windowrule`s in `~/.config/hypr/hyprland.conf`, then `hyprctl reload`.
