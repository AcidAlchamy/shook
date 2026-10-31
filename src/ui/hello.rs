//! The hello card: a small note in the corner when Shook starts, saying it's
//! running and that it's donationware. It goes away by itself after a few
//! seconds (not while you're hovering it).

use super::theme::{self, ACID, FAINT, LINE, MUTED, TEXT};
use super::widgets;
use crate::win;
use eframe::egui::{self, Color32, CornerRadius, Frame, Margin, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::time::Duration;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};

const SHOW_MS: u32 = 9_000;
const TICK_MS: u32 = 100;

pub struct Hello {
    hovered: Arc<AtomicBool>,
    shown_ms: Arc<AtomicU32>,
}

impl Hello {
    /// The countdown runs on its own thread and closes the window with WM_CLOSE,
    /// so it ends on time even if the window never gets to repaint.
    pub fn new(hwnd: Option<HWND>) -> Self {
        let hovered = Arc::new(AtomicBool::new(false));
        let shown_ms = Arc::new(AtomicU32::new(0));
        if let Some(hwnd) = hwnd {
            let (hovered, shown_ms, hwnd) = (hovered.clone(), shown_ms.clone(), hwnd as isize);
            std::thread::spawn(move || {
                while shown_ms.load(Relaxed) < SHOW_MS {
                    std::thread::sleep(Duration::from_millis(TICK_MS as u64));
                    if !hovered.load(Relaxed) {
                        shown_ms.fetch_add(TICK_MS, Relaxed);
                    }
                }
                unsafe { PostMessageW(hwnd as HWND, WM_CLOSE, 0, 0) };
            });
        }
        Self { hovered, shown_ms }
    }
}

impl eframe::App for Hello {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        #[cfg(feature = "scale-test")]
        super::test_shot::frame(ui.ctx());
        let now = ui.input(|i| i.time);
        let hovered = ui.input(|i| i.pointer.has_pointer());
        self.hovered.store(hovered, Relaxed);

        let frame = Frame::new()
            .fill(Color32::from_rgb(0x12, 0x0F, 0x1B))
            .stroke(Stroke::new(1.0, LINE))
            .inner_margin(Margin { left: 18, right: 14, top: 16, bottom: 14 });
        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            let full = ui.max_rect().expand2(vec2(18.0, 16.0));
            widgets::brand_rule(ui.painter(), Rect::from_min_max(full.left_top(), pos2(full.right(), full.top() + 3.0)));

            ui.horizontal(|ui| {
                let (logo, _) = ui.allocate_exact_size(vec2(54.0, 54.0), Sense::hover());
                let wiggle = (1.0 - (now as f32 / 1.4)).max(0.0);
                widgets::logo(ui.painter(), logo, wiggle, now);
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Shook is on duty").font(theme::bold(17.0)).color(TEXT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                            let close = ui.add(egui::Button::new(RichText::new("\u{00D7}").size(18.0).color(MUTED)).frame(false));
                            if close.clicked() {
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        });
                    });
                    ui.label(RichText::new("Lost the pointer? Give the mouse a quick shake.").color(MUTED));
                });
            });
            ui.add_space(8.0);
            ui.add(
                egui::Label::new(
                    RichText::new(
                        "Shook is donationware by Acid Alchamy: free to use, for good. If it saves you a hunt, \
                         a small donation keeps the lab building.",
                    )
                    .color(TEXT),
                )
                .wrap(),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if widgets::donate(ui, "Donate").clicked() {
                    win::open_url(crate::DONATE_URL);
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if widgets::ghost(ui, "Settings").clicked() {
                    win::spawn_self(&["--settings"]);
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("thank you \u{2665}").small().color(FAINT));
                });
            });

            // Time left, as a thin lime line along the bottom edge.
            let left = (1.0 - self.shown_ms.load(Relaxed) as f32 / SHOW_MS as f32).clamp(0.0, 1.0);
            let bar = Rect::from_min_size(pos2(full.left(), full.bottom() - 2.0), vec2(full.width() * left, 2.0));
            ui.painter().rect_filled(bar, CornerRadius::ZERO, ACID.gamma_multiply(if hovered { 0.35 } else { 0.8 }));
        });
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(if hovered { 200 } else { 33 }));
    }
}
