use std::time::Instant;

use eframe::egui;
use skia_safe::{
    AlphaType, Color, ColorType, ImageInfo, Paint, PaintStyle, PathBuilder, Point, Rect, surfaces,
};

use crate::{
    browser::BrowserState,
    ds::{components::SearchField, theming::Theme},
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

/// The new-tab scene deliberately lives outside the browser renderer. It stays available when
/// Chromium is unavailable and keeps its animated artwork entirely native.
pub struct NewTabScene {
    started_at: Instant,
    last_rendered_at: Option<Instant>,
    search_input: String,
    texture: Option<egui::TextureHandle>,
}

impl NewTabScene {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            last_rendered_at: None,
            search_input: String::new(),
            texture: None,
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
                    "wind-tunnel-aurora",
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

        let home = paint_home(ui, rect, browser, theme, None, Some(&mut self.search_input));
        if home.submitted && !self.search_input.trim().is_empty() {
            browser.submit_address_input(&self.search_input);
            *address_input = browser.active_url_for_input();
        } else if let Some(url) = home.selected_shortcut {
            browser.navigate_active(url);
            *address_input = browser.active_url_for_input();
        }

        ui.ctx().request_repaint_after(WIND_TUNNEL_FRAME_INTERVAL);
    }
}

const WIND_TUNNEL_FRAME_INTERVAL: std::time::Duration = std::time::Duration::from_millis(33);
const WIND_TUNNEL_MAX_WIDTH: f32 = 960.0;
const WIND_TUNNEL_MAX_HEIGHT: f32 = 640.0;
const SKY_BANDS: usize = 84;
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
    let _ = paint_home(ui, rect, browser, theme, Some(message), None);
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

/// Render a deliberately modest-resolution image, then let egui scale it to the viewport.
/// Skia handles every stroke and blend here; keeping this at <= 960px wide protects laptop
/// batteries while preserving the soft, atmospheric quality of the scene.
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
    // The backdrop deliberately stays almost white at the horizon. It gives the blue folds
    // somewhere to breathe, matching the generous negative space in the Wind mark.
    for band in 0..SKY_BANDS {
        let t = band as f32 / (SKY_BANDS - 1) as f32;
        let r = (224.0 - 53.0 * t) as u8;
        let g = (230.0 - 42.0 * t) as u8;
        let b = (250.0 - 16.0 * t) as u8;
        let mut paint = Paint::default();
        paint.set_color(Color::from_argb(255, r, g, b));
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

    // Soft, hand-painted light behind the crest. Drawing concentric translucent circles is
    // cheaper than a full-resolution blur but retains the same diffused highlight once egui
    // scales the texture.
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
    // One shared vanishing point makes the nested curves read as material folding over itself.
    // Motion is intentionally sub-pixel-ish: it should feel alive, never like an aurora.
    let breathe = (time * 0.32).sin();
    let focus = Point::new(
        w * (0.90 + (pointer.x - 0.5) * 0.018),
        h * (0.70 + breathe * 0.009 + (pointer.y - 0.5) * 0.012),
    );

    paint_fold(
        canvas,
        bounds,
        focus,
        0.31,
        0.47,
        Color::from_argb(255, 145, 162, 231),
    );
    paint_fold(
        canvas,
        bounds,
        focus,
        0.47,
        0.63,
        Color::from_argb(255, 112, 135, 221),
    );
    paint_fold(
        canvas,
        bounds,
        focus,
        0.63,
        0.84,
        Color::from_argb(255, 73, 99, 196),
    );

    // A few broad, transparent strokes provide the pearlescent light at each fold's edge.
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
    search_input: Option<&mut String>,
) -> HomeInteraction {
    let color = &theme.tokens.semantic.color;
    let active = browser.active_tab();
    let mut interaction = HomeInteraction::default();
    let mut placeholder_search = String::new();

    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(rect.height() * 0.22);
            wind_mark(ui, theme);
            ui.add_space(theme.tokens.primitive.space.lg);
            let (search_input, enabled) = match search_input {
                Some(value) => (value, true),
                None => (&mut placeholder_search, false),
            };
            let search = SearchField::empty_page(search_input)
                .desired_width(520.0)
                .enabled(enabled)
                .show(ui, theme);
            interaction.submitted = enabled
                && search.lost_focus()
                && ui.input(|input| input.key_pressed(egui::Key::Enter));
            ui.add_space(theme.tokens.primitive.space.xl);
            interaction.selected_shortcut = launch_tiles(ui, theme);

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

    interaction
}

#[derive(Default)]
struct HomeInteraction {
    selected_shortcut: Option<&'static str>,
    submitted: bool,
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

fn launch_tiles(ui: &mut egui::Ui, theme: &Theme) -> Option<&'static str> {
    let tiles = [
        ("N", "https://www.notion.so"),
        ("▶", "https://www.youtube.com"),
        ("X", "https://x.com"),
        ("L", "https://linear.app"),
        ("M", "https://mail.google.com"),
        ("+", "arc://new-tab"),
    ];
    let mut selected = None;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 28.0;
        for (label, url) in tiles {
            if launch_tile(ui, label, theme).clicked() && label != "+" {
                selected = Some(url);
            }
        }
    });
    selected
}

fn launch_tile(ui: &mut egui::Ui, label: &str, theme: &Theme) -> egui::Response {
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
        response
    })
    .inner
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
