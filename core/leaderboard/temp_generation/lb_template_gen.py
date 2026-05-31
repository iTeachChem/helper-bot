import io
import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

# palette 
CHARCOAL     = (28,  28,  28)
CHARCOAL_MID = (38,  38,  38)
CHARCOAL_ROW = (34,  34,  34)
SLATE        = (77,  92,  96)
AMYTHEST     = (82,  82,  102)
MIST         = (209, 224, 222)
MIST_DIM     = (140, 160, 160)
KEYLIME      = (238, 244, 206)
DIVIDER      = (55,  65,  70)
MUTED        = (80,  90,  90)

CARD_W   = 680
ROW_H    = 64
TOP_H    = 118
BOT_H    = 48
MAX_ROWS = 10

_SG = "core/assets/fonts/seguiemj.ttf"
_ZS = "core/assets/fonts/ZalandoSansExpanded.ttf"

def _f(p, s):
    try:    return ImageFont.truetype(p, s)
    except: return ImageFont.load_default()

def _rr(draw, xy, r, fill=None, outline=None, width=1):
    draw.rounded_rectangle(list(xy), radius=r, fill=fill, outline=outline, width=width)

def _hg(W, H, x, y, w, h, c1, c2):
    lay = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d   = ImageDraw.Draw(lay)
    for i in range(w):
        t  = i / max(w - 1, 1)
        r_ = int(c1[0] + (c2[0] - c1[0]) * t)
        g_ = int(c1[1] + (c2[1] - c1[1]) * t)
        b_ = int(c1[2] + (c2[2] - c1[2]) * t)
        d.line([(x + i, y), (x + i, y + h - 1)], fill=(r_, g_, b_))
    return lay

def _mask_dots(dots_img, card_mask_img):
    da = np.array(dots_img)
    cm = np.array(card_mask_img)
    da[:, :, 3] = np.minimum(da[:, :, 3], cm)
    return Image.fromarray(da)


def generate_lb_template(board_type: str) -> bytes:
    LH  = TOP_H + ROW_H * MAX_ROWS + BOT_H   # 806
    CX1, CY1 = 22, 22
    CX2, CY2 = CARD_W - 22, LH - 22
    CR  = 16

    f_title = _f(_SG, 24)
    f_sub   = _f(_ZS, 12)
    f_lbl   = _f(_ZS, 12)
    f_wm    = _f(_ZS, 11)

    is_doubts = board_type.lower() == "doubts"
    title     = "TOP DOUBT SOLVERS" if is_doubts else "TOP QUIZ SOLVERS"
    val_label = "doubts solved"     if is_doubts else "questions solved"
    sub       = f"server leaderboard  ·  top {MAX_ROWS}"

    img = Image.new("RGBA", (CARD_W, LH), CHARCOAL)

    glow = Image.new("RGBA", (CARD_W, LH), (0, 0, 0, 0))
    ImageDraw.Draw(glow).rounded_rectangle(
        (16, 16, CARD_W - 16, LH - 16), radius=20, fill=(*AMYTHEST, 25))
    glow = glow.filter(ImageFilter.GaussianBlur(18))
    img.paste(glow, mask=glow)

    ImageDraw.Draw(img).rounded_rectangle([CX1, CY1, CX2, CY2], radius=CR,
                                          fill=CHARCOAL_MID)

    # dot layer masked to card bounds
    cmask = Image.new("L", (CARD_W, LH), 0)
    ImageDraw.Draw(cmask).rounded_rectangle([CX1, CY1, CX2, CY2], radius=CR, fill=255)
    dots  = Image.new("RGBA", (CARD_W, LH), (0, 0, 0, 0))
    dd    = ImageDraw.Draw(dots)
    for gx in range(CX1 + 4, CX2 - 2, 28):
        for gy in range(CY1 + 4, CY2 - 2, 28):
            dd.ellipse((gx, gy, gx + 1, gy + 1), fill=(*MIST, 15))
    dots = _mask_dots(dots, cmask)
    img.paste(dots, mask=dots)

    bar = _hg(CARD_W, LH, CX1, CY1, CX2 - CX1, 4, SLATE, KEYLIME)
    img.paste(bar, mask=bar)

    draw = ImageDraw.Draw(img)
    _rr(draw, (CX1, CY1, CX1 + 4, CY1 + TOP_H - 10), r=2, fill=AMYTHEST)
    draw.text((44, 36), title, font=f_title, fill=MIST)
    draw.text((44, 72), sub,   font=f_sub,   fill=MIST_DIM)
    draw.line([(36, TOP_H + 20), (CARD_W - 36, TOP_H + 20)], fill=DIVIDER, width=1)

    pad_x = 40
    for i in range(MAX_ROWS):
        ry  = TOP_H + 22 + i * ROW_H
        mid = ry + ROW_H // 2

        if i % 2 == 0:
            _rr(draw, (28, ry + 4, CARD_W - 28, ry + ROW_H - 4),
                r=8, fill=CHARCOAL_ROW)

        badge_bg = CHARCOAL_ROW if i % 2 == 0 else CHARCOAL_MID
        _rr(draw, (pad_x, mid - 16, pad_x + 40, mid + 16),
            r=6, fill=badge_bg, outline=MIST_DIM, width=1)

        draw.text((CARD_W - pad_x - 110, mid + 2), val_label, font=f_lbl, fill=MIST_DIM)

        if i < MAX_ROWS - 1:
            draw.line([(pad_x + 50, ry + ROW_H), (CARD_W - pad_x, ry + ROW_H)],
                      fill=DIVIDER, width=1)

    fy = TOP_H + 22 + MAX_ROWS * ROW_H + 12
    ww = draw.textlength("iTeachChem Helper Bot", font=f_wm)
    draw.text((CARD_W - 22 - 18 - ww, fy), "iTeachChem Helper Bot",
              font=f_wm, fill=MUTED)

    out = io.BytesIO()
    img.convert("RGB").save(out, "PNG", optimize=True)
    out.seek(0)
    return out.read()


# COORDINATE MAP
# CARD_W=680  ROW_H=64  TOP_H=118  BOT_H=48  MAX_ROWS=10  H=806
# Per-row i (0-indexed):
#   ry  = 140 + i*64
#   mid = ry + 32
#   badge: x=40 y=mid-16 w=40 h=32  r=6
#   rank text centred inside badge, y=mid-11
#   username: x=98  y=mid-11
#   value number: x=CARD_W-pad_x-max(vw,lw)  y=mid-18
#   value label (static on template): y=mid+2
# date string (dynamic, renderer draws): top-right  y=36


"""

Use this to generate new templates after editing the PSD. 
Remember to update the coordinate maps in both the template and renderer
if you make any geometry changes.

"""


"""

if __name__ == "__main__":
    import os
    root = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))
    os.chdir(root)
    for bt in ("doubts", "quiz"):
        data = generate_lb_template(bt)
        path = f"core/assets/templates/lb_{bt}_template.png"
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "wb") as f:
            f.write(data)
        print(f"Written: {path}  ({len(data):,} bytes)")
        
"""