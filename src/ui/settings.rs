//! The settings window. Every change saves on its own (a quarter second after
//! you stop fiddling) and the resident picks it up straight away.

use super::theme::{self, ACID, BAD, BG, CARD_HI, FAINT, INK, LINE, MUTED, PINK, TEXT, VIOLET};
use super::widgets::{self, card, row, toggle};
use crate::config::{self, Config, Effect, Landing};
use crate::ipc::{self, WM_SHOOK_CALIBRATE, WM_SHOOK_FIND, WM_SHOOK_QUIT, WM_SHOOK_RELOAD};
use crate::live::{HotkeyState, Live};
use crate::win::{self, Monitor};
use crate::{autostart, hotkey};
use eframe::egui::{
    self, Align, Color32, CornerRadius, Frame, Layout, Margin, Pos2, Rect, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, Ui, pos2, vec2,
};
use std::collections::VecDeque;
use std::time::{Duration, SystemTime};

const SAVE_DELAY: f64 = 0.25;
const METER_LEN: usize = 26;
const CALIBRATE_SECS: f64 = 3.5;
const PICK_SECS: f64 = 3.0;

const SWATCHES: [([u8; 3], &str); 6] = [
    ([0xC6, 0xFF, 0x3A], "Acid"),
    ([0xFF, 0x2E, 0x7E], "Hot pink"),
    ([0x8B, 0x3D, 0xFF], "Violet"),
    ([0x2E, 0xE6, 0xFF], "Cyan"),
    ([0xFF, 0xB0, 0x20], "Amber"),
    ([0xFF, 0xFF, 0xFF], "White"),
];

enum Mode {
    Normal,
    Calibrating { until: f64, speeds: Vec<f32> },
    PickingSpot { until: f64 },
    PickingApp { until: f64 },
    RecordingHotkey,
}

struct Stroke2 {
    speed: f32,
    counts: bool,
    at: f64,
}

pub struct Settings {
    cfg: Config,
    saved: Config,
    changed_at: Option<f64>,
    file_stamp: Option<SystemTime>,
    welcome: bool,
    autostart: bool,
    live: Option<&'static Live>,
    seen: u32,
    meter: VecDeque<Stroke2>,
    last_shakes: u32,
    shake_flash: f64,
    mode: Mode,
    monitors: Vec<Monitor>,
    resident_up: bool,
    last_poll: f64,
    app_input: String,
    note: Option<(String, Color32, f64)>,
}

fn file_stamp() -> Option<SystemTime> {
    std::fs::metadata(config::path()).and_then(|m| m.modified()).ok()
}

impl Settings {
    pub fn new(welcome: bool) -> Self {
        let cfg = Config::load();
        let live = crate::live::open();
        let seen = live.map_or(0, |l| l.stroke_seq());
        Self {
            saved: cfg.clone(),
            cfg,
            changed_at: None,
            file_stamp: file_stamp(),
            welcome,
            autostart: autostart::is_enabled(),
            live,
            seen,
            meter: VecDeque::with_capacity(METER_LEN),
            last_shakes: live.map_or(0, |l| l.shakes()),
            shake_flash: -10.0,
            mode: Mode::Normal,
            monitors: win::monitors(),
            resident_up: ipc::resident().is_some(),
            last_poll: 0.0,
            app_input: String::new(),
            note: None,
        }
    }

    fn save_now(&mut self) {
        if self.cfg != self.saved {
            if self.cfg.save().is_ok() {
                self.saved = self.cfg.clone();
                self.file_stamp = file_stamp();
                ipc::send(WM_SHOOK_RELOAD, 0);
            }
        }
        self.changed_at = None;
    }

    fn note(&mut self, text: impl Into<String>, color: Color32, now: f64) {
        self.note = Some((text.into(), color, now + 4.0));
    }

    /// Once a second: is the resident there, did someone else change the file, screens.
    fn poll(&mut self, now: f64) {
        if now - self.last_poll < 1.0 {
            return;
        }
        self.last_poll = now;
        self.resident_up = ipc::resident().is_some();
        self.monitors = win::monitors();
        let stamp = file_stamp();
        if stamp != self.file_stamp && self.changed_at.is_none() {
            self.file_stamp = stamp;
            self.cfg = Config::load();
            self.saved = self.cfg.clone();
        }
    }

    fn read_live(&mut self, now: f64) {
        let Some(live) = self.live else { return };
        for (speed, counts) in live.strokes_since(&mut self.seen) {
            if self.meter.len() == METER_LEN {
                self.meter.pop_front();
            }
            self.meter.push_back(Stroke2 { speed, counts, at: now });
            if let Mode::Calibrating { speeds, .. } = &mut self.mode {
                speeds.push(speed);
            }
        }
        let shakes = live.shakes();
        if shakes != self.last_shakes {
            self.last_shakes = shakes;
            self.shake_flash = now;
        }
    }

