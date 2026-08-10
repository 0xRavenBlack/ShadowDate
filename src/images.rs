use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use resvg::tiny_skia;
use resvg::usvg;

const LOGO_SVG: &[u8] = include_bytes!("../resources/svg/logo.svg");
const PORTRAIT_SVG: &[u8] = include_bytes!("../resources/svg/face.svg");

/// Rasterize the embedded SVG down to at most `max_size` px on its longest
/// side before uploading it to a GPU texture. The source is vector art, so the
/// raster resolution tracks the display cap instead of a fixed source size.
fn texture_from(svg: &'static [u8], max_size: i32) -> Option<gdk::Texture> {
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(svg, &opt).ok()?;
    let (w, h) = (tree.size().width(), tree.size().height());
    let scale = (max_size as f32 / w.max(h).max(1.0)).min(1.0);
    let nw = (w * scale).max(1.0) as u32;
    let nh = (h * scale).max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(nw, nh)?;
    let transform = tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let bytes = glib::Bytes::from(&pixmap.data());
    let mem_tex = gdk::MemoryTexture::new(
        nw as i32,
        nh as i32,
        gdk::MemoryFormat::R8g8b8a8Premultiplied,
        &bytes,
        (nw * 4) as usize,
    );
    Some(mem_tex.upcast())
}

pub fn logo_texture() -> Option<gdk::Texture> {
    texture_from(LOGO_SVG, 64)
}

pub fn portrait_texture() -> Option<gdk::Texture> {
    texture_from(PORTRAIT_SVG, 512)
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
    Some(pic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_svgs_parse() {
        for svg in [LOGO_SVG, PORTRAIT_SVG] {
            let tree = usvg::Tree::from_data(svg, &usvg::Options::default()).unwrap();
            assert!(tree.size().width() > 0.0);
            assert!(tree.size().height() > 0.0);
        }
    }
}
