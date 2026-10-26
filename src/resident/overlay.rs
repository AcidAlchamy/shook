//! The effects: click-through, always-on-top layered windows that exist only
//! while an effect plays (about one second), then hide. Nothing is drawn and
//! no timer runs the rest of the time.
//!
//! - Ripple: three rings spread out from the pointer and fade.
//! - Spotlight: the screen dims, a hole closes in on the pointer, a ring marks it.
//!
//! Both follow the pointer while they play, so moving the mouse never loses them.

use crate::config::Effect;
use crate::win::{Monitor, cursor, monitor_at, wide};
use std::ptr::{null, null_mut};
use std::time::Instant;
use windows_sys::Win32::Foundation::{HWND, POINT, RECT, SIZE};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLACK_BRUSH, BLENDFUNCTION, CombineRgn,
    CreateCompatibleDC, CreateDIBSection, CreateEllipticRgn, CreateRectRgn, DIB_RGB_COLORS, DeleteObject, GetStockObject,
    HBITMAP, HDC, HGDIOBJ, RGN_DIFF, SelectObject, SetWindowRgn,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, HWND_TOPMOST, LWA_ALPHA, RegisterClassExW, SW_HIDE, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_SHOWWINDOW, SetLayeredWindowAttributes, SetWindowPos, ShowWindow, ULW_ALPHA, UpdateLayeredWindow,
    WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

const RIPPLE_RINGS: usize = 3;
const RIPPLE_STAGGER_MS: f32 = 150.0;
const RIPPLE_RING_MS: f32 = 640.0;
const RIPPLE_RADIUS: f32 = 110.0;

const SPOT_IN_MS: f32 = 380.0;
const SPOT_HOLD_MS: f32 = 950.0;
const SPOT_OUT_MS: f32 = 320.0;
const SPOT_RADIUS: f32 = 92.0;
const SPOT_DIM: f32 = 0.62;

pub struct Fx {
    layer: Layer,
    veil: Veil,
    anim: Option<Anim>,
}

struct Anim {
    kind: Effect,
    start: Instant,
    rgb: [u8; 3],
    size: f32,
    monitor: Monitor,
}

impl Fx {
    pub fn new() -> Self {
        let hinst = unsafe { GetModuleHandleW(null()) };
        register_class(hinst, "Shook.Layer", false);
        register_class(hinst, "Shook.Veil", true);
        Self { layer: Layer::new(hinst), veil: Veil::new(hinst), anim: None }
    }

    /// Starts an effect at the pointer. Call `tick` about every 15 ms until it returns false.
    pub fn play(&mut self, kind: Effect, rgb: [u8; 3], size: f32) -> bool {
        if kind == Effect::Off {
            self.stop();
            return false;
        }
        let Some(monitor) = monitor_at(cursor()) else { return false };
        self.anim = Some(Anim { kind, start: Instant::now(), rgb, size: size.clamp(0.4, 3.0), monitor });
        self.tick()
    }

    pub fn active(&self) -> bool {
        self.anim.is_some()
    }