    fn finish_modes(&mut self, now: f64) {
        match &mut self.mode {
            Mode::Calibrating { until, speeds } if now >= *until => {
                let mut v: Vec<f32> = speeds.iter().copied().filter(|&s| s > 300.0).collect();
                ipc::send(WM_SHOOK_CALIBRATE, 0);
                self.mode = Mode::Normal;
                if v.len() < 4 {
                    self.note("Didn't catch enough of a shake. Try again and shake a bit longer.", BAD, now);
                    return;
                }
                v.sort_by(|a, b| b.total_cmp(a));
                let top = &v[..(v.len() * 7 / 10).max(3)];
                let typical = top[top.len() / 2];
                let level = (config::level_for(typical * 0.55) * 10.0).round() / 10.0;
                self.cfg.speed = level;
                self.changed_at = Some(now);
                self.note(format!("Tuned to {level:.1}. A shake like that one will always work now."), ACID, now);
            }
            Mode::PickingSpot { until } if now >= *until => {
                let p = win::cursor();
                self.cfg.spot = Some([p.x, p.y]);
                self.cfg.landing = Landing::Spot;
                self.changed_at = Some(now);
                self.mode = Mode::Normal;
                self.note(format!("Spot set to {}, {}.", p.x, p.y), ACID, now);
            }
            Mode::PickingApp { until } if now >= *until => {
                self.mode = Mode::Normal;
                match crate::resident::target::foreground_exe() {
                    Some(exe) if exe == "shook.exe" => {
                        self.note("That was Shook itself. Click into the other app during the countdown.", BAD, now)
                    }
                    Some(exe) => {
                        if !self.cfg.ignored_apps().contains(&exe) {
                            self.cfg.ignore_apps.push(exe.clone());
                            self.changed_at = Some(now);
                        }
                        self.note(format!("Shaking is now ignored in {exe}."), ACID, now);
                    }
                    None => self.note("Couldn't tell which app was in front.", BAD, now),
                }
            }
            _ => {}
        }
    }

    fn record_hotkey(&mut self, ui: &Ui, now: f64) {
        if !matches!(self.mode, Mode::RecordingHotkey) {
            return;
        }
        let events = ui.input(|i| i.events.clone());
        for e in events {
            let egui::Event::Key { key, pressed: true, modifiers, .. } = e else { continue };
            match key {
                egui::Key::Escape => self.mode = Mode::Normal,
                egui::Key::Backspace | egui::Key::Delete => {
                    self.cfg.hotkey.clear();
                    self.changed_at = Some(now);
                    self.mode = Mode::Normal;
                }
                _ => {
                    let mut parts = vec![];
                    if modifiers.ctrl {
                        parts.push("Ctrl");
                    }
                    if modifiers.alt {
                        parts.push("Alt");
                    }
                    if modifiers.shift {
                        parts.push("Shift");
                    }
                    parts.push(key.name());
                    let combo = parts.join("+");
                    if hotkey::parse(&combo).is_some() {
                        self.cfg.hotkey = combo;
                        self.changed_at = Some(now);
                        self.mode = Mode::Normal;
                    } else {
                        self.note("Hold Ctrl or Alt with the key (or use F1 to F24).", BAD, now);
                    }
                }
            }
        }
    }
}

impl eframe::App for Settings {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        #[cfg(feature = "scale-test")]
        super::test_shot::frame(ui.ctx());
        let now = ui.input(|i| i.time);
        self.poll(now);
        self.read_live(now);
        self.finish_modes(now);
        self.record_hotkey(ui, now);

        if ui.input(|i| i.viewport().close_requested()) {
            self.save_now();
            if matches!(self.mode, Mode::Calibrating { .. }) {
                ipc::send(WM_SHOOK_CALIBRATE, 0);
            }
        }

