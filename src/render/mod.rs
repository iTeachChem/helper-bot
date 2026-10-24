//! Small raster toolkit for the stats / leaderboard cards. Coordinates follow
//! Pillow's conventions so the layouts port 1:1 from the old python code.

pub mod lb;
pub mod stats;

use std::sync::LazyLock;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{ImageFormat, Rgba, RgbaImage};

pub type Rgb = (u8, u8, u8);

pub struct Fonts {
    /// seguiemj: used for ranks / usernames / big numbers
    pub sg: FontRef<'static>,
    /// Zalando Sans Expanded: labels and values
    pub zs: FontRef<'static>,
}

pub static FONTS: LazyLock<Fonts> = LazyLock::new(|| Fonts {
    sg: FontRef::try_from_slice(include_bytes!("../../assets/fonts/seguiemj.ttf")).expect("seguiemj.ttf"),
    zs: FontRef::try_from_slice(include_bytes!("../../assets/fonts/ZalandoSansExpanded.ttf"))
        .expect("ZalandoSansExpanded.ttf"),
});

/// A font at a size, measured the way Pillow's `ImageFont.truetype(path, size)` is.
#[derive(Clone, Copy)]
pub struct Face<'a> {
    font: &'a FontRef<'static>,
    scale: PxScale,
}

impl<'a> Face<'a> {
    pub fn new(font: &'a FontRef<'static>, size: f32) -> Self {
        // Pillow's size is the em size in px; ab_glyph's PxScale is ascent - descent
        let upem = font.units_per_em().unwrap_or(1000.0);
        let scale = PxScale::from(size * font.height_unscaled() / upem);
        Self { font, scale }
    }

    /// Pillow's `draw.textlength`.
    pub fn width(&self, text: &str) -> f32 {
        let sf = self.font.as_scaled(self.scale);
        let mut w = 0.0;
        let mut prev = None;
        for c in text.chars() {
            let id = sf.glyph_id(c);
            if let Some(p) = prev {
                w += sf.kern(p, id);
            }
            w += sf.h_advance(id);
            prev = Some(id);
        }
        w
    }

    /// Pillow's `_fit`: truncate with an ellipsis only when the text is longer
    /// than 12 chars AND overflows `max_w`.
    pub fn fit(&self, text: &str, max_w: f32) -> String {
        if text.chars().count() <= 12 || self.width(text) <= max_w {
            return text.to_string();
        }
        let mut chars: Vec<char> = text.chars().collect();
        while chars.len() > 1 {
            let candidate: String = chars.iter().chain(std::iter::once(&'…')).collect();
            if self.width(&candidate) <= max_w {
                break;
            }
            chars.pop();
        }
        chars.into_iter().chain(std::iter::once('…')).collect()
    }
}

/// Draw text with its top-left at the ascender line (Pillow's default "la" anchor).
pub fn draw_text(img: &mut RgbaImage, face: Face, x: f32, y: f32, text: &str, color: Rgb) {
    let sf = face.font.as_scaled(face.scale);
    let baseline = y + sf.ascent();
    let mut caret = x;
    let mut prev = None;
    for c in text.chars() {
        let id = sf.glyph_id(c);
        if let Some(p) = prev {
            caret += sf.kern(p, id);
        }
        let glyph = id.with_scale_and_position(face.scale, ab_glyph::point(caret, baseline));
        caret += sf.h_advance(id);
        prev = Some(id);
        if let Some(outlined) = face.font.outline_glyph(glyph) {
            let b = outlined.px_bounds();
            outlined.draw(|gx, gy, cov| {
                blend(img, b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32, color, cov);
            });
        }
    }
}

/// Source-over blend of `color` at `alpha` (0..1) onto one pixel.
pub fn blend(img: &mut RgbaImage, x: i32, y: i32, color: Rgb, alpha: f32) {
    if x < 0 || y < 0 || x >= img.width() as i32 || y >= img.height() as i32 || alpha <= 0.0 {
        return;
    }
    let a = alpha.min(1.0);
    let p = img.get_pixel_mut(x as u32, y as u32);
    let mix = |s: u8, d: u8| (s as f32 * a + d as f32 * (1.0 - a)).round() as u8;
    *p = Rgba([
        mix(color.0, p[0]),
        mix(color.1, p[1]),
        mix(color.2, p[2]),
        (255.0 * a + p[3] as f32 * (1.0 - a)).round() as u8,
    ]);
}