    /// Draws the next frame. Returns false once the effect is over.
    pub fn tick(&mut self) -> bool {
        let Some(a) = &self.anim else { return false };
        let t = a.start.elapsed().as_secs_f32() * 1000.0;
        let p = cursor();
        let scale = a.monitor.scale() * a.size;
        let alive = match a.kind {
            Effect::Ripple => {
                let total = RIPPLE_STAGGER_MS * (RIPPLE_RINGS - 1) as f32 + RIPPLE_RING_MS;
                if t < total {
                    let side = ((RIPPLE_RADIUS + 14.0) * scale * 2.0).ceil() as i32;
                    self.layer.ensure(side, side);
                    draw_ripple(self.layer.pixels(), side, t, scale, a.rgb);
                    self.layer.present(p.x - side / 2, p.y - side / 2, null_mut());
                }
                t < total
            }
            Effect::Spotlight => {
                let total = SPOT_IN_MS + SPOT_HOLD_MS + SPOT_OUT_MS;
                if t < total {
                    let r_end = SPOT_RADIUS * scale;
                    let m = &a.monitor.rect;
                    let diag = (((m.right - m.left).pow(2) + (m.bottom - m.top).pow(2)) as f32).sqrt();
                    // Close in, hold, then open a little while everything fades.
                    let (radius, dim, ring) = if t < SPOT_IN_MS {
                        let k = ease_out_cubic(t / SPOT_IN_MS);
                        (diag + (r_end - diag) * k, (t / 180.0).min(1.0), k * k)
                    } else if t < SPOT_IN_MS + SPOT_HOLD_MS {
                        (r_end, 1.0, 1.0)
                    } else {
                        let k = (t - SPOT_IN_MS - SPOT_HOLD_MS) / SPOT_OUT_MS;
                        (r_end * (1.0 + 0.35 * ease_out_cubic(k)), 1.0 - k, 1.0 - k)
                    };
                    let side = ((r_end * 1.6 + 20.0 * scale) * 2.0).ceil() as i32;
                    self.layer.ensure(side, side);
                    let shown_ring = if radius < side as f32 / 2.0 - 6.0 * scale { ring } else { 0.0 };
                    draw_spot_ring(self.layer.pixels(), side, radius, scale, a.rgb, shown_ring);
                    self.layer.present(p.x - side / 2, p.y - side / 2, null_mut());
                    self.veil.frame(&a.monitor.rect, p, radius, (dim * SPOT_DIM * 255.0) as u8, self.layer.hwnd);
                }
                t < total
            }
            Effect::Off => false,
        };
        if !alive {
            self.stop();
        }
        alive
    }

    pub fn stop(&mut self) {
        self.anim = None;
        self.layer.hide();
        self.veil.hide();
    }
}

unsafe extern "system" fn quiet_proc(h: HWND, m: u32, w: usize, l: isize) -> isize {
    unsafe { DefWindowProcW(h, m, w, l) }
}

fn register_class(hinst: *mut core::ffi::c_void, name: &str, black: bool) {
    let class = wide(name);
    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(quiet_proc),
        hInstance: hinst,
        hbrBackground: if black { unsafe { GetStockObject(BLACK_BRUSH) } } else { null_mut() },
        lpszClassName: class.as_ptr(),
        ..Default::default()
    };
    unsafe { RegisterClassExW(&wc) };
}

fn overlay_window(hinst: *mut core::ffi::c_void, class: &str) -> HWND {
    let class = wide(class);
    unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            null(),
            WS_POPUP,
            0,
            0,
            1,
            1,
            null_mut(),
            null_mut(),
            hinst,
            null(),
        )
    }
}

/// A per-pixel-alpha window with its own 32-bit buffer.
struct Layer {
    hwnd: HWND,
    dc: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u32,
    w: i32,
    h: i32,
    shown: bool,
}

impl Layer {
    fn new(hinst: *mut core::ffi::c_void) -> Self {
        Self {
            hwnd: overlay_window(hinst, "Shook.Layer"),
            dc: unsafe { CreateCompatibleDC(null_mut()) },
            bmp: null_mut(),
            old: null_mut(),
            bits: null_mut(),
            w: 0,
            h: 0,
            shown: false,
        }
    }

    fn ensure(&mut self, w: i32, h: i32) {
        if w == self.w && h == self.h && !self.bits.is_null() {
            return;
        }
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..Default::default()
            },
            ..Default::default()
        };
        unsafe {
            if !self.bmp.is_null() {
                SelectObject(self.dc, self.old);
                DeleteObject(self.bmp);
            }
            let mut bits: *mut core::ffi::c_void = null_mut();
            self.bmp = CreateDIBSection(self.dc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            self.old = SelectObject(self.dc, self.bmp);
            self.bits = bits.cast();
        }
        self.w = w;
        self.h = h;
    }

    fn pixels(&mut self) -> &mut [u32] {
        if self.bits.is_null() {
            return &mut [];
        }
        unsafe { std::slice::from_raw_parts_mut(self.bits, (self.w * self.h) as usize) }
    }

    fn present(&mut self, x: i32, y: i32, _below: HWND) {
        let pos = POINT { x, y };
        let size = SIZE { cx: self.w, cy: self.h };
        let src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        unsafe {
            UpdateLayeredWindow(self.hwnd, null_mut(), &pos, &size, self.dc, &src, 0, &blend, ULW_ALPHA);
            let show = if self.shown { 0 } else { SWP_SHOWWINDOW };
            SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | show);
        }
        self.shown = true;
    }

    fn hide(&mut self) {
        if self.shown {
            unsafe { ShowWindow(self.hwnd, SW_HIDE) };
            self.shown = false;
        }
        // Give the memory back: the next effect may be a while.
        if !self.bmp.is_null() {
            unsafe {
                SelectObject(self.dc, self.old);
                DeleteObject(self.bmp);
            }
            self.bmp = null_mut();
            self.bits = null_mut();
            self.w = 0;
            self.h = 0;
        }
    }
}

