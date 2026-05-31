import io
import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

# palette
CHARCOAL     = (28,  28,  28)
CHARCOAL_MID = (38,  38,  38)
SLATE_DARK   = (42,  52,  56)
SLATE        = (77,  92,  96)
AMYTHEST     = (82,  82,  102)
MIST         = (209, 224, 222)
MIST_DIM     = (140, 160, 160)
KEYLIME      = (238, 244, 206)
DIVIDER      = (55,  65,  70)
PANEL_BG     = (32,  32,  32)
MUTED        = (80,  90,  90)

W, H = 900, 520
CX1, CY1, CX2, CY2, CR = 26, 26, 874, 494, 18

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
    """Clip dot alpha channel to card bounds using numpy."""
    da = np.array(dots_img)       # H×W×4 RGBA
    cm = np.array(card_mask_img)  # H×W greyscale
    da[:, :, 3] = np.minimum(da[:, :, 3], cm)
    return Image.fromarray(da)

def _pill(img, x, y, pw, ph, accent):
    d = ImageDraw.Draw(img)
    d.rounded_rectangle([x, y, x + pw, y + ph], radius=10, fill=PANEL_BG)
    d.rounded_rectangle([x, y, x + 4, y + ph],  radius=2,  fill=accent)


def generate_stats_template() -> bytes:
    f_sec  = _f(_ZS, 11)
    f_lbl  = _f(_ZS, 11)
    f_rnkl = _f(_ZS, 11)
    f_wm   = _f(_ZS, 11)

    img = Image.new("RGBA", (W, H), CHARCOAL)

    # glow
    glow = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ImageDraw.Draw(glow).rounded_rectangle((20, 20, W - 20, H - 20), radius=22,
                                           fill=(*AMYTHEST, 30))
    glow = glow.filter(ImageFilter.GaussianBlur(20))
    img.paste(glow, mask=glow)

    # card body
    ImageDraw.Draw(img).rounded_rectangle([CX1, CY1, CX2, CY2], radius=CR,
                                          fill=CHARCOAL_MID)

    # dot layer masked to card shape
    cmask = Image.new("L", (W, H), 0)
    ImageDraw.Draw(cmask).rounded_rectangle([CX1, CY1, CX2, CY2], radius=CR, fill=255)
    dots  = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    dd    = ImageDraw.Draw(dots)
    for gx in range(CX1 + 4, CX2 - 2, 30):
        for gy in range(CY1 + 4, CY2 - 2, 30):
            dd.ellipse((gx, gy, gx + 1, gy + 1), fill=(*MIST, 18))
    dots = _mask_dots(dots, cmask)
    img.paste(dots, mask=dots)

    # top gradient bar
    bar = _hg(W, H, CX1, CY1, CX2 - CX1, 5, SLATE, KEYLIME)
    img.paste(bar, mask=bar)

    draw = ImageDraw.Draw(img)

    # left panel
    px, py, pw, ph = 46, 50, 210, 420
    _rr(draw, (px, py, px + pw, py + ph), r=14, fill=SLATE_DARK)
    _rr(draw, (px, py, px + 4,  py + ph), r=2,  fill=AMYTHEST)

    # avatar ring + placeholder
    avs = 96
    ax  = px + (pw - avs) // 2
    ay  = py + 22
    ring = Image.new("RGBA", (avs + 6, avs + 6), (0, 0, 0, 0))
    ImageDraw.Draw(ring).ellipse((0, 0, avs + 5, avs + 5), outline=KEYLIME, width=3)
    img.paste(ring, (ax - 3, ay - 3), ring)
    mp  = Image.new("L", (avs, avs), 0)
    ImageDraw.Draw(mp).ellipse((0, 0, avs, avs), fill=255)
    avf = Image.new("RGBA", (avs, avs), (*CHARCOAL, 255))
    avf.putalpha(mp)
    img.paste(avf, (ax, ay), avf)

    div1 = ay + avs + 80
    draw.line([(px + 18, div1), (px + pw - 18, div1)], fill=(*CHARCOAL, 160), width=1)
    ry2 = div1 + 14
    draw.text((px + 14, ry2),      "QUIZ RANK",   font=f_rnkl, fill=MIST_DIM)
    draw.line([(px + 18, ry2 + 62), (px + pw - 18, ry2 + 62)],
              fill=(*CHARCOAL, 160), width=1)
    draw.text((px + 14, ry2 + 74), "DOUBTS RANK", font=f_rnkl, fill=MIST_DIM)

    # right content
    rx = px + pw + 22    # 278
    ry = py              # 50
    rw = W - 26 - rx - 18  # 578
    gap = 10

    draw.text((rx, ry), "PERFORMANCE", font=f_sec, fill=MIST_DIM)
    by = ry + 20
    draw.text((rx, by),      "Accuracy",  font=f_lbl, fill=MIST_DIM)
    draw.text((rx, by + 36), "Skip rate", font=f_lbl, fill=MIST_DIM)
    for yy in (by + 16, by + 52):
        ImageDraw.Draw(img).rounded_rectangle([rx, yy, rx + rw, yy + 9], radius=4,
                                              fill=PANEL_BG)

    d2y = by + 74
    draw.line([(rx, d2y), (rx + rw, d2y)], fill=DIVIDER, width=1)
    draw.text((rx, d2y + 8), "QUIZ STATS", font=f_sec, fill=MIST_DIM)

    pw3 = (rw - gap * 2) // 3
    py3 = d2y + 26
    ph3 = 68
    ar1 = [KEYLIME, (100, 180, 155), (180, 150, 80)]
    ar2 = [SLATE,   KEYLIME,         (90,  165, 185)]
    lr1 = ["Attempted", "Solved",     "Skipped"]
    lr2 = ["Points",    "Total Time", "Avg Time"]
    for i in range(3):
        for row, (acc, lbl) in enumerate(zip([ar1[i], ar2[i]], [lr1[i], lr2[i]])):
            yy = py3 + (ph3 + gap) * row
            xx = rx + i * (pw3 + gap)
            _pill(img, xx, yy, pw3, ph3, acc)
            draw.text((xx + 14, yy + 9), lbl.upper(), font=f_lbl, fill=MIST_DIM)

    d3y = py3 + ph3 * 2 + gap * 2 + 8
    draw.line([(rx, d3y), (rx + rw, d3y)], fill=DIVIDER, width=1)
    draw.text((rx, d3y + 8), "DOUBTS", font=f_sec, fill=MIST_DIM)

    dp_y = d3y + 26
    dp_h = 58
    _pill(img, rx, dp_y, rw, dp_h, KEYLIME) 
    draw.text((rx + 14, dp_y + 9), "DOUBTS SOLVED", font=f_lbl, fill=MIST_DIM)

    ww = draw.textlength("iTeachChem Helper Bot", font=f_wm)
    draw.text((W - 26 - 18 - ww, H - 26 - 16), "iTeachChem Helper Bot",
              font=f_wm, fill=MUTED)

    out = io.BytesIO()
    img.convert("RGB").save(out, "PNG", optimize=True)
    out.seek(0)
    return out.read()


# COORDINATE MAP
# px=46 py=50 pw=210 ph=420
# avs=96  ax=103  ay=72
# avatar ring paste: (100, 69)  avatar paste: (103, 72)
# username centre-x: px+pw//2=151   y: ay+avs+10=178
# server_name y: ay+avs+44=212
# div1=248  ry2=262
# quiz_rank text y: ry2+16=278
# doubts_rank text y: ry2+90=352
# date y: py+ph-20=450
# rx=278  ry=50  rw=578  gap=10  by=70
# acc bar:  (278, 86, 578, 9)   acc label x: rx+rw-52=804
# skip bar: (278, 122, 578, 9)  skip label x: 804
# d2y=144  pw3=186  py3=170  ph3=68
# pill r1 value y: py3+30=200
# pill r2 value y: py3+ph3+gap+30=278
# d3y=334  dp_y=360  dp_h=58
# doubts value y: dp_y+30=390  (full-width pill, rw=578)

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
    data = generate_stats_template()
    path = "core/assets/templates/stats_template.png"
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)
    print(f"Written: {path}  ({len(data):,} bytes)")


"""