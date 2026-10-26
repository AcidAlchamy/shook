//! The resident: a hidden window, a tray icon and a raw-input listener.
//!
//! It sleeps in GetMessage until the mouse moves. Each movement is one cheap
//! WM_INPUT (a few subtractions in the shake detector); nothing polls, nothing
//! draws, and no timer runs unless an effect is on screen.

mod overlay;
mod sound;
pub mod target;
mod tray;

use crate::config::{Config, Landing};
use crate::ipc::{RESIDENT_CLASS, WM_SHOOK_CALIBRATE, WM_SHOOK_FIND, WM_SHOOK_QUIT, WM_SHOOK_RELOAD, WM_SHOOK_SETTINGS};
use crate::live::{HotkeyState, Live};
use crate::shake::Detector;
use crate::win::{open_url, spawn_self, wide};
use crate::{autostart, config, hotkey, live};
use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};
use tray::{Pick, Tray, WM_TRAY};
use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND, POINT, RECT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey};
use windows_sys::Win32::UI::Input::{
    GetRawInputData, HRAWINPUT, MOUSE_MOVE_ABSOLUTE, MOUSE_VIRTUAL_DESKTOP, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
    RID_INPUT, RIDEV_INPUTSINK, RIDEV_REMOVE, RIM_TYPEMOUSE, RegisterRawInputDevices,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    ChangeWindowMessageFilterEx, ClipCursor, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetClipCursor, GetMessageW, GetSystemMetrics, KillTimer, MSG, MSGFLT_ALLOW, PostQuitMessage, RegisterClassExW,
    RegisterWindowMessageW, SM_CXSCREEN, SM_CXVIRTUALSCREEN, SM_CYSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN, SetCursorPos, SetTimer, TranslateMessage, WM_CONTEXTMENU, WM_DESTROY, WM_HOTKEY, WM_INPUT,
    WM_TIMER, WM_USER, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_POPUP,
};

const TIMER_FRAME: usize = 1;
const HOTKEY_ID: i32 = 1;
/// After a find, ignore shaking for this long (your hand is still moving).
const COOLDOWN: Duration = Duration::from_millis(900);
/// The pointer is held on its landing spot until the mouse has been still this
/// long, so the end of the shake can't drag it away again...
const SETTLE: Duration = Duration::from_millis(140);
/// ...but never longer than this.
const HOLD_MAX: Duration = Duration::from_millis(650);
/// Calibration ends by itself if the settings window never says it's done.
const CALIBRATE_MAX: Duration = Duration::from_secs(10);

const NIN_SELECT: u32 = WM_USER;
const NIN_KEYSELECT: u32 = WM_USER + 1;
const RI_MOUSE_BUTTONS_DOWN: u16 = 0x0001 | 0x0004 | 0x0010 | 0x0040 | 0x0100;

struct Hold {
    at: POINT,
    began: Instant,
    last_move: Instant,
    prev_clip: Option<RECT>,
}

struct App {
    hwnd: HWND,
    cfg: Config,
    ignored: Vec<String>,
    detector: Detector,
    live: Option<&'static Live>,
    tray: Tray,
    fx: overlay::Fx,
    hold: Option<Hold>,
    clock: Instant,
    cooldown_until: Instant,
    calibrating_until: Option<Instant>,
    abs_last: Option<(i32, i32)>,
    taskbar_created: u32,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Runs `f` on the app unless it's already borrowed further up the stack
/// (a modal loop like the tray menu re-enters the window procedure).
fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|cell| cell.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}

pub fn run(autostarted: bool) {
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

    let name = wide("Local\\Shook.Resident");
    let _mutex = unsafe { CreateMutexW(null(), 1, name.as_ptr()) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        // Already running: a second launch opens the settings instead.
        if !autostarted {
            crate::ipc::send(WM_SHOOK_SETTINGS, 0);
        }
        return;
    }

    let first_run = !config::exists();
    let cfg = Config::load();
    if first_run {
        let _ = cfg.save();
    }
    autostart::follow_moves();

    let hwnd = create_window();
    if hwnd.is_null() {
        return;
    }
    for msg in [WM_SHOOK_RELOAD, WM_SHOOK_FIND, WM_SHOOK_SETTINGS, WM_SHOOK_QUIT, WM_SHOOK_CALIBRATE] {
        unsafe { ChangeWindowMessageFilterEx(hwnd, msg, MSGFLT_ALLOW, null_mut()) };
    }

    let tray = Tray::new(hwnd);
    let now = Instant::now();
    let mut app = App {
        hwnd,
        ignored: cfg.ignored_apps(),
        cfg,
        detector: Detector::default(),
        live: live::open(),
        tray,
        fx: overlay::Fx::new(),
        hold: None,
        clock: now,
        cooldown_until: now,
        calibrating_until: None,
        abs_last: None,
        taskbar_created: unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) },
    };
    app.apply();
    let hello = app.cfg.hello_card;
    APP.with(|cell| *cell.borrow_mut() = Some(app));

    if first_run {
        spawn_self(&["--settings", "--welcome"]);
    } else if hello {
        spawn_self(&["--hello"]);
    }

    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) } > 0 {
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    APP.with(|cell| cell.borrow_mut().take());
}

