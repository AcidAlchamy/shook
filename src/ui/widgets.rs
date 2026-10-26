//! Shook's own widgets: the logo, cards, toggles, segmented buttons, keycaps.

use super::theme::{self, ACID, CARD, CARD_HI, FAINT, INK, LINE, MUTED, PINK, TEXT, VIOLET};
use eframe::egui::{
    self, Color32, CornerRadius, Frame, Margin, Mesh, Pos2, Rect, Response, RichText, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2, epaint::QuadraticBezierShape, pos2, vec2,
};

/// The classic arrow pointer in a 12.5 x 19 box.
const ARROW: [(f32, f32); 7] = [(0.0, 0.0), (0.0, 16.0), (4.5, 12.0), (7.5, 19.0), (10.0, 18.0), (7.0, 11.2), (12.5, 11.2)];
/// Triangles covering the arrow (it isn't convex).
const ARROW_TRIS: [[u32; 3]; 5] = [[0, 1, 2], [0, 2, 5], [0, 5, 6], [2, 3, 4], [2, 4, 5]];

/// A convex polygon filled with a diagonal violet-to-pink gradient.
fn gradient_polygon(points: &[Pos2], bounds: Rect, from: Color32, to: Color32) -> Shape {
    let mut mesh = Mesh::default();
    let color_at = |p: Pos2| {
        let t = ((p.x - bounds.left()) + (p.y - bounds.top())) / (bounds.width() + bounds.height());
        theme::lerp(from, to, t)
    };
    let c = bounds.center();
    mesh.colored_vertex(c, color_at(c));
    for &p in points {
        mesh.colored_vertex(p, color_at(p));
    }
    let n = points.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    Shape::mesh(mesh)
}

fn rounded_rect_points(r: Rect, radius: f32) -> Vec<Pos2> {
    let mut pts = Vec::with_capacity(40);
    let corners = [
        (pos2(r.right() - radius, r.top() + radius), -90.0f32),
        (pos2(r.right() - radius, r.bottom() - radius), 0.0),
        (pos2(r.left() + radius, r.bottom() - radius), 90.0),
        (pos2(r.left() + radius, r.top() + radius), 180.0),
    ];
    for (c, start) in corners {
        for i in 0..=8 {
            let a = (start + 90.0 * i as f32 / 8.0).to_radians();
            pts.push(c + vec2(a.cos(), a.sin()) * radius);
        }
    }
    pts
}

/// Paints the Shook mark into `rect`. `wiggle` (0..1) shakes the arcs.
pub fn logo(painter: &egui::Painter, rect: Rect, wiggle: f32, time: f64) {
    let s = rect.width() / 256.0;
    let at = |x: f32, y: f32| rect.min + vec2(x, y) * s;
    let tile = Rect::from_min_max(at(6.0, 6.0), at(250.0, 250.0));
    painter.add(gradient_polygon(&rounded_rect_points(tile, 58.0 * s), tile, VIOLET, PINK));

    let shake = (time * 38.0).sin() as f32 * 7.0 * wiggle;
    let arc = |x0: f32, x1: f32, y0: f32, y1: f32, w: f32, alpha: f32, dx: f32| {
        QuadraticBezierShape::from_points_stroke(
            [at(x0 + dx, y0), at(x1 + dx, 128.0), at(x0 + dx, y1)],
            false,
            Color32::TRANSPARENT,
            Stroke::new(w * s, ACID.gamma_multiply(alpha)),
        )
    };
    painter.add(arc(64.0, 46.0, 94.0, 162.0, 14.0, 1.0, -shake));
    painter.add(arc(192.0, 210.0, 94.0, 162.0, 14.0, 1.0, shake));
    painter.add(arc(40.0, 16.0, 76.0, 180.0, 11.0, 0.75, -shake * 1.4));
    painter.add(arc(216.0, 240.0, 76.0, 180.0, 11.0, 0.75, shake * 1.4));

    let k = 6.2;
    let (tx, ty) = (128.0 - 6.25 * k, 128.0 - 9.5 * k);
    let rot = (-6.0f32).to_radians();
    let (sn, cs) = rot.sin_cos();
    let pts: Vec<Pos2> = ARROW
        .iter()
        .map(|&(x, y)| {
            let (x, y) = (x * k - 6.25 * k, y * k - 9.5 * k);
            at(tx + 6.25 * k + x * cs - y * sn, ty + 9.5 * k + x * sn + y * cs)
        })
        .collect();
    let mut mesh = Mesh::default();
    for &p in &pts {
        mesh.colored_vertex(p, Color32::WHITE);
    }
    for t in ARROW_TRIS {
        mesh.add_triangle(t[0], t[1], t[2]);
    }
    painter.add(Shape::mesh(mesh));
    painter.add(Shape::closed_line(pts, Stroke::new(9.0 * s, INK)));
}

