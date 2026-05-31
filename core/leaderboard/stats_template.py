from __future__ import annotations
from pathlib import Path
import io
import os
import asyncio
from datetime import datetime, timezone
from typing import Optional

from PIL import Image, ImageDraw, ImageFont

# template path 
ROOT = Path(__file__).resolve().parents[1]
_TEMPLATE   = os.path.join(ROOT, "assets", "templates", "stats_template.png")

# palette 
MIST        = (209, 224, 222)
MIST_DIM    = (140, 160, 160)
KEYLIME     = (238, 244, 206)
SLATE       = (77,  92,  96)
AMYTHEST    = (82,  82,  102)
PANEL_BG    = (32,  32,  32)
CHARCOAL    = (28,  28,  28)
MUTED       = (80,  90,  90)

ACC_GREEN   = (100, 180, 155)
ACC_AMBER   = (180, 150, 80)
ACC_TEAL    = (90,  165, 185)

# fonts 
_SG  = "core/assets/fonts/seguiemj.ttf"
_ZS  = "core/assets/fonts/ZalandoSansExpanded.ttf"

def _f(path, size):
    try:    return ImageFont.truetype(path, size)
    except: return ImageFont.load_default()

# card dimensions
W, H = 900, 520

# left panel geometry
PX, PY    = 46, 50
PW        = 210
AVS       = 96
AX        = PX + (PW - AVS) // 2   # 103
AY        = PY + 22                 # 72

# right content geometry 
RX   = PX + PW + 22                 # 278
RY   = PY                           # 50
RW   = W - 26 - RX - 18            # 578
GAP  = 10

BY   = RY + 20                      # 70

D2Y  = BY + 74                      # 144
PW3  = (RW - GAP * 2) // 3         # 186
PY3  = D2Y + 26                     # 170
PH3  = 68

D3Y  = PY3 + PH3 * 2 + GAP * 2 + 8  # 334
DP_Y = D3Y + 26                       # 360
DP_H = 58
DP_W = (RW - GAP) // 2               # 284


# helpers
def _fit(draw, text: str, font, max_w: float) -> str:
    """Truncate with ellipsis only when text exceeds 12 chars AND overflows."""
    if len(text) <= 12 or draw.textlength(text, font=font) <= max_w:
        return text
    while len(text) > 1 and draw.textlength(text + "…", font=font) > max_w:
        text = text[:-1]
    return text + "…"


def _circle_crop(img: Image.Image, size: int) -> Image.Image:
    """Crop image into a circle. Caller must close the returned image."""
    img  = img.convert("RGBA").resize((size, size), Image.LANCZOS)
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).ellipse((0, 0, size, size), fill=255)
    out  = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    out.paste(img, mask=mask)
    img.close()
    mask.close()
    return out


def _h_grad_bar(canvas, x, y, w, h, c1, c2, pct):
    """Draw a filled gradient progress bar on top of the existing track."""
    if pct <= 0:
        return
    fw = max(h, int(w * min(pct, 1.0)))
    grad = Image.new("RGBA", (fw, h), (0, 0, 0, 0))
    gd   = ImageDraw.Draw(grad)
    r2   = h // 2
    mask = Image.new("L", (fw, h), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, fw, h), radius=r2, fill=255)
    for i in range(fw):
        t  = i / max(fw - 1, 1)
        r_ = int(c1[0] + (c2[0] - c1[0]) * t)
        g_ = int(c1[1] + (c2[1] - c1[1]) * t)
        b_ = int(c1[2] + (c2[2] - c1[2]) * t)
        gd.line([(i, 0), (i, h)], fill=(r_, g_, b_, 255))
    grad.putalpha(mask)
    canvas.paste(grad, (x, y), grad)
    grad.close()
    mask.close()


def _na(val, fmt=None):
    """Format a value or return 'N/A' if None/falsy."""
    if val is None:
        return "N/A"
    if fmt:
        return fmt.format(val)
    return str(val)