        egui::CentralPanel::default().frame(Frame::new().fill(BG)).show(ui, |ui| {
            self.header(ui, now);
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                Frame::new().inner_margin(Margin { left: 20, right: 20, top: 14, bottom: 18 }).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 12.0;
                    if self.welcome {
                        self.welcome_card(ui);
                    }
                    self.status_row(ui, now);
                    self.shake_card(ui, now);
                    self.landing_card(ui, now);
                    self.effect_card(ui, now);
                    self.behaviour_card(ui, now);
                    self.footer(ui);
                });
            });
        });

        if let Some((text, color, until)) = self.note.clone() {
            if now < until {
                egui::Area::new(egui::Id::unique("shook-note"))
                    .anchor(egui::Align2::CENTER_BOTTOM, vec2(0.0, -18.0))
                    .show(ui.ctx(), |ui| {
                        Frame::new()
                            .fill(Color32::from_rgb(0x1C, 0x17, 0x29))
                            .stroke(Stroke::new(1.0, color))
                            .corner_radius(CornerRadius::same(10))
                            .inner_margin(Margin::symmetric(14, 9))
                            .show(ui, |ui| ui.label(RichText::new(text).color(TEXT)));
                    });
            } else {
                self.note = None;
            }
        }

        if let Some(t) = self.changed_at.or((self.cfg != self.saved).then_some(now)) {
            self.changed_at = Some(t);
            if now - t >= SAVE_DELAY {
                self.save_now();
            } else {
                ui.ctx().request_repaint_after(Duration::from_millis(60));
            }
        }

        // Live parts (meter, countdowns) animate; otherwise just check in now and then.
        let busy = !matches!(self.mode, Mode::Normal)
            || self.meter.back().is_some_and(|s| now - s.at < 1.5)
            || now - self.shake_flash < 1.2
            || self.live.is_some_and(|l| l.run() > 0);
        ui.ctx().request_repaint_after(Duration::from_millis(if busy { 16 } else { 250 }));
    }
}

