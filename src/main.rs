mod calendar_view;
mod form_dialog;
mod images;
mod service_settings;

use calendar::i18n;
use calendar::io_ics;
use calendar::model::{Appointment, Store};
use calendar::paths;
use calendar::service::APP_ID;
use calendar_view::CalendarView;
use form_dialog::run_appointment_dialog;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Button, FileChooserAction, FileChooserDialog, HeaderBar,
};
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> gtk::glib::ExitCode {
    bail_if_already_running();
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| {
        load_css();
        gtk::Window::set_default_icon_name(APP_ID);
    });
    // Single instance: a `/proc` pre-check above quits a second process before
    // GTK is even up. Note that `APP_ID` starts with a digit, so it is not a
    // valid GApplication id and the session-bus registration never engages —
    // the pre-check is the app's only guard against a second window.
    app.connect_activate(|app| {
        if app.windows().is_empty() {
            build_ui(app);
        } else if let Some(w) = app.windows().first() {
            w.present();
        }
    });
    app.run()
}

/// Quit before GTK starts if another `shadowdate` process is already running.
///
/// `APP_ID` begins with a digit, so it is not a valid GApplication id and the
/// `gtk::Application` in `main` never registers on the session bus — this
/// `/proc` pre-check is the app's only single-instance guard. The second
/// process exits right away, before any window or GTK work happens.
fn bail_if_already_running() {
    if another_instance_running() {
        eprintln!("shadowdate: another instance is already running; quitting");
        std::process::exit(0);
    }
}

/// True when a `shadowdate` process owned by this user is already alive.
///
/// Scans `/proc` for a process whose `comm` is exactly `shadowdate` (the
/// `shadowdate-service` daemon's comm never matches), limited to the caller's
/// effective UID so a second user's instance — with its own data dir — does
/// not block this launch, and skipping zombies so a dead-but-unreaped process
/// cannot brick startup.
fn another_instance_running() -> bool {
    let my_pid = std::process::id();
    let my_uid = status_of(my_pid).map(|(uid, _)| uid);
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if pid == my_pid {
            continue;
        }
        let Ok(comm) = std::fs::read_to_string(format!("/proc/{pid}/comm")) else {
            continue;
        };
        let Some((uid, state)) = status_of(pid) else {
            continue;
        };
        if comm.trim() == "shadowdate" && my_uid == Some(uid) && state != 'Z' {
            return true;
        }
    }
    false
}

/// Effective UID and state char of `pid` from `/proc/<pid>/status`.
fn status_of(pid: u32) -> Option<(u32, char)> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let mut uid = None;
    let mut state = None;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("Uid:") {
            uid = rest.split_whitespace().nth(1).and_then(|s| s.parse().ok());
        } else if let Some(rest) = line.strip_prefix("State:") {
            state = rest.split_whitespace().next().and_then(|s| s.chars().next());
        }
    }
    match (uid, state) {
        (Some(uid), Some(state)) => Some((uid, state)),
        _ => None,
    }
}

