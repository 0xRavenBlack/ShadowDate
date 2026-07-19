mod calendar_view;
mod form_dialog;
mod i18n;
mod images;

use calendar::io_ics;
use calendar::model::Appointment;
use calendar_view::CalendarView;
use form_dialog::run_appointment_dialog;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Button, FileChooserAction, FileChooserDialog, HeaderBar,
};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

const APP_ID: &str = "0xravenblack.shadowdata";

fn data_path() -> PathBuf {
    let mut p = dirs_data().unwrap_or_else(std::env::temp_dir);
    p.push("calendar");
    p.push("calendar.ics");
    p
}

fn dirs_data() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| {
                let mut p = PathBuf::from(h);
                p.push(".local");
                p.push("share");
                p
            })
        })
}

fn main() -> gtk::glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| {
        load_css();
        gtk::Window::set_default_icon_name(APP_ID);
    });
    app.connect_activate(build_ui);
    app.run()
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

fn build_ui(app: &Application) {
    let path = data_path();
    let store = Rc::new(RefCell::new(io_ics::load_store(&path)));
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Shadow Date")
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
    let brand_label = gtk::Label::new(Some("Shadow Date"));
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
                        let _ = io_ics::save_store(&store.borrow(), &path);
                        if let Some(v) = view_ref.borrow().as_ref() {
                            v.refresh();
                        }
                    }
                }),
                Some(std::boxed::Box::new(move || {
                    del_store.borrow_mut().remove_series(&del_series2);
                    let _ = io_ics::save_store(&del_store.borrow(), &del_path);
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
                        let _ = io_ics::save_store(&store.borrow(), &path);
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
    let import_btn = Button::with_label(i18n::t("import"));
    let export_btn = Button::with_label(i18n::t("export"));
    let exit_btn = Button::with_label(i18n::t("exit"));
    exit_btn.add_css_class("exit-button");
    exit_btn.connect_clicked({
        let window = window.clone();
        move |_| window.close()
    });
    actions.append(&new_btn);
    actions.append(&import_btn);
    actions.append(&export_btn);
    actions.append(&exit_btn);
    header.pack_end(&actions);

    window.set_titlebar(Some(&header));
    window.set_child(Some(&cv.widget));

    // Responsive: stack panes vertically when the window is narrow.
    // Poll on the main loop (main-thread only, no Send bound) so we can react to
    // live resizes; the view is held in Rc<RefCell<...>> and is not Send.
    {
        let win = window.clone();
        let vref = view_ref.clone();
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
            let w = win.width();
            if let Some(v) = vref.borrow().as_ref() {
                v.apply_responsive(w);
            }
            gtk::glib::ControlFlow::Continue
        });
    }

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
                            match io_ics::import_ics(&p) {
                                Ok(imported) => {
                                    io_ics::merge_store(&mut store.borrow_mut(), imported);
                                    let _ = io_ics::save_store(&store.borrow(), &path);
                                    if let Some(v) = view_ref.borrow().as_ref() {
                                        v.refresh();
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
