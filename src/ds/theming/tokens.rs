use eframe::egui;

#[derive(Clone)]
pub struct Tokens {
    pub primitive: PrimitiveTokens,
    pub semantic: SemanticTokens,
    pub component: ComponentTokens,
}

#[derive(Clone)]
pub struct PrimitiveTokens {
    pub color: ColorPrimitives,
    pub space: SpaceTokens,
    pub radius: RadiusTokens,
    pub stroke: StrokeTokens,
    pub typography: TypographyTokens,
    pub size: SizeTokens,
    pub motion: MotionTokens,
}

#[derive(Clone)]
pub struct ColorPrimitives {
    pub neutral_0: egui::Color32,
    pub neutral_50: egui::Color32,
    pub neutral_100: egui::Color32,
    pub neutral_200: egui::Color32,
    pub neutral_300: egui::Color32,
    pub neutral_700: egui::Color32,
    pub neutral_800: egui::Color32,
    pub neutral_900: egui::Color32,
    pub sky_100: egui::Color32,
    pub blue_300: egui::Color32,
    pub blue_400: egui::Color32,
    pub blue_500: egui::Color32,
    pub navy_700: egui::Color32,
    pub navy_900: egui::Color32,
    pub green_400: egui::Color32,
    pub violet_400: egui::Color32,
    pub amber_400: egui::Color32,
    pub rose_400: egui::Color32,
    pub slate_400: egui::Color32,
    #[cfg(not(target_os = "macos"))]
    pub red_400: egui::Color32,
}

#[derive(Clone)]
pub struct SpaceTokens {
    pub xxs: f32,
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
}

#[derive(Clone)]
pub struct RadiusTokens {
    pub sm: u8,
    pub md: u8,
    pub lg: u8,
    pub round: u8,
}

#[derive(Clone)]
pub struct StrokeTokens {
    pub hairline: f32,
    pub thin: f32,
}

#[derive(Clone)]
pub struct TypographyTokens {
    pub body: f32,
    pub body_strong: f32,
    pub caption: f32,
    pub brand: f32,
}

#[derive(Clone)]
pub struct SizeTokens {
    pub control_sm: f32,
    pub control_md: f32,
    pub sidebar_width: f32,
}

#[derive(Clone)]
pub struct MotionTokens {
    pub sidebar_collapse_seconds: f32,
    pub space_switch_seconds: f32,
    pub tab_reorder_seconds: f32,
    pub space_bubble_seconds: f32,
}

#[derive(Clone)]
pub struct SemanticTokens {
    pub color: SemanticColors,
}

#[derive(Clone)]
pub struct SemanticColors {
    pub app_background: egui::Color32,
    pub sidebar_background: egui::Color32,
    pub sidebar_border: egui::Color32,
    pub surface: egui::Color32,
    pub surface_hover: egui::Color32,
    pub surface_active: egui::Color32,
    pub surface_overlay: egui::Color32,
    pub chrome: egui::Color32,
    #[cfg(not(target_os = "macos"))]
    pub chrome_hover: egui::Color32,
    pub tile: egui::Color32,
    pub tile_hover: egui::Color32,
    pub text: egui::Color32,
    pub text_strong: egui::Color32,
    pub text_muted: egui::Color32,
    pub border: egui::Color32,
    pub shadow: egui::Color32,
    pub focus: egui::Color32,
    pub accent: egui::Color32,
    pub accent_text: egui::Color32,
    #[cfg(not(target_os = "macos"))]
    pub danger: egui::Color32,
}

#[derive(Clone)]
pub struct ComponentTokens {
    pub button: ButtonTokens,
    pub input: InputTokens,
    #[cfg(not(target_os = "macos"))]
    pub menu: MenuTokens,
    pub tab: TabTokens,
    pub space_switcher: SpaceSwitcherTokens,
}

#[derive(Clone)]
pub struct ButtonTokens {
    pub height_sm: f32,
    pub height_md: f32,
    pub min_width: f32,
    pub radius: u8,
}

#[derive(Clone)]
pub struct InputTokens {
    pub height: f32,
    pub padding_x: f32,
    pub radius: u8,
}

#[cfg(not(target_os = "macos"))]
#[derive(Clone)]
pub struct MenuTokens {
    pub width: f32,
    pub item_height: f32,
    pub icon_size: f32,
    pub radius: u8,
}

#[derive(Clone)]
pub struct TabTokens {
    pub height: f32,
    pub radius: u8,
    pub close_size: f32,
}

#[derive(Clone)]
pub struct SpaceSwitcherTokens {
    pub bubble_size: f32,
    pub bubble_hit_size: f32,
}
