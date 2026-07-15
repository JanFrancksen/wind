use std::{collections::HashSet, time::Instant};

use eframe::egui;
use skia_safe::{
    AlphaType, Color, ColorType, ImageInfo, Paint, PaintStyle, PathBuilder, Point, Rect, surfaces,
};

use crate::{
    browser::{BrowserState, TabId},
    ds::{components::SearchField, theming::Theme},
};

/// The only page shown for `arc://new-tab`. It stays native so it works whether or not CEF is
/// available, and is intentionally separate from renderer fallback/status UI.
pub struct NewTabScene {
    started_at: Instant,
    last_rendered_at: Option<Instant>,
    search_input: String,
    texture: Option<egui::TextureHandle>,
    focused_sessions: HashSet<(TabId, u64)>,
}

impl NewTabScene {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            last_rendered_at: None,
            search_input: String::new(),
            texture: None,
            focused_sessions: HashSet::new(),
        }
    }

    pub fn paint(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        browser: &mut BrowserState,
        address_input: &mut String,
        theme: &Theme,
    ) {
        let should_render = self.texture.is_none()
            || self
                .last_rendered_at
                .is_none_or(|last| last.elapsed() >= WIND_TUNNEL_FRAME_INTERVAL);
        if should_render {
            let elapsed = self.started_at.elapsed().as_secs_f32();
            let pointer = ui
                .ctx()
                .input(|input| input.pointer.hover_pos())
                .unwrap_or(rect.center());
            let pointer = egui::pos2(
                ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
                ((pointer.y - rect.top()) / rect.height()).clamp(0.0, 1.0),
            );
            let image = render_wind_tunnel(rect.size(), elapsed, pointer);
            if let Some(texture) = &mut self.texture {
                texture.set(image, egui::TextureOptions::LINEAR);
            } else {
                self.texture = Some(ui.ctx().load_texture(
                    "wind-tunnel-new-tab",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
            }
            self.last_rendered_at = Some(Instant::now());
        }

        if let Some(texture) = &self.texture {
            ui.painter().image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }

        // A new tab becomes active as soon as it is created. Request focus once for each tab
        // rather than on every frame, so users can still move focus elsewhere on the page.
        let active_tab = browser.active_tab();
        let autofocus = self
            .focused_sessions
            .insert((active_tab.id, active_tab.session_revision));
        let submitted = paint_search(ui, rect, &mut self.search_input, theme, autofocus);
        if submitted && !self.search_input.trim().is_empty() {
            let outcome = browser.submit_address_input(&self.search_input);
            if outcome.active_page_changed() {
                *address_input = browser.active_url_for_input();
            }
            self.search_input.clear();
        }

        ui.ctx().request_repaint_after(WIND_TUNNEL_FRAME_INTERVAL);
    }
}

const WIND_TUNNEL_FRAME_INTERVAL: std::time::Duration = std::time::Duration::from_millis(33);
const WIND_TUNNEL_MAX_WIDTH: f32 = 960.0;
const WIND_TUNNEL_MAX_HEIGHT: f32 = 640.0;
const SKY_BANDS: usize = 84;

fn paint_search(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    input: &mut String,
    theme: &Theme,
    autofocus: bool,
) -> bool {
    let mut submitted = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(rect.height() * 0.19);
            ui.label(
                egui::RichText::new("Wind")
                    .color(egui::Color32::from_rgb(248, 250, 255))
                    .size(theme.tokens.primitive.typography.brand),
            );
            ui.add_space(theme.tokens.primitive.space.lg);
            let search = SearchField::empty_page(input)
                .desired_width(520.0)
                .show(ui, theme);
            if autofocus {
                search.request_focus();
            }
            submitted =
                search.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        });
    });
    submitted
}

fn render_wind_tunnel(size: egui::Vec2, time: f32, pointer: egui::Pos2) -> egui::ColorImage {
    let scale = (WIND_TUNNEL_MAX_WIDTH / size.x.max(1.0)).min(1.0);
    let width = (size.x * scale).round().clamp(2.0, WIND_TUNNEL_MAX_WIDTH) as i32;
    let height = (size.y * scale).round().clamp(2.0, WIND_TUNNEL_MAX_HEIGHT) as i32;
    let mut surface = surfaces::raster_n32_premul((width, height))
        .expect("Skia must create a raster surface for the new-tab scene");
    let canvas = surface.canvas();
    let bounds = Rect::from_wh(width as f32, height as f32);

    paint_logo_sky(canvas, bounds);
    paint_logo_folds(canvas, bounds, time, pointer);

    let info = ImageInfo::new(
        (width, height),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut pixels = vec![0; width as usize * height as usize * 4];
    surface.read_pixels(&info, &mut pixels, (width * 4) as usize, (0, 0));
    egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &pixels)
}

