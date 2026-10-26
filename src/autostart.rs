//! "Start with Windows": one value under HKCU\...\Run, nothing else.

use crate::win::wide;
use std::ptr::null_mut;
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "Shook";

fn command() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    Some(format!("\"{}\" --autostart", exe.display()))
}

fn current() -> Option<String> {
    let (key, value) = (wide(RUN_KEY), wide(VALUE));
    let mut buf = vec![0u16; 1024];
    let mut bytes = (buf.len() * 2) as u32;
    let rc = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr(), RRF_RT_REG_SZ, null_mut(), buf.as_mut_ptr().cast(), &mut bytes)
    };
    (rc == 0).then(|| crate::win::from_wide(&buf))
}

pub fn is_enabled() -> bool {
    current().is_some()
}

pub fn set(on: bool) {
    let (key, value) = (wide(RUN_KEY), wide(VALUE));
    unsafe {
        if on {
            if let Some(cmd) = command() {
                let data = wide(&cmd);
                RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr(), REG_SZ, data.as_ptr().cast(), (data.len() * 2) as u32);
            }
        } else {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr());
        }
    }
}

/// If Shook starts with Windows but the exe moved, point the entry at this copy.
pub fn follow_moves() {
    if let (Some(have), Some(want)) = (current(), command()) {
        if !have.eq_ignore_ascii_case(&want) {
            set(true);
        }
    }
}
