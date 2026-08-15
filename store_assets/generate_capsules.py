#!/usr/bin/env python3
"""Generate NV-2.0 store capsule images (placeholder art, procedurally).

Produces every size the stores require:

  Epic Games Store:
    3840x2160  key art            -> egs/key_art_3840x2160.png
    1024x1024  portrait           -> egs/portrait_1024x1024.png
    2560x1440  landscape          -> egs/landscape_2560x1440.png
     800x450   details            -> egs/details_800x450.png

  itch.io:
     630x500   header             -> itchio/header_630x500.png

The art is a placeholder: a voxel-style terrain silhouette with a sky
gradient and the NV-2.0 wordmark. Replace with real art before launch —
these files are meant to unblock store setup, not to be final.
"""

import os
import sys
from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = os.path.dirname(os.path.abspath(__file__))
FONT_PATH = os.path.join(
    ROOT, "..", "Assets", "Fonts", "Subtitles", "Doto-VariableFont_ROND,wght.ttf"
)
FALLBACK_FONTS = [
    "/usr/share/fonts/Fira_Sans/FiraSans-ExtraBold.ttf",
    "/usr/share/fonts/noto/NotoSans-Bold.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf",
]


def _font(size: int):
    candidates = [FONT_PATH] + FALLBACK_FONTS
    for path in candidates:
        if os.path.isfile(path):
            try:
                return ImageFont.truetype(path, size)
            except Exception:
                continue
    return ImageFont.load_default()


def _sky_gradient(w: int, h: int):
    """Vertical gradient: deep dusk blue -> warm horizon -> pale sky."""
    top = (16, 24, 46)
    mid = (58, 78, 118)
    hor = (214, 168, 130)
    img = Image.new("RGB", (w, h))
    px = img.load()
    for y in range(h):
        t = y / max(h - 1, 1)
        if t < 0.55:
            k = t / 0.55
            c = tuple(int(top[i] + (mid[i] - top[i]) * k) for i in range(3))
        else:
            k = (t - 0.55) / 0.45
            c = tuple(int(mid[i] + (hor[i] - mid[i]) * k) for i in range(3))
        for x in range(w):
            px[x, y] = c
    return img


