use crate::AppContext;
use shadowdate::model::{today, Appointment};
use chrono::{Datelike, NaiveDate};
use gtk::prelude::*;
use gtk::{Box, Button, Grid, Label, ListBox, ListBoxRow, Picture, ScrolledWindow};
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
    ctx: Rc<AppContext>,
    portrait: Option<Picture>,
    pub prev_btn: Button,
    pub next_btn: Button,
    pub today_btn: Button,
    pub new_btn: Button,
}

impl CalendarView {
    pub fn new(ctx: Rc<AppContext>) -> Rc<Self> {
        let sel = today();
        let state = Rc::new(RefCell::new(ViewState {
            selected: sel,
            view_year: sel.year(),
            view_month: sel.month(),
        }));

        // Root overlay: a translucent portrait sits behind the calendar content.
        let widget = Box::new(gtk::Orientation::Vertical, 0);
        let overlay = gtk::Overlay::new();
        overlay.set_hexpand(true);
        overlay.set_vexpand(true);

        let portrait = crate::images::portrait_widget();
        if let Some(p) = &portrait {
            p.set_hexpand(true);
            p.set_vexpand(true);
            p.set_halign(gtk::Align::Start);
            p.set_valign(gtk::Align::Fill);
            p.set_margin_start(12);
            p.add_css_class("bg-portrait");
            overlay.set_child(Some(p));
        }
        if let Some(p) = &portrait {
            p.set_visible(show_portrait_from_config());
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
        grid.add_css_class("calendar-grid");
        // Cells are a fixed 100x60, so the grid takes its natural size instead
        // of stretching to fill the window; day rows stay uniform via each
        // cell's minimum, while the weekday header row keeps its compact height.
        grid.set_halign(gtk::Align::Start);
        grid.set_valign(gtk::Align::Start);
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
        prev_btn.set_tooltip_text(Some(shadowdate::i18n::t("prev_month")));
        let next_btn = Button::with_label("›");
        next_btn.add_css_class("nav-button");
        next_btn.set_tooltip_text(Some(shadowdate::i18n::t("next_month")));
        let today_btn = Button::with_label(shadowdate::i18n::t("today"));
        today_btn.set_tooltip_text(Some(shadowdate::i18n::t("today")));
        let new_btn = Button::with_label(shadowdate::i18n::t("new"));
        new_btn.set_tooltip_text(Some(shadowdate::i18n::t("new")));
        new_btn.add_css_class("new-button");

        let view = Rc::new(Self {
            widget,
            grid,
            grid_scroll,
            list_box,
            month_label,
            day_label,
            state,
            ctx,
            portrait,
            prev_btn,
            next_btn,
            today_btn,
            new_btn,
        });

        view.wire_nav();
        view.refresh();
        view
    }

    fn wire_nav(self: &Rc<Self>) {
        {
            let this = self.clone();
            self.prev_btn.connect_clicked(move |_| {
                let mut s = this.state.borrow_mut();
                if s.view_month == 1 {
                    s.view_month = 12;
                    s.view_year -= 1;
                } else {
                    s.view_month -= 1;
                }
                drop(s);
                this.refresh_all();
            });
        }
        {
            let this = self.clone();
            self.next_btn.connect_clicked(move |_| {
                let mut s = this.state.borrow_mut();
                if s.view_month == 12 {
                    s.view_month = 1;
                    s.view_year += 1;
                } else {
                    s.view_month += 1;
                }
                drop(s);
                this.refresh_all();
            });
        }
        {
            let this = self.clone();
            self.today_btn.connect_clicked(move |_| {
                let t = today();
                {
                    let mut s = this.state.borrow_mut();
                    s.view_year = t.year();
                    s.view_month = t.month();
                    s.selected = t;
                }
                this.refresh_all();
            });
        }
        {
            let this = self.clone();
            self.new_btn.connect_clicked(move |_| {
                let d = this.state.borrow().selected;
                this.ctx.open_new(d);
            });
        }
        {
            // Arrow keys move the selection across the month grid; Enter /
            // Space open the new-appointment form for the selected day. The
            // controller lives on the (focusable) grid scroller, so it keeps
            // receiving keys no matter which day cell is rebuilt on refresh.
            let this = self.clone();
            let nav_keys = gtk::EventControllerKey::new();
            nav_keys.connect_key_pressed(move |_, keyval, _, _| {
                let selected = this.state.borrow().selected;
                let next = match keyval {
                    gtk::gdk::Key::Left => selected - chrono::TimeDelta::days(1),
                    gtk::gdk::Key::Right => selected + chrono::TimeDelta::days(1),
                    gtk::gdk::Key::Up => selected - chrono::TimeDelta::days(7),
                    gtk::gdk::Key::Down => selected + chrono::TimeDelta::days(7),
                    _ => return gtk::glib::Propagation::Proceed,
                };
                this.select_date(next);
                gtk::glib::Propagation::Stop
            });
            self.grid_scroll.add_controller(nav_keys);

            let this = self.clone();
            let action_keys = gtk::EventControllerKey::new();
            action_keys.connect_key_pressed(move |_, keyval, _, _| {
                let d = this.state.borrow().selected;
                match keyval {
                    gtk::gdk::Key::Return
                    | gtk::gdk::Key::KP_Enter
                    | gtk::gdk::Key::space => {
                        this.ctx.open_new(d);
                        gtk::glib::Propagation::Stop
                    }
                    _ => gtk::glib::Propagation::Proceed,
                }
            });
            self.grid_scroll.add_controller(action_keys);
        }
    }

    /// Select `date`, navigating the viewed month when it lies outside the
    /// currently displayed month, and rebuild the whole view around it.
    fn select_date(self: &Rc<Self>, date: NaiveDate) {
        {
            let mut s = self.state.borrow_mut();
            if s.view_year != date.year() || s.view_month != date.month() {
                s.view_year = date.year();
                s.view_month = date.month();
            }
            s.selected = date;
        }
        self.refresh_all();
    }

    pub fn refresh(self: &Rc<Self>) {
        self.refresh_all();
    }

    /// Show or hide the decorative background portrait. The widget stays in the
    /// overlay (so layout is untouched) but is only drawn while visible.
    pub fn set_portrait_visible(&self, visible: bool) {
        if let Some(p) = &self.portrait {
            p.set_visible(visible);
        }
    }

    fn refresh_all(self: &Rc<Self>) {
        self.render_month();
        self.render_day();
    }

    fn render_month(self: &Rc<Self>) {
        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }
        let (view_year, view_month, selected) = {
            let s = self.state.borrow();
            (s.view_year, s.view_month, s.selected)
        };
        let month_name = shadowdate::i18n::format_month_year(view_year, view_month);
        self.month_label.set_text(&month_name);

        let weekdays = shadowdate::i18n::weekday_abbrevs();
        for (i, wd) in weekdays.iter().enumerate() {
            let l = Label::new(Some(wd));
            l.add_css_class("weekday-header");
            if i >= 5 {
                l.add_css_class("weekend-header");
            } else {
                l.add_css_class("weekday-workday");
            }
            l.set_xalign(0.5);
            self.grid.attach(&l, i as i32, 0, 1, 1);
        }

        let first = NaiveDate::from_ymd_opt(view_year, view_month, 1)
            .expect("view_year/view_month should always be valid");
        let first_weekday = first.weekday().num_days_from_monday() as i32;
        let t = today();

        // Borrow the store once for the whole frame so each cell's `on_date`
        // lookup returns borrowed references instead of cloning appointments.
        let store = self.ctx.store().borrow();

        // Render the full 6x7 frame so the grid is always a solid rectangle.
        // Cells before the 1st and after the last day come from the previous/next
        // month and are dimmed; clicking one navigates to its month.
        for r in 1..=6 {
            for c in 0..7 {
                let cell_index = ((r - 1) * 7 + c) as usize;
                let offset = cell_offset(first_weekday, cell_index);
                let date = first + chrono::TimeDelta::days(offset as i64);
                let other_month = date.year() != view_year || date.month() != view_month;
                let appts: Vec<&Appointment> = store.on_date(date);
                let is_today = date == t;
                let is_selected = date == selected;
                let cell = build_cell(
                    &date.day().to_string(),
                    other_month,
                    is_today,
                    is_selected,
                    &appts,
                );
                let this = self.clone();
                // Cells are rebuilt on every render, so a fresh click gesture is
                // attached per cell; the old cell (and its controller) is dropped
                // when removed from the grid above, so this does not leak.
                let ev = gtk::GestureClick::new();
                ev.connect_pressed(move |_, _, _, _| {
                    this.select_date(date);
                });
                cell.add_controller(ev);
                self.grid.attach(&cell, c, r, 1, 1);
            }
        }

        // Keep keyboard focus on the grid scroller so arrow navigation continues
        // to work after every rebuild.
        if let Some(parent) = self.grid.parent() {
            parent.grab_focus();
        }
    }

