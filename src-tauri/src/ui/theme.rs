use egui::{Color32, CornerRadius, Context, Stroke, Visuals};

pub const BG: Color32 = Color32::from_rgb(10, 10, 10);
pub const PANEL: Color32 = Color32::from_rgb(15, 15, 15);
pub const CARD: Color32 = Color32::from_rgb(24, 24, 24);
pub const CARD_HOVER: Color32 = Color32::from_rgb(32, 32, 32);
pub const BORDER: Color32 = Color32::from_rgb(40, 40, 40);
pub const TEXT: Color32 = Color32::from_rgb(250, 250, 250);
pub const MUTED: Color32 = Color32::from_rgb(115, 115, 115);
pub const GREEN: Color32 = Color32::from_rgb(74, 222, 128);
pub const GREEN_DIM: Color32 = Color32::from_rgb(30, 80, 50);
pub const ERROR: Color32 = Color32::from_rgb(239, 68, 68);
pub const WARNING: Color32 = Color32::from_rgb(234, 179, 8);

pub fn apply(ctx: &Context) {
    let mut visuals = Visuals::dark();
    visuals.window_fill = BG;
    visuals.panel_fill = PANEL;
    visuals.faint_bg_color = CARD;
    visuals.extreme_bg_color = BG;
    visuals.code_bg_color = CARD;
    visuals.override_text_color = Some(TEXT);
    visuals.hyperlink_color = GREEN;

    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(74, 222, 128, 60);
    visuals.selection.stroke = Stroke::new(1.0, GREEN);

    visuals.widgets.noninteractive.bg_fill = CARD;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);

    visuals.widgets.inactive.bg_fill = CARD;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(6);

    visuals.widgets.hovered.bg_fill = CARD_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, GREEN);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, TEXT);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(6);

    visuals.widgets.active.bg_fill = GREEN_DIM;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, GREEN);
    visuals.widgets.active.fg_stroke = Stroke::new(2.0, GREEN);
    visuals.widgets.active.corner_radius = CornerRadius::same(6);

    visuals.widgets.open.bg_fill = CARD_HOVER;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, GREEN);
    visuals.widgets.open.corner_radius = CornerRadius::same(6);

    visuals.window_corner_radius = CornerRadius::same(8);
    visuals.menu_corner_radius = CornerRadius::same(6);

    ctx.set_visuals(visuals);
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
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
}