/// The spotlight's dim layer: a black window with one alpha for the whole
/// thing and a round hole cut out of its region. No pixels to push, so it stays
/// cheap even on a 4K screen.
struct Veil {
    hwnd: HWND,
    shown: bool,
}

impl Veil {
    fn new(hinst: *mut core::ffi::c_void) -> Self {
        Self { hwnd: overlay_window(hinst, "Shook.Veil"), shown: false }
    }

    fn frame(&mut self, m: &RECT, hole: POINT, radius: f32, alpha: u8, below: HWND) {
        let (w, h) = (m.right - m.left, m.bottom - m.top);
        let (cx, cy, r) = (hole.x - m.left, hole.y - m.top, radius.round() as i32);
        unsafe {
            SetLayeredWindowAttributes(self.hwnd, 0, alpha, LWA_ALPHA);
            let region = CreateRectRgn(0, 0, w, h);
            let cut = CreateEllipticRgn(cx - r, cy - r, cx + r + 1, cy + r + 1);
            CombineRgn(region, region, cut, RGN_DIFF);
            DeleteObject(cut);
            SetWindowRgn(self.hwnd, region, 1); // the window owns `region` now
            let show = if self.shown { 0 } else { SWP_SHOWWINDOW };
            SetWindowPos(self.hwnd, below, m.left, m.top, w, h, SWP_NOACTIVATE | show);
        }
        self.shown = true;
    }

    fn hide(&mut self) {
        if self.shown {
            unsafe {
                ShowWindow(self.hwnd, SW_HIDE);
                SetWindowRgn(self.hwnd, null_mut(), 0);
            }
            self.shown = false;
        }
    }
}

// ---- drawing ---------------------------------------------------------------

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// Coverage of a ring of half-width `half` around `center`, 1 px anti-aliased.
#[inline]
fn band(d: f32, center: f32, half: f32) -> f32 {
    (half + 0.5 - (d - center).abs()).clamp(0.0, 1.0)
}

/// Premultiplied colour + alpha, composited with "over".
#[derive(Clone, Copy, Default)]
struct Px {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl Px {
    #[inline]
    fn over(&mut self, rgb: [f32; 3], alpha: f32) {
        if alpha <= 0.0 {
            return;
        }
        let k = 1.0 - alpha;
        self.r = rgb[0] * alpha + self.r * k;
        self.g = rgb[1] * alpha + self.g * k;
        self.b = rgb[2] * alpha + self.b * k;
        self.a = alpha + self.a * k;
    }

