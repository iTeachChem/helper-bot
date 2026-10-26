use image::RgbaImage;

use super::{draw_text, draw_text_bold, encode_png_rgba, rounded_rect, Face, Rgb, FONTS};

// palette — Discord-grey card; gold/silver/bronze mark the top 3
const BG: Rgb = (43, 45, 49);
const BORDER: Rgb = (78, 80, 88);
const DIVIDER: Rgb = (63, 65, 71);
const TEXT: Rgb = (242, 243, 245);
const TEXT_DIM: Rgb = (181, 186, 193);
const GOLD: Rgb = (240, 178, 50);

// (ring/number, row tint) for ranks 1–3; tints are the medal colour at ~10% over BG
const MEDALS: [(Rgb, Rgb); 3] = [
    ((240, 178, 50), (67, 61, 49)),
    ((212, 216, 222), (58, 60, 65)),
    ((232, 146, 90), (64, 56, 54)),
];

// card geometry
const CARD_W: f32 = 800.0;
const PAD: f32 = 48.0;
const RADIUS: f32 = 28.0;
const HEAD_Y: f32 = 160.0;
const ROWS_Y: f32 = 206.0;
const ROW_H: f32 = 72.0;
const ROW_GAP: f32 = 4.0;
const RING: f32 = 48.0;
const MAX_ROWS: usize = 10;

pub struct LbRow {
    pub username: String,
    pub value: i64,
}

/// Shrink a long name a few px first, then truncate with an ellipsis.
fn fit_name(text: &str, size: f32, max_w: f32) -> (String, Face<'static>) {
    let mut face = Face::new(&FONTS.zs, size);
    for s in 0..6 {
        face = Face::new(&FONTS.zs, size - s as f32);
        if face.width(text) <= max_w {
            return (text.to_string(), face);
        }
    }
    let mut chars: Vec<char> = text.chars().collect();
    while chars.len() > 1 {
        let candidate: String = chars.iter().chain(std::iter::once(&'…')).collect();
        if face.width(&candidate) <= max_w {
            break;
        }
        chars.pop();
    }
    (chars.into_iter().chain(std::iter::once('…')).collect(), face)
}

pub fn render(board: &str, rows: &[LbRow]) -> anyhow::Result<Vec<u8>> {
    let is_doubts = board.eq_ignore_ascii_case("doubts");
    let title = if is_doubts { "Top Doubt Solvers" } else { "Top Quiz Solvers" };
    let val_label = if is_doubts { "DOUBTS SOLVED" } else { "QUESTIONS SOLVED" };
    let rows = &rows[..rows.len().min(MAX_ROWS)];

    let footer_y = ROWS_Y + rows.len() as f32 * (ROW_H + ROW_GAP) + 24.0;
    let card_h = footer_y + 18.0 + 40.0;

    // transparent outside the rounded card so the corners blend into either theme
    let mut img = RgbaImage::new(CARD_W as u32, card_h as u32);
    rounded_rect(&mut img, (0.0, 0.0, CARD_W - 1.0, card_h - 1.0), RADIUS, Some(BG), None);

    // header — the brand keeps its own casing; only the label after it is caps
    let f_kicker = Face::new(&FONTS.zs, 17.0);
    let brand = "iTeachChem";
    draw_text_bold(&mut img, f_kicker, PAD, 48.0, brand, GOLD, 0.6);
    let kx = PAD + f_kicker.width(brand) + 0.6;
    draw_text_bold(&mut img, f_kicker, kx, 48.0, "  ·  SERVER LEADERBOARD", GOLD, 0.6);

    let f_title = Face::new(&FONTS.zs, 40.0);
    draw_text_bold(&mut img, f_title, PAD, 82.0, title, TEXT, 2.0);

    let f_date = Face::new(&FONTS.zs, 18.0);
    let date_str = chrono::Utc::now().format("%d %b %Y").to_string();
    let title_mid = f_title.ascent() + 82.0 - f_title.cap_height() / 2.0;
    draw_text(&mut img, f_date, CARD_W - PAD - f_date.width(&date_str), f_date.y_centered(title_mid + 4.0), &date_str, TEXT_DIM);

    // column header, printed once instead of on every row
    let f_head = Face::new(&FONTS.zs, 14.0);
    let head_y = f_head.y_centered(HEAD_Y + 10.0);
    draw_text_bold(&mut img, f_head, PAD + 16.0, head_y, "RANK", TEXT_DIM, 0.5);
    draw_text_bold(&mut img, f_head, PAD + 16.0 + RING + 24.0, head_y, "MEMBER", TEXT_DIM, 0.5);
    let lw = f_head.width(val_label) + 0.5;
    draw_text_bold(&mut img, f_head, CARD_W - PAD - 16.0 - lw, head_y, val_label, TEXT_DIM, 0.5);
    rounded_rect(&mut img, (PAD, HEAD_Y + 30.0, CARD_W - PAD, HEAD_Y + 31.0), 0.0, Some(DIVIDER), None);

    let f_rank = Face::new(&FONTS.zs, 20.0);
    let f_val = Face::new(&FONTS.zs, 30.0);
    for (i, row) in rows.iter().enumerate() {
        let medal = MEDALS.get(i).copied();
        let ry = ROWS_Y + i as f32 * (ROW_H + ROW_GAP);
        let mid = ry + ROW_H / 2.0;

        if let Some((_, tint)) = medal {
            rounded_rect(&mut img, (PAD, ry, CARD_W - PAD, ry + ROW_H), 14.0, Some(tint), None);
        }

        // rank ring
        let rx = PAD + 16.0;
        let (ring_col, fill, num_col) = match medal {
            Some((col, tint)) => (col, tint, col),
            None => (BORDER, BG, TEXT_DIM),
        };
        rounded_rect(
            &mut img,
            (rx, mid - RING / 2.0, rx + RING, mid + RING / 2.0),
            RING / 2.0,
            Some(fill),
            Some((ring_col, 2.0)),
        );
        let rtext = (i + 1).to_string();
        let rw = f_rank.width(&rtext) + 1.0;
        draw_text_bold(&mut img, f_rank, rx + (RING - rw) / 2.0, f_rank.y_centered(mid), &rtext, num_col, 1.0);

        // value
        let vtext = row.value.to_string();
        let vw = f_val.width(&vtext) + 1.5;
        let vx = CARD_W - PAD - 16.0 - vw;
        draw_text_bold(&mut img, f_val, vx, f_val.y_centered(mid), &vtext, TEXT, 1.5);

        // username gets everything between the ring and the value
        let nx = rx + RING + 24.0;
        let (name, f_name) = fit_name(&row.username, 26.0, vx - 32.0 - nx);
        let ny = f_name.y_centered(mid);
        if medal.is_some() {
            draw_text_bold(&mut img, f_name, nx, ny, &name, TEXT, 1.0);
        } else {
            draw_text(&mut img, f_name, nx, ny, &name, TEXT);
        }
    }

    // footer
    let f_foot = Face::new(&FONTS.zs, 15.0);
    let cmd = format!("+lb {}", board.to_lowercase());
    draw_text(&mut img, f_foot, PAD, footer_y, "iTeachChem Helper", TEXT_DIM);
    draw_text(&mut img, f_foot, CARD_W - PAD - f_foot.width(&cmd), footer_y, &cmd, TEXT_DIM);

    encode_png_rgba(img)
}