impl Settings {
    fn header(&mut self, ui: &mut Ui, now: f64) {
        let height = 92.0;
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
        let painter = ui.painter_at(rect);

        let run = self.live.map_or(0, |l| l.run()) as f32 / self.cfg.wiggles.max(1) as f32;
        let flash = (1.0 - (now - self.shake_flash) as f32).max(0.0);
        let wiggle = ui.ctx().animate_value_with_time(egui::Id::unique("shook-wiggle"), run.max(flash).min(1.0), 0.15);
        let logo = Rect::from_center_size(rect.left_center() + vec2(58.0, 0.0), vec2(58.0, 58.0));
        widgets::logo(&painter, logo, wiggle, now);
        if wiggle > 0.01 {
            ui.ctx().request_repaint();
        }

        let x = logo.right() + 16.0;
        painter.text(pos2(x, rect.center().y - 14.0), egui::Align2::LEFT_CENTER, "Shook", theme::bold(28.0), TEXT);
        painter.text(
            pos2(x + 1.0, rect.center().y + 15.0),
            egui::Align2::LEFT_CENTER,
            format!("v{}  \u{00B7}  donationware by Acid Alchamy", crate::VERSION),
            egui::FontId::proportional(13.0),
            MUTED,
        );
        widgets::brand_rule(&painter, Rect::from_min_max(pos2(rect.left(), rect.bottom() - 2.0), rect.right_bottom()));

        let button = Rect::from_min_size(pos2(rect.right() - 20.0 - 118.0, rect.center().y - 17.0), vec2(118.0, 34.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(button));
        if widgets::donate(&mut child, "Donate").clicked() {
            win::open_url(crate::DONATE_URL);
        }
    }

    fn welcome_card(&mut self, ui: &mut Ui) {
        Frame::new()
            .fill(Color32::from_rgb(0x1A, 0x12, 0x2C))
            .stroke(Stroke::new(1.0, VIOLET.gamma_multiply(0.7)))
            .corner_radius(CornerRadius::same(14))
            .inner_margin(Margin::same(18))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Welcome to Shook").font(theme::bold(18.0)).color(TEXT));
                ui.add(
                    egui::Label::new(
                        RichText::new(
                            "Shook now lives in your system tray, bottom right (it may be tucked under the ^ arrow). \
                             Lost the pointer? Give the mouse a quick shake, side to side, and it jumps to the middle \
                             of your screen. Everything below is optional.",
                        )
                        .color(MUTED),
                    )
                    .wrap(),
                );
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(
                            "Shook is donationware: free to use, for good. If it ever saves you a hunt, a small donation \
                             keeps the lab building more little tools like it.",
                        )
                        .color(TEXT),
                    )
                    .wrap(),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if widgets::donate(ui, "Donate").clicked() {
                        win::open_url(crate::DONATE_URL);
                    }
                    if widgets::ghost(ui, "Got it").clicked() {
                        self.welcome = false;
                    }
                });
            });
    }

    fn status_row(&mut self, ui: &mut Ui, now: f64) {
        ui.horizontal(|ui| {
            if !self.resident_up {
                widgets::status(ui, BAD, "Shook isn't running");
            } else if self.cfg.paused {
                widgets::status(ui, Color32::from_rgb(0xFF, 0xC4, 0x3A), "Paused: shaking does nothing");
            } else {
                let found = self.last_shakes;
                let text = match found {
                    0 => "On duty. Shake the mouse to find the pointer.".to_owned(),
                    1 => "On duty. Found your pointer once so far.".to_owned(),
                    n => format!("On duty. Found your pointer {n} times so far."),
                };
                let pulse = (1.0 - (now - self.shake_flash) as f32).max(0.0);
                widgets::status(ui, theme::lerp(ACID, Color32::WHITE, pulse), &text);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !self.resident_up {
                    if widgets::primary(ui, "Start Shook").clicked() {
                        self.save_now();
                        win::spawn_self(&[]);
                        self.last_poll = 0.0;
                    }
                } else {
                    let mut on = !self.cfg.paused;
                    if toggle(ui, &mut on).changed() {
                        self.cfg.paused = !on;
                        self.changed_at = Some(now - SAVE_DELAY);
                    }
                    ui.label(RichText::new("Shake to find").color(MUTED));
                }
            });
        });
    }

    fn shake_card(&mut self, ui: &mut Ui, now: f64) {
        card(ui, "Shake", |ui| {
            row(ui, "How hard to shake", "Higher needs a more vigorous shake", |ui| {
                ui.label(RichText::new(format!("{:.1}", self.cfg.speed)).font(theme::semibold(14.0)).color(ACID));
                let r = ui.add(egui::Slider::new(&mut self.cfg.speed, config::SPEED_MIN..=config::SPEED_MAX).show_value(false));
                if r.changed() {
                    self.changed_at = Some(now);
                }
            });
            self.meter(ui, now);
            row(ui, "Wiggles", "Direction changes in a row", |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if widgets::ghost(ui, "+").clicked() && self.cfg.wiggles < config::WIGGLES_MAX {
                    self.cfg.wiggles += 1;
                    self.changed_at = Some(now);
                }
                ui.add_sized(
                    vec2(28.0, 30.0),
                    egui::Label::new(RichText::new(self.cfg.wiggles.to_string()).font(theme::semibold(16.0)).color(TEXT)),
                );
                if widgets::ghost(ui, "\u{2212}").clicked() && self.cfg.wiggles > config::WIGGLES_MIN {
                    self.cfg.wiggles -= 1;
                    self.changed_at = Some(now);
                }
            });
            row(ui, "Calibrate", "Shake your usual \u{201C}where is it?\u{201D} shake and Shook tunes itself", |ui| {
                let busy = matches!(self.mode, Mode::Calibrating { .. });
                if ui.add_enabled(!busy && self.resident_up, egui::Button::new("Calibrate")).clicked() {
                    self.meter.clear();
                    ipc::send(WM_SHOOK_CALIBRATE, 1);
                    self.mode = Mode::Calibrating { until: now + CALIBRATE_SECS, speeds: vec![] };
                }
            });
        });
    }

    /// Recent strokes as bars; the ones over the line count toward a shake.
    fn meter(&mut self, ui: &mut Ui, now: f64) {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(width, 86.0), Sense::hover());
        let p = ui.painter_at(rect);
        p.rect(rect, CornerRadius::same(10), Color32::from_rgb(0x0F, 0x0C, 0x17), Stroke::new(1.0, LINE), StrokeKind::Inside);

        let plot = Rect::from_min_max(rect.min + vec2(14.0, 12.0), rect.max - vec2(120.0, 22.0));
        let threshold = config::min_speed(self.cfg.speed);
        // Log scale: the line sits at 55% height wherever the threshold is.
        let height_of = |speed: f32| {
            let k = (speed.max(1.0) / threshold).log2() * 0.32 + 0.55;
            k.clamp(0.02, 1.0) * plot.height()
        };
        let line_y = plot.bottom() - 0.55 * plot.height();
        let gap = 4.0;
        let bar_w = (plot.width() - gap * (METER_LEN as f32 - 1.0)) / METER_LEN as f32;
        let start = METER_LEN - self.meter.len();
        for (i, s) in self.meter.iter().enumerate() {
            let x = plot.left() + (start + i) as f32 * (bar_w + gap);
            let h = height_of(s.speed);
            let age = ((now - s.at) / 8.0).clamp(0.0, 1.0) as f32;
            let color = if s.counts { ACID } else { theme::lerp(Color32::from_rgb(0x5A, 0x52, 0x72), LINE, age) };
            let color = color.gamma_multiply(1.0 - age * 0.6);
            p.rect_filled(
                Rect::from_min_max(pos2(x, plot.bottom() - h), pos2(x + bar_w, plot.bottom())),
                CornerRadius::same(3),
                color,
            );
        }
        // The threshold, dashed.
        let mut x = plot.left();
        while x < plot.right() {
            p.line_segment([pos2(x, line_y), pos2((x + 7.0).min(plot.right()), line_y)], Stroke::new(1.5, PINK.gamma_multiply(0.85)));
            x += 12.0;
        }
        p.text(pos2(plot.left(), rect.bottom() - 11.0), egui::Align2::LEFT_CENTER, "your strokes  \u{2022}  over the line counts", egui::FontId::proportional(11.5), FAINT);

        // Wiggle dots on the right: they fill as you shake.
        let run = self.live.map_or(0, |l| l.run()).min(self.cfg.wiggles);
        let n = self.cfg.wiggles;
        let dots = Rect::from_min_max(pos2(plot.right() + 14.0, rect.top()), rect.max - vec2(12.0, 0.0));
        let flash = (now - self.shake_flash) < 1.0;
        let label = match &self.mode {
            Mode::Calibrating { until, .. } => format!("Shake now!  {:.1}s", (until - now).max(0.0)),
            _ if flash => "Found it!".to_owned(),
            _ if self.cfg.paused => "Paused".to_owned(),
            _ => format!("{run} of {n}"),
        };
        let label_color = if flash || matches!(self.mode, Mode::Calibrating { .. }) { ACID } else { MUTED };
        p.text(pos2(dots.center().x, dots.top() + 24.0), egui::Align2::CENTER_CENTER, label, theme::semibold(13.5), label_color);
        let size = 10.0;
        let total_w = n as f32 * size + (n as f32 - 1.0) * 5.0;
        for i in 0..n {
            let c = pos2(dots.center().x - total_w / 2.0 + size / 2.0 + i as f32 * (size + 5.0), dots.top() + 52.0);
            let lit = flash || i < run;
            if lit {
                p.circle_filled(c, size / 2.0, ACID);
            } else {
                p.circle_stroke(c, size / 2.0 - 0.75, Stroke::new(1.5, Color32::from_rgb(0x4A, 0x43, 0x60)));
            }
        }
        if let Mode::Calibrating { until, .. } = &self.mode {
            let k = (1.0 - (until - now) / CALIBRATE_SECS).clamp(0.0, 1.0) as f32;
            let bar = Rect::from_min_size(pos2(rect.left() + 1.0, rect.bottom() - 4.0), vec2((rect.width() - 2.0) * k, 3.0));
            p.rect_filled(bar, CornerRadius::same(2), ACID);
        }
    }

    fn landing_card(&mut self, ui: &mut Ui, now: f64) {
        card(ui, "Where it lands", |ui| {
            let options = [
                (Landing::ScreenCenter, "Middle of a screen", "Pick which one below"),
                (Landing::WindowCenter, "Middle of the window you're in", "Follows whatever app is in front"),
                (Landing::Spot, "A spot you pick", "Anywhere on any screen"),
                (Landing::StayPut, "Leave it where it is", "Shook just lights it up"),
            ];
            for (value, title, hint) in options {
                let selected = self.cfg.landing == value;
                let resp = Frame::new()
                    .fill(if selected { Color32::from_rgb(0x1E, 0x19, 0x2D) } else { Color32::TRANSPARENT })
                    .stroke(Stroke::new(1.0, if selected { VIOLET.gamma_multiply(0.8) } else { Color32::TRANSPARENT }))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin::symmetric(10, 7))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                            if selected {
                                ui.painter().circle_filled(r.center(), 8.0, ACID);
                                ui.painter().circle_filled(r.center(), 3.5, INK);
                            } else {
                                ui.painter().circle_stroke(r.center(), 7.0, Stroke::new(1.5, Color32::from_rgb(0x5A, 0x52, 0x72)));
                            }
                            ui.label(RichText::new(title).color(TEXT));
                            ui.label(RichText::new(hint).small().color(FAINT));
                        });
                    })
                    .response
                    .interact(Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if resp.clicked() && !selected {
                    self.cfg.landing = value;
                    self.changed_at = Some(now);
                }
            }

            ui.add_space(2.0);
            match self.cfg.landing {
                Landing::ScreenCenter => {
                    row(ui, "Which screen", "", |ui| {
                        let label = |key: &str, monitors: &[Monitor]| match key {
                            "pointer" => "The one the pointer is on".to_owned(),
                            "primary" => "The main screen".to_owned(),
                            name => monitors
                                .iter()
                                .position(|m| m.name.eq_ignore_ascii_case(name))
                                .map(|i| screen_label(i, &monitors[i]))
                                .unwrap_or_else(|| format!("{name} (not connected)")),
                        };
                        let mut choice = self.cfg.screen.clone();
                        egui::ComboBox::from_id_salt("screen").selected_text(label(&choice, &self.monitors)).width(260.0).show_ui(
                            ui,
                            |ui| {
                                ui.selectable_value(&mut choice, "pointer".to_owned(), label("pointer", &self.monitors));
                                ui.selectable_value(&mut choice, "primary".to_owned(), label("primary", &self.monitors));
                                for (i, m) in self.monitors.iter().enumerate() {
                                    ui.selectable_value(&mut choice, m.name.clone(), screen_label(i, m));
                                }
                            },
                        );
                        if choice != self.cfg.screen {
                            self.cfg.screen = choice;
                            self.changed_at = Some(now);
                        }
                    });
                }
                Landing::Spot => {
                    row(ui, "The spot", "Or click on the map below", |ui| {
                        let picking = matches!(self.mode, Mode::PickingSpot { .. });
                        let text = match &self.mode {
                            Mode::PickingSpot { until } => format!("Point at it\u{2026} {:.0}", (until - now).ceil()),
                            _ => "Pick with the pointer".to_owned(),
                        };
                        if ui.add_enabled(!picking, egui::Button::new(text)).clicked() {
                            self.mode = Mode::PickingSpot { until: now + PICK_SECS };
                        }
                        if let Some([x, y]) = self.cfg.spot {
                            ui.label(RichText::new(format!("{x}, {y}")).monospace().color(ACID));
                        }
                    });
                }
                _ => {}
            }
            self.screen_map(ui, now);
        });
    }

    /// Your screens, to scale: the pointer as a white dot, the landing spot in lime.
    fn screen_map(&mut self, ui: &mut Ui, now: f64) {
        if self.monitors.is_empty() {
            return;
        }
        let (mut l, mut t, mut r, mut b) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for m in &self.monitors {
            l = l.min(m.rect.left);
            t = t.min(m.rect.top);
            r = r.max(m.rect.right);
            b = b.max(m.rect.bottom);
        }
        let (vw, vh) = ((r - l).max(1) as f32, (b - t).max(1) as f32);
        let avail = ui.available_width();
        let scale = (avail / vw).min(120.0 / vh);
        let size = vec2(vw * scale, vh * scale);
        let (outer, resp) = ui.allocate_exact_size(vec2(avail, size.y + 8.0), Sense::click());
        let area = Rect::from_center_size(outer.center(), size);
        let to_ui = |x: i32, y: i32| pos2(area.left() + (x - l) as f32 * scale, area.top() + (y - t) as f32 * scale);
        let to_px = |p: Pos2| ((l as f32 + (p.x - area.left()) / scale) as i32, (t as f32 + (p.y - area.top()) / scale) as i32);
        let painter = ui.painter_at(outer);

        let cursor = win::cursor();
        let pointer_mon = win::monitor_at(cursor).map(|m| m.name);
        let target_name = match (self.cfg.landing, self.cfg.screen.as_str()) {
            (Landing::ScreenCenter, "pointer") => pointer_mon.clone(),
            (Landing::ScreenCenter, "primary") => self.monitors.iter().find(|m| m.primary).map(|m| m.name.clone()),
            (Landing::ScreenCenter, name) => Some(name.to_owned()),
            _ => None,
        };
        for (i, m) in self.monitors.iter().enumerate() {
            let rect = Rect::from_min_max(to_ui(m.rect.left, m.rect.top), to_ui(m.rect.right, m.rect.bottom)).shrink(2.0);
            let is_target = target_name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(&m.name));
            let hovered = resp.hover_pos().is_some_and(|p| rect.contains(p));
            let fill = if is_target { Color32::from_rgb(0x22, 0x1C, 0x33) } else { CARD_HI };
            let stroke = if is_target { ACID } else if hovered { VIOLET } else { LINE };
            painter.rect(rect, CornerRadius::same(6), fill, Stroke::new(if is_target { 1.5 } else { 1.0 }, stroke), StrokeKind::Inside);
            let tag = if m.primary { format!("{}  main", i + 1) } else { (i + 1).to_string() };
            painter.text(rect.left_top() + vec2(8.0, 6.0), egui::Align2::LEFT_TOP, tag, egui::FontId::proportional(11.5), MUTED);
            if is_target {
                let c = m.center();
                crosshair(&painter, to_ui(c.x, c.y));
            }
        }
        if self.cfg.landing == Landing::Spot {
            if let Some([x, y]) = self.cfg.spot {
                crosshair(&painter, to_ui(x, y));
            }
        }
        let cp = to_ui(cursor.x, cursor.y);
        painter.circle_filled(cp, 3.5, Color32::WHITE);
        painter.circle_stroke(cp, 6.0, Stroke::new(1.0, Color32::WHITE.gamma_multiply(0.4)));

        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let (x, y) = to_px(p);
                if self.cfg.landing == Landing::Spot {
                    self.cfg.spot = Some([x, y]);
                    self.changed_at = Some(now);
                } else if let Some(m) =
                    self.monitors.iter().find(|m| x >= m.rect.left && x < m.rect.right && y >= m.rect.top && y < m.rect.bottom)
                {
                    self.cfg.landing = Landing::ScreenCenter;
                    self.cfg.screen = m.name.clone();
                    self.changed_at = Some(now);
                }
            }
        }
    }

    fn effect_card(&mut self, ui: &mut Ui, now: f64) {
        card(ui, "Make it obvious", |ui| {
            ui.horizontal(|ui| {
                if widgets::segmented(ui, &mut self.cfg.effect, &[(Effect::Ripple, "Ripple"), (Effect::Spotlight, "Spotlight"), (Effect::Off, "None")]) {
                    self.changed_at = Some(now);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let r = widgets::primary(ui, "\u{25B6}  Try it").on_hover_text("Plays the effect where your pointer is");
                    if r.clicked() {
                        self.save_now();
                        ipc::send(WM_SHOOK_FIND, 1);
                    }
                });
            });
            let effect_on = self.cfg.effect != Effect::Off;
            ui.add_enabled_ui(effect_on, |ui| {
                row(ui, "Color", "", |ui| {
                    let mut c = self.cfg.color;
                    if egui::color_picker::color_edit_button_srgb(ui, &mut c).changed() {
                        self.cfg.color = c;
                        self.changed_at = Some(now);
                    }
                    for (rgb, name) in SWATCHES.iter().rev() {
                        let (r, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                        let color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                        let selected = self.cfg.color == *rgb;
                        ui.painter().circle_filled(r.center(), 10.0, color);
                        if selected {
                            ui.painter().circle_stroke(r.center(), 12.5, Stroke::new(2.0, TEXT));
                        } else if resp.hovered() {
                            ui.painter().circle_stroke(r.center(), 12.0, Stroke::new(1.0, MUTED));
                        }
                        if resp.on_hover_text(*name).clicked() {
                            self.cfg.color = *rgb;
                            self.changed_at = Some(now);
                        }
                    }
                });
                row(ui, "Size", "", |ui| {
                    ui.label(RichText::new(format!("{:.0}%", self.cfg.size * 100.0)).font(theme::semibold(14.0)).color(ACID));
                    if ui.add(egui::Slider::new(&mut self.cfg.size, 0.6..=2.0).show_value(false)).changed() {
                        self.changed_at = Some(now);
                    }
                });
            });
            row(ui, "Sound", "A soft two-note chime", |ui| {
                if toggle(ui, &mut self.cfg.sound).changed() {
                    self.changed_at = Some(now);
                }
            });
        });
    }

    fn behaviour_card(&mut self, ui: &mut Ui, now: f64) {
        card(ui, "Behavior", |ui| {
            row(ui, "Hotkey", "Finds the pointer without shaking", |ui| {
                let recording = matches!(self.mode, Mode::RecordingHotkey);
                if !self.cfg.hotkey.is_empty() && !recording && widgets::ghost(ui, "Clear").clicked() {
                    self.cfg.hotkey.clear();
                    self.changed_at = Some(now);
                }
                let set_text = if recording { "Cancel" } else { "Set" };
                if widgets::ghost(ui, set_text).clicked() {
                    self.mode = if recording { Mode::Normal } else { Mode::RecordingHotkey };
                }
                if recording {
                    ui.label(RichText::new("Press the keys\u{2026}").color(ACID));
                } else if self.cfg.hotkey.is_empty() {
                    ui.label(RichText::new("Off").color(FAINT));
                } else {
                    match self.live.map(|l| l.hotkey()) {
                        Some(HotkeyState::Taken) => {
                            ui.label(RichText::new("taken by another app").small().color(BAD));
                        }
                        Some(HotkeyState::Invalid) => {
                            ui.label(RichText::new("not a usable key").small().color(BAD));
                        }
                        _ => {}
                    }
                    ui.horizontal(|ui| widgets::keycaps(ui, &self.cfg.hotkey));
                }
            });
            let flag = |ui: &mut Ui, value: &mut bool, label: &str, hint: &str| {
                let mut changed = false;
                row(ui, label, hint, |ui| changed = toggle(ui, value).changed());
                changed
            };
            let mut changed = false;
            changed |= flag(ui, &mut self.cfg.ignore_dragging, "Ignore while a button is held", "Dragging, drawing, selecting text");
            changed |= flag(ui, &mut self.cfg.ignore_fullscreen, "Ignore full-screen games and video", "Shaking a crosshair isn't losing it");
            if changed {
                self.changed_at = Some(now);
            }

            ui.label(RichText::new("Ignore these apps").color(TEXT));
            ui.horizontal_wrapped(|ui| {
                let mut remove = None;
                for (i, app) in self.cfg.ignore_apps.iter().enumerate() {
                    Frame::new()
                        .fill(CARD_HI)
                        .stroke(Stroke::new(1.0, LINE))
                        .corner_radius(CornerRadius::same(14))
                        .inner_margin(Margin { left: 10, right: 4, top: 2, bottom: 2 })
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.label(RichText::new(app).color(TEXT));
                            if ui.add(egui::Button::new(RichText::new("\u{00D7}").color(MUTED)).frame(false)).clicked() {
                                remove = Some(i);
                            }
                        });
                }
                if let Some(i) = remove {
                    self.cfg.ignore_apps.remove(i);
                    self.changed_at = Some(now);
                }
                if self.cfg.ignore_apps.is_empty() {
                    ui.label(RichText::new("None yet").color(FAINT));
                }
            });
            ui.horizontal(|ui| {
                let edit = ui.add(egui::TextEdit::singleline(&mut self.app_input).hint_text("photoshop.exe").desired_width(170.0));
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (widgets::ghost(ui, "Add").clicked() || enter) && !self.app_input.trim().is_empty() {
                    self.cfg.ignore_apps.push(self.app_input.trim().to_owned());
                    self.app_input.clear();
                    self.changed_at = Some(now);
                }
                let text = match &self.mode {
                    Mode::PickingApp { until } => format!("Click into the app\u{2026} {:.0}", (until - now).ceil()),
                    _ => "Pick an app in 3 s".to_owned(),
                };
                let picking = matches!(self.mode, Mode::PickingApp { .. });
                if ui.add_enabled(!picking, egui::Button::new(text)).clicked() {
                    self.mode = Mode::PickingApp { until: now + PICK_SECS };
                }
            });
            ui.add_space(4.0);

            let mut auto = self.autostart;
            row(ui, "Start with Windows", "Quietly, in the tray", |ui| {
                if toggle(ui, &mut auto).changed() {
                    autostart::set(auto);
                    self.autostart = autostart::is_enabled();
                }
            });
            if flag(ui, &mut self.cfg.hello_card, "Hello card at start", "The little donationware note") {
                self.changed_at = Some(now);
            }
        });
    }

    fn footer(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                let links_w = 430.0;
                ui.add_space(((ui.available_width() - links_w) / 2.0).max(0.0));
                if widgets::link(ui, "acidalchamy.com/donationware").clicked() {
                    win::open_url(crate::HOME_URL);
                }
                if widgets::link(ui, "Source code").clicked() {
                    win::open_url(crate::REPO_URL);
                }
                if widgets::link(ui, "Settings file").clicked() {
                    win::open_url(&config::dir().display().to_string());
                }
                if self.resident_up && widgets::link(ui, "Quit Shook").clicked() {
                    self.save_now();
                    ipc::send(WM_SHOOK_QUIT, 0);
                    self.last_poll = 0.0;
                }
            });
            ui.label(
                RichText::new(format!(
                    "Shook {}  \u{00B7}  MIT License  \u{00B7}  Donationware by Acid Alchamy  \u{00B7}  changes save as you go",
                    crate::VERSION
                ))
                .small()
                .color(FAINT),
            );
        });
    }
}

fn screen_label(i: usize, m: &Monitor) -> String {
    let (w, h) = (m.rect.right - m.rect.left, m.rect.bottom - m.rect.top);
    format!("Screen {}  \u{00B7}  {w}\u{00D7}{h}{}", i + 1, if m.primary { "  \u{00B7}  main" } else { "" })
}

fn crosshair(p: &egui::Painter, c: Pos2) {
    p.circle_stroke(c, 7.0, Stroke::new(2.0, ACID));
    p.line_segment([c - vec2(11.0, 0.0), c - vec2(4.0, 0.0)], Stroke::new(2.0, ACID));
    p.line_segment([c + vec2(4.0, 0.0), c + vec2(11.0, 0.0)], Stroke::new(2.0, ACID));
    p.line_segment([c - vec2(0.0, 11.0), c - vec2(0.0, 4.0)], Stroke::new(2.0, ACID));
    p.line_segment([c + vec2(0.0, 4.0), c + vec2(0.0, 11.0)], Stroke::new(2.0, ACID));
}