    #[inline]
    fn pack(self) -> u32 {
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
        (c(self.a) << 24) | (c(self.r) << 16) | (c(self.g) << 8) | c(self.b)
    }
}

fn unit(rgb: [u8; 3]) -> [f32; 3] {
    [rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0]
}

const SHADOW: [f32; 3] = [0.03, 0.02, 0.06];

fn draw_ripple(px: &mut [u32], side: i32, t: f32, scale: f32, rgb: [u8; 3]) {
    let color = unit(rgb);
    let c = side as f32 / 2.0;
    let r_max = RIPPLE_RADIUS * scale;

    // (radius, half width, alpha) of each ring at time t.
    let mut rings = [(0.0f32, 0.0f32, 0.0f32); RIPPLE_RINGS];
    for (i, ring) in rings.iter_mut().enumerate() {
        let local = t - i as f32 * RIPPLE_STAGGER_MS;
        if (0.0..RIPPLE_RING_MS).contains(&local) {
            let p = local / RIPPLE_RING_MS;
            let r = 8.0 * scale + (r_max - 8.0 * scale) * ease_out_cubic(p);
            *ring = (r, (3.6 - 2.4 * p) * scale, (1.0 - p).powf(1.5));
        }
    }
    // A soft glow under the pointer for the first half second.
    let glow_r = 30.0 * scale;
    let glow = if t < 90.0 { t / 90.0 } else { (1.0 - (t - 90.0) / 420.0).max(0.0) } * 0.38;

    for y in 0..side {
        let dy = y as f32 + 0.5 - c;
        let row = &mut px[(y * side) as usize..((y + 1) * side) as usize];
        for (x, out) in row.iter_mut().enumerate() {
            let dx = x as f32 + 0.5 - c;
            let d = (dx * dx + dy * dy).sqrt();
            let mut p = Px::default();
            if glow > 0.0 && d < glow_r {
                let k = 1.0 - d / glow_r;
                p.over(color, glow * k * k);
            }
            for &(r, half, a) in &rings {
                if a <= 0.0 {
                    continue;
                }
                // A thin dark edge outside each ring keeps it readable on white.
                p.over(SHADOW, band(d, r + half + 1.4 * scale, 1.2 * scale) * a * 0.35);
                p.over(color, band(d, r, half) * a);
            }
            *out = p.pack();
        }
    }
}

fn draw_spot_ring(px: &mut [u32], side: i32, radius: f32, scale: f32, rgb: [u8; 3], alpha: f32) {
    let color = unit(rgb);
    let c = side as f32 / 2.0;
    let half = 2.6 * scale;
    for y in 0..side {
        let dy = y as f32 + 0.5 - c;
        let row = &mut px[(y * side) as usize..((y + 1) * side) as usize];
        for (x, out) in row.iter_mut().enumerate() {
            let dx = x as f32 + 0.5 - c;
            let d = (dx * dx + dy * dy).sqrt();
            let mut p = Px::default();
            if alpha > 0.0 && (d - radius).abs() < 12.0 * scale {
                // Outer glow, then the crisp ring that also hides the hole's stair-stepped edge.
                p.over(color, band(d, radius + half + 3.0 * scale, 3.0 * scale) * alpha * 0.25);
                p.over(color, band(d, radius, half) * alpha);
            }
            *out = p.pack();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ripple_draws_premultiplied_pixels() {
        let side = 240;
        let mut px = vec![0u32; side * side];
        draw_ripple(&mut px, side as i32, 200.0, 1.0, [0xC6, 0xFF, 0x3A]);
        assert!(px.iter().any(|&p| p >> 24 > 200), "some ring pixels are near opaque");
        for &p in &px {
            let a = p >> 24;
            for shift in [0, 8, 16] {
                assert!((p >> shift) & 0xFF <= a, "colour never exceeds alpha (premultiplied)");
            }
        }
        // Corners stay clear.
        assert_eq!(px[0], 0);
    }

    /// Writes raw premultiplied BGRA frames for the README art:
    /// `SHOOK_FRAMES=<dir> cargo test dump_frames -- --ignored`
    #[test]
    #[ignore]
    fn dump_frames() {
        let Some(dir) = std::env::var_os("SHOOK_FRAMES") else { return };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let scale = 1.5;
        let side = ((RIPPLE_RADIUS + 14.0) * scale * 2.0).ceil() as i32;
        let every_frame = std::env::var_os("SHOOK_FRAMES_ALL").is_some();
        let times: Vec<u32> = if every_frame { (0..=1000).step_by(16).collect() } else { vec![40, 120, 220, 340, 480, 640, 820] };
        for t in times {
            let mut px = vec![0u32; (side * side) as usize];
            draw_ripple(&mut px, side, t as f32, scale, [0xC6, 0xFF, 0x3A]);
            let bytes: Vec<u8> = px.iter().flat_map(|p| p.to_le_bytes()).collect();
            std::fs::write(dir.join(format!("ripple-{side}-{t}.bgra")), bytes).unwrap();
        }
        let r_end = SPOT_RADIUS * scale;
        let side = ((r_end * 1.6 + 20.0 * scale) * 2.0).ceil() as i32;
        let mut px = vec![0u32; (side * side) as usize];
        draw_spot_ring(&mut px, side, r_end, scale, [0xC6, 0xFF, 0x3A], 1.0);
        let bytes: Vec<u8> = px.iter().flat_map(|p| p.to_le_bytes()).collect();
        std::fs::write(dir.join(format!("spot-{side}.bgra")), bytes).unwrap();
    }
}