fn load_css() {
    let provider = gtk::CssProvider::new();
    let css = include_str!("../resources/style.css");
    provider.load_from_data(css);
    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("no display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// Headerbar action button with an icon, a label, and a tooltip. The label is
/// always present so the action stays recognizable even when the icon theme
/// lacks the symbolic icon.
fn header_button(label: &str, icon: &str) -> Button {
    let b = Button::new();
    b.set_label(label);
    b.set_icon_name(icon);
    b.set_tooltip_text(Some(label));
    b
}

fn build_ui(app: &Application) {
    let path = paths::data_path();
    let (store, load_warning) = match io_ics::load_store(&path) {
        Ok((store, warnings)) => {
            if warnings.is_empty() {
                (store, None)
            } else {
                let msg = format!("{}\n\n{}", i18n::t("load_warnings"), warnings.join("\n"));
                eprintln!("warning: loading {}: {}", path.display(), msg);
                (store, Some(msg))
            }
        }
        Err(e) => {
            // Unreadable or corrupt file: preserve the bytes before starting
            // empty, otherwise the next save would overwrite the calendar.
            let backup = io_ics::backup_corrupt(&path);
            let msg = match backup {
                Some(b) => format!(
                    "{}\n\n{}\n\n{}",
                    i18n::t("load_failed_backed_up"),
                    b.display(),
                    e
                ),
                None => format!("{}\n\n{}", i18n::t("load_failed"), e),
            };
            eprintln!("warning: {}", msg);
            (Store::new(), Some(msg))
        }
    };
    let store = Rc::new(RefCell::new(store));
    let window = ApplicationWindow::builder()
        .application(app)
        .title("ShadowDate")
        .default_width(1024)
        .default_height(560)
        .build();

    // Floating, fixed-size, non-resizable, non-maximizable on Wayland/Hyprland.
    window.set_decorated(true);
    window.set_resizable(false);
    window.set_default_size(1024, 560);
    window.set_hide_on_close(false);

    let header = HeaderBar::new();
    // Hide the default icon close button; provide a textual "Exit" button instead.
    header.set_show_title_buttons(false);

    // Branding: app logo + title on the far left of the headerbar.
    let brand = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    brand.add_css_class("brand-box");
    if let Some(logo) = images::logo_widget(30) {
        brand.append(&logo);
    }
    let brand_label = gtk::Label::new(Some("ShadowDate"));
    brand_label.add_css_class("brand-title");
    brand.append(&brand_label);
    header.pack_start(&brand);
    header.set_title_widget(Some(&gtk::Label::new(None)));

    let nav_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);

    let view_ref: Rc<RefCell<Option<CalendarView>>> = Rc::new(RefCell::new(None));

    let on_edit: std::boxed::Box<dyn Fn(&Appointment) + 'static> = {
        let window = window.clone();
        let store = store.clone();
        let path = path.clone();
        let view_ref = view_ref.clone();
        std::boxed::Box::new(move |appt: &Appointment| {
            let window = window.clone();
            let result_window = window.clone();
            let del_window = window.clone();
            let store = store.clone();
            let path = path.clone();
            let view_ref = view_ref.clone();
            let existing = appt.clone();
            let del_series = existing.series_uid.clone();
            let del_series2 = del_series.clone();
            let del_store = store.clone();
            let del_path = path.clone();
            let del_view = view_ref.clone();
            run_appointment_dialog(
                &window,
                appt.start.date_naive(),
                Some(&existing),
                std::boxed::Box::new(move |result| {
                    if let Some(result) = result {
                        // Editing replaces the entire series with the single
                        // (now non-recurring) appointment the user submitted.
                        store.borrow_mut().remove_series(&del_series);
                        store.borrow_mut().insert(result);
                        if let Err(e) = io_ics::save_store(&store.borrow(), &path) {
                            show_error(&result_window, &e.to_string());
                        }
                        if let Some(v) = view_ref.borrow().as_ref() {
                            v.refresh();
                        }
                    }
                }),
                Some(std::boxed::Box::new(move || {
                    del_store.borrow_mut().remove_series(&del_series2);
                    if let Err(e) = io_ics::save_store(&del_store.borrow(), &del_path) {
                        show_error(&del_window, &e.to_string());
                    }
                    if let Some(v) = del_view.borrow().as_ref() {
                        v.refresh();
                    }
                })),
            );
        })
    };

    let on_new: std::boxed::Box<dyn Fn(chrono::NaiveDate) + 'static> = {
        let window = window.clone();
        let store = store.clone();
        let path = path.clone();
        let view_ref = view_ref.clone();
        std::boxed::Box::new(move |date: chrono::NaiveDate| {
            let window = window.clone();
            let result_window = window.clone();
            let store = store.clone();
            let path = path.clone();
            let view_ref = view_ref.clone();
            run_appointment_dialog(
                &window,
                date,
                None,
                std::boxed::Box::new(move |result| {
                    if let Some(result) = result {
                        store.borrow_mut().insert(result);
                        if let Err(e) = io_ics::save_store(&store.borrow(), &path) {
                            show_error(&result_window, &e.to_string());
                        }
                        if let Some(v) = view_ref.borrow().as_ref() {
                            v.refresh();
                        }
                    }
                }),
                None,
            );
        })
    };

    let cv = CalendarView::new(store.clone(), on_edit, on_new);
    nav_box.append(&cv.prev_btn);
    nav_box.append(&cv.today_btn);
    nav_box.append(&cv.next_btn);
    header.pack_start(&nav_box);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let new_btn = cv.new_btn.clone();
    let import_btn = header_button(i18n::t("import"), "document-open-symbolic");
    let export_btn = header_button(i18n::t("export"), "document-save-symbolic");
    let settings_btn = header_button(i18n::t("settings"), "emblem-system-symbolic");
    let exit_btn = header_button(i18n::t("exit"), "application-exit-symbolic");
    exit_btn.add_css_class("exit-button");
    exit_btn.connect_clicked({
        let window = window.clone();
        move |_| window.close()
    });
    actions.append(&new_btn);
    actions.append(&import_btn);
    actions.append(&export_btn);
    actions.append(&settings_btn);
    actions.append(&exit_btn);
    header.pack_end(&actions);

    settings_btn.connect_clicked({
        let window = window.clone();
        move |_| service_settings::run_service_settings(&window)
    });

    window.set_titlebar(Some(&header));
    window.set_child(Some(&cv.widget));

    *view_ref.borrow_mut() = Some(cv);

    // Import
    {
        let window = window.clone();
        let store = store.clone();
        let path = path.clone();
        let view_ref = view_ref.clone();
        import_btn.connect_clicked(move |_| {
            let dlg = FileChooserDialog::new(
                Some(i18n::t("import_ics")),
                Some(&window),
                FileChooserAction::Open,
                &[
                    (i18n::t("open"), gtk::ResponseType::Accept),
                    (i18n::t("cancel"), gtk::ResponseType::Cancel),
                ],
            );
            let filter = gtk::FileFilter::new();
            filter.add_pattern("*.ics");
            dlg.set_filter(&filter);
            let w = window.clone();
            let store = store.clone();
            let path = path.clone();
            let view_ref = view_ref.clone();
            dlg.run_async(move |dlg, response| {
                if response == gtk::ResponseType::Accept {
                    if let Some(file) = dlg.file() {
                        if let Some(p) = file.path() {
                            match io_ics::import_ics_with_warnings(&p) {
                                Ok((imported, warnings)) => {
                                    io_ics::merge_store(&mut store.borrow_mut(), imported);
                                    if let Err(e) = io_ics::save_store(&store.borrow(), &path) {
                                        show_error(&w, &e.to_string());
                                    }
                                    if let Some(v) = view_ref.borrow().as_ref() {
                                        v.refresh();
                                    }
                                    if !warnings.is_empty() {
                                        show_warning(
                                            &w,
                                            &format!(
                                                "{}\n\n{}",
                                                i18n::t("import_warnings"),
                                                warnings.join("\n")
                                            ),
                                        );
                                    }
                                }
                                Err(e) => show_error(&w, &e.to_string()),
                            }
                        }
                    }
                }
                dlg.close();
            });
        });
    }

    // Export
    {
        let window = window.clone();
        let store = store.clone();
        export_btn.connect_clicked(move |_| {
            let dlg = FileChooserDialog::new(
                Some(i18n::t("export_ics")),
                Some(&window),
                FileChooserAction::Save,
                &[
                    (i18n::t("save"), gtk::ResponseType::Accept),
                    (i18n::t("cancel"), gtk::ResponseType::Cancel),
                ],
            );
            dlg.set_current_name("shadowdate.ics");
            let filter = gtk::FileFilter::new();
            filter.add_pattern("*.ics");
            dlg.set_filter(&filter);
            let store = store.clone();
            let parent = window.clone();
            dlg.run_async(move |dlg, response| {
                if response == gtk::ResponseType::Accept {
                    if let Some(file) = dlg.file() {
                        if let Some(p) = file.path() {
                            if let Err(e) =
                                io_ics::export_ics(&store.borrow(), &p, "-//ravenblack//calendar//EN")
                            {
                                show_error(&parent, &e.to_string());
                            }
                        }
                    }
                }
                dlg.close();
            });
        });
    }

    window.present();

    if let Some(msg) = load_warning {
        show_warning(&window, &msg);
    }
}

fn show_error(parent: &impl IsA<gtk::Window>, msg: &str) {
    let dlg = gtk::MessageDialog::new(
        Some(parent),
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Error,
        gtk::ButtonsType::Ok,
        msg,
    );
    dlg.connect_response(|d, _| d.close());
    dlg.present();
}

fn show_warning(parent: &impl IsA<gtk::Window>, msg: &str) {
    let dlg = gtk::MessageDialog::new(
        Some(parent),
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Warning,
        gtk::ButtonsType::Ok,
        msg,
    );
    dlg.connect_response(|d, _| d.close());
    dlg.present();
}
