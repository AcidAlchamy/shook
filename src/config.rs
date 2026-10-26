//! Settings, stored as TOML in %APPDATA%\Shook\settings.toml.
//!
//! The resident reads this file at start and whenever the settings window says
//! it changed; the settings window writes it. Unknown or missing keys fall back
//! to the defaults, so old files keep working.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where the pointer appears after a shake.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Landing {
    /// The middle of a screen (see `Config::screen`).
    ScreenCenter,
    /// The middle of the window you're working in.
    WindowCenter,
    /// A fixed spot you picked.
    Spot,
    /// Don't move it, just make it obvious.
    StayPut,
}

/// What draws your eye to the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Effect {
    Ripple,
    Spotlight,
    Off,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Shaking does nothing while paused (the hotkey still works).
    pub paused: bool,
    /// How hard you have to shake, 1 (gentle) to 10 (vigorous).
    pub speed: f32,
    /// Direction changes in a row that make a shake.
    pub wiggles: u32,
    pub landing: Landing,
    /// For `Landing::ScreenCenter`: "primary", "pointer", or a display name like `\\.\DISPLAY2`.
    pub screen: String,
    /// For `Landing::Spot`: physical pixels on the virtual desktop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spot: Option<[i32; 2]>,
    pub effect: Effect,
    /// Effect colour, written as "#C6FF3A".
    #[serde(with = "hex_color")]
    pub color: [u8; 3],
    /// Effect size, 0.6 to 2.0.
    pub size: f32,
    pub sound: bool,
    /// Global "find my pointer" hotkey like "Ctrl+Alt+M"; empty = off.
    pub hotkey: String,
    /// Ignore shakes while a mouse button is held (dragging, drawing, selecting).
    pub ignore_dragging: bool,
    /// Ignore shakes while a full-screen app or game is in front.
    pub ignore_fullscreen: bool,
    /// Ignore shakes while one of these programs is in front (exe names).
    pub ignore_apps: Vec<String>,
    /// Show the donationware hello card when Shook starts.
    pub hello_card: bool,
}

pub const ACID: [u8; 3] = [0xC6, 0xFF, 0x3A];

impl Default for Config {
    fn default() -> Self {
        Self {
            paused: false,
            speed: 4.0,
            wiggles: 4,
            landing: Landing::ScreenCenter,
            screen: "pointer".into(),
            spot: None,
            effect: Effect::Ripple,
            color: ACID,
            size: 1.0,
            sound: false,
            hotkey: String::new(),
            ignore_dragging: true,
            ignore_fullscreen: true,
            ignore_apps: Vec::new(),
            hello_card: true,
        }
    }
}

pub const SPEED_MIN: f32 = 1.0;
pub const SPEED_MAX: f32 = 10.0;
pub const WIGGLES_MIN: u32 = 3;
pub const WIGGLES_MAX: u32 = 8;

/// Speed level (1..10) to the minimum stroke speed in counts per second.
pub fn min_speed(level: f32) -> f32 {
    900.0 * 10f32.powf((level.clamp(SPEED_MIN, SPEED_MAX) - 1.0) / 9.0)
}

/// The speed level whose threshold is `counts_per_sec`.
pub fn level_for(counts_per_sec: f32) -> f32 {
    (1.0 + 9.0 * (counts_per_sec.max(1.0) / 900.0).log10()).clamp(SPEED_MIN, SPEED_MAX)
}

impl Config {
    pub fn tuning(&self) -> crate::shake::Tuning {
        crate::shake::Tuning {
            min_speed: min_speed(self.speed),
            wiggles: self.wiggles.clamp(WIGGLES_MIN, WIGGLES_MAX),
        }
    }

    /// Lower-case exe names, each ending in ".exe".
    pub fn ignored_apps(&self) -> Vec<String> {
        self.ignore_apps
            .iter()
            .map(|a| a.trim().to_ascii_lowercase())
            .filter(|a| !a.is_empty())
            .map(|a| if a.ends_with(".exe") { a } else { format!("{a}.exe") })
            .collect()
    }

    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, format!("# Shook settings. Edit in the app: right-click the tray icon, Settings.\n\n{text}"))?;
        std::fs::rename(tmp, path)
    }
}

mod hex_color {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer>(c: &[u8; 3], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 3], D::Error> {
        let text = String::deserialize(d)?;
        let hex = text.trim().trim_start_matches('#');
        let byte = |i: usize| hex.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok());
        match (hex.len(), byte(0), byte(2), byte(4)) {
            (6, Some(r), Some(g), Some(b)) => Ok([r, g, b]),
            _ => Err(D::Error::custom(format!("not a colour: {text}"))),
        }
    }
}

pub fn dir() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("Shook")
}

pub fn path() -> PathBuf {
    dir().join("settings.toml")
}

pub fn exists() -> bool {
    path().exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_levels_round_trip() {
        for l in [1.0, 2.5, 4.0, 7.0, 10.0] {
            assert!((level_for(min_speed(l)) - l).abs() < 1e-3);
        }
    }

    #[test]
    fn toml_round_trip_and_partial_files() {
        let mut c = Config::default();
        c.spot = Some([1920, 540]);
        c.ignore_apps = vec!["Photoshop".into()];
        let back: Config = toml::from_str(&toml::to_string_pretty(&c).unwrap()).unwrap();
        assert_eq!(back, c);
        assert_eq!(back.ignored_apps(), vec!["photoshop.exe".to_string()]);

        assert!(toml::to_string(&c).unwrap().contains("color = \"#C6FF3A\""));

        let partial: Config = toml::from_str("speed = 6.5\nlanding = \"stay-put\"\ncolor = \"#ff2e7e\"").unwrap();
        assert_eq!(partial.color, [0xFF, 0x2E, 0x7E]);
        assert_eq!(partial.speed, 6.5);
        assert_eq!(partial.landing, Landing::StayPut);
        assert_eq!(partial.wiggles, Config::default().wiggles);
    }
}
