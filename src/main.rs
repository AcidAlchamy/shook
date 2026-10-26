//! Shook: lost your pointer? Shake the mouse and it comes to you.
//!
//! One small exe with three faces:
//!   Shook.exe             the resident: tray icon, shake detection, effects
//!                         (plain Win32, sleeps until the mouse moves)
//!   Shook.exe --settings  the settings window (only alive while it's open)
//!   Shook.exe --hello     the donationware hello card shown at start
//!
//! Shook is donationware by Acid Alchamy: free to use, and if it helps, please
//! consider a donation at acidalchamy.com/donationware.

#![windows_subsystem = "windows"]

mod autostart;
mod config;
mod hotkey;
mod ipc;
mod live;
mod resident;
mod shake;
mod ui;
mod win;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DONATE_URL: &str = "https://acidalchamy.com/donationware/shook/?from=app#donate";
pub const HOME_URL: &str = "https://acidalchamy.com/donationware/shook/";
pub const REPO_URL: &str = "https://github.com/AcidAlchamy/shook";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    if has("--settings") {
        ui::run_settings(has("--welcome"));
    } else if has("--hello") {
        ui::run_hello();
    } else {
        resident::run(has("--autostart"));
    }
}
