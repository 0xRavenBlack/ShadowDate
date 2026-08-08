use calendar::model::{today, Appointment, Store};
use chrono::{Datelike, NaiveDate};
use gtk::prelude::*;
use gtk::{Box, Button, Grid, Label, ListBox, ListBoxRow, ScrolledWindow};
use std::cell::RefCell;
use std::rc::Rc;

struct ViewState {
    selected: NaiveDate,
    view_year: i32,
    view_month: u32,
}

pub struct CalendarView {
    pub widget: Box,
    grid: Grid,
    grid_scroll: ScrolledWindow,
    list_box: ListBox,
    month_label: Label,
    day_label: Label,
    state: Rc<RefCell<ViewState>>,
    store: Rc<RefCell<Store>>,
    on_edit: Rc<dyn Fn(&Appointment) + 'static>,
    on_new: Rc<dyn Fn(NaiveDate) + 'static>,
    pub prev_btn: Button,
    pub next_btn: Button,
    pub today_btn: Button,
    pub new_btn: Button,
}

impl CalendarView {
    pub fn new(
        store: Rc<RefCell<Store>>,
        on_edit: std::boxed::Box<dyn Fn(&Appointment) + 'static>,
        on_new: std::boxed::Box<dyn Fn(NaiveDate) + 'static>,
    ) -> Self {
        let sel = today();
        let state = Rc::new(RefCell::new(ViewState {
            selected: sel,
            view_year: sel.year(),
            view_month: sel.month(),
        }));
        let on_edit = Rc::from(on_edit);
        let on_new = Rc::from(on_new);

        // Root overlay: a translucent portrait sits behind the calendar content.
        let widget = Box::new(gtk::Orientation::Vertical, 0);
        let overlay = gtk::Overlay::new();
        overlay.set_hexpand(true);
        overlay.set_vexpand(true);

        if let Some(portrait) = crate::images::portrait_widget() {
            portrait.set_hexpand(true);
            portrait.set_vexpand(true);
            portrait.set_halign(gtk::Align::Start);
            portrait.set_valign(gtk::Align::Fill);
            portrait.set_margin_start(12);
            portrait.add_css_class("bg-portrait");
            overlay.set_child(Some(&portrait));
        }

        let inner = Box::new(gtk::Orientation::Vertical, 8);
        inner.set_margin_top(10);
        inner.set_margin_bottom(10);
        inner.set_margin_start(10);
        inner.set_margin_end(10);
        inner.set_hexpand(true);
        inner.set_vexpand(true);
        overlay.add_overlay(&inner);
        widget.append(&overlay);

        let month_label = Label::new(None);
        month_label.add_css_class("month-title");
        month_label.set_xalign(0.5);
        inner.append(&month_label);

        let content = Box::new(gtk::Orientation::Horizontal, 12);
        content.set_hexpand(true);
        content.set_vexpand(true);

        let grid = Grid::new();
        grid.set_column_spacing(4);
        grid.set_row_spacing(4);
        grid.set_column_homogeneous(true);
        grid.set_row_homogeneous(true);
        grid.add_css_class("calendar-grid");
        grid.set_halign(gtk::Align::Fill);
        // Fill the viewport so the 7 rows share the available height evenly;
        // cells are compact by construction and never drive the grid taller.
        grid.set_valign(gtk::Align::Fill);
        grid.set_vexpand(true);
        let grid_scroll = ScrolledWindow::builder()
            .child(&grid)
            .hexpand(true)
            .vexpand(true)
            .build();
        // Focusable so arrow-key navigation keeps working across cell rebuilds.
        grid_scroll.add_css_class("day-grid");
        grid_scroll.set_can_focus(true);
        content.append(&grid_scroll);

        let right = Box::new(gtk::Orientation::Vertical, 8);
        right.set_hexpand(false);
        right.set_vexpand(true);
        right.set_size_request(260, -1);

        let day_label = Label::new(None);
        day_label.add_css_class("day-title");
        right.append(&day_label);

        let list_box = ListBox::new();
        list_box.add_css_class("list-box");
        let list_scroll = ScrolledWindow::builder()
            .child(&list_box)
            .vexpand(true)
            .build();
        right.append(&list_scroll);

        content.append(&right);
        inner.append(&content);

        let prev_btn = Button::with_label("‹");
        prev_btn.add_css_class("nav-button");
        prev_btn.set_tooltip_text(Some(calendar::i18n::t("prev_month")));
        let next_btn = Button::with_label("›");
        next_btn.add_css_class("nav-button");
        next_btn.set_tooltip_text(Some(calendar::i18n::t("next_month")));
        let today_btn = Button::with_label(calendar::i18n::t("today"));
        today_btn.set_tooltip_text(Some(calendar::i18n::t("today")));
        let new_btn = Button::with_label(calendar::i18n::t("new"));
        new_btn.set_tooltip_text(Some(calendar::i18n::t("new")));
        new_btn.add_css_class("new-button");

        let view = Self {
            widget,
            grid,
            grid_scroll,
            list_box,
            month_label,
            day_label,
            state,
            store,
            on_edit,
            on_new,
            prev_btn,
            next_btn,
            today_btn,
            new_btn,
        };

        view.wire_nav();
        view.refresh();
        view
    }

