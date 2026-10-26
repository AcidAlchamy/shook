//! The two windows Shook ever shows: settings and the hello card. Each runs in
//! its own short-lived process, so the resident never carries a GPU context or
//! a UI toolkit while it waits for you to shake.

mod hello;
mod settings;
mod theme;
mod widgets;

use crate::win::{self, wide};
use eframe::egui;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ptr::null;
use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND, RECT};
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetSystemMetrics, GetWindowRect, HWND_TOPMOST, ICON_BIG, ICON_SMALL, IMAGE_ICON, IsIconic,
    LR_DEFAULTCOLOR, LoadImageW, SM_CXICON, SM_CXSMICON, SW_RESTORE, SWP_NOACTIVATE, SWP_NOSIZE, SendMessageW,
    SetForegroundWindow, SetWindowPos, ShowWindow, WM_SETICON,
};

const SETTINGS_TITLE: &str = "Shook Settings";

// DWM attributes (Windows 11; older systems ignore them).
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
const DWMWA_BORDER_COLOR: u32 = 34;
const DWMWA_CAPTION_COLOR: u32 = 35;
const DWMWA_TEXT_COLOR: u32 = 36;

fn icon_data() -> egui::IconData {
    egui::IconData { rgba: include_bytes!("../../res/icon-64.rgba").to_vec(), width: 64, height: 64 }
}

fn hwnd_of(cc: &eframe::CreationContext<'_>) -> Option<HWND> {
    match cc.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as HWND),
        _ => None,
    }
}

fn dwm<T>(hwnd: HWND, attr: u32, value: T) {
    unsafe { DwmSetWindowAttribute(hwnd, attr, &value as *const T as *const _, size_of::<T>() as u32) };
}

fn colorref(c: egui::Color32) -> u32 {
    c.r() as u32 | (c.g() as u32) << 8 | (c.b() as u32) << 16
}

/// Crisp multi-size icons from the exe's own resources.
fn set_icons(hwnd: HWND) {
    unsafe {
        let hinst = GetModuleHandleW(null());
        for (which, metric) in [(ICON_BIG, SM_CXICON), (ICON_SMALL, SM_CXSMICON)] {
            let px = GetSystemMetrics(metric);
            let icon = LoadImageW(hinst, 1 as *const u16, IMAGE_ICON, px, px, LR_DEFAULTCOLOR);
            if !icon.is_null() {
                SendMessageW(hwnd, WM_SETICON, which as usize, icon as isize);
            }
        }
    }
}

pub fn run_settings(welcome: bool) {
    // One settings window at a time: a second request brings the first forward.
    let name = wide("Local\\Shook.Settings");
    let _mutex = unsafe { CreateMutexW(null(), 1, name.as_ptr()) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        let title = wide(SETTINGS_TITLE);
        let existing = unsafe { FindWindowW(null(), title.as_ptr()) };
        if !existing.is_null() {
            unsafe {
                if IsIconic(existing) != 0 {
                    ShowWindow(existing, SW_RESTORE);
                }
                SetForegroundWindow(existing);
            }
        }
        return;
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(SETTINGS_TITLE)
            .with_app_id("AcidAlchamy.Shook")
            .with_inner_size([620.0, 860.0])
            .with_min_inner_size([560.0, 520.0])
            .with_icon(icon_data()),
        centered: true,
        ..Default::default()
    };
    let result = eframe::run_native(
        "Shook",
        options,
        Box::new(move |cc| {
            theme::install(&cc.egui_ctx);
            if let Some(hwnd) = hwnd_of(cc) {
                set_icons(hwnd);
                // A title bar in Shook's own ink, so the window reads as one piece.
                dwm(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, 1i32);
                dwm(hwnd, DWMWA_CAPTION_COLOR, colorref(theme::BG));
                dwm(hwnd, DWMWA_TEXT_COLOR, colorref(theme::TEXT));
                dwm(hwnd, DWMWA_BORDER_COLOR, colorref(theme::LINE));
            }
            Ok(Box::new(settings::Settings::new(welcome)))
        }),
    );
    if let Err(err) = result {
        cannot_draw(&err.to_string());
    }
}

/// Without OpenGL 2 (some virtual machines, very old drivers) egui can't draw.
/// Say so plainly and offer the settings file instead of failing silently.
fn cannot_draw(why: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IDYES, MB_ICONINFORMATION, MB_YESNO, MessageBoxW};
    let text = format!(
        "Shook is running, but its settings window can't be drawn on this PC (it needs OpenGL 2 or newer).

         You can still change every setting in settings.toml; Shook reloads it when you choose Settings again.

         Open the folder with settings.toml now?

({why})"
    );
    let (text, title) = (wide(&text), wide("Shook"));
    let answer = unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_YESNO | MB_ICONINFORMATION) };
    if answer == IDYES {
        win::open_url(&crate::config::dir().display().to_string());
        crate::ipc::send(crate::ipc::WM_SHOOK_RELOAD, 0);
    }
}

pub fn run_hello() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Shook hello")
            .with_inner_size([450.0, 196.0])
            .with_resizable(false)
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_active(false)
            .with_icon(icon_data()),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "Shook hello",
        options,
        Box::new(|cc| {
            theme::install(&cc.egui_ctx);
            let hwnd = hwnd_of(cc);
            if let Some(hwnd) = hwnd {
                dwm(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, 1i32);
                dwm(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, 2i32); // rounded
                dwm(hwnd, DWMWA_BORDER_COLOR, colorref(theme::LINE));
                place_bottom_right(hwnd);
            }
            Ok(Box::new(hello::Hello::new(hwnd)))
        }),
    );
}

/// Bottom-right of the main screen's work area, above the clock, in physical pixels.
fn place_bottom_right(hwnd: HWND) {
    let Some(m) = win::primary_monitor() else { return };
    let mut r = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut r) };
    let margin = (16.0 * m.scale()) as i32;
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    unsafe {
        SetWindowPos(hwnd, HWND_TOPMOST, m.work.right - w - margin, m.work.bottom - h - margin, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE)
    };
}
