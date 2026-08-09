mod calendar_view;
mod form_dialog;
mod images;
mod service_settings;
mod ui;

use calendar::ical_export::{export_ics, PRODID};
use calendar::ical_import::import_ics_with_warnings;
use calendar::i18n;
use calendar::model::{Appointment, Store};
use calendar::paths;
use calendar::service::APP_ID;
use calendar::store_io::{backup_corrupt, load_store, merge_store, save_store};
use calendar_view::CalendarView;
use form_dialog::run_appointment_dialog;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Button, FileChooserAction, FileChooserDialog, HeaderBar,
};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use ui::{show_error, show_warning};

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

/// Shared application state: the appointment store, its persistence path, the
/// parent window for dialogs, and (weakly, to avoid a reference cycle) the view
/// that must be rebuilt after every mutation.
///
/// Holding the store and path here lets the header actions and the calendar
/// view share one object instead of each closure capturing a growing pile of
/// `Rc` clones. The edit/new callbacks that used to be threaded into the view
/// live here as methods, which also breaks the old
/// `Rc<RefCell<Option<CalendarView>>>` cycle: the view calls back into the
/// context, and the context reaches the view through the weak reference.
struct AppContext {
    store: Rc<RefCell<Store>>,
    path: PathBuf,
    window: ApplicationWindow,
    view: RefCell<Weak<CalendarView>>,
}

impl AppContext {
    fn new(store: Rc<RefCell<Store>>, path: PathBuf, window: ApplicationWindow) -> Rc<Self> {
        Rc::new(Self {
            store,
            path,
            window,
            view: RefCell::new(Weak::new()),
        })
    }

    /// Read access to the store, for rendering and export.
    fn store(&self) -> &Rc<RefCell<Store>> {
        &self.store
    }

    /// Register the view after construction so `commit` can refresh it.
    fn set_view(&self, view: &Rc<CalendarView>) {
        *self.view.borrow_mut() = Rc::downgrade(view);
    }

    /// Persist the store after a mutation and rebuild the view. Surfaces a save
    /// error to the user; the view is only rebuilt once the write attempt
    /// finished so a failed save still shows the last good state on disk.
    fn commit(&self) {
        if let Err(e) = save_store(&self.store.borrow(), &self.path) {
            show_error(&self.window, &e.to_string());
        }
        if let Some(view) = self.view.borrow().upgrade() {
            view.refresh();
        }
    }

    /// Open the edit dialog for an existing appointment. On save the whole
    /// series is replaced with the single submitted (now non-recurring)
    /// appointment; on delete the whole series is removed.
    fn open_edit(self: &Rc<Self>, appt: &Appointment) {
        let existing = appt.clone();
        let series_uid = existing.series_uid.clone();
        let ctx = self.clone();
        let ctx_delete = ctx.clone();
        let series_uid_delete = series_uid.clone();
        run_appointment_dialog(
            &self.window,
            appt.start.date_naive(),
            Some(&existing),
            std::boxed::Box::new(move |result| {
                if let Some(result) = result {
                    // Editing replaces the entire series with the single
                    // (now non-recurring) appointment the user submitted.
                    ctx.store.borrow_mut().remove_series(&series_uid);
                    ctx.store.borrow_mut().insert(result);
                    ctx.commit();
                }
            }),
            Some(std::boxed::Box::new(move || {
                ctx_delete.store.borrow_mut().remove_series(&series_uid_delete);
                ctx_delete.commit();
            })),
        );
    }

    /// Open the new-appointment dialog for a day.
    fn open_new(self: &Rc<Self>, date: chrono::NaiveDate) {
        let ctx = self.clone();
        run_appointment_dialog(
            &self.window,
            date,
            None,
            std::boxed::Box::new(move |result| {
                if let Some(result) = result {
                    ctx.store.borrow_mut().insert(result);
                    ctx.commit();
                }
            }),
            None,
        );
    }
}