    fn wire_nav(&self) {
        {
            let st = self.state.clone();
            let g = self.grid.clone();
            let ml = self.month_label.clone();
            let lb = self.list_box.clone();
            let dl = self.day_label.clone();
            let oe = self.on_edit.clone();
            let on = self.on_new.clone();
            let sto = self.store.clone();
            self.prev_btn.connect_clicked(move |_| {
                let mut s = st.borrow_mut();
                if s.view_month == 1 {
                    s.view_month = 12;
                    s.view_year -= 1;
                } else {
                    s.view_month -= 1;
                }
                drop(s);
                refresh_all(&g, &ml, &lb, &dl, &st, &sto, &oe, &on);
            });
        }
        {
            let st = self.state.clone();
            let g = self.grid.clone();
            let ml = self.month_label.clone();
            let lb = self.list_box.clone();
            let dl = self.day_label.clone();
            let oe = self.on_edit.clone();
            let on = self.on_new.clone();
            let sto = self.store.clone();
            self.next_btn.connect_clicked(move |_| {
                let mut s = st.borrow_mut();
                if s.view_month == 12 {
                    s.view_month = 1;
                    s.view_year += 1;
                } else {
                    s.view_month += 1;
                }
                drop(s);
                refresh_all(&g, &ml, &lb, &dl, &st, &sto, &oe, &on);
            });
        }
        {
            let st = self.state.clone();
            let g = self.grid.clone();
            let ml = self.month_label.clone();
            let lb = self.list_box.clone();
            let dl = self.day_label.clone();
            let oe = self.on_edit.clone();
            let on = self.on_new.clone();
            let sto = self.store.clone();
            self.today_btn.connect_clicked(move |_| {
                let t = today();
                {
                    let mut s = st.borrow_mut();
                    s.view_year = t.year();
                    s.view_month = t.month();
                    s.selected = t;
                }
                refresh_all(&g, &ml, &lb, &dl, &st, &sto, &oe, &on);
            });
        }
        {
            let st = self.state.clone();
            let on_new = self.on_new.clone();
            self.new_btn.connect_clicked(move |_| {
                let d = st.borrow().selected;
                on_new(d);
            });
        }
        {
            // Arrow keys move the selection across the month grid; Enter /
            // Space open the new-appointment form for the selected day. The
            // controller lives on the (focusable) grid scroller, so it keeps
            // receiving keys no matter which day cell is rebuilt on refresh.
            let st = self.state.clone();
            let g = self.grid.clone();
            let ml = self.month_label.clone();
            let lb = self.list_box.clone();
            let dl = self.day_label.clone();
            let oe = self.on_edit.clone();
            let on = self.on_new.clone();
            let sto = self.store.clone();
            let nav_keys = gtk::EventControllerKey::new();
            nav_keys.connect_key_pressed(move |_, keyval, _, _| {
                let selected = st.borrow().selected;
                let next = match keyval {
                    gtk::gdk::Key::Left => selected - chrono::TimeDelta::days(1),
                    gtk::gdk::Key::Right => selected + chrono::TimeDelta::days(1),
                    gtk::gdk::Key::Up => selected - chrono::TimeDelta::days(7),
                    gtk::gdk::Key::Down => selected + chrono::TimeDelta::days(7),
                    _ => return gtk::glib::Propagation::Proceed,
                };
                select_date(&st, &g, &ml, &lb, &dl, &sto, &oe, &on, next);
                gtk::glib::Propagation::Stop
            });
            self.grid_scroll.add_controller(nav_keys);

            let st = self.state.clone();
            let on = self.on_new.clone();
            let action_keys = gtk::EventControllerKey::new();
            action_keys.connect_key_pressed(move |_, keyval, _, _| {
                let d = st.borrow().selected;
                match keyval {
                    gtk::gdk::Key::Return
                    | gtk::gdk::Key::KP_Enter
                    | gtk::gdk::Key::space => {
                        on(d);
                        gtk::glib::Propagation::Stop
                    }
                    _ => gtk::glib::Propagation::Proceed,
                }
            });
            self.grid_scroll.add_controller(action_keys);
        }
    }

