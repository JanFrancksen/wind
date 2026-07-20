use std::time::Duration;

use eframe::egui;

use crate::ds::{icons::Icon, theming::Theme};

const ICON_SIZE: f32 = 14.0;
const COPY_OUT_SECONDS: f32 = 0.14;
const CHECK_IN_SECONDS: f32 = 0.16;
const CHECK_HOLD_SECONDS: f32 = 1.0;
const CHECK_OUT_SECONDS: f32 = CHECK_IN_SECONDS;
const COPY_IN_SECONDS: f32 = COPY_OUT_SECONDS;
const ANIMATION_SECONDS: f32 =
    COPY_OUT_SECONDS + CHECK_IN_SECONDS + CHECK_HOLD_SECONDS + CHECK_OUT_SECONDS + COPY_IN_SECONDS;

#[derive(Clone, Copy, Debug)]
struct CopyAnimation {
    started_at: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct IconFrame {
    icon: Icon,
    scale: f32,
    blur: f32,
}

pub struct CopyUrlButton<'a> {
    url: &'a str,
}

impl<'a> CopyUrlButton<'a> {
    pub fn new(url: &'a str) -> Self {
        Self { url }
    }

    pub fn show(self, ui: &mut egui::Ui, theme: &Theme) -> egui::Response {
        let tokens = &theme.tokens;
        let size = tokens.primitive.size.control_sm;
        let enabled = !self.url.is_empty();
        let (rect, mut response) = ui.allocate_exact_size(
            egui::Vec2::splat(size),
            if enabled {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            },
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Copy URL")
        });

        let animation_id = response.id.with("copy-url-animation");
        let now = ui.input(|input| input.time);
        if response.clicked() && enabled {
            ui.ctx().copy_text(self.url.to_owned());
            ui.ctx().data_mut(|data| {
                data.insert_temp(animation_id, CopyAnimation { started_at: now });
            });
        }

        let elapsed = ui.ctx().data(|data| {
            data.get_temp::<CopyAnimation>(animation_id)
                .map(|animation| (now - animation.started_at).max(0.0) as f32)
        });
        let frame = elapsed
            .filter(|elapsed| *elapsed < ANIMATION_SECONDS)
            .map(icon_frame)
            .unwrap_or(IconFrame {
                icon: Icon::Copy,
                scale: 1.0,
                blur: 0.0,
            });

        if elapsed.is_some_and(|elapsed| elapsed < ANIMATION_SECONDS) {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }

        let hover = ui.ctx().animate_bool_with_time_and_easing(
            response.id.with("hover"),
            response.hovered() && enabled,
            0.12,
            egui::emath::easing::cubic_out,
        );
        let color = &tokens.semantic.color;
        let fill = color
            .surface
            .lerp_to_gamma(color.surface_hover, hover)
            .gamma_multiply(if enabled { 1.0 } else { 0.55 });
        let border = color
            .border
            .lerp_to_gamma(color.text_muted.gamma_multiply(0.55), hover);
        let painter = ui.painter();
        painter.rect_filled(rect, tokens.component.button.radius, fill);
        painter.rect_stroke(
            rect,
            tokens.component.button.radius,
            egui::Stroke::new(tokens.primitive.stroke.hairline, border),
            egui::StrokeKind::Inside,
        );

        paint_icon(
            ui,
            rect.center(),
            frame,
            color.text.gamma_multiply(if enabled { 1.0 } else { 0.45 }),
        );

        if response.has_focus() {
            painter.rect_stroke(
                rect.shrink(tokens.primitive.stroke.hairline),
                tokens.component.button.radius,
                egui::Stroke::new(tokens.primitive.stroke.thin, color.focus),
                egui::StrokeKind::Inside,
            );
        }

        response = response.on_hover_text(if enabled {
            "Copy URL"
        } else {
            "No URL to copy"
        });
        response
    }
}

