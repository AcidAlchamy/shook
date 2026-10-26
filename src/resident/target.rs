//! Where the pointer lands, and when a shake should be left alone.

use crate::config::{Config, Landing};
use crate::win::{cursor, monitor_at, monitors, primary_monitor};
use windows_sys::Win32::Foundation::{CloseHandle, HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows_sys::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromPoint};
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON};
use windows_sys::Win32::UI::Shell::{
    QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
};

/// The point to move the pointer to, or None to leave it where it is.
pub fn resolve(cfg: &Config) -> Option<POINT> {
    match cfg.landing {
        Landing::StayPut => None,
        Landing::ScreenCenter => screen_center(&cfg.screen),
        Landing::WindowCenter => window_center().or_else(|| screen_center("pointer")),
        Landing::Spot => cfg
            .spot
            .map(|[x, y]| POINT { x, y })
            .filter(|&p| !unsafe { MonitorFromPoint(p, MONITOR_DEFAULTTONULL) }.is_null())
            .or_else(|| screen_center("primary")),
    }
}

fn screen_center(which: &str) -> Option<POINT> {
    let m = match which {
        "pointer" => monitor_at(cursor()),
        "primary" => primary_monitor(),
        name => monitors().into_iter().find(|m| m.name.eq_ignore_ascii_case(name)).or_else(primary_monitor),
    };
    m.map(|m| m.center())
}

fn window_center() -> Option<POINT> {
    let fg = foreground()?;
    if unsafe { IsIconic(fg) } != 0 {
        return None;
    }
    let r = window_rect(fg)?;
    Some(POINT { x: (r.left + r.right) / 2, y: (r.top + r.bottom) / 2 })
}

/// The window in front, unless it's the desktop or the taskbar.
fn foreground() -> Option<HWND> {
    let fg = unsafe { GetForegroundWindow() };
    if fg.is_null() || unsafe { IsWindowVisible(fg) } == 0 {
        return None;
    }
    let mut class = [0u16; 64];
    let n = unsafe { GetClassNameW(fg, class.as_mut_ptr(), class.len() as i32) } as usize;
    let class = String::from_utf16_lossy(&class[..n]);
    let shell = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"];
    (!shell.contains(&class.as_str())).then_some(fg)
}

fn window_rect(h: HWND) -> Option<RECT> {
    let mut r = RECT::default();
    // The visible frame, without the invisible resize borders.
    let ok = unsafe {
        DwmGetWindowAttribute(h, DWMWA_EXTENDED_FRAME_BOUNDS as u32, &mut r as *mut _ as *mut _, size_of::<RECT>() as u32)
    } == 0;
    (ok || unsafe { GetWindowRect(h, &mut r) } != 0).then_some(r)
}

pub fn buttons_held() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON].iter().any(|&vk| unsafe { GetAsyncKeyState(vk as i32) } as u16 & 0x8000 != 0)
}

/// A game, video or slideshow is full screen.
pub fn fullscreen_in_front() -> bool {
    let mut state = 0;
    if unsafe { SHQueryUserNotificationState(&mut state) } == 0
        && matches!(state, QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE)
    {
        return true;
    }
    // Borderless "full screen windowed" games don't always say so.
    let Some(fg) = foreground() else { return false };
    let (Some(r), Some(m)) = (window_rect(fg), monitor_at(cursor())) else { return false };
    r.left <= m.rect.left && r.top <= m.rect.top && r.right >= m.rect.right && r.bottom >= m.rect.bottom
}

/// File name of the program in front, lower case ("photoshop.exe").
pub fn foreground_exe() -> Option<String> {
    let fg = foreground()?;
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(fg, &mut pid) };
    let proc = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if proc.is_null() {
        return None;
    }
    let mut buf = [0u16; 512];
    let mut len = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(proc, 0, buf.as_mut_ptr(), &mut len) } != 0;
    unsafe { CloseHandle(proc) };
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    ok.then(|| path.rsplit('\\').next().unwrap_or_default().to_ascii_lowercase())
}