    pub fn refresh(&self) {
        refresh_all(
            &self.grid,
            &self.month_label,
            &self.list_box,
            &self.day_label,
            &self.state,
            &self.store,
            &self.on_edit,
            &self.on_new,
        );
    }
}

/// Select `date`, navigating the viewed month when it lies outside the
/// currently displayed month, and rebuild the whole view around it.
#[allow(clippy::too_many_arguments)]
fn select_date(
    state: &Rc<RefCell<ViewState>>,
    grid: &Grid,
    month_label: &Label,
    list_box: &ListBox,
    day_label: &Label,
    store: &Rc<RefCell<Store>>,
    on_edit: &Rc<dyn Fn(&Appointment) + 'static>,
    on_new: &Rc<dyn Fn(NaiveDate) + 'static>,
    date: NaiveDate,
) {
    {
        let mut s = state.borrow_mut();
        if s.view_year != date.year() || s.view_month != date.month() {
            s.view_year = date.year();
            s.view_month = date.month();
        }
        s.selected = date;
    }
    refresh_all(grid, month_label, list_box, day_label, state, store, on_edit, on_new);
}

#[allow(clippy::too_many_arguments)]
fn refresh_all(
    grid: &Grid,
    month_label: &Label,
    list_box: &ListBox,
    day_label: &Label,
    state: &Rc<RefCell<ViewState>>,
    store: &Rc<RefCell<Store>>,
    on_edit: &Rc<dyn Fn(&Appointment) + 'static>,
    on_new: &Rc<dyn Fn(NaiveDate) + 'static>,
) {
    render_month(grid, month_label, list_box, day_label, state, store, on_edit, on_new);
    render_day(list_box, day_label, state, store, on_edit, on_new);
}

#[allow(clippy::too_many_arguments)]
fn render_month(
    grid: &Grid,
    month_label: &Label,
    list_box: &ListBox,
    day_label: &Label,
    state: &Rc<RefCell<ViewState>>,
    store: &Rc<RefCell<Store>>,
    on_edit: &Rc<dyn Fn(&Appointment) + 'static>,
    on_new: &Rc<dyn Fn(NaiveDate) + 'static>,
) {
    while let Some(child) = grid.first_child() {
        grid.remove(&child);
    }
    let (view_year, view_month, selected) = {
        let s = state.borrow();
        (s.view_year, s.view_month, s.selected)
    };
    let month_name = calendar::i18n::format_month_year(view_year, (view_month - 1) as usize);
    month_label.set_text(&month_name);

    let weekdays = calendar::i18n::weekday_abbrevs();
    for (i, wd) in weekdays.iter().enumerate() {
        let l = Label::new(Some(wd));
        l.add_css_class("weekday-header");
        if i >= 5 {
            l.add_css_class("weekend-header");
        } else {
            l.add_css_class("weekday-workday");
        }
        l.set_xalign(0.5);
        grid.attach(&l, i as i32, 0, 1, 1);
    }

    let first = NaiveDate::from_ymd_opt(view_year, view_month, 1)
        .expect("view_year/view_month should always be valid");
    let first_weekday = first.weekday().num_days_from_monday() as i32;
    let t = today();

    // Render the full 6x7 frame so the grid is always a solid rectangle.
    // Cells before the 1st and after the last day come from the previous/next
    // month and are dimmed; clicking one navigates to its month.
    for r in 1..=6 {
        for c in 0..7 {
            let cell_index = ((r - 1) * 7 + c) as usize;
            let offset = cell_offset(first_weekday, cell_index);
            let date = first + chrono::TimeDelta::days(offset as i64);
            let other_month = date.year() != view_year || date.month() != view_month;
            let appts: Vec<Appointment> =
                store.borrow().on_date(date).into_iter().cloned().collect();
            let is_today = date == t;
            let is_selected = date == selected;
            let cell = build_cell(
                &date.day().to_string(),
                other_month,
                is_today,
                is_selected,
                &appts,
            );
            let st = state.clone();
            let g = grid.clone();
            let ml = month_label.clone();
            let lb = list_box.clone();
            let dl = day_label.clone();
            let oe = on_edit.clone();
            let on = on_new.clone();
            let sto = store.clone();
            // Cells are rebuilt on every render, so a fresh click gesture is
            // attached per cell; the old cell (and its controller) is dropped
            // when removed from the grid above, so this does not leak.
            let ev = gtk::GestureClick::new();
            ev.connect_pressed(move |_, _, _, _| {
                select_date(&st, &g, &ml, &lb, &dl, &sto, &oe, &on, date);
            });
            cell.add_controller(ev);
            grid.attach(&cell, c, r, 1, 1);
        }
    }

    // Keep keyboard focus on the grid scroller so arrow navigation continues
    // to work after every rebuild.
    if let Some(parent) = grid.parent() {
        parent.grab_focus();
    }
}

