"""A short demo clip (1280x720, 60 fps): the pointer is lost in a corner, the mouse shakes,
the pointer lands in the middle and the ripple plays. The ripple frames come from Shook's own
renderer (`SHOOK_FRAMES=art/build/frames-all SHOOK_FRAMES_ALL=1 cargo test dump_frames -- --ignored`).

    python art/clip.py   ->  art/build/shook-demo.mp4  (needs ffmpeg)
"""
import math, pathlib, subprocess, shutil
from PIL import Image, ImageDraw, ImageFont
from effects import ROOT, cursor, frame, mock_desktop

FRAMES = ROOT / "build" / "frames-all"
OUT = ROOT / "build" / "clip"
W, H, FPS = 1280, 720, 60
FONT = ImageFont.truetype("C:/Windows/Fonts/segoeuib.ttf", 34)
SMALL = ImageFont.truetype("C:/Windows/Fonts/seguisb.ttf", 22)


def caption(img, text, sub=None):
    d = ImageDraw.Draw(img)
    w = d.textlength(text, font=FONT)
    x, y = (W - w) / 2, H - 92
    d.rounded_rectangle((x - 22, y - 10, x + w + 22, y + 50), 30, fill=(13, 11, 20, 235))
    d.text((x, y), text, font=FONT, fill=(198, 255, 58, 255))
    if sub:
        sw = d.textlength(sub, font=SMALL)
        d.text(((W - sw) / 2, y + 60), sub, font=SMALL, fill=(238, 236, 244, 255))


def main():
    shutil.rmtree(OUT, ignore_errors=True)
    OUT.mkdir(parents=True)
    ripples = {int(p.stem.split("-")[2]): frame(p) for p in FRAMES.glob("ripple-*.bgra")}
    times = sorted(ripples)
    base = mock_desktop(W, H, light=False)
    lost, center = (1150, 640), (W // 2, H // 2)
    n = 0

    def save(img):
        nonlocal n
        img.convert("RGB").save(OUT / f"f{n:04d}.png")
        n += 1

    # 0.6 s: the pointer sits in a corner, easy to miss.
    for _ in range(int(0.6 * FPS)):
        img = base.copy(); cursor(img, lost, 1.3); caption(img, "Where did it go?"); save(img)
    # 0.7 s: shake. Faint ghosts trail the pointer.
    shake_frames = int(0.7 * FPS)
    for i in range(shake_frames):
        img = base.copy()
        for lag in (3, 2, 1):
            t = max(0, i - lag * 2) / FPS
            ghost = Image.new("RGBA", (W, H))
            cursor(ghost, (lost[0] - 40 + 60 * math.sin(t * 2 * math.pi * 5), lost[1]), 1.3)
            ghost.putalpha(ghost.getchannel("A").point(lambda a: a * (0.12 * (4 - lag)) // 1))
            img.alpha_composite(ghost)
        cursor(img, (lost[0] - 40 + 60 * math.sin(i / FPS * 2 * math.pi * 5), lost[1]), 1.3)
        caption(img, "Shake the mouse"); save(img)
    # 1.4 s: it lands in the middle and the ripple plays (Shook's real frames).
    for i in range(int(1.4 * FPS)):
        ms = i * 1000 / FPS
        img = base.copy()
        t = min(times, key=lambda x: abs(x - ms))
        if ms <= times[-1] + 8:
            f = ripples[t]
            img.alpha_composite(f, (center[0] - f.width // 2, center[1] - f.height // 2))
        cursor(img, center, 1.3)
        caption(img, "Found it.", "Shook  ·  free donationware for Windows")
        save(img)
    # 1.2 s: the box.
    box = Image.open(ROOT.parent / "docs" / "img" / "box.png").convert("RGBA")
    box.thumbnail((380, 520))
    for i in range(int(1.2 * FPS)):
        img = Image.new("RGBA", (W, H), (11, 10, 17, 255))
        img.alpha_composite(box, (W // 2 - box.width // 2 - 230, H // 2 - box.height // 2))
        d = ImageDraw.Draw(img)
        d.text((W // 2 - 20, H // 2 - 70), "Shook", font=ImageFont.truetype("C:/Windows/Fonts/segoeuib.ttf", 76), fill=(255, 255, 255, 255))
        d.text((W // 2 - 16, H // 2 + 22), "Lost your pointer? Shake it.", font=SMALL, fill=(198, 255, 58, 255))
        d.text((W // 2 - 16, H // 2 + 58), "acidalchamy.com/donationware", font=SMALL, fill=(163, 157, 182, 255))
        save(img)

    out = ROOT / "build" / "shook-demo.mp4"
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-framerate", str(FPS), "-i", str(OUT / "f%04d.png"),
                    "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18", "-movflags", "+faststart", str(out)], check=True)
    print(out, n, "frames")


if __name__ == "__main__":
    main()
