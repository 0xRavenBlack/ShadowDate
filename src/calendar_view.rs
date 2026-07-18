use calendar::model::{today, Appointment, Store};
use chrono::{Datelike, NaiveDate};
use gtk::prelude::*;
use gtk::{Box, Button, CenterBox, Grid, Label, ListBox, ListBoxRow, ScrolledWindow};
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
    list_box: ListBox,
    month_label: Label,
    day_label: Label,
    content: Box,
    right_col: Box,
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
        grid.add_css_class("calendar-grid");
        grid.set_halign(gtk::Align::Fill);
        grid.set_valign(gtk::Align::Start);
        let grid_scroll = ScrolledWindow::builder()
            .child(&grid)
            .hexpand(true)
            .vexpand(true)
            .propagate_natural_width(true)
            .propagate_natural_height(true)
            .build();
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
        let next_btn = Button::with_label("›");
        let today_btn = Button::with_label(crate::i18n::t("today"));
        let new_btn = Button::with_label(crate::i18n::t("new"));
        new_btn.add_css_class("new-button");

        let view = Self {
            widget,
            grid,
            list_box,
            month_label,
            day_label,
            content,
            right_col: right,
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
                refresh_all(&g, &ml, &lb, &dl, &st, &sto, &oe);
            });
        }
        {
            let st = self.state.clone();
            let g = self.grid.clone();
            let ml = self.month_label.clone();
            let lb = self.list_box.clone();
            let dl = self.day_label.clone();
            let oe = self.on_edit.clone();
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
                refresh_all(&g, &ml, &lb, &dl, &st, &sto, &oe);
            });
        }
        {
            let st = self.state.clone();
            let g = self.grid.clone();
            let ml = self.month_label.clone();
            let lb = self.list_box.clone();
            let dl = self.day_label.clone();
            let oe = self.on_edit.clone();
            let sto = self.store.clone();
            self.today_btn.connect_clicked(move |_| {
                let t = today();
                {
                    let mut s = st.borrow_mut();
                    s.view_year = t.year();
                    s.view_month = t.month();
                    s.selected = t;
                }
                refresh_all(&g, &ml, &lb, &dl, &st, &sto, &oe);
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
    }

    /// Switch the two-pane layout to vertical when the window is narrow.
    pub fn apply_responsive(&self, width: i32) {
        if width < 680 {
            self.content.set_orientation(gtk::Orientation::Vertical);
            self.right_col.set_size_request(-1, 200);
            self.right_col.set_hexpand(true);
        } else {
            self.content.set_orientation(gtk::Orientation::Horizontal);
            self.right_col.set_size_request(260, -1);
            self.right_col.set_hexpand(false);
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
        );
    }
}

fn refresh_all(
    grid: &Grid,
    month_label: &Label,
    list_box: &ListBox,
    day_label: &Label,
    state: &Rc<RefCell<ViewState>>,
    store: &Rc<RefCell<Store>>,
    on_edit: &Rc<dyn Fn(&Appointment) + 'static>,
) {
    render_month(grid, month_label, list_box, day_label, state, store, on_edit);
    render_day(list_box, day_label, state, store, on_edit);
}

fn render_month(
    grid: &Grid,
    month_label: &Label,
    list_box: &ListBox,
    day_label: &Label,
    state: &Rc<RefCell<ViewState>>,
    store: &Rc<RefCell<Store>>,
    on_edit: &Rc<dyn Fn(&Appointment) + 'static>,
) {
    while let Some(child) = grid.first_child() {
        grid.remove(&child);
    }
    let (view_year, view_month, selected) = {
        let s = state.borrow();
        (s.view_year, s.view_month, s.selected)
    };
    let month_name = crate::i18n::format_month_year(view_year, (view_month - 1) as usize);
    month_label.set_text(&month_name);

    let weekdays = crate::i18n::weekday_abbrevs();
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

    let first = NaiveDate::from_ymd_opt(view_year, view_month, 1).unwrap();
    let first_weekday = (first.weekday().num_days_from_monday() as i32) + 1;
    let days_in_month = days_in(view_year, view_month);
    let t = today();

    let mut day = 1;
    let rows = 6;
    for r in 1..=rows {
        for c in 0..7 {
            if (r == 1 && c + 1 < first_weekday) || day > days_in_month {
                let pad_date = if r == 1 && c + 1 < first_weekday {
                    first - chrono::Duration::days((first_weekday - (c + 1)) as i64)
                } else {
                    first + chrono::Duration::days((day - days_in_month) as i64)
                };
                let cell = build_cell(&pad_date.to_string(), true, false, false, &[]);
                grid.attach(&cell, c, r, 1, 1);
                continue;
            }
            let date = NaiveDate::from_ymd_opt(view_year, view_month, day).unwrap();
            let appts: Vec<Appointment> =
                store.borrow().on_date(date).into_iter().cloned().collect();
            let is_today = date == t;
            let is_selected = date == selected;
            let chip_texts: Vec<String> = appts.iter().map(|a| a.title.clone()).collect();
            let cell = build_cell(
                &day.to_string(),
                false,
                is_today,
                is_selected,
                &chip_texts,
            );
            let st = state.clone();
            let g = grid.clone();
            let ml = month_label.clone();
            let lb = list_box.clone();
            let dl = day_label.clone();
            let oe = on_edit.clone();
            let sto = store.clone();
            // Cells are rebuilt on every render, so a fresh click gesture is
            // attached per cell; the old cell (and its controller) is dropped
            // when removed from the grid above, so this does not leak.
            let ev = gtk::GestureClick::new();
            ev.connect_pressed(move |_, _, _, _| {
                st.borrow_mut().selected = date;
                refresh_all(&g, &ml, &lb, &dl, &st, &sto, &oe);
            });
            cell.add_controller(ev);
            grid.attach(&cell, c, r, 1, 1);
            day += 1;
        }
    }
}

fn render_day(
    list_box: &ListBox,
    day_label: &Label,
    state: &Rc<RefCell<ViewState>>,
    store: &Rc<RefCell<Store>>,
    on_edit: &Rc<dyn Fn(&Appointment) + 'static>,
) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }
    let s = state.borrow();
    day_label.set_text(&crate::i18n::format_date(s.selected));
    let appts: Vec<Appointment> = store.borrow().on_date(s.selected).into_iter().cloned().collect();
    for a in &appts {
        let row = build_appt_row(a);
        let uid = a.uid.clone();
        let st = state.clone();
        let lb = list_box.clone();
        let dl = day_label.clone();
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
            render_day(&lb, &dl, &st, &sto, &on_edit);
        });
        row.add_controller(ev);
        let lbrow = ListBoxRow::new();
        lbrow.set_child(Some(&row));
        list_box.append(&lbrow);
    }
    if appts.is_empty() {
        let empty = Label::new(Some(crate::i18n::t("no_appointments")));
        empty.add_css_class("empty-label");
        let lbrow = ListBoxRow::new();
        lbrow.set_child(Some(&empty));
        lbrow.set_sensitive(false);
        list_box.append(&lbrow);
    }
}