def generate_stats_card_sync(
    avatar_bytes: Optional[bytes],
    data: dict,
) -> bytes:
    with Image.open(_TEMPLATE) as tmpl:
        img = tmpl.copy().convert("RGBA")

    draw = ImageDraw.Draw(img)

    f_name   = _f(_SG,  28)
    f_server = _f(_ZS,  14)
    f_rank   = _f(_SG,  34)
    f_rankl  = _f(_ZS,  11)
    f_label  = _f(_ZS,  11)
    f_val    = _f(_ZS,  22)
    f_big    = _f(_SG,  24)
    f_date   = _f(_ZS, 11)

    username      = data.get("username")  or "Unknown"
    server_name   = data.get("server_name") or "Discord"
    quiz_rank     = data.get("quiz_rank")   or "N/A"
    doubts_rank   = data.get("doubts_rank") or "N/A"
    attempted     = data.get("attempted")
    solved        = data.get("solved")
    skipped       = data.get("skipped")
    points        = data.get("points")
    total_time_str= data.get("total_time_str")
    doubts_solved = data.get("doubts_solved")
    accuracy      = data.get("accuracy")
    avg_time      = data.get("avg_time")

    # avatar 
    av_raw = None
    try:
        if avatar_bytes:
            try:
                av_raw = Image.open(io.BytesIO(avatar_bytes))
            except Exception:
                av_raw = Image.new("RGBA", (AVS, AVS), (50, 60, 65, 255))
        else:
            av_raw = Image.new("RGBA", (AVS, AVS), (50, 60, 65, 255))

        av_img = _circle_crop(av_raw, AVS)
        try:
            img.paste(av_img, (AX, AY), av_img)
        finally:
            av_img.close()
    finally:
        if av_raw is not None:
            av_raw.close()

    # username
    uname = _fit(draw, username, f_name, PW - 28)
    uw    = draw.textlength(uname, font=f_name)
    draw.text((PX + (PW - uw) / 2, AY + AVS + 10), uname, font=f_name, fill=MIST)

    # server name
    srv = _fit(draw, server_name, f_server, PW - 28)
    sw  = draw.textlength(srv, font=f_server)
    draw.text((PX + (PW - sw) / 2, AY + AVS + 44), srv,
              font=f_server, fill=(170, 195, 195))

    # ranks 
    div1 = AY + AVS + 80
    ry2  = div1 + 14

    qr  = _fit(draw, quiz_rank,   f_rank, PW - 28)
    draw.text((PX + 14, ry2 + 16), qr, font=f_rank, fill=KEYLIME)

    dr  = _fit(draw, doubts_rank, f_rank, PW - 28)
    draw.text((PX + 14, ry2 + 90), dr, font=f_rank, fill=MIST)

    # date 
    date_str = datetime.now(timezone.utc).strftime("%d %b %Y · %H:%M UTC")
    ds  = _fit(draw, date_str, f_date, PW - 20)
    dw  = draw.textlength(ds, font=f_date)
    ph  = H - 100
    draw.text((PX + (PW - dw) / 2, PY + ph - 20), ds,
              font=f_date, fill=(120, 155, 155))

    # performance bars 
    acc_pct  = (accuracy  / 100.0) if accuracy  is not None else 0.0
    skip_pct = ((skipped / attempted) if (skipped is not None and attempted) else 0.0)

    acc_text  = f"{accuracy:.1f}%"  if accuracy  is not None else "N/A"
    skip_text = f"{skip_pct*100:.1f}%"

    draw.text((RX + RW - 52, BY),       acc_text,  font=f_label, fill=KEYLIME)
    draw.text((RX + RW - 52, BY + 36),  skip_text, font=f_label, fill=(200, 185, 120))

    _h_grad_bar(img, RX, BY + 16, RW, 9, SLATE, KEYLIME, acc_pct)
    _h_grad_bar(img, RX, BY + 52, RW, 9, PANEL_BG, ACC_AMBER, skip_pct)

    # quiz stats pills 
    pills_r1 = [
        _na(attempted),
        _na(solved),
        _na(skipped),
    ]
    pills_r2 = [
        _na(points),
        total_time_str or "N/A",
        f"{avg_time:.1f}s" if avg_time is not None else "N/A",
    ]

    for i, val in enumerate(pills_r1):
        x = RX + i * (PW3 + GAP) + 14
        draw.text((x, PY3 + 30), val, font=f_big, fill=MIST)
    for i, val in enumerate(pills_r2):
        x = RX + i * (PW3 + GAP) + 14
        draw.text((x, PY3 + PH3 + GAP + 30), val, font=f_big, fill=MIST)

    # doubts pill (full-width, doubts_solved only)
    draw.text((RX + 14, DP_Y + 30),
              _na(doubts_solved), font=f_big, fill=MIST)

    # export to bytes then free all image memory immediately before return
    out = io.BytesIO()
    try:
        img.convert("RGB").save(out, format="PNG", optimize=True)
        result = out.getvalue()
    finally:
        out.close()
        img.close()
        del draw, img

    return result


async def generate_stats_card(avatar_bytes: Optional[bytes], data: dict) -> bytes:
    return await asyncio.to_thread(generate_stats_card_sync, avatar_bytes, data)