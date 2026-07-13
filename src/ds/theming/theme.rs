use eframe::egui;

use super::tokens::{
    ButtonTokens, ColorPrimitives, ComponentTokens, InputTokens, MotionTokens, PrimitiveTokens,
    RadiusTokens, SemanticColors, SemanticTokens, SizeTokens, SpaceTokens, StrokeTokens, TabTokens,
    Tokens, TypographyTokens,
};

#[derive(Clone)]
pub struct Theme {
    pub tokens: Tokens,
    pub appearance: ThemeAppearance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeAppearance {
    Alpine,
    Night,
}

impl Theme {
    pub fn wind(appearance: ThemeAppearance) -> Self {
        match appearance {
            ThemeAppearance::Alpine => Self::alpine(),
            ThemeAppearance::Night => Self::night(),
        }
    }

    pub fn toggled(&self) -> Self {
        Self::wind(match self.appearance {
            ThemeAppearance::Alpine => ThemeAppearance::Night,
            ThemeAppearance::Night => ThemeAppearance::Alpine,
        })
    }

    pub fn appearance_label(&self) -> &'static str {
        match self.appearance {
            ThemeAppearance::Alpine => "Alpine",
            ThemeAppearance::Night => "Night",
        }
    }

    fn alpine() -> Self {
        let color = ColorPrimitives {
            neutral_0: egui::Color32::from_rgb(255, 255, 255),
            neutral_50: egui::Color32::from_rgb(248, 251, 255),
            neutral_100: egui::Color32::from_rgb(232, 239, 250),
            neutral_200: egui::Color32::from_rgb(200, 212, 232),
            neutral_300: egui::Color32::from_rgb(132, 148, 179),
            neutral_700: egui::Color32::from_rgb(61, 76, 111),
            neutral_800: egui::Color32::from_rgb(34, 48, 82),
            neutral_900: egui::Color32::from_rgb(15, 31, 64),
            sky_50: egui::Color32::from_rgb(245, 250, 255),
            sky_100: egui::Color32::from_rgb(229, 242, 255),
            sky_200: egui::Color32::from_rgb(199, 225, 252),
            sky_300: egui::Color32::from_rgb(145, 195, 244),
            blue_300: egui::Color32::from_rgb(109, 170, 241),
            blue_400: egui::Color32::from_rgb(78, 150, 242),
            blue_500: egui::Color32::from_rgb(45, 115, 238),
            navy_700: egui::Color32::from_rgb(28, 51, 100),
            navy_900: egui::Color32::from_rgb(9, 24, 56),
            mint_300: egui::Color32::from_rgb(140, 228, 205),
            violet_400: egui::Color32::from_rgb(132, 109, 239),
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
            sm: 8,
            md: 12,
            lg: 18,
            xl: 28,
            round: 96,
        };

        let stroke = StrokeTokens {
            hairline: 1.0,
            thin: 1.5,
        };

        let typography = TypographyTokens {
            body: 13.5,
            body_strong: 14.0,
            caption: 11.5,
            title: 17.0,
            brand: 45.0,
        };

        let size = SizeTokens {
            control_sm: 28.0,
            control_md: 38.0,
            sidebar_width: 282.0,
            app_padding: 20.0,
            tile: 64.0,
        };

        let motion = MotionTokens {
            sidebar_collapse_seconds: 0.26,
            tab_reorder_seconds: 0.16,
        };

        let semantic = SemanticTokens {
            color: SemanticColors {
                app_background: color.sky_100,
                app_background_top: color.sky_200,
                app_background_bottom: egui::Color32::from_rgb(133, 181, 225),
                sidebar_background: egui::Color32::from_rgba_unmultiplied(244, 249, 255, 224),
                sidebar_border: egui::Color32::from_rgba_unmultiplied(255, 255, 255, 154),
                surface: egui::Color32::from_rgba_unmultiplied(250, 253, 255, 226),
                surface_hover: egui::Color32::from_rgba_unmultiplied(255, 255, 255, 238),
                surface_active: egui::Color32::from_rgba_unmultiplied(224, 235, 255, 232),
                surface_overlay: egui::Color32::from_rgba_unmultiplied(255, 255, 255, 176),
                chrome: egui::Color32::from_rgba_unmultiplied(239, 245, 255, 206),
                chrome_hover: egui::Color32::from_rgba_unmultiplied(250, 253, 255, 236),
                tile: egui::Color32::from_rgba_unmultiplied(248, 251, 255, 232),
                tile_hover: egui::Color32::from_rgba_unmultiplied(255, 255, 255, 248),
                text: color.navy_700,
                text_strong: color.navy_900,
                text_muted: color.neutral_300,
                border: egui::Color32::from_rgba_unmultiplied(132, 156, 200, 72),
                shadow: egui::Color32::from_rgba_unmultiplied(52, 101, 166, 36),
                cloud: egui::Color32::from_rgba_unmultiplied(255, 255, 255, 188),
                mountain_far: egui::Color32::from_rgb(163, 196, 226),
                mountain_mid: egui::Color32::from_rgb(116, 165, 213),
                mountain_near: egui::Color32::from_rgb(68, 124, 183),
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
                radius: radius.lg,
            },
            input: InputTokens {
                height: size.control_md,
                padding_x: space.lg,
                radius: radius.round,
            },
            tab: TabTokens {
                height: 36.0,
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
                    motion,
                },
                semantic,
                component,
            },
            appearance: ThemeAppearance::Alpine,
        }
    }

    fn night() -> Self {
        let mut theme = Self::alpine();
        let color = &mut theme.tokens.primitive.color;
        color.neutral_0 = egui::Color32::from_rgb(255, 255, 255);
        color.neutral_50 = egui::Color32::from_rgb(239, 246, 255);
        color.neutral_100 = egui::Color32::from_rgb(198, 213, 236);
        color.neutral_200 = egui::Color32::from_rgb(139, 160, 194);
        color.neutral_300 = egui::Color32::from_rgb(117, 139, 176);
        color.neutral_700 = egui::Color32::from_rgb(41, 59, 99);
        color.neutral_800 = egui::Color32::from_rgb(25, 38, 70);
        color.neutral_900 = egui::Color32::from_rgb(8, 20, 45);

        theme.tokens.semantic.color = SemanticColors {
            app_background: color.neutral_900,
            app_background_top: egui::Color32::from_rgb(18, 43, 86),
            app_background_bottom: egui::Color32::from_rgb(7, 18, 42),
            sidebar_background: egui::Color32::from_rgba_unmultiplied(18, 32, 63, 232),
            sidebar_border: egui::Color32::from_rgba_unmultiplied(156, 188, 231, 42),
            surface: egui::Color32::from_rgba_unmultiplied(30, 48, 86, 232),
            surface_hover: egui::Color32::from_rgba_unmultiplied(43, 66, 111, 238),
            surface_active: egui::Color32::from_rgba_unmultiplied(61, 90, 145, 232),
            surface_overlay: egui::Color32::from_rgba_unmultiplied(17, 31, 62, 196),
            chrome: egui::Color32::from_rgba_unmultiplied(28, 45, 82, 218),
            chrome_hover: egui::Color32::from_rgba_unmultiplied(45, 68, 112, 236),
            tile: egui::Color32::from_rgba_unmultiplied(32, 51, 91, 234),
            tile_hover: egui::Color32::from_rgba_unmultiplied(47, 74, 121, 246),
            text: color.neutral_50,
            text_strong: color.neutral_0,
            text_muted: color.neutral_200,
            border: egui::Color32::from_rgba_unmultiplied(155, 186, 229, 50),
            shadow: egui::Color32::from_rgba_unmultiplied(0, 7, 23, 88),
            cloud: egui::Color32::from_rgba_unmultiplied(190, 211, 241, 42),
            mountain_far: egui::Color32::from_rgb(57, 78, 119),
            mountain_mid: egui::Color32::from_rgb(41, 70, 122),
            mountain_near: egui::Color32::from_rgb(24, 49, 98),
            focus: color.blue_300,
            accent: color.blue_400,
            accent_text: color.neutral_0,
            danger: color.red_400,
        };
        theme.appearance = ThemeAppearance::Night;
        theme
    }

    pub fn apply(&self, ctx: &egui::Context) {
        let tokens = &self.tokens;

        ctx.all_styles_mut(|style| {
            style.spacing.item_spacing =
                egui::vec2(tokens.primitive.space.sm, tokens.primitive.space.sm);
            style.visuals.dark_mode = self.appearance == ThemeAppearance::Night;
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
