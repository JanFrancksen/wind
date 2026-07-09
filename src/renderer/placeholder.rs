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
const TWILIGHT_BANDS: usize = 72;
const AURORA_RIBBONS: usize = 7;
const STAR_COUNT: usize = 84;
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

    paint_twilight(canvas, bounds);
    paint_contours(canvas, bounds, time, pointer);
    paint_aurora(canvas, bounds, time, pointer);
    paint_stars(canvas, bounds, time);

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

fn paint_twilight(canvas: &skia_safe::Canvas, bounds: Rect) {
    for band in 0..TWILIGHT_BANDS {
        let t = band as f32 / (TWILIGHT_BANDS - 1) as f32;
        let r = (7.0 + 6.0 * t) as u8;
        let g = (15.0 + 20.0 * t) as u8;
        let b = (31.0 + 28.0 * t) as u8;
        let mut paint = Paint::default();
        paint.set_color(Color::from_argb(255, r, g, b));
        canvas.draw_rect(
            Rect::from_xywh(
                0.0,
                bounds.height() * t,
                bounds.width(),
                bounds.height() / TWILIGHT_BANDS as f32 + 1.0,
            ),
            &paint,
        );
    }
}

fn paint_contours(canvas: &skia_safe::Canvas, bounds: Rect, time: f32, pointer: egui::Pos2) {
    let drift = (pointer.x - 0.5) * 38.0;
    for row in 0..17 {
        let y = bounds.height() * (0.26 + row as f32 * 0.056);
        let mut path = PathBuilder::new();
        path.move_to(Point::new(-20.0, y));
        for segment in 0..8 {
            let x = segment as f32 * bounds.width() / 7.0;
            let crest = y
                + (time * 0.45 + row as f32 * 0.61 + segment as f32).sin() * 10.0
                + drift * (segment as f32 / 7.0 - 0.5);
            path.cubic_to(
                Point::new(x + bounds.width() * 0.04, crest - 9.0),
                Point::new(x + bounds.width() * 0.10, crest + 9.0),
                Point::new(x + bounds.width() / 7.0, crest),
            );
        }
        let path = path.detach();
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_style(PaintStyle::Stroke);
        paint.set_stroke_width(0.75);
        paint.set_color(Color::from_argb(20, 142, 230, 207));
        canvas.draw_path(&path, &paint);
    }
}

fn paint_aurora(canvas: &skia_safe::Canvas, bounds: Rect, time: f32, pointer: egui::Pos2) {
    let pointer_pull = (pointer.y - 0.5) * 52.0;
    for ribbon in 0..AURORA_RIBBONS {
        let phase = time * (0.34 + ribbon as f32 * 0.025) + ribbon as f32 * 0.91;
        let base_y = bounds.height() * (0.36 + ribbon as f32 * 0.045) + pointer_pull;
        let mut path = PathBuilder::new();
        path.move_to(Point::new(-40.0, base_y));
        for section in 0..7 {
            let x = section as f32 * bounds.width() / 6.0;
            let wave = phase.sin() * 35.0
                + (phase * 1.7 + section as f32 * 0.9).cos() * 22.0
                + (pointer.x - 0.5) * 70.0 * (section as f32 / 6.0);
            path.cubic_to(
                Point::new(x + bounds.width() * 0.06, base_y + wave * 1.15),
                Point::new(x + bounds.width() * 0.11, base_y - wave * 0.55),
                Point::new(x + bounds.width() / 6.0, base_y + wave),
            );
        }

        let path = path.detach();
        for (width, alpha) in [(46.0, 10), (24.0, 21), (8.0, 68), (2.0, 175)] {
            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(width);
            let color = if ribbon % 2 == 0 {
                Color::from_argb(alpha, 83, 255, 196)
            } else {
                Color::from_argb(alpha, 78, 179, 255)
            };
            paint.set_color(color);
            canvas.draw_path(&path, &paint);
        }
    }
}

fn paint_stars(canvas: &skia_safe::Canvas, bounds: Rect, time: f32) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    for index in 0..STAR_COUNT {
        let seed = index as f32 * 12.9898;
        let x = fract(seed.sin() * 43_758.547) * bounds.width();
        let y = fract((seed + 1.0).cos() * 18_293.64) * bounds.height() * 0.63;
        let pulse = ((time * 1.6 + seed).sin() + 1.0) * 0.5;
        paint.set_color(Color::from_argb(
            (35.0 + pulse * 110.0) as u8,
            205,
            245,
            240,
        ));
        canvas.draw_circle(Point::new(x, y), 0.45 + pulse * 0.9, &paint);
    }
}

fn fract(value: f32) -> f32 {
    value - value.floor()
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
