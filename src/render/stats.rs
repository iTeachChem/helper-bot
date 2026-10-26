use image::{imageops, RgbaImage};

use super::{blend, draw_text, encode_png, load_template, Face, Rgb, FONTS};

static TEMPLATE: &[u8] = include_bytes!("../../assets/templates/stats_template.png");

// palette
const MIST: Rgb = (209, 224, 222);
const KEYLIME: Rgb = (238, 244, 206);
const SLATE: Rgb = (77, 92, 96);
const PANEL_BG: Rgb = (32, 32, 32);
const ACC_AMBER: Rgb = (180, 150, 80);

// card dimensions
const H: i64 = 520;

// left panel geometry
const PX: i64 = 46;
const PY: i64 = 50;
const PW: i64 = 210;
const AVS: u32 = 96;
const AX: i64 = PX + (PW - AVS as i64) / 2; // 103
const AY: i64 = PY + 22; // 72

// right content geometry
const RX: i64 = PX + PW + 22; // 278
const RY: i64 = PY; // 50
const RW: i64 = 900 - 26 - RX - 18; // 578
const GAP: i64 = 10;

const BY: i64 = RY + 20; // 70

const D2Y: i64 = BY + 74; // 144
const PW3: i64 = (RW - GAP * 2) / 3; // 186
const PY3: i64 = D2Y + 26; // 170
const PH3: i64 = 68;

const D3Y: i64 = PY3 + PH3 * 2 + GAP * 2 + 8; // 334
const DP_Y: i64 = D3Y + 26; // 360

pub struct StatsCard {
    pub username: String,
    pub server_name: String,
    pub quiz_rank: String,
    pub doubts_rank: String,
    pub attempted: i64,
    pub solved: i64,
    pub skipped: i64,
    pub points: Option<i64>,
    pub total_time_str: Option<String>,
    pub doubts_solved: i64,
    pub accuracy: f64,
    pub avg_time: f64,
}

fn circle_crop(src: &RgbaImage, size: u32) -> RgbaImage {
    let mut img = imageops::resize(src, size, size, imageops::FilterType::Lanczos3);
    let r = size as f32 / 2.0;
    for (x, y, p) in img.enumerate_pixels_mut() {
        let d = ((x as f32 + 0.5 - r).powi(2) + (y as f32 + 0.5 - r).powi(2)).sqrt();
        let cov = (r - d + 0.5).clamp(0.0, 1.0);
        p[3] = (p[3] as f32 * cov).round() as u8;
    }
    img
}

/// Paste `src` at (x, y) using its alpha as the mask.
fn paste(dst: &mut RgbaImage, src: &RgbaImage, x: i64, y: i64) {
    for (sx, sy, p) in src.enumerate_pixels() {
        blend(dst, (x + sx as i64) as i32, (y + sy as i64) as i32, (p[0], p[1], p[2]), p[3] as f32 / 255.0);
    }
}

/// Filled gradient progress bar drawn on top of the template's track.
fn h_grad_bar(img: &mut RgbaImage, x: i64, y: i64, w: i64, h: i64, c1: Rgb, c2: Rgb, pct: f64) {
    if pct <= 0.0 {
        return;
    }
    let fw = h.max((w as f64 * pct.min(1.0)) as i64);
    let r = (h / 2) as f32;
    let rect = (0.0, 0.0, fw as f32, h as f32);
    for i in 0..fw {
        let t = i as f32 / (fw - 1).max(1) as f32;
        let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
        let col = (lerp(c1.0, c2.0), lerp(c1.1, c2.1), lerp(c1.2, c2.2));
        for j in 0..h {
            let cov = super::rr_cover(i as u32, j as u32, rect, r);
            blend(img, (x + i) as i32, (y + j) as i32, col, cov);
        }
    }
}

