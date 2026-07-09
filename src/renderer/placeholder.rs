use eframe::egui;

use crate::{
    browser::BrowserState,
    ds::theming::Theme,
    renderer::{PageTarget, RendererStatus},
};

pub struct PlaceholderRenderer;

impl PlaceholderRenderer {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&mut self, _frame: &mut eframe::Frame, target: &PageTarget) -> RendererStatus {
        if target.page.url.starts_with("http://") || target.page.url.starts_with("https://") {
            RendererStatus::Unavailable("CEF renderer is not enabled for this build".to_string())
        } else {
            RendererStatus::UnsupportedUrl(target.page.url.clone())
        }
    }

    pub fn show(&mut self) {}

    pub fn hide(&mut self) {}

    pub fn focus(&mut self) {}

    pub fn shutdown(&mut self) {}

    pub fn tick(&mut self) {}
}

pub fn paint_new_tab(ui: &mut egui::Ui, rect: egui::Rect, browser: &BrowserState, theme: &Theme) {
    paint_browser_canvas(ui, rect, theme);
    paint_home(ui, rect, browser, theme, None);
}

pub fn paint_status(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    browser: &BrowserState,
    theme: &Theme,
    status: &RendererStatus,
) {
    let message = match status {
        RendererStatus::Ready => "Page renderer ready",
        RendererStatus::WaitingForNativeBrowser => "Starting Chromium renderer",
        RendererStatus::UnsupportedUrl(_) => "This URL is handled by Wind",
        RendererStatus::Unavailable(message) => message,
    };

    paint_browser_canvas(ui, rect, theme);
    paint_home(ui, rect, browser, theme, Some(message));
}

fn paint_browser_canvas(ui: &mut egui::Ui, rect: egui::Rect, theme: &Theme) {
    let color = &theme.tokens.semantic.color;
    let painter = ui.painter();
    let steps = 22;

    for step in 0..steps {
        let t0 = step as f32 / steps as f32;
        let t1 = (step + 1) as f32 / steps as f32;
        let band = egui::Rect::from_min_max(
            egui::pos2(rect.left(), egui::lerp(rect.top()..=rect.bottom(), t0)),
            egui::pos2(rect.right(), egui::lerp(rect.top()..=rect.bottom(), t1)),
        );
        painter.rect_filled(
            band,
            0,
            lerp_color(color.app_background_top, color.app_background, t0),
        );
    }

    painter.circle_filled(
        egui::pos2(
            rect.left() + rect.width() * 0.30,
            rect.top() + rect.height() * 0.40,
        ),
        rect.width() * 0.07,
        color.cloud,
    );
    painter.circle_filled(
        egui::pos2(
            rect.right() - rect.width() * 0.13,
            rect.top() + rect.height() * 0.38,
        ),
        rect.width() * 0.06,
        color.cloud,
    );

    paint_mountains(ui, rect, theme);
}

fn paint_mountains(ui: &mut egui::Ui, rect: egui::Rect, theme: &Theme) {
    let color = &theme.tokens.semantic.color;
    let painter = ui.painter();
    let base = rect.bottom();
    let far_y = rect.top() + rect.height() * 0.75;
    let mid_y = rect.top() + rect.height() * 0.68;
    let near_y = rect.top() + rect.height() * 0.61;

    let far = vec![
        egui::pos2(rect.left(), base),
        egui::pos2(rect.left(), far_y + 30.0),
        egui::pos2(rect.left() + rect.width() * 0.15, far_y - 18.0),
        egui::pos2(rect.left() + rect.width() * 0.27, far_y + 10.0),
        egui::pos2(rect.left() + rect.width() * 0.42, far_y - 34.0),
        egui::pos2(rect.left() + rect.width() * 0.57, far_y + 4.0),
        egui::pos2(rect.left() + rect.width() * 0.73, far_y - 26.0),
        egui::pos2(rect.right(), far_y + 12.0),
        egui::pos2(rect.right(), base),
    ];
    painter.add(egui::Shape::convex_polygon(
        far,
        color.mountain_far,
        egui::Stroke::NONE,
    ));

    let mid = vec![
        egui::pos2(rect.left(), base),
        egui::pos2(rect.left(), mid_y + 80.0),
        egui::pos2(rect.left() + rect.width() * 0.25, mid_y + 10.0),
        egui::pos2(rect.left() + rect.width() * 0.43, mid_y + 64.0),
        egui::pos2(rect.left() + rect.width() * 0.58, mid_y - 10.0),
        egui::pos2(rect.left() + rect.width() * 0.76, mid_y + 54.0),
        egui::pos2(rect.right(), mid_y - 8.0),
        egui::pos2(rect.right(), base),
    ];
    painter.add(egui::Shape::convex_polygon(
        mid,
        color.mountain_mid,
        egui::Stroke::NONE,
    ));

    let near = vec![
        egui::pos2(rect.left() + rect.width() * 0.48, base),
        egui::pos2(rect.left() + rect.width() * 0.68, near_y + 84.0),
        egui::pos2(rect.left() + rect.width() * 0.84, near_y),
        egui::pos2(rect.right(), near_y + 72.0),
        egui::pos2(rect.right(), base),
    ];
    painter.add(egui::Shape::convex_polygon(
        near,
        color.mountain_near,
        egui::Stroke::NONE,
    ));
}

