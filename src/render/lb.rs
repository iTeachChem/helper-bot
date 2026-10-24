use super::{draw_text, encode_png, load_template, rect, rounded_rect, Face, Rgb, FONTS};

static TMPL_D: &[u8] = include_bytes!("../../assets/templates/lb_doubts_template.png");
static TMPL_Q: &[u8] = include_bytes!("../../assets/templates/lb_quiz_template.png");

// palette
const MIST: Rgb = (209, 224, 222);
const MIST_DIM: Rgb = (140, 160, 160);
const KEYLIME: Rgb = (238, 244, 206);
const CHARCOAL_ROW: Rgb = (34, 34, 34);
const CHARCOAL_MID: Rgb = (38, 38, 38);

const GOLD: Rgb = (220, 185, 60);
const SILVER: Rgb = (170, 180, 195);
const BRONZE: Rgb = (185, 120, 70);
const BADGE_COL_DEF: Rgb = MIST_DIM;

const GOLD_BG: Rgb = (55, 48, 15);
const SILVER_BG: Rgb = (32, 40, 50);
const BRONZE_BG: Rgb = (50, 32, 18);

// card geometry
const CARD_W: f32 = 680.0;
const ROW_H: f32 = 64.0;
const TOP_H: f32 = 118.0;
const MAX_ROWS: usize = 10;
const PAD_X: f32 = 40.0;

pub struct LbRow {
    pub username: String,
    pub value: i64,
}

pub fn render(board: &str, rows: &[LbRow]) -> anyhow::Result<Vec<u8>> {
    let is_doubts = board.eq_ignore_ascii_case("doubts");
    let mut img = load_template(if is_doubts { TMPL_D } else { TMPL_Q });
    let val_label = if is_doubts { "doubts solved" } else { "questions solved" };
    let rows = &rows[..rows.len().min(MAX_ROWS)];

    let f_rank = Face::new(&FONTS.sg, 17.0);
    let f_name = Face::new(&FONTS.zs, 16.0);
    let f_val = Face::new(&FONTS.zs, 16.0);
    let f_label = Face::new(&FONTS.zs, 12.0);
    let f_date = Face::new(&FONTS.zs, 11.0);

    // date string in header
    let date_str = chrono::Utc::now().format("%d %b %Y").to_string();
    let dw = f_date.width(&date_str);
    draw_text(&mut img, f_date, CARD_W - 22.0 - 18.0 - dw, 36.0, &date_str, MIST_DIM);

    // max username width: space between badge and value block
    let max_name_w = CARD_W - PAD_X * 2.0 - 60.0 - 130.0;

    for (i, row) in rows.iter().enumerate() {
        let ry = TOP_H + 22.0 + i as f32 * ROW_H;
        let mid = ry + (ROW_H / 2.0).floor();

        let rank_num = i + 1;
        let (badge_col, badge_bg) = match rank_num {
            1 => (GOLD, GOLD_BG),
            2 => (SILVER, SILVER_BG),
            3 => (BRONZE, BRONZE_BG),
            _ => (BADGE_COL_DEF, if i % 2 == 0 { CHARCOAL_ROW } else { CHARCOAL_MID }),
        };

        let (bx, bw, bh) = (PAD_X, 40.0, 32.0);

        // actual badge (overwrites template placeholder)
        rounded_rect(
            &mut img,
            (bx, mid - bh / 2.0, bx + bw, mid + bh / 2.0),
            6.0,
            Some(badge_bg),
            Some((badge_col, 1.0)),
        );
        let rtext = format!("#{rank_num}");
        let rtw = f_rank.width(&rtext);
        draw_text(&mut img, f_rank, bx + (bw - rtw) / 2.0, mid - 11.0, &rtext, badge_col);

        // username
        let nx = bx + bw + 18.0;
        let uname = f_name.fit(&row.username, max_name_w);
        draw_text(&mut img, f_name, nx, mid - 11.0, &uname, MIST);

        let vtext = row.value.to_string();
        let vw = f_val.width(&vtext);
        let lw = f_label.width(val_label);
        let vx = CARD_W - PAD_X - vw.max(lw);
        draw_text(&mut img, f_val, vx, mid - 18.0, &vtext, KEYLIME);
    }

    // blank out unused row placeholders
    for i in rows.len()..MAX_ROWS {
        let ry = (TOP_H + 22.0 + i as f32 * ROW_H) as u32;
        let fill = if i % 2 == 0 { CHARCOAL_ROW } else { CHARCOAL_MID };
        rect(&mut img, (28, ry + 4, CARD_W as u32 - 28, ry + ROW_H as u32 - 4), fill);
    }

    encode_png(img)
}
