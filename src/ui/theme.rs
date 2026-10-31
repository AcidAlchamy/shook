//! Shook's look: ink background, violet-to-pink brand gradient, acid-lime accent,
//! Segoe UI from the system (no fonts are bundled).

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke, TextStyle, Vec2,
};
use std::sync::Arc;

pub const BG: Color32 = Color32::from_rgb(0x0D, 0x0B, 0x14);
pub const CARD: Color32 = Color32::from_rgb(0x16, 0x13, 0x20);
pub const CARD_HI: Color32 = Color32::from_rgb(0x1F, 0x1A, 0x2C);
pub const LINE: Color32 = Color32::from_rgb(0x2A, 0x24, 0x3B);
pub const TEXT: Color32 = Color32::from_rgb(0xEE, 0xEC, 0xF4);
pub const MUTED: Color32 = Color32::from_rgb(0xA3, 0x9D, 0xB6);
pub const FAINT: Color32 = Color32::from_rgb(0x6E, 0x68, 0x84);
pub const ACID: Color32 = Color32::from_rgb(0xC6, 0xFF, 0x3A);
pub const VIOLET: Color32 = Color32::from_rgb(0x8B, 0x3D, 0xFF);
pub const PINK: Color32 = Color32::from_rgb(0xFF, 0x2E, 0x7E);
pub const INK: Color32 = Color32::from_rgb(0x14, 0x0A, 0x2A);
pub const BAD: Color32 = Color32::from_rgb(0xFF, 0x5A, 0x5A);

pub const SEMIBOLD: &str = "semibold";
pub const BOLD: &str = "bold";

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD.into()))
}

pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()), m(a.a(), b.a()))
}

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    if let Some(scale) = crate::win::test_scale() {
        ctx.set_zoom_factor(scale);
    }
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        use TextStyle::*;
        style.text_styles = [
            (Small, FontId::proportional(12.0)),
            (Body, FontId::proportional(14.5)),
            (Button, FontId::proportional(14.5)),
            (Heading, FontId::new(22.0, FontFamily::Name(BOLD.into()))),
            (Monospace, FontId::monospace(13.0)),
        ]
        .into();

        let s = &mut style.spacing;
        s.item_spacing = Vec2::new(10.0, 9.0);
        s.button_padding = Vec2::new(14.0, 7.0);
        s.interact_size = Vec2::new(36.0, 30.0);
        s.slider_width = 250.0;
        s.slider_rail_height = 6.0;
        s.combo_width = 230.0;
        s.window_margin = Margin::same(12);
        s.menu_margin = Margin::same(8);

        let v = &mut style.visuals;
        v.dark_mode = true;
        v.override_text_color = None;
        v.panel_fill = BG;
        v.window_fill = CARD;
        v.window_stroke = Stroke::new(1.0, LINE);
        v.window_corner_radius = CornerRadius::same(12);
        v.menu_corner_radius = CornerRadius::same(10);
        v.extreme_bg_color = Color32::from_rgb(0x0A, 0x08, 0x10);
        v.faint_bg_color = CARD_HI;
        v.code_bg_color = CARD_HI;
        v.hyperlink_color = ACID;
        v.selection.bg_fill = VIOLET.gamma_multiply(0.55);
        v.selection.stroke = Stroke::new(1.5, ACID);
        v.slider_trailing_fill = true;
        v.warn_fg_color = Color32::from_rgb(0xFF, 0xC4, 0x3A);
        v.error_fg_color = BAD;

        let w = &mut v.widgets;
        let r = CornerRadius::same(9);
        w.noninteractive.bg_fill = CARD;
        w.noninteractive.weak_bg_fill = CARD;
        w.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
        w.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
        w.noninteractive.corner_radius = r;

        w.inactive.bg_fill = CARD_HI;
        w.inactive.weak_bg_fill = CARD_HI;
        w.inactive.bg_stroke = Stroke::new(1.0, LINE);
        w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        w.inactive.corner_radius = r;

        w.hovered.bg_fill = Color32::from_rgb(0x2A, 0x23, 0x3D);
        w.hovered.weak_bg_fill = Color32::from_rgb(0x2A, 0x23, 0x3D);
        w.hovered.bg_stroke = Stroke::new(1.0, VIOLET);
        w.hovered.fg_stroke = Stroke::new(1.5, TEXT);
        w.hovered.corner_radius = r;
        w.hovered.expansion = 0.0;

        w.active.bg_fill = Color32::from_rgb(0x34, 0x2B, 0x4C);
        w.active.weak_bg_fill = Color32::from_rgb(0x34, 0x2B, 0x4C);
        w.active.bg_stroke = Stroke::new(1.0, ACID);
        w.active.fg_stroke = Stroke::new(1.5, TEXT);
        w.active.corner_radius = r;
        w.active.expansion = 0.0;

        w.open = w.active;
    });
}

/// Segoe UI (regular, semibold, bold), Segoe UI Symbol for ♥ ✓ ▶ and
/// friends, Consolas for monospace. All read from C:\Windows\Fonts.
fn install_fonts(ctx: &egui::Context) {
    let dir = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("Fonts");
    let mut fonts = FontDefinitions::empty();
    let mut load = |name: &str, file: &str| -> bool {
        match std::fs::read(dir.join(file)) {
            Ok(bytes) => {
                fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_owned(bytes)));
                true
            }
            Err(_) => false,
        }
    };
    let regular = load("segoe", "segoeui.ttf");
    let semi = load("segoe-semibold", "seguisb.ttf");
    let bold = load("segoe-bold", "segoeuib.ttf");
    let symbols = load("segoe-symbol", "seguisym.ttf");
    let mono = load("consolas", "consola.ttf");

    let chain = |first: (&str, bool)| -> Vec<String> {
        [first, ("segoe", regular), ("segoe-symbol", symbols)]
            .into_iter()
            .filter(|(_, ok)| *ok)
            .map(|(n, _)| n.to_owned())
            .fold(Vec::new(), |mut v, n| {
                if !v.contains(&n) {
                    v.push(n);
                }
                v
            })
    };
    fonts.families.insert(FontFamily::Proportional, chain(("segoe", regular)));
    fonts.families.insert(FontFamily::Name(SEMIBOLD.into()), chain(("segoe-semibold", semi)));
    fonts.families.insert(FontFamily::Name(BOLD.into()), chain(("segoe-bold", bold)));
    let mut monos = chain(("consolas", mono));
    if monos.is_empty() {
        monos = chain(("segoe", regular));
    }
    fonts.families.insert(FontFamily::Monospace, monos);
    ctx.set_fonts(fonts);
}
