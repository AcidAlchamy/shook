//! Small Win32 helpers shared by the resident and the settings window.

use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY,
    MONITORINFO, MONITORINFOEXW, MonitorFromPoint,
};
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, GetCursorPos, SW_SHOWNORMAL};

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub fn from_wide(w: &[u16]) -> String {
    let end = w.iter().position(|&c| c == 0).unwrap_or(w.len());
    String::from_utf16_lossy(&w[..end])
}

pub fn open_url(url: &str) {
    let (verb, url) = (wide("open"), wide(url));
    unsafe { ShellExecuteW(null_mut(), verb.as_ptr(), url.as_ptr(), null(), null(), SW_SHOWNORMAL) };
}

/// Starts another copy of this exe with `args` (the settings window, the hello card).
pub fn spawn_self(args: &[&str]) {
    if let Ok(exe) = std::env::current_exe() {
        // Let the new window come to the front: we were the one the user clicked.
        unsafe { AllowSetForegroundWindow(u32::MAX) };
        let _ = std::process::Command::new(exe).args(args).spawn();
    }
}

pub fn cursor() -> POINT {
    let mut p = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut p) };
    p
}

#[derive(Clone)]
pub struct Monitor {
    pub handle: HMONITOR,
    /// `\\.\DISPLAY1` and so on.
    pub name: String,
    pub rect: RECT,
    pub work: RECT,
    pub primary: bool,
}

impl Monitor {
    pub fn center(&self) -> POINT {
        POINT { x: (self.work.left + self.work.right) / 2, y: (self.work.top + self.work.bottom) / 2 }
    }

    /// Effective DPI scale (1.0 = 96 dpi).
    pub fn scale(&self) -> f32 {
        if let Some(scale) = test_scale() {
            return scale;
        }
        let (mut x, mut y) = (96u32, 96u32);
        unsafe { GetDpiForMonitor(self.handle, MDT_EFFECTIVE_DPI, &mut x, &mut y) };
        x.max(96) as f32 / 96.0
    }
}

/// A scale-test build's stand-in for Windows' display scale.
pub fn test_scale() -> Option<f32> {
    if !cfg!(feature = "scale-test") {
        return None;
    }
    std::env::var("SHOOK_TEST_SCALE").ok()?.parse().ok()
}

pub fn monitor_info(handle: HMONITOR) -> Option<Monitor> {
    let mut mi: MONITORINFOEXW = unsafe { std::mem::zeroed() };
    mi.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    if unsafe { GetMonitorInfoW(handle, &mut mi as *mut _ as *mut MONITORINFO) } == 0 {
        return None;
    }
    Some(Monitor {
        handle,
        name: from_wide(&mi.szDevice),
        rect: mi.monitorInfo.rcMonitor,
        work: mi.monitorInfo.rcWork,
        primary: mi.monitorInfo.dwFlags & 1 != 0,
    })
}

pub fn monitor_at(p: POINT) -> Option<Monitor> {
    monitor_info(unsafe { MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST) })
}

pub fn primary_monitor() -> Option<Monitor> {
    monitor_info(unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) })
}

/// Every display, left to right.
pub fn monitors() -> Vec<Monitor> {
    unsafe extern "system" fn each(m: HMONITOR, _: HDC, _: *mut RECT, data: isize) -> i32 {
        let list = unsafe { &mut *(data as *mut Vec<Monitor>) };
        if let Some(info) = monitor_info(m) {
            list.push(info);
        }
        1
    }
    let mut list: Vec<Monitor> = Vec::new();
    unsafe { EnumDisplayMonitors(null_mut(), null(), Some(each), &mut list as *mut _ as isize) };
    list.sort_by_key(|m| (m.rect.left, m.rect.top));
    list
}

/// The HWND of a window or null, as an Option.
pub fn some(h: HWND) -> Option<HWND> {
    (!h.is_null()).then_some(h)
}