    fn render_day(self: &Rc<Self>) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }
        let s = self.state.borrow();
        self.day_label.set_text(&shadowdate::i18n::format_date(s.selected));
        let store = self.ctx.store().borrow();
        let appts = store.on_date(s.selected);
        for a in &appts {
            let row = build_appt_row(a);
            let uid = a.uid.clone();
            let this = self.clone();
            // Rows are rebuilt on each render; the old row and its controller drop
            // when removed from the list box above, so attaching a fresh gesture
            // per row does not leak.
            let ev = gtk::GestureClick::new();
            ev.connect_pressed(move |_, _, _, _| {
                let appt_opt = this.ctx.store().borrow().get(&uid).cloned();
                if let Some(appt) = appt_opt {
                    this.ctx.open_edit(&appt);
                }
            });
            row.add_controller(ev);
            let lbrow = ListBoxRow::new();
            lbrow.set_child(Some(&row));
            self.list_box.append(&lbrow);
        }
        if appts.is_empty() {
            let empty_box = Box::new(gtk::Orientation::Vertical, 6);
            empty_box.set_halign(gtk::Align::Center);
            empty_box.set_margin_top(16);
            let empty = Label::new(Some(shadowdate::i18n::t("no_appointments")));
            empty.add_css_class("empty-label");
            empty_box.append(&empty);

            let add_btn = Button::with_label(shadowdate::i18n::t("add_appointment"));
            add_btn.add_css_class("empty-cta");
            let this = self.clone();
            let selected = s.selected;
            add_btn.connect_clicked(move |_| {
                this.ctx.open_new(selected);
            });
            empty_box.append(&add_btn);

            let lbrow = ListBoxRow::new();
            lbrow.set_child(Some(&empty_box));
            lbrow.set_selectable(false);
            self.list_box.append(&lbrow);
        }
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