fn render_day(
    list_box: &ListBox,
    day_label: &Label,
    state: &Rc<RefCell<ViewState>>,
    store: &Rc<RefCell<Store>>,
    on_edit: &Rc<dyn Fn(&Appointment) + 'static>,
    on_new: &Rc<dyn Fn(NaiveDate) + 'static>,
) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }
    let s = state.borrow();
    day_label.set_text(&calendar::i18n::format_date(s.selected));
    let appts: Vec<Appointment> = store.borrow().on_date(s.selected).into_iter().cloned().collect();
    for a in &appts {
        let row = build_appt_row(a);
        let uid = a.uid.clone();
        let on_edit = on_edit.clone();
        let sto = store.clone();
        // Rows are rebuilt on each render; the old row and its controller drop
        // when removed from the list box above, so attaching a fresh gesture
        // per row does not leak.
        let ev = gtk::GestureClick::new();
        ev.connect_pressed(move |_, _, _, _| {
            let appt_opt = sto.borrow().get(&uid).cloned();
            if let Some(appt) = appt_opt {
                on_edit(&appt);
            }
        });
        row.add_controller(ev);
        let lbrow = ListBoxRow::new();
        lbrow.set_child(Some(&row));
        list_box.append(&lbrow);
    }
    if appts.is_empty() {
        let empty_box = Box::new(gtk::Orientation::Vertical, 6);
        empty_box.set_halign(gtk::Align::Center);
        empty_box.set_margin_top(16);
        let empty = Label::new(Some(calendar::i18n::t("no_appointments")));
        empty.add_css_class("empty-label");
        empty_box.append(&empty);

        let add_btn = Button::with_label(calendar::i18n::t("add_appointment"));
        add_btn.add_css_class("empty-cta");
        let selected = s.selected;
        let on_new = on_new.clone();
        add_btn.connect_clicked(move |_| {
            on_new(selected);
        });
        empty_box.append(&add_btn);

        let lbrow = ListBoxRow::new();
        lbrow.set_child(Some(&empty_box));
        lbrow.set_selectable(false);
        list_box.append(&lbrow);
    }
}

/// Map a 0-based cell index in the 7-column grid to the day-of-month offset
/// from the 1st, given the weekday of the 1st (Monday = 0). Row 0 holds the
/// weekday headers, so day rows start at cell 0 in row 1. Negative offsets are
/// days of the previous month; offsets >= days in the month are days of the
/// next month. This is the pure core of the grid alignment so it can be
/// unit-tested without a display.
fn cell_offset(first_weekday: i32, cell: usize) -> i32 {
    cell as i32 - first_weekday
}