def _terrain(w: int, h: int, seed: int = 42):
    """Voxel-style terrain band along the bottom edge."""
    band_h = max(int(h * 0.34), 60)
    img = Image.new("RGB", (w, band_h))
    px = img.load()
    import random

    rng = random.Random(seed)
    # column heights -> blocky silhouette
    heights = []
    col = 0
    while col < w:
        step = rng.randint(14, 34)
        heights.append((col, min(col + step, w), rng.randint(int(band_h * 0.35), band_h)))
        col += step
    # grass/dirt palette (blocky, low saturation so text stays readable)
    grass = [(74, 106, 63), (66, 96, 57), (82, 116, 70), (58, 88, 52)]
    dirt = [(122, 92, 62), (110, 82, 56), (134, 102, 68)]
    for y in range(band_h):
        for x in range(w):
            top_y = None
            for (sx, ex, hh) in heights:
                if sx <= x < ex:
                    top_y = hh
                    break
            if top_y is None:
                top_y = band_h
            # below the skyline -> blocks
            if band_h - y > top_y:
                depth = band_h - y - top_y
                if depth <= 3:
                    px[x, y] = grass[(x // 16 + depth) % len(grass)]
                else:
                    px[x, y] = dirt[(x // 16) % len(dirt)]
            else:
                px[x, y] = (16, 24, 46)  # blend into sky bottom
    return img


def _vignette(w: int, h: int, strength: float = 0.28):
    """Soft darkening at edges to lift the center title."""
    mask = Image.new("L", (w, h), 0)
    md = ImageDraw.Draw(mask)
    md.ellipse((-w * 0.25, -h * 0.25, w * 1.25, h * 1.25), fill=255)
    mask = mask.filter(ImageFilter.GaussianBlur(w * 0.12))
    black = Image.new("RGB", (w, h), (8, 10, 18))
    return Image.composite(black, Image.new("RGB", (w, h), (255, 255, 255)), mask)


def _title_text(w: int, h: int, words: tuple, max_w: float = 0.82):
    """Two-line wordmark: 'NV-2.0' large, tagline smaller."""
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    big, small = words
    size = int(w * 0.16)
    while size > 20:
        f = _font(size)
        bb = d.textbbox((0, 0), big, font=f)
        if bb[2] - bb[0] <= w * max_w:
            break
        size = int(size * 0.92)
    f = _font(size)
    bb = d.textbbox((0, 0), big, font=f)
    tw, th = bb[2] - bb[0], bb[3] - bb[1]
    x = (w - tw) // 2 - bb[0]
    y = int(h * 0.36) - bb[1]
    # soft shadow
    d.text((x + max(2, size // 28), y + max(2, size // 28)), big, font=f,
           fill=(0, 0, 0, 200))
    d.text((x, y), big, font=f, fill=(240, 244, 250, 255))

    if small:
        fs = int(size * 0.28)
        f2 = _font(fs)
        bb2 = d.textbbox((0, 0), small, font=f2)
        tw2 = bb2[2] - bb2[0]
        x2 = (w - tw2) // 2 - bb2[0]
        y2 = y + th + int(fs * 0.55)
        d.text((x2 + 2, y2 + 2), small, font=f2, fill=(0, 0, 0, 180))
        d.text((x2, y2), small, font=f2, fill=(224, 178, 120, 255))
    return img


def _crop_to_ratio(img: Image.Image, ratio: float):
    """Center-crop the art to the requested aspect ratio."""
    w, h = img.size
    target = w / h
    if target > ratio:  # too wide -> crop width
        nw = int(h * ratio)
        x = (w - nw) // 2
        return img.crop((x, 0, x + nw, h))
    nh = int(w / ratio)
    y = (h - nh) // 2
    return img.crop((0, y, w, y + nh))


def make_capsule(w: int, h: int, tagline: str, out: str, seed: int = 42):
    sky = _sky_gradient(w, h)
    terr = _terrain(w, h, seed)
    sky.paste(terr, (0, h - terr.height))
    # vignette: dark edges, bright center (mask = 255 at edges, 0 in middle)
    vig_mask = Image.new("L", (w, h), 255)
    ImageDraw.Draw(vig_mask).ellipse((-w * 0.2, -h * 0.2, w * 1.2, h * 1.2), fill=0)
    vig_mask = vig_mask.filter(ImageFilter.GaussianBlur(w * 0.12))
    dark = Image.new("RGB", (w, h), (8, 10, 18))
    sky = Image.composite(dark, sky, vig_mask)

    title = _title_text(w, h, ("NV-2.0", tagline))
    sky.paste(title, (0, 0), title)
    sky.save(out)
    print(f"  {out} ({w}x{h})")


def main():
    os.makedirs(os.path.join(ROOT, "egs"), exist_ok=True)
    os.makedirs(os.path.join(ROOT, "itchio"), exist_ok=True)

    print("NV-2.0 capsule generator")
    make_capsule(3840, 2160, "Every world is a real place on Earth", out=os.path.join(ROOT, "egs", "key_art_3840x2160.png"))
    make_capsule(1024, 1024, "Voxel survival. Real NASA climate.", out=os.path.join(ROOT, "egs", "portrait_1024x1024.png"))
    make_capsule(2560, 1440, "Mine. Craft. Survive the climate.", out=os.path.join(ROOT, "egs", "landscape_2560x1440.png"))
    make_capsule(800, 450, "NV-2.0", out=os.path.join(ROOT, "egs", "details_800x450.png"))
    make_capsule(630, 500, "Voxel survival. Real NASA climate.", out=os.path.join(ROOT, "itchio", "header_630x500.png"))
    print("done")


if __name__ == "__main__":
    main()
