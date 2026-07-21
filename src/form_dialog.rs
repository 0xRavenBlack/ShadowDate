use calendar::model::{make_datetime, Appointment};
use chrono::{Datelike, NaiveDate, Timelike};
use gtk::prelude::*;
use gtk::{
    Box, Button, ButtonsType, Calendar, CheckButton, Dialog, Entry, Label, MessageDialog,
    ResponseType, SpinButton,
};

/// Show a form dialog to create or edit an appointment.
/// `initial_date` is used when creating a new appointment.
/// `existing` is Some when editing.
/// The chosen appointment (or None if cancelled) is delivered via `on_result`.
/// `on_delete`, if provided, is invoked when the user deletes an existing
/// appointment (only shown when `existing` is Some).
pub fn run_appointment_dialog(
    parent: &impl IsA<gtk::Window>,
    initial_date: NaiveDate,
    existing: Option<&Appointment>,
    on_result: std::boxed::Box<dyn Fn(Option<Appointment>) + 'static>,
    on_delete: Option<std::boxed::Box<dyn Fn() + 'static>>,
) {
    let dialog = Dialog::with_buttons(
        Some(if existing.is_some() {
            crate::i18n::t("edit_appointment")
        } else {
            crate::i18n::t("new_appointment")
        }),
        Some(parent),
        gtk::DialogFlags::MODAL,
        &[],
    );
    dialog.set_default_response(ResponseType::Accept);
    // Keep the dialog within the fixed application window (1024x560) and avoid
    // any scrolling: it must fit 100% of the available height.
    dialog.set_default_size(620, 520);
    dialog.set_resizable(false);
    dialog.add_css_class("appt-dialog");
    // Wrap the callback in an Rc so it can be shared with the (optional) delete button.
    let on_result = std::rc::Rc::new(on_result);
    let content = dialog.content_area();
    content.set_spacing(0);
    content.set_margin_top(0);
    content.set_margin_bottom(0);
    content.set_margin_start(0);
    content.set_margin_end(0);

    let form = Box::new(gtk::Orientation::Vertical, 12);
    form.add_css_class("appt-form");
    form.set_margin_top(16);
    form.set_margin_bottom(16);
    form.set_margin_start(20);
    form.set_margin_end(20);
    form.set_hexpand(true);
    form.set_vexpand(true);
    content.append(&form);

    let title_entry = Entry::builder()
        .placeholder_text(crate::i18n::t("add_title"))
        .hexpand(true)
        .build();
    let desc_entry = Entry::builder()
        .placeholder_text(crate::i18n::t("add_description"))
        .hexpand(true)
        .build();
    let loc_entry = Entry::builder()
        .placeholder_text(crate::i18n::t("add_location"))
        .hexpand(true)
        .build();

    let cal = Calendar::builder().hexpand(true).build();
    let start_hour = time_spin(23.0);
    let start_min = time_spin(59.0);
    let end_hour = time_spin(23.0);
    let end_min = time_spin(59.0);
    let all_day = CheckButton::builder().label(crate::i18n::t("all_day")).build();

    if let Some(a) = existing {
        title_entry.set_text(&a.title);
        desc_entry.set_text(&a.description);
        loc_entry.set_text(&a.location);
        select_calendar_day(&cal, a.start.date_naive());
        start_hour.set_value(a.start.hour() as f64);
        start_min.set_value(a.start.minute() as f64);
        end_hour.set_value(a.end.hour() as f64);
        end_min.set_value(a.end.minute() as f64);
        all_day.set_active(a.all_day);
    } else {
        select_calendar_day(&cal, initial_date);
        start_hour.set_value(9.0);
        start_min.set_value(0.0);
        end_hour.set_value(10.0);
        end_min.set_value(0.0);
    }

    // --- Details section ---
    let heading = Label::new(Some(if existing.is_some() {
        crate::i18n::t("edit_appointment")
    } else {
        crate::i18n::t("new_appointment")
    }));
    heading.add_css_class("form-heading");
    heading.set_xalign(0.0);
    form.append(&heading);

    let details = section_box();
    details.append(&row_widget(crate::i18n::t("title"), &title_entry));
    details.append(&row_widget(crate::i18n::t("description"), &desc_entry));
    details.append(&row_widget(crate::i18n::t("location"), &loc_entry));
    form.append(&details);

    // --- Date & time section ---
    let dt_heading = Label::new(Some(crate::i18n::t("date_time")));
    dt_heading.add_css_class("form-section-title");
    dt_heading.set_xalign(0.0);
    form.append(&dt_heading);

    let dt_section = section_box();

    cal.set_hexpand(true);
    dt_section.append(&cal);

    // Time inputs laid out in a grid: column headers (Hours / Minutes) with a
    // row each for Start and End so the two times align and read clearly.
    let time_grid = gtk::Grid::new();
    time_grid.add_css_class("time-box");
    time_grid.set_halign(gtk::Align::Start);
    time_grid.set_row_spacing(8);
    time_grid.set_column_spacing(8);

    let hours_hdr = Label::new(Some(crate::i18n::t("hours")));
    hours_hdr.add_css_class("time-caption");
    hours_hdr.set_xalign(0.5);
    let mins_hdr = Label::new(Some(crate::i18n::t("minutes")));
    mins_hdr.add_css_class("time-caption");
    mins_hdr.set_xalign(0.5);
    time_grid.attach(&hours_hdr, 1, 0, 1, 1);
    time_grid.attach(&mins_hdr, 3, 0, 1, 1);

    let start_lbl = Label::new(Some(crate::i18n::t("start")));
    start_lbl.add_css_class("time-row-label");
    start_lbl.set_xalign(0.0);
    start_lbl.set_valign(gtk::Align::Center);
    let start_colon = Label::new(Some(":"));
    start_colon.add_css_class("time-colon");
    time_grid.attach(&start_lbl, 0, 1, 1, 1);
    time_grid.attach(&start_hour, 1, 1, 1, 1);
    time_grid.attach(&start_colon, 2, 1, 1, 1);
    time_grid.attach(&start_min, 3, 1, 1, 1);

    let end_lbl = Label::new(Some(crate::i18n::t("end")));
    end_lbl.add_css_class("time-row-label");
    end_lbl.set_xalign(0.0);
    end_lbl.set_valign(gtk::Align::Center);
    let end_colon = Label::new(Some(":"));
    end_colon.add_css_class("time-colon");
    time_grid.attach(&end_lbl, 0, 2, 1, 1);
    time_grid.attach(&end_hour, 1, 2, 1, 1);
    time_grid.attach(&end_colon, 2, 2, 1, 1);
    time_grid.attach(&end_min, 3, 2, 1, 1);

    dt_section.append(&time_grid);
    dt_section.append(&all_day);

    // Action row: Delete (left) … Cancel Save (right), all on one line under
    // the all-day checkbox.
    let action_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    action_row.set_hexpand(true);
    action_row.set_margin_top(6);
    let cancel_btn = Button::with_label(crate::i18n::t("cancel"));
    {
        let dialog = dialog.clone();
        cancel_btn.connect_clicked(move |_| {
            dialog.response(ResponseType::Cancel);
        });
    }
    let save_btn = Button::with_label(crate::i18n::t("save"));
    save_btn.add_css_class("suggested-action");
    {
        let dialog = dialog.clone();
        save_btn.connect_clicked(move |_| {
            dialog.response(ResponseType::Accept);
        });
    }
    // If editing, add a delete button on the left of the action row.
    if let Some(on_delete) = on_delete {
        let del_btn = Button::with_label(crate::i18n::t("delete"));
        del_btn.add_css_class("delete-button");
        let on_result = on_result.clone();
        let on_delete = std::rc::Rc::new(on_delete);
        del_btn.connect_clicked(move |b: &Button| {
            let dialog = b.root().and_downcast::<Dialog>().unwrap();
            let confirm = MessageDialog::new(
                Some(&dialog),
                gtk::DialogFlags::MODAL,
                gtk::MessageType::Warning,
                ButtonsType::None,
                crate::i18n::t("confirm_delete_title"),
            );
            confirm.set_secondary_text(Some(crate::i18n::t("confirm_delete_body")));
            confirm.add_button(crate::i18n::t("cancel"), ResponseType::Cancel);
            let del_resp = confirm.add_button(crate::i18n::t("delete"), ResponseType::Accept);
            del_resp.add_css_class("delete-button");
            confirm.set_default_response(ResponseType::Cancel);
            let dialog = dialog.clone();
            let on_delete = on_delete.clone();
            let on_result = on_result.clone();
            confirm.connect_response(move |c, resp| {
                c.close();
                if resp == ResponseType::Accept {
                    dialog.close();
                    on_delete();
                    on_result(None);
                }
            });
            confirm.present();
        });
        action_row.append(&del_btn);
    }

    // Cancel + Save right-aligned inside the same row.
    let btn_group = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    btn_group.set_halign(gtk::Align::End);
    btn_group.set_hexpand(true);
    btn_group.append(&cancel_btn);
    btn_group.append(&save_btn);
    action_row.append(&btn_group);
    dt_section.append(&action_row);
    form.append(&dt_section);

    // Grey out the time inputs while "All day" is active.
    {
        let widgets = [
            start_hour.clone(),
            start_min.clone(),
            end_hour.clone(),
            end_min.clone(),
        ];
        let apply = move |active: bool| {
            for w in &widgets {
                w.set_sensitive(!active);
            }
        };
        apply(all_day.is_active());
        all_day.connect_toggled(move |cb| apply(cb.is_active()));
    }

    dialog.present();

    let existing_owned: Option<Appointment> = existing.cloned();
    let res = dialog.clone();
    dialog.connect_response(move |d, response| {
        if response == ResponseType::Cancel {
            d.close();
            on_result(None);
            return;
        }
        if response != ResponseType::Accept {
            return;
        }
        let fields = FormFields {
            title: &title_entry,
            desc: &desc_entry,
            loc: &loc_entry,
            cal: &cal,
            sh: &start_hour,
            sm: &start_min,
            eh: &end_hour,
            em: &end_min,
            all_day: &all_day,
        };
        match build_appointment(existing_owned.as_ref(), &fields) {
            Ok(a) => {
                d.close();
                on_result(Some(a));
            }
            Err(msg) => {
                // Keep the form open so the user can correct the input.
                let err = MessageDialog::new(
                    Some(d),
                    gtk::DialogFlags::MODAL,
                    gtk::MessageType::Error,
                    ButtonsType::Ok,
                    &msg,
                );
                err.connect_response(|e, _| e.close());
                err.present();
            }
        }
        let _ = res;
    });
}