/// Whether the background portrait should be shown, read from the shared app
/// config (a missing config defaults to visible).
fn show_portrait_from_config() -> bool {
    shadowdate::config::ServiceConfig::load(&shadowdate::paths::config_path())
        .appearance
        .show_portrait
}

fn build_cell(
    day_text: &str,
    other_month: bool,
    is_today: bool,
    is_selected: bool,
    appts: &[&Appointment],
) -> Box {
    let cell = Box::new(gtk::Orientation::Vertical, 2);
    cell.add_css_class("day-cell");
    // Fixed 100x60 footprint: the number plus both dot rows always fit, and the
    // uniform size keeps every cell (and the rows they share) perfectly aligned.
    cell.set_size_request(100, 60);
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

    // Two dot rows with fixed heights, both always present so their slots are
    // pinned: the small dots for timed events on top, and a separate row of
    // slightly larger dots for all-day events on the bottom. Each row keeps its
    // exact height (13px / 15px) even when empty, so the all-day row is always
    // the bottom row of the cell — on days with only all-day events the empty
    // timed slot still holds the top row open. Each row is capped (5 / 4 dots)
    // so the cell keeps its fixed footprint no matter how full the day is; both
    // rows use line-height 1 so they never grow. Full details stay in the hover
    // tooltip below.
    let timed: Vec<&Appointment> = appts.iter().copied().filter(|a| !a.all_day).collect();
    let allday: Vec<&Appointment> = appts.iter().copied().filter(|a| a.all_day).collect();

    let dot_row = Box::new(gtk::Orientation::Horizontal, 2);
    dot_row.add_css_class("dot-row");
    dot_row.set_size_request(0, 13);
    dot_row.set_halign(gtk::Align::Center);
    for a in timed.iter().take(5) {
        let dot = Label::new(Some("●"));
        dot.add_css_class("appt-dot");
        dot.add_css_class(&format!("c{}", a.color_index % 6));
        dot_row.append(&dot);
    }
    if timed.len() > 5 {
        let more = Label::new(Some(&shadowdate::i18n::more_compact(timed.len() - 5)));
        more.add_css_class("dot-count");
        dot_row.append(&more);
    }
    cell.append(&dot_row);

    let ad_row = Box::new(gtk::Orientation::Horizontal, 2);
    ad_row.add_css_class("dot-row");
    ad_row.add_css_class("allday-row");
    ad_row.set_size_request(0, 15);
    ad_row.set_halign(gtk::Align::Center);
    for a in allday.iter().take(4) {
        let dot = Label::new(Some("●"));
        dot.add_css_class("allday-dot");
        dot.add_css_class(&format!("c{}", a.color_index % 6));
        ad_row.append(&dot);
    }
    if allday.len() > 4 {
        let more = Label::new(Some("+"));
        more.add_css_class("dot-count");
        ad_row.append(&more);
    }
    cell.append(&ad_row);

    if !appts.is_empty() {
        let detail: Vec<String> = appts
            .iter()
            .map(|a| {
                let mut s = format!("• {}  {}", appt_time_label(a), a.title);
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

/// Localized time meta for an appointment row/cell: the "All day" tag for
/// all-day events, otherwise the start–end range. Translation lives in the
/// view/i18n layer; the model never formats localized strings.
fn appt_time_label(a: &Appointment) -> String {
    if a.all_day {
        shadowdate::i18n::t("all_day_short").to_string()
    } else {
        shadowdate::i18n::time_range(&a.start, &a.end)
    }
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
        let tag = Label::new(Some(shadowdate::i18n::t("all_day_short")));
        tag.add_css_class("all-day-tag");
        tag.set_xalign(0.0);
        row.append(&tag);
    }
    let meta = Label::new(Some(&format!("{}   {}", appt_time_label(a), a.location)));
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