/// A rounded panel with a title in small caps.
pub fn card<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin { left: 18, right: 18, top: 14, bottom: 16 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title.to_uppercase()).font(theme::bold(11.5)).color(ACID.gamma_multiply(0.85)));
            ui.add_space(2.0);
            add(ui)
        })
        .inner
}

/// Left: a label with a muted hint under it. Right: whatever `add` puts there.
pub fn row<R>(ui: &mut Ui, label: &str, hint: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width((ui.available_width() * 0.42).max(150.0));
            ui.label(RichText::new(label).color(TEXT));
            if !hint.is_empty() {
                ui.label(RichText::new(hint).small().color(FAINT));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add).inner
    })
    .inner
}

/// An on/off switch.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let size = vec2(44.0, 24.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let track = theme::lerp(Color32::from_rgb(0x2B, 0x25, 0x3A), ACID, t);
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(12),
        track,
        Stroke::new(1.0, if resp.hovered() { VIOLET } else { LINE }),
        StrokeKind::Inside,
    );
    let x = egui::lerp((rect.left() + 12.0)..=(rect.right() - 12.0), t);
    painter.circle_filled(pos2(x, rect.center().y), 8.5, theme::lerp(MUTED, INK, t));
    resp
}

/// A row of mutually exclusive choices, the selected one lit.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    Frame::new()
        .fill(Color32::from_rgb(0x0F, 0x0C, 0x17))
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(11))
        .inner_margin(Margin::same(3))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            ui.horizontal(|ui| {
                for &(v, label) in options {
                    let selected = *value == v;
                    let text = RichText::new(label).font(theme::semibold(14.0)).color(if selected { INK } else { MUTED });
                    let button = egui::Button::new(text)
                        .fill(if selected { ACID } else { Color32::TRANSPARENT })
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::same(8))
                        .min_size(vec2(96.0, 30.0));
                    if ui.add(button).clicked() && !selected {
                        *value = v;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// A filled call-to-action button.
pub fn primary(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).font(theme::semibold(14.5)).color(INK))
            .fill(ACID)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(10))
            .min_size(vec2(0.0, 32.0)),
    )
}

/// The donate button: pink, with a heart.
pub fn donate(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(format!("\u{2665}  {text}")).font(theme::semibold(14.5)).color(Color32::WHITE))
            .fill(PINK)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(10))
            .min_size(vec2(0.0, 32.0)),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A quiet outlined button.
pub fn ghost(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(TEXT))
            .fill(CARD_HI)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(CornerRadius::same(10))
            .min_size(vec2(0.0, 32.0)),
    )
}

/// Keys drawn as keycaps: "Ctrl" "Alt" "M".
pub fn keycaps(ui: &mut Ui, combo: &str) {
    ui.spacing_mut().item_spacing.x = 4.0;
    for (i, key) in combo.split('+').enumerate() {
        if i > 0 {
            ui.label(RichText::new("+").color(FAINT));
        }
        Frame::new()
            .fill(CARD_HI)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::symmetric(8, 3))
            .show(ui, |ui| ui.label(RichText::new(key.trim()).font(theme::semibold(13.0)).color(TEXT)));
    }
}

/// A small status dot and text.
pub fn status(ui: &mut Ui, color: Color32, text: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 4.5, color);
        ui.label(RichText::new(text).color(MUTED));
    });
}

/// A clickable link-styled label.
pub fn link(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Label::new(RichText::new(text).color(MUTED).underline()).sense(Sense::click()))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A thin horizontal brand gradient (violet to pink to lime).
pub fn brand_rule(painter: &egui::Painter, rect: Rect) {
    let mut mesh = Mesh::default();
    let stops = [(0.0, VIOLET), (0.6, PINK), (1.0, ACID)];
    for (i, &(t, c)) in stops.iter().enumerate() {
        let x = egui::lerp(rect.left()..=rect.right(), t);
        mesh.colored_vertex(pos2(x, rect.top()), c);
        mesh.colored_vertex(pos2(x, rect.bottom()), c);
        if i > 0 {
            let b = (i as u32) * 2;
            mesh.add_triangle(b - 2, b - 1, b);
            mesh.add_triangle(b - 1, b, b + 1);
        }
    }
    painter.add(Shape::mesh(mesh));
}
