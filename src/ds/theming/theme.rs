use eframe::egui;

use super::tokens::{
    ButtonTokens, ColorPrimitives, ComponentTokens, InputTokens, PrimitiveTokens, RadiusTokens,
    SemanticColors, SemanticTokens, SizeTokens, SpaceTokens, StrokeTokens, TabTokens, Tokens,
    TypographyTokens,
};

#[derive(Clone)]
pub struct Theme {
    pub tokens: Tokens,
}

impl Theme {
    pub fn arc_dark() -> Self {
        let color = ColorPrimitives {
            neutral_0: egui::Color32::from_rgb(255, 255, 255),
            neutral_50: egui::Color32::from_rgb(244, 245, 247),
            neutral_100: egui::Color32::from_rgb(224, 227, 232),
            neutral_200: egui::Color32::from_rgb(185, 191, 200),
            neutral_300: egui::Color32::from_rgb(128, 137, 150),
            neutral_700: egui::Color32::from_rgb(45, 49, 58),
            neutral_800: egui::Color32::from_rgb(31, 34, 41),
            neutral_900: egui::Color32::from_rgb(19, 21, 26),
            blue_400: egui::Color32::from_rgb(109, 166, 255),
            blue_500: egui::Color32::from_rgb(72, 139, 255),
            red_400: egui::Color32::from_rgb(248, 105, 105),
        };

        let space = SpaceTokens {
            xxs: 2.0,
            xs: 4.0,
            sm: 8.0,
            md: 12.0,
            lg: 16.0,
            xl: 24.0,
        };

        let radius = RadiusTokens {
            sm: 4,
            md: 6,
            lg: 8,
        };

        let stroke = StrokeTokens {
            hairline: 1.0,
            thin: 1.5,
        };

        let typography = TypographyTokens {
            body: 14.0,
            body_strong: 14.0,
            caption: 12.0,
            title: 18.0,
        };

        let size = SizeTokens {
            control_sm: 28.0,
            control_md: 34.0,
            sidebar_width: 248.0,
        };

        let semantic = SemanticTokens {
            color: SemanticColors {
                app_background: color.neutral_900,
                sidebar_background: color.neutral_800,
                surface: color.neutral_700,
                surface_hover: egui::Color32::from_rgb(58, 63, 74),
                surface_active: egui::Color32::from_rgb(70, 77, 90),
                text: color.neutral_50,
                text_muted: color.neutral_300,
                border: egui::Color32::from_rgb(67, 73, 84),
                focus: color.blue_400,
                accent: color.blue_500,
                accent_text: color.neutral_0,
                danger: color.red_400,
            },
        };

        let component = ComponentTokens {
            button: ButtonTokens {
                height_sm: size.control_sm,
                height_md: size.control_md,
                min_width: 72.0,
                padding_x: space.md,
                radius: radius.md,
            },
            input: InputTokens {
                height: size.control_md,
                padding_x: space.md,
                radius: radius.md,
            },
            tab: TabTokens {
                height: 32.0,
                radius: radius.md,
                close_size: 24.0,
            },
        };

        Self {
            tokens: Tokens {
                primitive: PrimitiveTokens {
                    color,
                    space,
                    radius,
                    stroke,
                    typography,
                    size,
                },
                semantic,
                component,
            },
        }
    }

    pub fn apply(&self, ctx: &egui::Context) {
        let tokens = &self.tokens;

        ctx.all_styles_mut(|style| {
            style.spacing.item_spacing =
                egui::vec2(tokens.primitive.space.sm, tokens.primitive.space.sm);
            style.visuals.dark_mode = true;
            style.visuals.panel_fill = tokens.semantic.color.app_background;
            style.visuals.window_fill = tokens.semantic.color.surface;
            style.visuals.faint_bg_color = tokens.semantic.color.sidebar_background;
            style.visuals.widgets.inactive.fg_stroke.color = tokens.semantic.color.text;
            style.visuals.widgets.hovered.fg_stroke.color = tokens.semantic.color.text;
            style.visuals.widgets.active.fg_stroke.color = tokens.semantic.color.text;
            style.visuals.selection.bg_fill = tokens.semantic.color.accent;
            style.visuals.selection.stroke.color = tokens.semantic.color.accent_text;
        });
    }
}
