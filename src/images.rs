use gtk::gdk;
use gtk::gdk::gdk_pixbuf::Pixbuf;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

const LOGO_BYTES: &[u8] = include_bytes!("../resources/img/Logo.png");
const PORTRAIT_BYTES: &[u8] = include_bytes!("../resources/img/portrait_face.png");

fn texture_from(bytes: &'static [u8]) -> Option<gdk::Texture> {
    let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from_static(bytes));
    let pixbuf = Pixbuf::from_stream(&stream, gio::Cancellable::NONE).ok()?;
    Some(gdk::Texture::for_pixbuf(&pixbuf))
}

pub fn logo_texture() -> Option<gdk::Texture> {
    texture_from(LOGO_BYTES)
}

pub fn portrait_texture() -> Option<gdk::Texture> {
    texture_from(PORTRAIT_BYTES)
}

/// Small square logo for the headerbar.
pub fn logo_widget(px: i32) -> Option<gtk::Image> {
    let tex = logo_texture()?;
    let img = gtk::Image::from_paintable(Some(&tex));
    img.set_pixel_size(px);
    img.add_css_class("app-logo");
    Some(img)
}

/// Portrait as a decorative accent (used in the day panel).
pub fn portrait_widget() -> Option<gtk::Picture> {
    let tex = portrait_texture()?;
    let pic = gtk::Picture::for_paintable(&tex);
    pic.set_can_shrink(true);
    pic.set_keep_aspect_ratio(true);
    pic.add_css_class("portrait-accent");
    Some(pic)
}
