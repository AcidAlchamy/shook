//! Scale-test builds only: saves what egui really drew to SHOOK_TEST_SHOT as a BMP, twice a second. A screen capture
//! can't be trusted here: it sees OpenGL windows as blank on a PC whose panel isn't showing.

use eframe::egui;
use std::sync::atomic::{AtomicU32, Ordering};

static FRAMES: AtomicU32 = AtomicU32::new(0);

pub fn frame(ctx: &egui::Context) {
    let Ok(path) = std::env::var("SHOOK_TEST_SHOT") else { return };
    ctx.request_repaint();
    if FRAMES.fetch_add(1, Ordering::Relaxed) % 30 == 29 {
        ctx.request_screenshot(move |image| save(&path, &image));
    }
}

fn save(path: &str, image: &egui::ColorImage) {
    let [w, h] = image.size;
    let mut bytes = vec![0u8; 54];
    bytes[..2].copy_from_slice(b"BM");
    bytes[2..6].copy_from_slice(&((54 + w * h * 4) as u32).to_le_bytes());
    bytes[10] = 54;
    bytes[14] = 40;
    bytes[18..22].copy_from_slice(&(w as i32).to_le_bytes());
    bytes[22..26].copy_from_slice(&(-(h as i32)).to_le_bytes());
    bytes[26] = 1;
    bytes[28] = 32;
    for c in &image.pixels {
        bytes.extend_from_slice(&[c.b(), c.g(), c.r(), 255]);
    }
    // Written beside the target and moved, so a reader never sees half a frame.
    let tmp = format!("{path}.tmp");
    if std::fs::write(&tmp, bytes).is_ok() {
        let _ = std::fs::rename(tmp, path);
    }
}
