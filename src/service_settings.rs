//! Service settings dialog: configure reminder timing, test notifications, and
//! start/stop the background `shadowdate-service` via its systemd user unit.
//!
//! The config is persisted to `$XDG_CONFIG_HOME/shadowdate/service.toml`; the
//! daemon watches that file and picks up changes without a restart.

use calendar::i18n;
use calendar::paths;
use calendar::service::{self, ServiceConfig, SERVICE_NAME};
use gtk::prelude::*;
use gtk::{Box, Button, Dialog, Label, MessageDialog, MessageType, ResponseType, SpinButton};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

const SYSTEMD_UNIT: &str = "shadowdate-service";

pub fn run_service_settings(parent: &impl IsA<gtk::Window>) {
    let dlg = Dialog::with_buttons(
        Some(i18n::t("service_settings")),
        Some(parent),
        gtk::DialogFlags::MODAL,
        &[],
    );
    dlg.set_default_size(560, 400);
    dlg.set_resizable(false);

    let content = dlg.content_area();
    content.set_spacing(0);
    let form = Box::new(gtk::Orientation::Vertical, 12);
    form.add_css_class("appt-form");
    form.set_margin_top(16);
    form.set_margin_bottom(16);
    form.set_margin_start(20);
    form.set_margin_end(20);
    form.set_hexpand(true);
    form.set_vexpand(true);
    content.append(&form);

    let cfg = Rc::new(RefCell::new(ServiceConfig::load(&paths::config_path())));

    // --- Reminders ---
    let rem_heading = Label::new(Some(i18n::t("reminders")));
    rem_heading.add_css_class("form-section-title");
    rem_heading.set_xalign(0.0);
    form.append(&rem_heading);

    let rem_section = section_box();
    let lead_spin = SpinButton::with_range(0.0, 180.0, 1.0);
    lead_spin.set_value(cfg.borrow().reminders.lead_min as f64);
    lead_spin.set_digits(0);
    lead_spin.set_numeric(true);
    lead_spin.set_width_chars(3);
    let lead_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let lead_lbl = Label::new(Some(i18n::t("lead_time")));
    lead_lbl.add_css_class("form-label");
    lead_lbl.set_valign(gtk::Align::Center);
    lead_row.append(&lead_lbl);
    lead_row.append(&lead_spin);
    let min_lbl = Label::new(Some(i18n::t("minutes")));
    min_lbl.add_css_class("time-caption");
    min_lbl.set_valign(gtk::Align::Center);
    lead_row.append(&min_lbl);
    rem_section.append(&lead_row);

    let ad_hour = time_spin(23.0);
    ad_hour.set_value(cfg.borrow().reminders.all_day_hour as f64);
    let ad_min = time_spin(59.0);
    ad_min.set_value(cfg.borrow().reminders.all_day_minute as f64);
    let ad_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let ad_lbl = Label::new(Some(i18n::t("all_day_time")));
    ad_lbl.add_css_class("form-label");
    ad_lbl.set_valign(gtk::Align::Center);
    ad_row.append(&ad_lbl);
    ad_row.append(&ad_hour);
    let colon = Label::new(Some(":"));
    colon.add_css_class("time-colon");
    colon.set_valign(gtk::Align::Center);
    ad_row.append(&colon);
    ad_row.append(&ad_min);
    rem_section.append(&ad_row);
    form.append(&rem_section);

    // --- Service status + control ---
    let svc_heading = Label::new(Some(i18n::t("service_settings")));
    svc_heading.add_css_class("form-section-title");
    svc_heading.set_xalign(0.0);
    form.append(&svc_heading);

    let svc_section = section_box();
    let status = Label::new(None);
    status.add_css_class("service-status");
    status.set_xalign(0.0);
    svc_section.append(&status);

    let enable_btn = Button::with_label(i18n::t("enable"));
    let disable_btn = Button::with_label(i18n::t("disable"));
    let test_btn = Button::with_label(i18n::t("test_notification"));
    let btn_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    btn_row.append(&enable_btn);
    btn_row.append(&disable_btn);
    btn_row.append(&test_btn);
    svc_section.append(&btn_row);
    form.append(&svc_section);

    refresh_status(&status, &enable_btn, &disable_btn);

    // Enable / disable the systemd user unit.
    {
        let status = status.clone();
        let enable_btn = enable_btn.clone();
        let disable_btn = disable_btn.clone();
        let dlg = dlg.clone();
        let btn = enable_btn.clone();
        btn.connect_clicked(move |_| match systemctl(&["enable", "--now", SYSTEMD_UNIT]) {
            Ok(()) => refresh_status(&status, &enable_btn, &disable_btn),
            Err(msg) => show_error(&dlg, &msg),
        });
    }
    {
        let status = status.clone();
        let enable_btn = enable_btn.clone();
        let disable_btn = disable_btn.clone();
        let dlg = dlg.clone();
        let btn = disable_btn.clone();
        btn.connect_clicked(move |_| match systemctl(&["disable", "--now", SYSTEMD_UNIT]) {
            Ok(()) => refresh_status(&status, &enable_btn, &disable_btn),
            Err(msg) => show_error(&dlg, &msg),
        });
    }
    // Test notification through the real D-Bus notification daemon.
    {
        let dlg = dlg.clone();
        test_btn.connect_clicked(move |_| match service::notification_proxy() {
            Ok(proxy) => {
                if let Err(e) = service::notify(
                    &proxy,
                    "ShadowDate",
                    i18n::t("test_notification_body"),
                    service::APP_ID,
                ) {
                    show_error(&dlg, &e.to_string());
                }
            }
            Err(e) => show_error(&dlg, &e.to_string()),
        });
    }

    // --- Cancel / Save ---
    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    btn_box.set_halign(gtk::Align::End);
    btn_box.set_hexpand(true);
    let cancel_btn = Button::with_label(i18n::t("cancel"));
    let save_btn = Button::with_label(i18n::t("save"));
    save_btn.add_css_class("suggested-action");
    btn_box.append(&cancel_btn);
    btn_box.append(&save_btn);
    form.append(&btn_box);

    {
        let dlg = dlg.clone();
        cancel_btn.connect_clicked(move |_| dlg.response(ResponseType::Cancel));
    }
    {
        let dlg = dlg.clone();
        save_btn.connect_clicked(move |_| dlg.response(ResponseType::Accept));
    }

    dlg.present();

    dlg.connect_response(move |d, resp| {
        if resp == ResponseType::Accept {
            let mut cfg = cfg.borrow_mut();
            cfg.reminders.lead_min = lead_spin.value_as_int().max(0) as u32;
            cfg.reminders.all_day_hour = ad_hour.value_as_int().clamp(0, 23) as u32;
            cfg.reminders.all_day_minute = ad_min.value_as_int().clamp(0, 59) as u32;
            match cfg.save(&paths::config_path()) {
                Ok(()) => d.close(),
                Err(e) => show_error(d, &e.to_string()),
            }
        } else {
            d.close();
        }
    });
}