fn create_window() -> HWND {
    let hinst = unsafe { GetModuleHandleW(null()) };
    let class = wide(RESIDENT_CLASS);
    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(wndproc),
        hInstance: hinst,
        lpszClassName: class.as_ptr(),
        ..Default::default()
    };
    unsafe {
        RegisterClassExW(&wc);
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class.as_ptr(),
            wide("Shook resident").as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            hinst,
            null(),
        )
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    match msg {
        WM_INPUT => {
            with_app(|app| app.on_raw_input(lparam as HRAWINPUT));
        }
        WM_TIMER if wparam == TIMER_FRAME => {
            with_app(|app| app.on_frame());
        }
        WM_HOTKEY if wparam as i32 == HOTKEY_ID => {
            with_app(|app| app.find(false));
        }
        WM_TRAY => {
            let event = (lparam & 0xFFFF) as u32;
            match event {
                WM_CONTEXTMENU => {
                    let at = POINT { x: (wparam & 0xFFFF) as i16 as i32, y: ((wparam >> 16) & 0xFFFF) as i16 as i32 };
                    // Read what the menu needs, then let go of the app: the menu
                    // runs its own message loop.
                    let state = with_app(|app| (app.cfg.paused, app.tray));
                    if let Some((paused, tray)) = state {
                        if let Some(pick) = tray.menu(at, paused) {
                            with_app(|app| app.on_pick(pick));
                        }
                    }
                }
                NIN_SELECT | NIN_KEYSELECT => {
                    with_app(|app| app.open_settings());
                }
                _ => {}
            }
        }
        WM_SHOOK_RELOAD => {
            with_app(|app| app.reload());
        }
        WM_SHOOK_FIND => {
            with_app(|app| app.find(wparam == 1));
        }
        WM_SHOOK_SETTINGS => {
            with_app(|app| app.open_settings());
        }
        WM_SHOOK_CALIBRATE => {
            with_app(|app| {
                app.calibrating_until = (wparam == 1).then(|| Instant::now() + CALIBRATE_MAX);
                app.detector.reset();
                app.listen();
            });
        }
        WM_SHOOK_QUIT => {
            with_app(|app| app.quit());
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
        }
        _ => {
            if with_app(|app| msg == app.taskbar_created && msg != 0).unwrap_or(false) {
                with_app(|app| app.tray.add(app.cfg.paused));
            }
        }
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

impl App {
    fn now_ms(&self) -> u32 {
        self.clock.elapsed().as_millis() as u32
    }

    /// Mouse input reaches Shook only while it's needed: not at all while paused.
    fn listen(&self) {
        let on = !self.cfg.paused || self.calibrating_until.is_some();
        let device = if on {
            RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: 0x02, dwFlags: RIDEV_INPUTSINK, hwndTarget: self.hwnd }
        } else {
            RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: 0x02, dwFlags: RIDEV_REMOVE, hwndTarget: null_mut() }
        };
        unsafe { RegisterRawInputDevices(&device, 1, size_of::<RAWINPUTDEVICE>() as u32) };
    }

    fn apply(&mut self) {
        self.listen();
        self.ignored = self.cfg.ignored_apps();
        self.detector.reset();
        self.tray.set_paused(self.cfg.paused);

        unsafe { UnregisterHotKey(self.hwnd, HOTKEY_ID) };
        let state = if self.cfg.hotkey.trim().is_empty() {
            HotkeyState::Off
        } else if let Some((mods, vk)) = hotkey::parse(&self.cfg.hotkey) {
            if unsafe { RegisterHotKey(self.hwnd, HOTKEY_ID, mods, vk) } != 0 { HotkeyState::Ready } else { HotkeyState::Taken }
        } else {
            HotkeyState::Invalid
        };
        if let Some(live) = self.live {
            live.set_hotkey(state);
        }
    }

    fn reload(&mut self) {
        self.cfg = Config::load();
        self.apply();
    }

    fn on_pick(&mut self, pick: Pick) {
        match pick {
            Pick::Find => self.find(false),
            Pick::Pause => {
                self.cfg.paused = !self.cfg.paused;
                let _ = self.cfg.save();
                self.apply();
            }
            Pick::Settings => self.open_settings(),
            Pick::Donate => open_url(crate::DONATE_URL),
            Pick::Website => open_url(crate::HOME_URL),
            Pick::Quit => self.quit(),
        }
    }

    /// Opening the settings also picks up hand edits to settings.toml.
    fn open_settings(&mut self) {
        self.reload();
        spawn_self(&["--settings"]);
    }

    fn quit(&mut self) {
        self.release_hold();
        self.fx.stop();
        self.tray.remove();
        unsafe { DestroyWindow(self.hwnd) };
    }

    fn on_raw_input(&mut self, handle: HRAWINPUT) {
        let mut raw = RAWINPUT::default();
        let mut size = size_of::<RAWINPUT>() as u32;
        let got = unsafe {
            GetRawInputData(handle, RID_INPUT, &mut raw as *mut _ as *mut _, &mut size, size_of::<RAWINPUTHEADER>() as u32)
        };
        if got == u32::MAX || raw.header.dwType != RIM_TYPEMOUSE {
            return;
        }
        let m = unsafe { raw.data.mouse };
        let buttons = unsafe { m.Anonymous.Anonymous.usButtonFlags };

        let (dx, dy) = if m.usFlags & MOUSE_MOVE_ABSOLUTE as u16 != 0 {
            self.absolute_delta(m.lLastX, m.lLastY, m.usFlags & MOUSE_VIRTUAL_DESKTOP as u16 != 0)
        } else {
            self.abs_last = None;
            (m.lLastX, m.lLastY)
        };

        if let Some(hold) = &mut self.hold {
            if dx != 0 || dy != 0 {
                hold.last_move = Instant::now();
            }
            return;
        }
        let calibrating = self.calibrating_until.is_some_and(|t| Instant::now() < t);
        if !calibrating && (self.cfg.paused || Instant::now() < self.cooldown_until) {
            return;
        }
        if self.cfg.ignore_dragging && buttons & RI_MOUSE_BUTTONS_DOWN != 0 {
            self.detector.reset();
        }
        if dx == 0 && dy == 0 {
            return;
        }

        let tuning = self.cfg.tuning();
        let now = self.now_ms();
        let out = self.detector.feed(dx as f32, dy as f32, now, &tuning);
        if let Some(live) = self.live {
            for s in out.strokes.iter().flatten() {
                live.push_stroke(s.speed, s.counts);
            }
            live.set_run(self.detector.run());
        }
        if out.shake && !calibrating && !self.should_ignore() {
            if let Some(live) = self.live {
                live.add_shake();
            }
            self.find(false);
        }
    }

    /// Tablets, remote desktop and VMs send absolute positions (0..65535).
    fn absolute_delta(&mut self, x: i32, y: i32, virtual_desktop: bool) -> (i32, i32) {
        let (w, h) = unsafe {
            if virtual_desktop {
                (GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN))
            } else {
                (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN))
            }
        };
        let (px, py) = ((x as i64 * w as i64 / 65535) as i32, (y as i64 * h as i64 / 65535) as i32);
        let delta = self.abs_last.map_or((0, 0), |(lx, ly)| (px - lx, py - ly));
        self.abs_last = Some((px, py));
        delta
    }

    fn should_ignore(&self) -> bool {
        (self.cfg.ignore_dragging && target::buttons_held())
            || (self.cfg.ignore_fullscreen && target::fullscreen_in_front())
            || (!self.ignored.is_empty()
                && target::foreground_exe().is_some_and(|exe| self.ignored.iter().any(|a| *a == exe)))
    }

    /// Bring the pointer to its landing spot (unless `effect_only`) and make it obvious.
    fn find(&mut self, effect_only: bool) {
        self.detector.reset();
        self.cooldown_until = Instant::now() + COOLDOWN;
        if !effect_only && self.cfg.landing != Landing::StayPut {
            if let Some(p) = target::resolve(&self.cfg) {
                unsafe { SetCursorPos(p.x, p.y) };
                self.begin_hold(p);
            }
        }
        self.fx.play(self.cfg.effect, self.cfg.color, self.cfg.size);
        if self.cfg.sound {
            sound::play();
        }
        self.ensure_timer();
    }

    fn begin_hold(&mut self, at: POINT) {
        self.release_hold();
        let mut prev = RECT::default();
        let prev_clip = (unsafe { GetClipCursor(&mut prev) } != 0 && !is_virtual_screen(&prev)).then_some(prev);
        let now = Instant::now();
        self.hold = Some(Hold { at, began: now, last_move: now, prev_clip });
        pin(at);
    }

    fn release_hold(&mut self) {
        if let Some(hold) = self.hold.take() {
            unsafe { ClipCursor(hold.prev_clip.as_ref().map_or(null(), |r| r as *const RECT)) };
        }
    }

    fn ensure_timer(&self) {
        if self.fx.active() || self.hold.is_some() {
            unsafe { SetTimer(self.hwnd, TIMER_FRAME, 10, None) };
        }
    }

    fn on_frame(&mut self) {
        if let Some(hold) = &self.hold {
            if hold.began.elapsed() >= HOLD_MAX || hold.last_move.elapsed() >= SETTLE {
                self.release_hold();
            } else {
                pin(hold.at); // re-assert: some apps reset the clip on focus changes
            }
        }
        if self.fx.active() {
            self.fx.tick();
        }
        if !self.fx.active() && self.hold.is_none() {
            unsafe { KillTimer(self.hwnd, TIMER_FRAME) };
        }
    }
}

fn pin(at: POINT) {
    let r = RECT { left: at.x, top: at.y, right: at.x + 1, bottom: at.y + 1 };
    unsafe { ClipCursor(&r) };
}

fn is_virtual_screen(r: &RECT) -> bool {
    unsafe {
        let (x, y) = (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN));
        let (w, h) = (GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN));
        r.left <= x && r.top <= y && r.right >= x + w && r.bottom >= y + h
    }
}
