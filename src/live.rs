//! A few bytes of shared memory between the resident and the settings window.
//!
//! The resident writes every finished stroke here; the settings window reads it
//! to draw the live shake meter and to calibrate. Plain atomics, no locks, no
//! messages: writing costs the resident nothing while nobody is watching.

use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Memory::{CreateFileMappingW, FILE_MAP_ALL_ACCESS, MapViewOfFile, PAGE_READWRITE};

const RING: usize = 32;
const COUNTS_BIT: u32 = 1 << 31;

#[repr(C)]
pub struct Live {
    stroke_seq: AtomicU32,
    strokes: [AtomicU32; RING],
    run: AtomicU32,
    shakes: AtomicU32,
    hotkey: AtomicU32,
}

/// What became of the hotkey the user asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyState {
    Off,
    Ready,
    /// Another program already owns that key combination.
    Taken,
    /// The text couldn't be understood.
    Invalid,
}

impl Live {
    pub fn push_stroke(&self, speed: f32, counts: bool) {
        let seq = self.stroke_seq.load(Relaxed).wrapping_add(1);
        let packed = (speed.clamp(0.0, 1e9) as u32 & !COUNTS_BIT) | if counts { COUNTS_BIT } else { 0 };
        self.strokes[seq as usize % RING].store(packed, Relaxed);
        self.stroke_seq.store(seq, Relaxed);
    }

    /// Strokes finished since `*seen` (oldest first, at most the ring size).
    pub fn strokes_since(&self, seen: &mut u32) -> Vec<(f32, bool)> {
        let now = self.stroke_seq.load(Relaxed);
        let new = now.wrapping_sub(*seen).min(RING as u32);
        let out = (0..new)
            .map(|i| {
                let p = self.strokes[(now.wrapping_sub(new - 1 - i)) as usize % RING].load(Relaxed);
                ((p & !COUNTS_BIT) as f32, p & COUNTS_BIT != 0)
            })
            .collect();
        *seen = now;
        out
    }

    pub fn stroke_seq(&self) -> u32 {
        self.stroke_seq.load(Relaxed)
    }

    pub fn set_run(&self, run: u32) {
        self.run.store(run, Relaxed);
    }

    pub fn run(&self) -> u32 {
        self.run.load(Relaxed)
    }

    pub fn add_shake(&self) {
        self.shakes.fetch_add(1, Relaxed);
    }

    pub fn shakes(&self) -> u32 {
        self.shakes.load(Relaxed)
    }

    pub fn set_hotkey(&self, s: HotkeyState) {
        self.hotkey.store(s as u32, Relaxed);
    }

    pub fn hotkey(&self) -> HotkeyState {
        match self.hotkey.load(Relaxed) {
            1 => HotkeyState::Ready,
            2 => HotkeyState::Taken,
            3 => HotkeyState::Invalid,
            _ => HotkeyState::Off,
        }
    }
}

/// Opens (or creates) the shared block. Lives for the rest of the process.
pub fn open() -> Option<&'static Live> {
    let name = crate::win::wide("Local\\Shook.Live.v1");
    unsafe {
        let map = CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            std::ptr::null(),
            PAGE_READWRITE,
            0,
            size_of::<Live>() as u32,
            name.as_ptr(),
        );
        if map.is_null() {
            return None;
        }
        let view = MapViewOfFile(map, FILE_MAP_ALL_ACCESS, 0, 0, size_of::<Live>());
        // A fresh mapping is zeroed, which is a valid `Live`.
        (view.Value as *const Live).as_ref()
    }
}
