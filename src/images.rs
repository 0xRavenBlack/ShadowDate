use gtk::gdk;
use gtk::gdk::gdk_pixbuf::{InterpType, Pixbuf};
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

const LOGO_BYTES: &[u8] = include_bytes!("../resources/img/Logo.png");
const PORTRAIT_BYTES: &[u8] = include_bytes!("../resources/img/portrait_face.png");

/// Decode the embedded PNG and scale it down to at most `max_size` px on its
/// longest side before uploading it to a GPU texture. The source art is kept
/// small on disk, but this guard ensures we never hold a multi-megabyte
/// full-resolution texture for an icon shown at a few dozen pixels.
fn texture_from(bytes: &'static [u8], max_size: i32) -> Option<gdk::Texture> {
    let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from_static(bytes));
    let pixbuf = Pixbuf::from_stream(&stream, gio::Cancellable::NONE).ok()?;
    let (w, h) = (pixbuf.width(), pixbuf.height());
    let scale = (max_size as f64 / w.max(h).max(1) as f64).min(1.0);
    let tex_pixbuf = if scale < 1.0 {
        let nw = (w as f64 * scale).max(1.0) as i32;
        let nh = (h as f64 * scale).max(1.0) as i32;
        pixbuf.scale_simple(nw, nh, InterpType::Bilinear)?
    } else {
        pixbuf
    };
    Some(gdk::Texture::for_pixbuf(&tex_pixbuf))
}

pub fn logo_texture() -> Option<gdk::Texture> {
    texture_from(LOGO_BYTES, 64)
}

pub fn portrait_texture() -> Option<gdk::Texture> {
    texture_from(PORTRAIT_BYTES, 512)
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
