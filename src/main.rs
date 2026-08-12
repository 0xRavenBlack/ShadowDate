mod calendar_view;
mod form_dialog;
mod images;
mod settings;
mod ui;

use shadowdate::config::ServiceConfig;
use shadowdate::ical_export::{export_ics, PRODID};
use shadowdate::ical_import::import_ics_with_warnings;
use shadowdate::i18n;
use shadowdate::model::{Appointment, Store};
use shadowdate::paths;
use shadowdate::service::APP_ID;
use shadowdate::store_io::{backup_corrupt, load_store, merge_store, save_store};
use calendar_view::CalendarView;
use form_dialog::run_appointment_dialog;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Button, FileChooserAction, FileChooserDialog, HeaderBar,
};
use std::cell::RefCell;
use std::fs::File;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use ui::{show_error, show_warning};

fn main() -> gtk::glib::ExitCode {
    // The window's Wayland app_id (and X11 WM_CLASS) is derived from prgname
    // whenever the GtkApplication has no registered application id — which is
    // always the case here (`APP_ID` is deliberately not a valid GApplication
    // id, see below). Pin prgname to the brand identity BEFORE GTK initializes
    // (GTK keeps an already-set prgname) so the Hyprland windowrules
    // (`class:(0xravenblack.shadowdata)`) and the desktop entry's
    // `StartupWMClass` actually match the running window.
    glib::set_prgname(Some(APP_ID));
    let _single_instance = acquire_single_instance_lock();
    let app = Application::builder().build();
    app.connect_startup(|_| {
        load_css();
        gtk::Window::set_default_icon_name(APP_ID);
    });
    // Single instance: `APP_ID` starts with a digit, so it is not a valid
    // GApplication id and the session-bus registration — and with it
    // GApplication's own single-instance machinery — never engages. The
    // `flock`-based lock acquired above is the app's only guard against a
    // second window; the prgname set above determines the window class.
    app.connect_activate(|app| {
        if app.windows().is_empty() {
            build_ui(app);
        } else if let Some(w) = app.windows().first() {
            w.present();
        }
    });
    app.run()
}

/// Acquire a process-lifetime exclusive lock on `paths::lock_path()`; the
/// caller keeps the returned `File` alive until exit so the lock is held for
/// the whole run. Returns `None` (proceeding without a guard) if the lock file
/// cannot even be opened.
///
/// The lock replaces the old `/proc` scan: `flock` is atomic in the kernel, so
/// two near-simultaneous launches can never both pass the check (no TOCTOU
/// window) and a second launch quits with a clear message instead of silently
/// doing nothing. A crashed first instance releases the lock automatically, so
/// there are no stale-lock or zombie edge cases to handle.
fn acquire_single_instance_lock() -> Option<File> {
    let path = paths::lock_path();
    let mut file = File::options()
        .create(true)
        .read(true)
        .write(true)
        // Never truncate on open: the pid is written only after the lock is
        // actually won, so a contender must not clobber the winner's contents.
        .truncate(false)
        .open(&path)
        .ok()?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        // First instance: record the pid for debugging / future IPC.
        let _ = file.set_len(0);
        let _ = file.write_all(format!("{}\n", std::process::id()).as_bytes());
        Some(file)
    } else {
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::WouldBlock {
            eprintln!("shadowdate: another instance is already running; quitting");
            std::process::exit(1);
        }
        // Any other lock failure (e.g. no runtime dir): proceed best-effort
        // rather than brick startup.
        Some(file)
    }
}

