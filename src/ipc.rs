//! How the settings window and a second launch talk to the running resident:
//! window messages to its hidden window, found by class name.

use crate::win::{some, wide};
use std::ptr::null;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, WM_APP};

pub const RESIDENT_CLASS: &str = "Shook.Resident";

/// Re-read settings.toml.
pub const WM_SHOOK_RELOAD: u32 = WM_APP + 1;
/// Find the pointer now. wParam 0 = the full find, 1 = just show the effect where it is.
pub const WM_SHOOK_FIND: u32 = WM_APP + 2;
/// Open the settings window.
pub const WM_SHOOK_SETTINGS: u32 = WM_APP + 3;
/// Quit.
pub const WM_SHOOK_QUIT: u32 = WM_APP + 4;
/// wParam 1: calibrating, report strokes but don't jump. wParam 0: back to normal.
pub const WM_SHOOK_CALIBRATE: u32 = WM_APP + 5;

pub fn resident() -> Option<HWND> {
    let class = wide(RESIDENT_CLASS);
    some(unsafe { FindWindowW(class.as_ptr(), null()) })
}

pub fn send(msg: u32, wparam: usize) -> bool {
    resident().is_some_and(|h| unsafe { PostMessageW(h, msg, wparam, 0) } != 0)
}