fn paint_home(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    browser: &BrowserState,
    theme: &Theme,
    message: Option<&str>,
) {
    let color = &theme.tokens.semantic.color;
    let active = browser.active_tab();

    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(rect.height() * 0.22);
            wind_mark(ui, theme);
            ui.add_space(theme.tokens.primitive.space.lg);
            search_capsule(ui, theme);
            ui.add_space(theme.tokens.primitive.space.xl);
            launch_tiles(ui, theme);

            if let Some(message) = message {
                ui.add_space(theme.tokens.primitive.space.xl);
                ui.label(
                    egui::RichText::new(message)
                        .color(color.text_muted)
                        .size(theme.tokens.primitive.typography.caption),
                );
                ui.label(
                    egui::RichText::new(&active.url)
                        .color(color.text_muted)
                        .size(theme.tokens.primitive.typography.caption),
                );
            }
        });
    });
}

fn wind_mark(ui: &mut egui::Ui, theme: &Theme) {
    let color = &theme.tokens.semantic.color;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(170.0, 96.0), egui::Sense::hover());
    let painter = ui.painter();
    let top = rect.top() + 6.0;
    let left = rect.center().x - 36.0;

    for offset in [0.0, 14.0, 28.0] {
        let y = top + offset;
        let points = vec![
            egui::pos2(left, y + 8.0),
            egui::pos2(left + 16.0, y - 4.0),
            egui::pos2(left + 36.0, y + 8.0),
            egui::pos2(left + 58.0, y - 4.0),
            egui::pos2(left + 78.0, y + 5.0),
        ];
        painter.add(egui::Shape::line(
            points,
            egui::Stroke::new(5.0, color.accent),
        ));
    }

    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 20.0),
        egui::Align2::CENTER_CENTER,
        "wind",
        egui::FontId::proportional(theme.tokens.primitive.typography.brand),
        color.text_strong,
    );
}

fn search_capsule(ui: &mut egui::Ui, theme: &Theme) {
    let width = ui.available_width().min(520.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 52.0), egui::Sense::click());
    let color = &theme.tokens.semantic.color;

    ui.painter()
        .rect_filled(rect, theme.tokens.primitive.radius.round, color.surface);
    ui.painter().rect_stroke(
        rect,
        theme.tokens.primitive.radius.round,
        egui::Stroke::new(theme.tokens.primitive.stroke.hairline, color.border),
        egui::StrokeKind::Inside,
    );
    ui.painter().circle_stroke(
        egui::pos2(rect.left() + 30.0, rect.center().y - 1.0),
        7.0,
        egui::Stroke::new(2.0, color.text_muted),
    );
    ui.painter().line_segment(
        [
            egui::pos2(rect.left() + 35.5, rect.center().y + 5.0),
            egui::pos2(rect.left() + 42.0, rect.center().y + 11.0),
        ],
        egui::Stroke::new(2.0, color.text_muted),
    );
    ui.painter().text(
        egui::pos2(rect.left() + 58.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "Search the web...",
        egui::FontId::proportional(theme.tokens.primitive.typography.body),
        color.text_muted,
    );
}

fn launch_tiles(ui: &mut egui::Ui, theme: &Theme) {
    let labels = ["N", "▶", "X", "L", "M", "+"];
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 28.0;
        for label in labels {
            launch_tile(ui, label, theme);
        }
    });
}

fn launch_tile(ui: &mut egui::Ui, label: &str, theme: &Theme) {
    let size = theme.tokens.primitive.size.tile;
    ui.vertical_centered(|ui| {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
        let color = &theme.tokens.semantic.color;
        ui.painter().rect_filled(
            rect,
            theme.tokens.primitive.radius.lg,
            if response.hovered() {
                color.tile_hover
            } else {
                color.tile
            },
        );
        ui.painter().rect_stroke(
            rect,
            theme.tokens.primitive.radius.lg,
            egui::Stroke::new(theme.tokens.primitive.stroke.hairline, color.border),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(24.0),
            if label == "+" {
                color.text_muted
            } else {
                color.text_strong
            },
        );
        ui.add_space(theme.tokens.primitive.space.xs);
        ui.label(
            egui::RichText::new(match label {
                "N" => "Notion",
                "▶" => "YouTube",
                "X" => "Twitter",
                "L" => "Linear",
                "M" => "Gmail",
                _ => "Add",
            })
            .color(color.text)
            .size(theme.tokens.primitive.typography.caption),
        );
    });
}

fn lerp_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let [ar, ag, ab, aa] = a.to_array();
    let [br, bg, bb, ba] = b.to_array();
    egui::Color32::from_rgba_unmultiplied(
        egui::lerp(ar as f32..=br as f32, t) as u8,
        egui::lerp(ag as f32..=bg as f32, t) as u8,
        egui::lerp(ab as f32..=bb as f32, t) as u8,
        egui::lerp(aa as f32..=ba as f32, t) as u8,
    )
}
