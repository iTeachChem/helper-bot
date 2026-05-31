from __future__ import annotations
from pathlib import Path
import io
import os
import asyncio
from datetime import datetime, timezone

from PIL import Image, ImageDraw, ImageFont

# template paths 
ROOT = Path(__file__).resolve().parents[1]
_TMPL_D   = os.path.join(ROOT, "assets", "templates", "lb_doubts_template.png")
_TMPL_Q   = os.path.join(ROOT, "assets", "templates", "lb_quiz_template.png")

# palette 
MIST      = (209, 224, 222)
MIST_DIM  = (140, 160, 160)
KEYLIME   = (238, 244, 206)
CHARCOAL_ROW = (34, 34, 34)
CHARCOAL_MID = (38, 38, 38)

GOLD      = (220, 185,  60)
SILVER    = (170, 180, 195)
BRONZE    = (185, 120,  70)
BADGE_COL_DEF = MIST_DIM

GOLD_BG   = (55,  48,  15)
SILVER_BG = (32,  40,  50)
BRONZE_BG = (50,  32,  18)
DEF_BG    = (38,  38,  38)

# fonts
_SG  = "core/assets/fonts/seguiemj.ttf"
_ZS  = "core/assets/fonts/ZalandoSansExpanded.ttf"

def _f(path, size):
    try:    return ImageFont.truetype(path, size)
    except: return ImageFont.load_default()

# card geometry 
CARD_W   = 680
ROW_H    = 64
TOP_H    = 118
BOT_H    = 48
MAX_ROWS = 10
PAD_X    = 40


def _rr(draw, xy, r, fill=None, outline=None, width=1):
    draw.rounded_rectangle(list(xy), radius=r, fill=fill, outline=outline, width=width)


def _fit(draw, text: str, font, max_w: float) -> str:
    """Truncate with ellipsis only when text exceeds 12 chars AND pixel-overflows."""
    if len(text) <= 12 or draw.textlength(text, font=font) <= max_w:
        return text
    while len(text) > 1 and draw.textlength(text + "…", font=font) > max_w:
        text = text[:-1]
    return text + "…"


def generate_lb_card_sync(board_type: str, rows: list[dict]) -> bytes:
    is_doubts = board_type.lower() == "doubts"
    tmpl_path = _TMPL_D if is_doubts else _TMPL_Q
    val_label = "doubts solved" if is_doubts else "questions solved"

    n    = min(len(rows), MAX_ROWS)
    rows = rows[:n]

    with Image.open(tmpl_path) as tmpl:
        img = tmpl.copy().convert("RGBA")

    draw = ImageDraw.Draw(img)

    f_rank  = _f(_SG,  17)
    f_name  = _f(_ZS,  16)
    f_val   = _f(_ZS,  16)
    f_label = _f(_ZS,  12)
    f_date  = _f(_ZS, 11)

    # date string in header
    date_str = datetime.now(timezone.utc).strftime("%d %b %Y")
    dw = draw.textlength(date_str, font=f_date)
    draw.text((CARD_W - 22 - 18 - dw, 36), date_str, font=f_date, fill=MIST_DIM)

    # max username width: space between badge and value block
    max_name_w = CARD_W - PAD_X * 2 - 60 - 130

    for i, row in enumerate(rows):
        ry  = TOP_H + 22 + i * ROW_H
        mid = ry + ROW_H // 2

        rank_num = i + 1
        if rank_num == 1:
            badge_col = GOLD;   badge_bg = GOLD_BG
        elif rank_num == 2:
            badge_col = SILVER; badge_bg = SILVER_BG
        elif rank_num == 3:
            badge_col = BRONZE; badge_bg = BRONZE_BG
        else:
            badge_col = BADGE_COL_DEF
            badge_bg  = CHARCOAL_ROW if i % 2 == 0 else CHARCOAL_MID

        bx, bw, bh = PAD_X, 40, 32

        # actual badge (overwrites template placeholder)
        _rr(draw,
            (bx, mid - bh // 2, bx + bw, mid + bh // 2),
            r=6, fill=badge_bg, outline=badge_col, width=1)
        rtext = f"#{rank_num}"
        rtw   = draw.textlength(rtext, font=f_rank)
        draw.text(
            (bx + (bw - rtw) / 2, mid - 11),
            rtext, font=f_rank, fill=badge_col)

        # username
        nx    = bx + bw + 18
        uname = _fit(draw, row["username"], f_name, max_name_w)
        draw.text((nx, mid - 11), uname, font=f_name, fill=MIST)

        vtext = str(row["value"])
        vw    = draw.textlength(vtext, font=f_val)
        lw    = draw.textlength(val_label, font=f_label)
        vx    = CARD_W - PAD_X - max(vw, lw)
        draw.text((vx, mid - 18), vtext, font=f_val, fill=KEYLIME)

    for i in range(n, MAX_ROWS):
        ry = TOP_H + 22 + i * ROW_H
        fill = (34, 34, 34) if i % 2 == 0 else (38, 38, 38)
        draw.rectangle((28, ry + 4, CARD_W - 28, ry + ROW_H - 4), fill=fill)

    # exporting to bytes then releasing all image memory before returning
    out = io.BytesIO()
    try:
        img.convert("RGB").save(out, format="PNG", optimize=True)
        result = out.getvalue()
    finally:
        out.close()
        img.close()
        del draw, img

    return result


async def generate_lb_card(board_type: str, rows: list[dict]) -> bytes:
    return await asyncio.to_thread(generate_lb_card_sync, board_type, rows)