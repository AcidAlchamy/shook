//! Hotkey text ("Ctrl+Alt+M") to RegisterHotKey's modifier flags and key code.

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN};

/// (modifier flags, virtual-key code), or None if the text isn't a usable hotkey.
pub fn parse(text: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = text.split('+').map(str::trim).filter(|p| !p.is_empty()).collect();
    let (key, mods) = parts.split_last()?;
    let mut flags = MOD_NOREPEAT;
    for m in mods {
        flags |= match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => MOD_CONTROL,
            "alt" => MOD_ALT,
            "shift" => MOD_SHIFT,
            "win" | "windows" => MOD_WIN,
            _ => return None,
        };
    }
    let key = key.to_ascii_uppercase();
    let vk = match key.as_str() {
        "SPACE" => 0x20,
        k if k.len() == 1 && k.chars().all(|c| c.is_ascii_alphanumeric()) => k.as_bytes()[0] as u32,
        k if k.starts_with('F') => match k[1..].parse::<u32>() {
            Ok(n @ 1..=24) => 0x70 + n - 1,
            _ => return None,
        },
        _ => return None,
    };
    // A bare letter would swallow normal typing; function keys alone are fine.
    let has_mod = flags & (MOD_CONTROL | MOD_ALT | MOD_WIN) != 0;
    (has_mod || (0x70..=0x87).contains(&vk)).then_some((flags, vk))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_combos() {
        assert_eq!(parse("Ctrl+Alt+M"), Some((MOD_NOREPEAT | MOD_CONTROL | MOD_ALT, b'M' as u32)));
        assert_eq!(parse("shift + win + f9"), Some((MOD_NOREPEAT | MOD_SHIFT | MOD_WIN, 0x78)));
        assert_eq!(parse("F13"), Some((MOD_NOREPEAT, 0x7C)));
    }

    #[test]
    fn rejects_junk_and_bare_letters() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("M"), None);
        assert_eq!(parse("Shift+M"), None);
        assert_eq!(parse("Ctrl+Hyper+M"), None);
        assert_eq!(parse("Ctrl+F99"), None);
    }
}
