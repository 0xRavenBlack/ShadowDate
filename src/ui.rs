//! Small shared GTK widget builders used by the app's dialogs (the appointment
//! form and the service settings). Kept here so identical helpers are not
//! redefined in every dialog module.

use gtk::prelude::*;
use gtk::{Box, SpinButton};

/// A zero-padded, wrapping spin button for HH or MM time entry. Constraining
/// input to a valid range prevents the invalid-time errors that free-text
/// entries allowed, and the two-digit display keeps times aligned.
pub fn time_spin(max: f64) -> SpinButton {
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

/// A vertically stacked section container used inside the dialogs.
pub fn section_box() -> Box {
    let b = Box::new(gtk::Orientation::Vertical, 12);
    b.add_css_class("form-section");
    b.set_hexpand(true);
    b
}

/// Modal error dialog that closes itself on any response.
pub fn show_error(parent: &impl IsA<gtk::Window>, msg: &str) {
    let d = gtk::MessageDialog::new(
        Some(parent),
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Error,
        gtk::ButtonsType::Ok,
        msg,
    );
    d.connect_response(|d, _| d.close());
    d.present();
}

/// Modal warning dialog that closes itself on any response.
pub fn show_warning(parent: &impl IsA<gtk::Window>, msg: &str) {
    let d = gtk::MessageDialog::new(
        Some(parent),
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Warning,
        gtk::ButtonsType::Ok,
        msg,
    );
    d.connect_response(|d, _| d.close());
    d.present();
}