fn days_in(year: i32, month: u32) -> u32 {
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    };
    let cur = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    (next - cur).num_days() as u32
}

fn build_cell(
    day_text: &str,
    other_month: bool,
    is_today: bool,
    is_selected: bool,
    chips: &[String],
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
    // Center the day number vertically within the cell using a CenterBox.
    let num_center = CenterBox::new();
    num_center.set_orientation(gtk::Orientation::Vertical);
    num_center.set_hexpand(true);
    num_center.set_vexpand(true);
    let num = Label::new(Some(day_text));
    num.add_css_class("day-number");
    num.set_xalign(0.5);
    num.set_halign(gtk::Align::Center);
    num.set_valign(gtk::Align::Center);
    if is_today {
        num.add_css_class("today-label");
    }
    num_center.set_center_widget(Some(&num));
    cell.append(&num_center);
    for (i, chip) in chips.iter().take(3).enumerate() {
        let c = Label::new(Some(chip));
        c.add_css_class("appt-chip");
        c.add_css_class(&format!("c{}", i % 6));
        c.set_xalign(0.0);
        c.set_ellipsize(gtk::pango::EllipsizeMode::End);
        cell.append(&c);
    }
    if chips.len() > 3 {
        let more = Label::new(Some(&crate::i18n::more_label(chips.len() - 3)));
        more.add_css_class("empty-label");
        cell.append(&more);
    }
    cell
}

fn build_appt_row(a: &Appointment) -> Box {
    let row = Box::new(gtk::Orientation::Vertical, 2);
    row.add_css_class("appt-row");
    row.add_css_class(&format!("c{}", a.color_index));
    let title = Label::new(Some(&a.title));
    title.add_css_class("appt-title");
    title.set_xalign(0.0);
    row.append(&title);
    let meta = Label::new(Some(&format!("{}   {}", a.time_label(), a.location)));
    meta.add_css_class("appt-meta");
    meta.set_xalign(0.0);
    row.append(&meta);
    if !a.description.is_empty() {
        let d = Label::new(Some(&a.description));
        d.add_css_class("appt-meta");
        d.set_xalign(0.0);
        d.set_ellipsize(gtk::pango::EllipsizeMode::End);
        row.append(&d);
    }
    row
}
