from __future__ import annotations
import io
import asyncio
from functools import lru_cache
from datetime import datetime, timezone

from PIL import Image, ImageDraw, ImageFont

# palette — one solid ink-blue card with a single accent, so it reads the same
# on Discord light and dark themes
BG        = ( 31,  35,  64)
SURFACE   = ( 42,  47,  85)
BORDER    = ( 78,  86, 140)
DIVIDER   = ( 52,  58, 102)
TEXT      = (245, 246, 251)
TEXT_DIM  = (185, 191, 220)
ACCENT    = (139, 155, 255)

# fonts
_ZS = "core/assets/fonts/ZalandoSansExpanded.ttf"

@lru_cache(maxsize=32)
def _f(size: int, weight: int = 400):
    try:
        font = ImageFont.truetype(_ZS, size)
        font.set_variation_by_axes([weight])
        return font
    except Exception:
        return ImageFont.load_default()

# card geometry
CARD_W    = 800
PAD       = 48
RADIUS    = 28
HEAD_Y    = 160
ROWS_Y    = 206
ROW_H     = 72
ROW_GAP   = 4
RING      = 48
MAX_ROWS  = 10


def _rr(draw, xy, r, fill=None, outline=None, width=1):
    draw.rounded_rectangle(list(xy), radius=r, fill=fill, outline=outline, width=width)


def _fit(draw, text: str, size: int, weight: int, max_w: float):
    """Shrink a long name a few steps first, then truncate with an ellipsis."""
    for s in range(size, size - 6, -1):
        font = _f(s, weight)
        if draw.textlength(text, font=font) <= max_w:
            return text, font
    while len(text) > 1 and draw.textlength(text + "…", font=font) > max_w:
        text = text[:-1]
    return text + "…", font


def generate_lb_card_sync(board_type: str, rows: list[dict]) -> bytes:
    is_doubts = board_type.lower() == "doubts"
    title     = "Top Doubt Solvers" if is_doubts else "Top Quiz Solvers"
    val_label = "DOUBTS SOLVED" if is_doubts else "QUESTIONS SOLVED"

    rows     = rows[:MAX_ROWS]
    footer_y = ROWS_Y + len(rows) * (ROW_H + ROW_GAP) + 24
    card_h   = footer_y + 18 + 40

    # transparent outside the rounded card so the corners blend into either theme
    img  = Image.new("RGBA", (CARD_W, card_h), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    _rr(draw, (0, 0, CARD_W - 1, card_h - 1), r=RADIUS, fill=BG)

    # header
    date_str = datetime.now(timezone.utc).strftime("%d %b %Y").upper()
    draw.text((PAD, 48), f"SERVER LEADERBOARD  ·  {date_str}", font=_f(17, 600), fill=ACCENT)
    draw.text((PAD, 82), title, font=_f(40, 800), fill=TEXT)

    # column header, printed once instead of on every row
    f_head = _f(14, 600)
    draw.text((PAD + 16, HEAD_Y), "RANK", font=f_head, fill=TEXT_DIM)
    draw.text((PAD + 16 + RING + 24, HEAD_Y), "MEMBER", font=f_head, fill=TEXT_DIM)
    draw.text((CARD_W - PAD - 16, HEAD_Y), val_label, font=f_head, fill=TEXT_DIM, anchor="ra")
    draw.line([(PAD, HEAD_Y + 30), (CARD_W - PAD, HEAD_Y + 30)], fill=DIVIDER, width=2)

    f_rank = _f(20, 700)
    f_val  = _f(30, 800)
    for i, row in enumerate(rows):
        top3 = i < 3
        ry   = ROWS_Y + i * (ROW_H + ROW_GAP)
        mid  = ry + ROW_H / 2

        if top3:
            _rr(draw, (PAD, ry, CARD_W - PAD, ry + ROW_H), r=14, fill=SURFACE)

        # rank ring
        rx = PAD + 16
        draw.ellipse((rx, mid - RING / 2, rx + RING, mid + RING / 2),
                     outline=ACCENT if top3 else BORDER, width=2)
        draw.text((rx + RING / 2, mid), str(i + 1), font=f_rank,
                  fill=ACCENT if top3 else TEXT_DIM, anchor="mm")

        # value
        vtext = str(row["value"])
        vx    = CARD_W - PAD - 16
        draw.text((vx, mid), vtext, font=f_val, fill=TEXT, anchor="rm")

        # username gets everything between the ring and the value
        nx    = rx + RING + 24
        max_w = vx - draw.textlength(vtext, font=f_val) - 32 - nx
        name, f_name = _fit(draw, row["username"], 26, 700 if top3 else 500, max_w)
        draw.text((nx, mid), name, font=f_name, fill=TEXT, anchor="lm")

    # footer
    f_foot = _f(15, 500)
    draw.text((PAD, footer_y), "iTeachChem Helper", font=f_foot, fill=TEXT_DIM)
    draw.text((CARD_W - PAD, footer_y), f"+lb {board_type.lower()}",
              font=f_foot, fill=TEXT_DIM, anchor="ra")

    # exporting to bytes then releasing all image memory before returning
    out = io.BytesIO()
    try:
        img.save(out, format="PNG", optimize=True)
        result = out.getvalue()
    finally:
        out.close()
        img.close()
        del draw, img

    return result


async def generate_lb_card(board_type: str, rows: list[dict]) -> bytes:
    return await asyncio.to_thread(generate_lb_card_sync, board_type, rows)