/// Signed distance from (px, py) to a rounded rect spanning [x0, x1] x [y0, y1].
fn rr_sdf(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> f32 {
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (hw, hh) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
    let r = r.min(hw).min(hh).max(0.0);
    let qx = (px - cx).abs() - (hw - r);
    let qy = (py - cy).abs() - (hh - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

/// Anti-aliased coverage of a rounded rect for the pixel at (x, y).
fn rr_cover(x: u32, y: u32, rect: (f32, f32, f32, f32), r: f32) -> f32 {
    let d = rr_sdf(x as f32 + 0.5, y as f32 + 0.5, rect.0, rect.1, rect.2, rect.3, r);
    (0.5 - d).clamp(0.0, 1.0)
}

fn bounds(img: &RgbaImage, x0: f32, y0: f32, x1: f32, y1: f32) -> (u32, u32, u32, u32) {
    let clamp = |v: f32, max: u32| (v.max(0.0) as u32).min(max);
    (
        clamp(x0.floor(), img.width()),
        clamp(y0.floor(), img.height()),
        clamp(x1.ceil() + 1.0, img.width()),
        clamp(y1.ceil() + 1.0, img.height()),
    )
}

/// Pillow's `draw.rounded_rectangle` (inclusive pixel coords).
pub fn rounded_rect(
    img: &mut RgbaImage,
    (x0, y0, x1, y1): (f32, f32, f32, f32),
    r: f32,
    fill: Option<Rgb>,
    outline: Option<(Rgb, f32)>,
) {
    // Pillow's [x0, x1] covers pixels x0..=x1, i.e. edges at x0 and x1 + 1
    let outer = (x0, y0, x1 + 1.0, y1 + 1.0);
    let (bx0, by0, bx1, by1) = bounds(img, x0, y0, x1, y1);
    for y in by0..by1 {
        for x in bx0..bx1 {
            let c_out = rr_cover(x, y, outer, r);
            if c_out <= 0.0 {
                continue;
            }
            match outline {
                Some((ocol, w)) => {
                    let inner = (outer.0 + w, outer.1 + w, outer.2 - w, outer.3 - w);
                    let c_in = rr_cover(x, y, inner, (r - w).max(0.0));
                    if let Some(f) = fill {
                        blend(img, x as i32, y as i32, f, c_in);
                    }
                    blend(img, x as i32, y as i32, ocol, c_out - c_in);
                }
                None => {
                    if let Some(f) = fill {
                        blend(img, x as i32, y as i32, f, c_out);
                    }
                }
            }
        }
    }
}

/// Pillow's `draw.rectangle` (inclusive pixel coords, no AA needed).
pub fn rect(img: &mut RgbaImage, (x0, y0, x1, y1): (u32, u32, u32, u32), fill: Rgb) {
    for y in y0..=y1.min(img.height() - 1) {
        for x in x0..=x1.min(img.width() - 1) {
            img.put_pixel(x, y, Rgba([fill.0, fill.1, fill.2, 255]));
        }
    }
}

pub fn load_template(bytes: &[u8]) -> RgbaImage {
    image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .expect("bundled template is a valid png")
        .to_rgba8()
}

/// Drop alpha (like Pillow's `convert("RGB")`) and encode as a compressed PNG.
pub fn encode_png(img: RgbaImage) -> anyhow::Result<Vec<u8>> {
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    use image::ImageEncoder;

    let rgb = image::DynamicImage::ImageRgba8(img).into_rgb8();
    let mut out = Vec::new();
    PngEncoder::new_with_quality(&mut out, CompressionType::Best, FilterType::Adaptive).write_image(
        rgb.as_raw(),
        rgb.width(),
        rgb.height(),
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(out)
}