fn load_css() {
    let provider = gtk::CssProvider::new();
    let css = include_str!("../resources/style.css");
    provider.load_from_data(css);
    // Headless launches (e.g. running under a CI harness) have no display; fail
    // gracefully instead of panicking in a GTK startup callback.
    let Some(display) = gtk::gdk::Display::default() else {
        eprintln!("shadowdate: no display available; styling skipped");
        return;
    };
    gtk::style_context_add_provider_for_display(
        &display,
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
///
/// The window is held weakly: the window (via its headerbar closures) reaches
/// the view, which reaches the context — a strong window reference here would
/// form a cycle that leaks until process exit. The application owns the window
/// for its whole lifetime, so the upgrade always succeeds while the app runs.
struct AppContext {
    store: Rc<RefCell<Store>>,
    path: PathBuf,
    window: glib::WeakRef<ApplicationWindow>,
    view: RefCell<Weak<CalendarView>>,
}

impl AppContext {
    fn new(store: Rc<RefCell<Store>>, path: PathBuf, window: ApplicationWindow) -> Rc<Self> {
        Rc::new(Self {
            store,
            path,
            window: window.downgrade(),
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
            if let Some(window) = self.window.upgrade() {
                show_error(&window, &e.to_string());
            }
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
        let series_count = self
            .store
            .borrow()
            .items()
            .iter()
            .filter(|a| a.series_uid == series_uid)
            .count();
        let ctx = self.clone();
        let ctx_delete = ctx.clone();
        let series_uid_delete = series_uid.clone();
        let Some(window) = self.window.upgrade() else {
            return;
        };
        run_appointment_dialog(
            &window,
            appt.date(),
            Some(&existing),
            series_count,
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
        let Some(window) = self.window.upgrade() else {
            return;
        };
        run_appointment_dialog(
            &window,
            date,
            None,
            0,
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
                // A partially-corrupt file loads with warnings (the tolerant
                // importer skips bad entries) rather than an error. Back the
                // bytes up so the next save cannot silently drop the entries
                // that failed to parse.
                let msg = match backup_corrupt(&path) {
                    Some(b) => format!(
                        "{}\n\n{}\n\n{}",
                        i18n::t("load_warnings_backed_up"),
                        b.display(),
                        warnings.join("\n")
                    ),
                    None => format!(
                        "{}\n\n{}",
                        i18n::t("load_warnings"),
                        warnings.join("\n")
                    ),
                };
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
fn build_header(window: &ApplicationWindow, cv: &Rc<CalendarView>) -> (HeaderBar, Button, Button) {
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
    // An empty title widget keeps the headerbar from reserving space for the
    // title, leaving the brand + nav + actions to fill the bar.
    let title_widget = gtk::Label::new(None);
    header.set_title_widget(Some(&title_widget));

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
        let cv = cv.clone();
        move |_| {
            let cv = cv.clone();
            settings::run_settings(
                &window,
                Some(std::boxed::Box::new(move |cfg: &ServiceConfig| {
                    cv.set_portrait_visible(cfg.appearance.show_portrait);
                })),
            )
        }
    });

    (header, import_btn, export_btn)
}

/// Wire the Import header action: choose an `.ics` file, merge it into the
/// store, persist, and surface any skipped-entry warnings.
fn setup_import(ctx: &Rc<AppContext>, import_btn: &Button) {
    let ctx = ctx.clone();
    import_btn.connect_clicked(move |_| {
        let Some(window) = ctx.window.upgrade() else {
            return;
        };
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
                                    if let Some(window) = ctx.window.upgrade() {
                                        show_warning(
                                            &window,
                                            &format!(
                                                "{}\n\n{}",
                                                i18n::t("import_warnings"),
                                                warnings.join("\n")
                                            ),
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                if let Some(window) = ctx.window.upgrade() {
                                    show_error(&window, &e.to_string());
                                }
                            }
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
        let Some(window) = ctx.window.upgrade() else {
            return;
        };
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
        let ctx = ctx.clone();
        dlg.run_async(move |dlg, response| {
            if response == gtk::ResponseType::Accept {
                if let Some(file) = dlg.file() {
                    if let Some(p) = file.path() {
                        if let Err(e) = export_ics(&ctx.store.borrow(), &p, PRODID) {
                            if let Some(window) = ctx.window.upgrade() {
                                show_error(&window, &e.to_string());
                            }
                        }
                    }
                }
            }
            dlg.close();
        });
    });
}
