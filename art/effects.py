"""Composites the effect frames dumped by `cargo test dump_frames -- --ignored`
(SHOOK_FRAMES=art/build/frames) into README images, with a real-size cursor.

    python art/effects.py
"""
import pathlib
from PIL import Image, ImageDraw

ROOT = pathlib.Path(__file__).resolve().parent
FRAMES = ROOT / "build" / "frames"
ARROW = [(0, 0), (0, 16), (4.5, 12), (7.5, 19), (10, 18), (7, 11.2), (12.5, 11.2)]


def frame(path: pathlib.Path) -> Image.Image:
    side = int(path.stem.split("-")[1])
    raw = path.read_bytes()
    # Premultiplied BGRA -> straight RGBA.
    img = Image.frombuffer("RGBA", (side, side), raw, "raw", "BGRA", 0, 1)
    px = img.load()
    out = Image.new("RGBA", img.size)
    po = out.load()
    for y in range(side):
        for x in range(side):
            r, g, b, a = px[x, y]
            if a:
                po[x, y] = (min(255, r * 255 // a), min(255, g * 255 // a), min(255, b * 255 // a), a)
    return out


def cursor(img: Image.Image, at, scale=1.5):
    d = ImageDraw.Draw(img)
    pts = [(at[0] + x * scale * 1.3, at[1] + y * scale * 1.3) for x, y in ARROW]
    d.polygon(pts, fill=(255, 255, 255, 255), outline=(20, 10, 42, 255), width=2)


def desktop(w, h, light: bool) -> Image.Image:
    bg = Image.new("RGBA", (w, h), (238, 239, 244, 255) if light else (24, 26, 34, 255))
    d = ImageDraw.Draw(bg)
    line = (210, 212, 222, 255) if light else (44, 47, 60, 255)
    for y in range(18, h, 26):
        d.rounded_rectangle((24, y, w - 24 - (y * 37 % 160), y + 9), 4, fill=line)
    return bg


def ripple_strip():
    times = [40, 120, 220, 340, 480]
    frames = [frame(next(FRAMES.glob(f"ripple-*-{t}.bgra"))) for t in times]
    side = frames[0].width
    tile = side - 40
    out = Image.new("RGBA", (tile * len(times), tile * 2), (0, 0, 0, 0))
    for row, light in enumerate([False, True]):
        for i, f in enumerate(frames):
            bg = desktop(tile, tile, light)
            bg.alpha_composite(f, (-20, -20))
            cursor(bg, (tile // 2, tile // 2))
            out.alpha_composite(bg, (i * tile, row * tile))
    out.save(ROOT / "ripple-frames.png")


def spotlight():
    ring = frame(next(FRAMES.glob("spot-*.bgra")))
    w, h = 900, 420
    bg = desktop(w, h, True)
    veil = Image.new("RGBA", (w, h), (0, 0, 0, int(0.62 * 255)))
    cx, cy = w // 2, h // 2
    r = 92 * 1.5
    mask = Image.new("L", (w, h), 255)
    ImageDraw.Draw(mask).ellipse((cx - r, cy - r, cx + r, cy + r), fill=0)
    bg.paste(veil, (0, 0), Image.composite(veil.split()[3], Image.new("L", (w, h), 0), mask))
    bg.alpha_composite(ring, (cx - ring.width // 2, cy - ring.height // 2))
    cursor(bg, (cx, cy))
    bg.save(ROOT / "spotlight-frame.png")


def mock_desktop(w, h, light: bool) -> Image.Image:
    """A wallpaper with two app windows, so the effects have something to sit on."""
    top, bottom = ((214, 222, 236), (238, 232, 246)) if light else ((22, 24, 40), (52, 28, 72))
    bg = Image.new("RGBA", (w, h))
    d = ImageDraw.Draw(bg)
    for y in range(h):
        k = y / h
        d.line([(0, y), (w, y)], fill=tuple(int(a + (b - a) * k) for a, b in zip(top, bottom)) + (255,))
    win_fill, bar, line = ((250, 250, 252), (232, 233, 240), (214, 216, 226)) if light else ((30, 32, 44), (40, 42, 58), (56, 59, 78))
    for (x0, y0, x1, y1) in [(28, 34, w * 0.62, h - 46), (w * 0.46, 70, w - 26, h - 22)]:
        d.rounded_rectangle((x0, y0, x1, y1), 12, fill=win_fill)
        d.rounded_rectangle((x0, y0, x1, y0 + 30), 12, fill=bar)
        d.rectangle((x0, y0 + 18, x1, y0 + 30), fill=bar)
        for i, c in enumerate([(255, 95, 86), (255, 189, 46), (39, 201, 63)]):
            d.ellipse((x0 + 12 + i * 16, y0 + 10, x0 + 22 + i * 16, y0 + 20), fill=c)
        for y in range(int(y0) + 46, int(y1) - 16, 22):
            d.rounded_rectangle((x0 + 18, y, x0 + 18 + (x1 - x0 - 36) * (0.55 + 0.4 * ((y * 7) % 10) / 10), y + 9), 4, fill=line)
    return bg


def label(img, text, light):
    from PIL import ImageFont
    font = ImageFont.truetype("C:/Windows/Fonts/seguisb.ttf", 22)
    d = ImageDraw.Draw(img)
    w = d.textlength(text, font=font)
    d.rounded_rectangle((18, 16, 18 + w + 28, 16 + 38), 19, fill=(13, 11, 20, 230))
    d.text((32, 20), text, font=font, fill=(198, 255, 58, 255))


def panel():
    """Ripple and spotlight side by side, 1200 x 520 (the site's "A look inside")."""
    w, h, gap = 590, 520, 20
    out = Image.new("RGBA", (w * 2 + gap, h), (0, 0, 0, 0))
    # Ripple, on a dark desktop.
    left = mock_desktop(w, h, light=False)
    f = frame(next(FRAMES.glob("ripple-*-220.bgra")))
    cx, cy = w // 2 - 20, h // 2 + 10
    left.alpha_composite(f, (cx - f.width // 2, cy - f.height // 2))
    cursor(left, (cx, cy))
    label(left, "Ripple", False)
    # Spotlight, on a light desktop.
    right = mock_desktop(w, h, light=True)
    ring = frame(next(FRAMES.glob("spot-*.bgra")))
    cx, cy = w // 2 + 30, h // 2 + 20
    r = 92 * 1.5
    veil = Image.new("RGBA", (w, h), (0, 0, 0, int(0.62 * 255)))
    hole = Image.new("L", (w, h), 255)
    ImageDraw.Draw(hole).ellipse((cx - r, cy - r, cx + r, cy + r), fill=0)
    right.paste(veil, (0, 0), Image.composite(veil.split()[3], Image.new("L", (w, h), 0), hole))
    right.alpha_composite(ring, (cx - ring.width // 2, cy - ring.height // 2))
    cursor(right, (cx, cy))
    label(right, "Spotlight", True)
    mask = Image.new("L", (w, h), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, w - 1, h - 1), 16, fill=255)
    out.paste(left, (0, 0), mask)
    out.paste(right, (w + gap, 0), mask)
    out.save(ROOT / "effects-panel.png", optimize=True)


if __name__ == "__main__":
    ripple_strip()
    spotlight()
    panel()
    print("effect images written")