struct FormFields<'a> {
    title: &'a Entry,
    desc: &'a Entry,
    loc: &'a Entry,
    cal: &'a Calendar,
    sh: &'a SpinButton,
    sm: &'a SpinButton,
    eh: &'a SpinButton,
    em: &'a SpinButton,
    all_day: &'a CheckButton,
}

fn build_appointment(
    existing: Option<&Appointment>,
    f: &FormFields,
) -> Result<Appointment, String> {
    let title_text = f.title.text().to_string();
    if title_text.trim().is_empty() {
        return Err(crate::i18n::t("title_required").to_string());
    }
    let dt = f.cal.date();
    let y = dt.year();
    let m = dt.month() as u32;
    let d = dt.day_of_month() as u32;
    let date = NaiveDate::from_ymd_opt(y, m, d)
        .ok_or_else(|| crate::i18n::t("invalid_date").to_string())?;

    let all = f.all_day.is_active();
    let (sh_v, sm_v, eh_v, em_v) = if all {
        (0, 0, 23, 59)
    } else {
        (
            f.sh.value_as_int() as u32,
            f.sm.value_as_int() as u32,
            f.eh.value_as_int() as u32,
            f.em.value_as_int() as u32,
        )
    };

    if sh_v > 23 || sm_v > 59 || eh_v > 23 || em_v > 59 {
        return Err(crate::i18n::t("time_out_of_range").to_string());
    }

    let start = make_datetime(date, sh_v, sm_v);
    let mut end = make_datetime(date, eh_v, em_v);
    if all {
        // iCalendar all-day DTEND is exclusive: store start of the day after.
        end = start + chrono::Duration::days(1);
    } else if end <= start {
        end = start + chrono::Duration::hours(1);
    }

    let appt = if let Some(ex) = existing {
        Appointment::with_uid(
            ex.uid.clone(),
            title_text,
            f.desc.text().to_string(),
            f.loc.text().to_string(),
            start,
            end,
            all,
        )
    } else {
        Appointment::with_uid(
            uuid::Uuid::new_v4().to_string(),
            title_text,
            f.desc.text().to_string(),
            f.loc.text().to_string(),
            start,
            end,
            all,
        )
    };
    Ok(appt)
}