fn icon_frame(elapsed: f32) -> IconFrame {
    let copy_out_end = COPY_OUT_SECONDS;
    let check_in_end = copy_out_end + CHECK_IN_SECONDS;
    let hold_end = check_in_end + CHECK_HOLD_SECONDS;
    let check_out_end = hold_end + CHECK_OUT_SECONDS;

    if elapsed < copy_out_end {
        let progress = eased(elapsed / COPY_OUT_SECONDS);
        IconFrame {
            icon: Icon::Copy,
            scale: egui::lerp(1.0..=0.9, progress),
            blur: progress,
        }
    } else if elapsed < check_in_end {
        let progress = eased((elapsed - copy_out_end) / CHECK_IN_SECONDS);
        IconFrame {
            icon: Icon::Check,
            scale: egui::lerp(0.9..=1.0, progress),
            blur: 1.0 - progress,
        }
    } else if elapsed < hold_end {
        IconFrame {
            icon: Icon::Check,
            scale: 1.0,
            blur: 0.0,
        }
    } else if elapsed < check_out_end {
        let progress = eased((elapsed - hold_end) / CHECK_OUT_SECONDS);
        IconFrame {
            icon: Icon::Check,
            scale: egui::lerp(1.0..=0.9, progress),
            blur: progress,
        }
    } else {
        let progress = eased((elapsed - check_out_end) / COPY_IN_SECONDS);
        IconFrame {
            icon: Icon::Copy,
            scale: egui::lerp(0.9..=1.0, progress),
            blur: 1.0 - progress,
        }
    }
}

fn eased(progress: f32) -> f32 {
    egui::emath::easing::cubic_in_out(progress.clamp(0.0, 1.0))
}

fn paint_icon(ui: &egui::Ui, center: egui::Pos2, frame: IconFrame, color: egui::Color32) {
    let size = ICON_SIZE * frame.scale;
    let rect = egui::Rect::from_center_size(center, egui::Vec2::splat(size));
    let blur = frame.blur.clamp(0.0, 1.0);

    if blur > 0.0 {
        let radius = 1.35 * blur;
        let haze = color.gamma_multiply(0.09 * blur);
        for offset in [
            egui::vec2(-radius, 0.0),
            egui::vec2(radius, 0.0),
            egui::vec2(0.0, -radius),
            egui::vec2(0.0, radius),
            egui::vec2(-radius * 0.7, -radius * 0.7),
            egui::vec2(radius * 0.7, -radius * 0.7),
            egui::vec2(-radius * 0.7, radius * 0.7),
            egui::vec2(radius * 0.7, radius * 0.7),
        ] {
            frame
                .icon
                .image(size, haze)
                .paint_at(ui, rect.translate(offset));
        }
    }

    frame
        .icon
        .image(size, color.gamma_multiply(1.0 - blur * 0.48))
        .paint_at(ui, rect);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_animation_reverses_after_the_check_holds_for_one_second() {
        let initial = icon_frame(0.0);
        assert_eq!(initial.icon, Icon::Copy);
        assert_eq!(initial.scale, 1.0);
        assert_eq!(initial.blur, 0.0);

        let checked = icon_frame(COPY_OUT_SECONDS + CHECK_IN_SECONDS);
        assert_eq!(checked.icon, Icon::Check);
        assert_eq!(checked.scale, 1.0);
        assert_eq!(checked.blur, 0.0);

        let reversing = icon_frame(
            COPY_OUT_SECONDS + CHECK_IN_SECONDS + CHECK_HOLD_SECONDS + CHECK_OUT_SECONDS / 2.0,
        );
        assert_eq!(reversing.icon, Icon::Check);
        assert!(reversing.scale < 1.0);
        assert!(reversing.blur > 0.0);

        let restored = icon_frame(ANIMATION_SECONDS);
        assert_eq!(restored.icon, Icon::Copy);
        assert_eq!(restored.scale, 1.0);
        assert_eq!(restored.blur, 0.0);
    }
}