fn paint_logo_sky(canvas: &skia_safe::Canvas, bounds: Rect) {
    for band in 0..SKY_BANDS {
        let t = band as f32 / (SKY_BANDS - 1) as f32;
        let mut paint = Paint::default();
        paint.set_color(Color::from_argb(
            255,
            (224.0 - 53.0 * t) as u8,
            (230.0 - 42.0 * t) as u8,
            (250.0 - 16.0 * t) as u8,
        ));
        canvas.draw_rect(
            Rect::from_xywh(
                0.0,
                bounds.height() * t,
                bounds.width(),
                bounds.height() / SKY_BANDS as f32 + 1.0,
            ),
            &paint,
        );
    }
    paint_soft_glow(
        canvas,
        Point::new(bounds.width() * 0.48, bounds.height() * 0.30),
        bounds.width() * 0.38,
        Color::from_argb(11, 255, 255, 255),
    );
    paint_soft_glow(
        canvas,
        Point::new(bounds.width() * 0.90, bounds.height() * 0.64),
        bounds.width() * 0.19,
        Color::from_argb(13, 232, 239, 255),
    );
}

fn paint_logo_folds(canvas: &skia_safe::Canvas, bounds: Rect, time: f32, pointer: egui::Pos2) {
    let w = bounds.width();
    let h = bounds.height();
    let breathe = (time * 0.32).sin();
    let focus = Point::new(
        w * (0.90 + (pointer.x - 0.5) * 0.018),
        h * (0.70 + breathe * 0.009 + (pointer.y - 0.5) * 0.012),
    );
    for (upper, lower, color) in [
        (0.31, 0.47, Color::from_argb(255, 145, 162, 231)),
        (0.47, 0.63, Color::from_argb(255, 112, 135, 221)),
        (0.63, 0.84, Color::from_argb(255, 73, 99, 196)),
    ] {
        paint_fold(canvas, bounds, focus, upper, lower, color);
    }
    for (start, alpha, width) in [(0.31, 24, 36.0), (0.47, 22, 28.0), (0.63, 18, 22.0)] {
        let mut edge = PathBuilder::new();
        edge.move_to(Point::new(-w * 0.08, h * start));
        edge.cubic_to(
            Point::new(w * 0.30, h * (start - 0.16)),
            Point::new(w * 0.57, h * (start - 0.13)),
            focus,
        );
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_style(PaintStyle::Stroke);
        paint.set_stroke_width(width);
        paint.set_color(Color::from_argb(alpha, 244, 247, 255));
        canvas.draw_path(&edge.detach(), &paint);
    }
    let mut foreground = PathBuilder::new();
    foreground.move_to(Point::new(-w * 0.03, h * 1.05));
    foreground.cubic_to(
        Point::new(w * 0.12, h * 0.82),
        Point::new(w * 0.15, h * 0.54),
        Point::new(w * 0.45, h * 0.45),
    );
    foreground.cubic_to(
        Point::new(w * 0.66, h * 0.38),
        Point::new(w * 0.78, h * 0.55),
        focus,
    );
    foreground.cubic_to(
        Point::new(w * 0.96, h * 0.83),
        Point::new(w * 1.03, h * 0.79),
        Point::new(w * 1.03, h * 0.79),
    );
    foreground.line_to(Point::new(w * 1.03, h * 1.05));
    foreground.close();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::from_argb(255, 5, 13, 40));
    canvas.draw_path(&foreground.detach(), &paint);
    paint_soft_glow(
        canvas,
        Point::new(w * 0.88, h * 0.70),
        w * 0.07,
        Color::from_argb(18, 126, 148, 255),
    );
}

fn paint_fold(
    canvas: &skia_safe::Canvas,
    bounds: Rect,
    focus: Point,
    upper: f32,
    lower: f32,
    color: Color,
) {
    let w = bounds.width();
    let h = bounds.height();
    let mut path = PathBuilder::new();
    path.move_to(Point::new(-w * 0.08, h * upper));
    path.cubic_to(
        Point::new(w * 0.28, h * (upper - 0.17)),
        Point::new(w * 0.59, h * (upper - 0.12)),
        focus,
    );
    path.cubic_to(
        Point::new(w * 0.72, h * (lower - 0.02)),
        Point::new(w * 0.32, h * (lower - 0.04)),
        Point::new(-w * 0.08, h * lower),
    );
    path.close();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(color);
    canvas.draw_path(&path.detach(), &paint);
}

fn paint_soft_glow(canvas: &skia_safe::Canvas, center: Point, radius: f32, color: Color) {
    for step in (1..=20).rev() {
        let t = step as f32 / 20.0;
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(Color::from_argb(
            ((color.a() as f32) * (1.0 - t).powf(1.7)) as u8,
            color.r(),
            color.g(),
            color.b(),
        ));
        canvas.draw_circle(center, radius * t, &paint);
    }
}