/// A zero-padded, wrapping spin button for HH or MM time entry. Constraining
/// input to a valid range prevents the invalid-time errors that free-text
/// entries allowed, and the two-digit display keeps times aligned.
fn time_spin(max: f64) -> SpinButton {
    let sb = SpinButton::with_range(0.0, max, 1.0);
    sb.set_digits(0);
    sb.set_numeric(true);
    sb.set_wrap(true);
    sb.set_width_chars(2);
    sb.connect_output(|sb| {
        sb.set_text(&format!("{:02}", sb.value_as_int()));
        gtk::glib::Propagation::Stop
    });
    sb
}

fn section_box() -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 12);
    b.add_css_class("form-section");
    b.set_hexpand(true);
    b
}

fn row_widget(label: &str, w: &impl IsA<gtk::Widget>) -> gtk::Box {
    let h = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    h.set_hexpand(true);
    let l = Label::new(Some(label));
    l.add_css_class("form-label");
    l.set_width_chars(12);
    l.set_xalign(0.0);
    l.set_valign(gtk::Align::Center);
    h.append(&l);
    h.append(w);
    h
}

/// Select a date on the GTK `Calendar` in a single call. Using `select_day`
/// (rather than three separate year/month/day setters) is robust around month
/// boundaries (e.g. Jan 31 -> month change) where sequential setters can clamp.
fn select_calendar_day(cal: &gtk::Calendar, date: NaiveDate) {
    if let Ok(dt) = gtk::glib::DateTime::from_local(
        date.year(),
        date.month() as i32,
        date.day() as i32,
        0,
        0,
        0.0,
    ) {
        cal.select_day(&dt);
    }
}