fn build_cell(
    day_text: &str,
    other_month: bool,
    is_today: bool,
    is_selected: bool,
    appts: &[Appointment],
) -> Box {
    let cell = Box::new(gtk::Orientation::Vertical, 2);
    cell.add_css_class("day-cell");
    cell.set_valign(gtk::Align::Fill);
    if other_month {
        cell.add_css_class("other-month");
    }
    if is_today {
        cell.add_css_class("today");
    }
    if is_selected {
        cell.add_css_class("selected");
    }

    // Day number pinned to the top so the numbers align across every row,
    // regardless of how many appointments a day holds.
    let num = Label::new(Some(day_text));
    num.add_css_class("day-number");
    num.set_xalign(0.5);
    num.set_halign(gtk::Align::Center);
    if is_today {
        num.add_css_class("today-label");
    }
    cell.append(&num);

    // Compact colored dots: one per appointment, capped so the cell keeps a
    // fixed footprint no matter how full the day is. Full details stay in the
    // hover tooltip below.
    if !appts.is_empty() {
        let dot_row = Box::new(gtk::Orientation::Horizontal, 2);
        dot_row.add_css_class("dot-row");
        dot_row.set_halign(gtk::Align::Center);
        for a in appts.iter().take(5) {
            let dot = Label::new(Some(if a.all_day { "○" } else { "●" }));
            dot.add_css_class("appt-dot");
            dot.add_css_class(&format!("c{}", a.color_index % 6));
            if a.all_day {
                dot.add_css_class("all-day");
            }
            dot_row.append(&dot);
        }
        if appts.len() > 5 {
            let more = Label::new(Some(&calendar::i18n::more_compact(appts.len() - 5)));
            more.add_css_class("dot-count");
            dot_row.append(&more);
        }
        cell.append(&dot_row);
    }

    if !appts.is_empty() {
        let detail: Vec<String> = appts
            .iter()
            .map(|a| {
                let mut s = format!("• {}  {}", a.time_label(calendar::i18n::t("all_day_short")), a.title);
                if !a.location.is_empty() {
                    s.push_str(&format!("  @ {}", a.location));
                }
                if !a.description.is_empty() {
                    s.push_str(&format!("\n  {}", a.description));
                }
                s
            })
            .collect();
        cell.set_tooltip_text(Some(&detail.join("\n")));
    }
    cell
}

fn build_appt_row(a: &Appointment) -> Box {
    let row = Box::new(gtk::Orientation::Vertical, 2);
    row.add_css_class("appt-row");
    row.add_css_class(&format!("c{}", a.color_index));
    if a.all_day {
        row.add_css_class("all-day");
    }
    // Keep rows compact: single-line ellipsized title and meta so long titles
    // never inflate the row; the full text is available on hover.
    let title = Label::new(Some(&a.title));
    title.add_css_class("appt-title");
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.set_max_width_chars(32);
    row.append(&title);
    if a.all_day {
        let tag = Label::new(Some(calendar::i18n::t("all_day_short")));
        tag.add_css_class("all-day-tag");
        tag.set_xalign(0.0);
        row.append(&tag);
    }
    let meta = Label::new(Some(&format!("{}   {}", a.time_label(calendar::i18n::t("all_day_short")), a.location)));
    meta.add_css_class("appt-meta");
    meta.set_xalign(0.0);
    meta.set_ellipsize(gtk::pango::EllipsizeMode::End);
    meta.set_max_width_chars(32);
    row.append(&meta);
    if !a.description.is_empty() {
        let d = Label::new(Some(&a.description));
        d.add_css_class("appt-meta");
        d.set_xalign(0.0);
        d.set_ellipsize(gtk::pango::EllipsizeMode::End);
        d.set_max_width_chars(40);
        row.append(&d);
    }
    row.set_tooltip_text(Some(&format!(
        "{}\n{}",
        a.title,
        if a.location.is_empty() {
            String::new()
        } else {
            format!("@ {}", a.location)
        }
    )));
    row
}

#[cfg(test)]
mod tests {
    use super::cell_offset;

    #[test]
    fn first_of_month_landing_on_its_weekday() {
        // 1st is a Monday (first_weekday = 0): cell 0 is the 1st, cell 6 the 7th.
        assert_eq!(cell_offset(0, 0), 0);
        assert_eq!(cell_offset(0, 6), 6);
        // Cell 7 wraps to the next row: the 8th.
        assert_eq!(cell_offset(0, 7), 7);
    }

    #[test]
    fn weekday_offset_shifts_columns() {
        // 1st is a Wednesday (first_weekday = 2): cell 0 is two days before the
        // 1st (previous month), cell 2 is the 1st itself.
        assert_eq!(cell_offset(2, 0), -2);
        assert_eq!(cell_offset(2, 2), 0);
        // Cell 6 falls on the 5th of the month (offset 4).
        assert_eq!(cell_offset(2, 6), 4);
    }

    #[test]
    fn offsets_stay_within_grid_frame() {
        // The 6x7 frame holds 42 cells (indices 0..=41). With the 1st landing
        // anywhere in the week (first_weekday 0..=6) the offsets span at most
        // from -6 (day before a Sunday-leading week) to +41.
        for first in 0..7i32 {
            for cell in 0..42usize {
                let offset = cell_offset(first, cell);
                assert!(
                    (-6..=41).contains(&offset),
                    "offset {} out of range (first={}, cell={})",
                    offset,
                    first,
                    cell
                );
            }
        }
    }
}