pub fn render(avatar: Option<&[u8]>, data: &StatsCard) -> anyhow::Result<Vec<u8>> {
    let mut img = load_template(TEMPLATE);

    let f_name = Face::new(&FONTS.sg, 28.0);
    let f_server = Face::new(&FONTS.zs, 14.0);
    let f_rank = Face::new(&FONTS.sg, 34.0);
    let f_label = Face::new(&FONTS.zs, 11.0);
    let f_big = Face::new(&FONTS.sg, 24.0);
    let f_date = Face::new(&FONTS.zs, 11.0);

    let username = if data.username.is_empty() { "Unknown" } else { &data.username };
    let server_name = if data.server_name.is_empty() { "Discord" } else { &data.server_name };

    // avatar (falls back to a flat placeholder if missing or undecodable)
    let av_raw = avatar
        .and_then(|b| image::load_from_memory(b).ok())
        .map(|i| i.to_rgba8())
        .unwrap_or_else(|| RgbaImage::from_pixel(AVS, AVS, image::Rgba([50, 60, 65, 255])));
    let av = circle_crop(&av_raw, AVS);
    drop(av_raw);
    paste(&mut img, &av, AX, AY);

    let (pxf, pwf) = (PX as f32, PW as f32);
    let (ay, avs) = (AY as f32, AVS as f32);

    // username
    let uname = f_name.fit(username, pwf - 28.0);
    let uw = f_name.width(&uname);
    draw_text(&mut img, f_name, pxf + (pwf - uw) / 2.0, ay + avs + 10.0, &uname, MIST);

    // server name
    let srv = f_server.fit(server_name, pwf - 28.0);
    let sw = f_server.width(&srv);
    draw_text(&mut img, f_server, pxf + (pwf - sw) / 2.0, ay + avs + 44.0, &srv, (170, 195, 195));

    // ranks
    let ry2 = ay + avs + 80.0 + 14.0;
    let qr = f_rank.fit(&data.quiz_rank, pwf - 28.0);
    draw_text(&mut img, f_rank, pxf + 14.0, ry2 + 16.0, &qr, KEYLIME);
    let dr = f_rank.fit(&data.doubts_rank, pwf - 28.0);
    draw_text(&mut img, f_rank, pxf + 14.0, ry2 + 90.0, &dr, MIST);

    // date
    let date_str = chrono::Utc::now().format("%d %b %Y · %H:%M UTC").to_string();
    let ds = f_date.fit(&date_str, pwf - 20.0);
    let dw = f_date.width(&ds);
    let ph = (H - 100) as f32;
    draw_text(&mut img, f_date, pxf + (pwf - dw) / 2.0, PY as f32 + ph - 20.0, &ds, (120, 155, 155));

    // performance bars
    let acc_pct = data.accuracy / 100.0;
    let skip_pct = if data.attempted != 0 { data.skipped as f64 / data.attempted as f64 } else { 0.0 };
    let acc_text = format!("{:.1}%", data.accuracy);
    let skip_text = format!("{:.1}%", skip_pct * 100.0);

    let (rxf, rwf, byf) = (RX as f32, RW as f32, BY as f32);
    draw_text(&mut img, f_label, rxf + rwf - 52.0, byf, &acc_text, KEYLIME);
    draw_text(&mut img, f_label, rxf + rwf - 52.0, byf + 36.0, &skip_text, (200, 185, 120));

    h_grad_bar(&mut img, RX, BY + 16, RW, 9, SLATE, KEYLIME, acc_pct);
    h_grad_bar(&mut img, RX, BY + 52, RW, 9, PANEL_BG, ACC_AMBER, skip_pct);

    // quiz stats pills
    let pills_r1 = [data.attempted.to_string(), data.solved.to_string(), data.skipped.to_string()];
    let pills_r2 = [
        data.points.map_or("N/A".to_string(), |p| p.to_string()),
        data.total_time_str.clone().unwrap_or_else(|| "N/A".into()),
        format!("{:.1}s", data.avg_time),
    ];
    for (i, val) in pills_r1.iter().enumerate() {
        let x = RX + i as i64 * (PW3 + GAP) + 14;
        draw_text(&mut img, f_big, x as f32, (PY3 + 30) as f32, val, MIST);
    }
    for (i, val) in pills_r2.iter().enumerate() {
        let x = RX + i as i64 * (PW3 + GAP) + 14;
        draw_text(&mut img, f_big, x as f32, (PY3 + PH3 + GAP + 30) as f32, val, MIST);
    }

    // doubts pill (full-width, doubts_solved only)
    draw_text(&mut img, f_big, rxf + 14.0, (DP_Y + 30) as f32, &data.doubts_solved.to_string(), MIST);

    encode_png(img)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::lb::{self, LbRow};

    fn sample() -> StatsCard {
        StatsCard {
            username: "Picchiii".into(),
            server_name: "iTeachChem".into(),
            quiz_rank: "#3".into(),
            doubts_rank: "#12".into(),
            attempted: 120,
            solved: 87,
            skipped: 9,
            points: Some(431),
            total_time_str: Some("2h 14m".into()),
            doubts_solved: 41,
            accuracy: 72.5,
            avg_time: 67.0,
        }
    }

    #[test]
    fn renders_cards() {
        let png = render(None, &sample()).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        let rows: Vec<LbRow> = (0..7)
            .map(|i| LbRow { username: format!("a_rather_long_username_{i}"), value: 100 - i * 7 })
            .collect();
        let lbpng = lb::render("doubts", &rows).unwrap();
        assert_eq!(&lbpng[1..4], b"PNG");

        // RENDER_OUT=dir cargo test  -> writes the cards out for eyeballing
        if let Ok(dir) = std::env::var("RENDER_OUT") {
            std::fs::write(format!("{dir}/stats.png"), &png).unwrap();
            std::fs::write(format!("{dir}/lb_doubts.png"), &lbpng).unwrap();
        }
    }
}