/// Ask the session bus whether the service well-known name is currently owned.
fn service_running() -> bool {
    let Ok(conn) = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) else {
        return false;
    };
    let reply = conn.call_sync(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "NameHasOwner",
        Some(&(SERVICE_NAME,).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        -1,
        None::<&gio::Cancellable>,
    );
    match reply {
        Ok(v) => v.get::<(bool,)>().map(|(b,)| b).unwrap_or(false),
        Err(_) => false,
    }
}

fn refresh_status(status: &Label, enable_btn: &Button, disable_btn: &Button) {
    let running = service_running();
    status.set_text(if running {
        i18n::t("service_running")
    } else {
        i18n::t("service_stopped")
    });
    enable_btn.set_sensitive(!running);
    disable_btn.set_sensitive(running);
}

/// Run a `systemctl --user` subcommand. On failure returns a localized hint plus
/// systemd's stderr.
fn systemctl(args: &[&str]) -> Result<(), String> {
    let out = Command::new("systemctl").arg("--user").args(args).output();
    match out {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
            Err(if stderr.is_empty() {
                i18n::t("service_not_installed").to_string()
            } else {
                format!("{}\n\n{}", i18n::t("service_not_installed"), stderr)
            })
        }
        Err(e) => Err(format!("systemctl: {}", e)),
    }
}

/// A zero-padded, wrapping HH/MM spin button (mirrors the form dialog's style).
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

fn section_box() -> Box {
    let b = Box::new(gtk::Orientation::Vertical, 12);
    b.add_css_class("form-section");
    b.set_hexpand(true);
    b
}

fn show_error(parent: &impl IsA<gtk::Window>, msg: &str) {
    let d = MessageDialog::new(
        Some(parent),
        gtk::DialogFlags::MODAL,
        MessageType::Error,
        gtk::ButtonsType::Ok,
        msg,
    );
    d.connect_response(|d, _| d.close());
    d.present();
}
