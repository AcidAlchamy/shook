"""Renders Shook's icon from SVG at every size (crisp, not downscaled) and packs
res/shook.ico and res/shook-paused.ico. Needs Microsoft Edge and Pillow.

    python art/icons.py
"""
import pathlib, subprocess, tempfile
from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
EDGE = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
SIZES = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256]

# The classic arrow pointer in a 12.5 x 19 box.
ARROW = "M0 0 L0 16 L4.5 12 L7.5 19 L10 18 L7 11.2 L12.5 11.2 Z"

def svg(px: int, paused: bool) -> str:
    small = px <= 24
    top, bottom = ("#5B5B6E", "#2E2E3A") if paused else ("#8B3DFF", "#FF2E7E")
    cursor = "#D9D9E3" if paused else "#FFFFFF"
    arcs = ""
    if not paused:
        if small:
            arcs = """<g fill="none" stroke="#C6FF3A" stroke-width="24" stroke-linecap="round">
              <path d="M58 84 Q34 128 58 172"/><path d="M198 84 Q222 128 198 172"/></g>"""
        else:
            arcs = """<g fill="none" stroke="#C6FF3A" stroke-linecap="round">
              <path stroke-width="14" d="M64 94 Q46 128 64 162"/>
              <path stroke-width="14" d="M192 94 Q210 128 192 162"/>
              <path stroke-width="11" opacity=".75" d="M40 76 Q16 128 40 180"/>
              <path stroke-width="11" opacity=".75" d="M216 76 Q240 128 216 180"/></g>"""
    scale, outline = (8.0, 15) if small else (6.2, 9)
    tx, ty = 128 - 6.25 * scale, 128 - 9.5 * scale
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="{px}" height="{px}">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/></linearGradient>
    <radialGradient id="shine" cx=".28" cy=".18" r=".85"><stop offset="0" stop-color="#fff" stop-opacity=".32"/><stop offset=".6" stop-color="#fff" stop-opacity="0"/></radialGradient>
  </defs>
  <rect x="6" y="6" width="244" height="244" rx="{50 if small else 58}" fill="url(#bg)"/>
  <rect x="6" y="6" width="244" height="244" rx="{50 if small else 58}" fill="url(#shine)"/>
  {arcs}
  <g transform="translate({tx:.1f} {ty:.1f}) rotate(-6 {6.25*scale:.1f} {9.5*scale:.1f}) scale({scale})">
    <path d="{ARROW}" fill="{cursor}" stroke="#140A2A" stroke-width="{outline/scale:.3f}" stroke-linejoin="round"/>
  </g>
</svg>"""

def render(markup: str, px: int, out: pathlib.Path) -> Image.Image:
    with tempfile.TemporaryDirectory() as tmp:
        page = pathlib.Path(tmp) / "icon.html"
        page.write_text(f'<html><body style="margin:0;background:transparent">{markup}</body></html>', encoding="utf-8")
        shot = pathlib.Path(tmp) / "shot.png"
        subprocess.run([EDGE, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1",
                        "--default-background-color=00000000", "--window-size=300,300",
                        f"--screenshot={shot}", page.as_uri()], check=True, capture_output=True, timeout=60)
        img = Image.open(shot).convert("RGBA").crop((0, 0, px, px))
        img.load()
    img.save(out)
    return img

def main():
    out = ROOT / "art" / "build"
    out.mkdir(parents=True, exist_ok=True)
    for paused, name in [(False, "shook"), (True, "shook-paused")]:
        imgs = [render(svg(s, paused), s, out / f"{name}-{s}.png") for s in SIZES]
        imgs[-1].save(ROOT / "res" / f"{name}.ico", sizes=[(s, s) for s in SIZES], append_images=imgs[:-1])
        (out / f"{name}.svg").write_text(svg(256, paused), encoding="utf-8")
    render(svg(512, False), 512, ROOT / "art" / "shook-icon-512.png")
    # Raw RGBA for the settings window's title-bar icon (no image decoder needed at runtime).
    (ROOT / "res" / "icon-64.rgba").write_bytes(Image.open(out / "shook-64.png").convert("RGBA").tobytes())
    print("icons written")

if __name__ == "__main__":
    main()