fn build_ui(app: &Application) {
    let path = paths::data_path();
    let (store, load_warning) = match load_store(&path) {
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
            let backup = backup_corrupt(&path);
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

    let ctx = AppContext::new(store, path, window.clone());
    let cv = CalendarView::new(ctx.clone());
    ctx.set_view(&cv);
    let (header, import_btn, export_btn) = build_header(&window, &cv);
    setup_import(&ctx, &import_btn);
    setup_export(&ctx, &export_btn);

    window.set_titlebar(Some(&header));
    window.set_child(Some(&cv.widget));
    window.present();

    if let Some(msg) = load_warning {
        show_warning(&window, &msg);
    }
}

/// Build the headerbar: brand on the far left, month navigation next, then the
/// action buttons on the right. The view owns its nav/new buttons, so they are
/// appended here from the view. Returns the Import/Export buttons so their
/// handlers can be wired separately.
fn build_header(window: &ApplicationWindow, cv: &CalendarView) -> (HeaderBar, Button, Button) {
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
    nav_box.append(&cv.prev_btn);
    nav_box.append(&cv.today_btn);
    nav_box.append(&cv.next_btn);
    header.pack_start(&nav_box);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let import_btn = header_button(i18n::t("import"), "document-open-symbolic");
    let export_btn = header_button(i18n::t("export"), "document-save-symbolic");
    let settings_btn = header_button(i18n::t("settings"), "emblem-system-symbolic");
    let exit_btn = header_button(i18n::t("exit"), "application-exit-symbolic");
    exit_btn.add_css_class("exit-button");
    exit_btn.connect_clicked({
        let window = window.clone();
        move |_| window.close()
    });
    actions.append(&cv.new_btn);
    actions.append(&import_btn);
    actions.append(&export_btn);
    actions.append(&settings_btn);
    actions.append(&exit_btn);
    header.pack_end(&actions);

    settings_btn.connect_clicked({
        let window = window.clone();
        move |_| service_settings::run_service_settings(&window)
    });

    (header, import_btn, export_btn)
}

/// Wire the Import header action: choose an `.ics` file, merge it into the
/// store, persist, and surface any skipped-entry warnings.
fn setup_import(ctx: &Rc<AppContext>, import_btn: &Button) {
    let ctx = ctx.clone();
    import_btn.connect_clicked(move |_| {
        let dlg = FileChooserDialog::new(
            Some(i18n::t("import_ics")),
            Some(&ctx.window),
            FileChooserAction::Open,
            &[
                (i18n::t("open"), gtk::ResponseType::Accept),
                (i18n::t("cancel"), gtk::ResponseType::Cancel),
            ],
        );
        let filter = gtk::FileFilter::new();
        filter.add_pattern("*.ics");
        dlg.set_filter(&filter);
        let ctx = ctx.clone();
        dlg.run_async(move |dlg, response| {
            if response == gtk::ResponseType::Accept {
                if let Some(file) = dlg.file() {
                    if let Some(p) = file.path() {
                        match import_ics_with_warnings(&p) {
                            Ok((imported, warnings)) => {
                                merge_store(&mut ctx.store.borrow_mut(), imported);
                                ctx.commit();
                                if !warnings.is_empty() {
                                    show_warning(
                                        &ctx.window,
                                        &format!(
                                            "{}\n\n{}",
                                            i18n::t("import_warnings"),
                                            warnings.join("\n")
                                        ),
                                    );
                                }
                            }
                            Err(e) => show_error(&ctx.window, &e.to_string()),
                        }
                    }
                }
            }
            dlg.close();
        });
    });
}

/// Wire the Export header action: write the store to a chosen `.ics` file.
fn setup_export(ctx: &Rc<AppContext>, export_btn: &Button) {
    let ctx = ctx.clone();
    export_btn.connect_clicked(move |_| {
        let dlg = FileChooserDialog::new(
            Some(i18n::t("export_ics")),
            Some(&ctx.window),
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
        let ctx = ctx.clone();
        dlg.run_async(move |dlg, response| {
            if response == gtk::ResponseType::Accept {
                if let Some(file) = dlg.file() {
                    if let Some(p) = file.path() {
                        if let Err(e) = export_ics(&ctx.store.borrow(), &p, PRODID) {
                            show_error(&ctx.window, &e.to_string());
                        }
                    }
                }
            }
            dlg.close();
        });
    });
}
