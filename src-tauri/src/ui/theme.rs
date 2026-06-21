use egui::{Color32, CornerRadius, Context, Stroke, Visuals};

pub const GREEN: Color32 = Color32::from_rgb(74, 222, 128);
pub const GREEN_DIM: Color32 = Color32::from_rgba_premultiplied(16, 48, 29, 220);
pub const ERROR: Color32 = Color32::from_rgb(239, 68, 68);
pub const WARNING: Color32 = Color32::from_rgb(234, 179, 8);

// Fallback constants for places that can't reach ctx.visuals
pub const BG: Color32 = Color32::from_gray(18);
pub const PANEL: Color32 = Color32::from_gray(25);
// CARD: rgba(26,30,38,230) premultiplied = (23,27,34,230)
pub const CARD: Color32 = Color32::from_rgba_premultiplied(23, 27, 34, 230);
pub const CARD_HOVER: Color32 = Color32::from_gray(50);
// BORDER: rgba(255,255,255,25) premultiplied = (25,25,25,25)
pub const BORDER: Color32 = Color32::from_rgba_premultiplied(25, 25, 25, 25);
pub const TEXT: Color32 = Color32::from_gray(235);
pub const MUTED: Color32 = Color32::from_gray(140);

pub fn apply(ctx: &Context, prefer_dark: bool) {
    let mut v = if prefer_dark { Visuals::dark() } else { Visuals::light() };

    // Green accents only - base palette comes from system dark/light
    v.hyperlink_color = GREEN;
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(74, 222, 128, 55);
    v.selection.stroke = Stroke::new(1.0, GREEN);

    v.widgets.active.bg_fill = Color32::from_rgba_unmultiplied(30, 80, 50, 200);
    v.widgets.active.bg_stroke = Stroke::new(1.0, GREEN);
    v.widgets.active.fg_stroke = Stroke::new(2.0, GREEN);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, GREEN);

    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(8);

    ctx.set_visuals(v);
}

pub fn apply_accents(_ctx: &Context) {
    // no-op: visuals applied once at startup
}

pub fn green_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(GREEN_DIM)
        .stroke(Stroke::new(1.0, GREEN))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::same(8))
}

pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::same(14))
}
