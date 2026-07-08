use eframe::egui;

mod browser;
mod ds;

use browser::BrowserState;
use ds::{
    components::{DsButton, Surface, TabButton, TextField, divider},
    theming::Theme,
};

struct BrowserApp {
    browser: BrowserState,
    address_input: String,
    theme: Theme,
}

impl Default for BrowserApp {
    fn default() -> Self {
        let browser = BrowserState::default();
        let address_input = browser.active_url_for_input();

        Self {
            browser,
            address_input,
            theme: Theme::arc_dark(),
        }
    }
}

impl eframe::App for BrowserApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.theme.apply(ui.ctx());

        let available = ui.available_size();
        let sidebar_width = self
            .theme
            .tokens
            .primitive
            .size
            .sidebar_width
            .min(available.x * 0.42)
            .max(224.0);
        let content_width = (available.x - sidebar_width).max(320.0);

        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(sidebar_width, available.y),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    Surface::sidebar(&self.theme).show(ui, |ui| {
                        ui.set_min_size(egui::vec2(sidebar_width, available.y));
                        ui.set_max_width(sidebar_width);
                        self.sidebar(ui);
                    });
                },
            );

            ui.allocate_ui_with_layout(
                egui::vec2(content_width, available.y),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    Surface::panel(&self.theme).show(ui, |ui| {
                        ui.set_min_size(egui::vec2(content_width, available.y));
                        self.browser_surface(ui);
                    });
                },
            );
        });
    }
}

impl BrowserApp {
    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let space = &self.theme.tokens.primitive.space;
        let color = &self.theme.tokens.semantic.color;

        ui.add_space(space.md);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Wind")
                    .color(color.text)
                    .size(self.theme.tokens.primitive.typography.title)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                DsButton::new("⌘")
                    .ghost()
                    .small()
                    .width(self.theme.tokens.component.tab.close_size)
                    .show(ui, &self.theme);
            });
        });

        ui.add_space(space.sm);
        self.space_switcher(ui);

        ui.add_space(space.sm);
        if DsButton::new("+ New Tab")
            .ghost()
            .width(ui.available_width())
            .show(ui, &self.theme)
            .clicked()
        {
            self.browser.add_tab("arc://new-tab");
            self.address_input = self.browser.active_url_for_input();
        }

        divider(ui, &self.theme);

        let tabs = self.browser.tabs().to_vec();
        let has_pinned_tabs = tabs.iter().any(|tab| tab.pinned);

        if has_pinned_tabs {
            ui.label(
                egui::RichText::new("Pinned")
                    .color(color.text_muted)
                    .size(self.theme.tokens.primitive.typography.caption),
            );
            ui.add_space(space.xs);
        }

        ui.label(
            egui::RichText::new(if has_pinned_tabs { "Today" } else { "Tabs" })
                .color(color.text_muted)
                .size(self.theme.tokens.primitive.typography.caption),
        );
        ui.add_space(space.xs);

        let mut selected_tab = None;
        let mut closed_tab = None;

        for (index, tab) in tabs.iter().enumerate() {
            let is_active = self.browser.active_index() == index;
            let row_width = ui.available_width();
            let close_size = self.theme.tokens.component.tab.close_size;
            let spacing = self.theme.tokens.primitive.space.xs;
            let tab_width = (row_width - close_size - spacing).max(48.0);

            ui.push_id(format!("{:?}", tab.id), |ui| {
                ui.horizontal(|ui| {
                    if TabButton::new(&tab.title)
                        .selected(is_active)
                        .width(tab_width)
                        .show(ui, &self.theme)
                        .clicked()
                    {
                        selected_tab = Some(index);
                    }

                    if DsButton::new("×")
                        .danger()
                        .small()
                        .width(close_size)
                        .show(ui, &self.theme)
                        .clicked()
                    {
                        closed_tab = Some(index);
                    }
                });
            });
        }

        if let Some(index) = selected_tab {
            self.browser.select_tab(index);
            self.address_input = self.browser.active_url_for_input();
        }

        if let Some(index) = closed_tab {
            self.browser.close_tab(index);
            self.address_input = self.browser.active_url_for_input();
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(space.md);
            ui.label(
                egui::RichText::new("Personal")
                    .color(color.text_muted)
                    .size(self.theme.tokens.primitive.typography.caption),
            );
        });
    }

    fn space_switcher(&self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        let half = ((width - self.theme.tokens.primitive.space.xs) / 2.0).max(80.0);

        ui.horizontal(|ui| {
            DsButton::new("Personal")
                .ghost()
                .selected(true)
                .width(half)
                .show(ui, &self.theme);
            DsButton::new("Work")
                .ghost()
                .width(half)
                .show(ui, &self.theme);
        });
    }

    fn browser_surface(&mut self, ui: &mut egui::Ui) {
        self.toolbar(ui);
        divider(ui, &self.theme);
        self.webview_placeholder(ui);
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let control = self.theme.tokens.primitive.size.control_md;
        let gap = self.theme.tokens.primitive.space.sm;
        let nav_width = (control * 3.0) + (gap * 3.0);
        let go_width = 72.0;
        let address_width = (ui.available_width() - nav_width - go_width).max(control);

        ui.horizontal(|ui| {
            if DsButton::new("‹")
                .ghost()
                .width(control)
                .show(ui, &self.theme)
                .clicked()
                && self.browser.can_go_back()
            {
                self.browser.go_back();
                self.address_input = self.browser.active_url_for_input();
            }

            if DsButton::new("›")
                .ghost()
                .width(control)
                .show(ui, &self.theme)
                .clicked()
                && self.browser.can_go_forward()
            {
                self.browser.go_forward();
                self.address_input = self.browser.active_url_for_input();
            }

            if DsButton::new("↻")
                .ghost()
                .width(control)
                .show(ui, &self.theme)
                .clicked()
            {
                self.browser.reload();
            }

            let response = TextField::singleline(&mut self.address_input)
                .desired_width(address_width)
                .show(ui, &self.theme);

            let pressed_enter =
                response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

            if DsButton::new("Go")
                .primary()
                .width(go_width)
                .show(ui, &self.theme)
                .clicked()
                || pressed_enter
            {
                self.browser.navigate_active(&self.address_input);
                self.address_input = self.browser.active_url_for_input();
            }
        });
    }

    fn webview_placeholder(&self, ui: &mut egui::Ui) {
        let color = &self.theme.tokens.semantic.color;
        let active = self.browser.active_tab();

        ui.add_space(self.theme.tokens.primitive.space.xl);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new(&active.title)
                    .color(color.text)
                    .size(24.0)
                    .strong(),
            );
            ui.add_space(self.theme.tokens.primitive.space.xs);
            ui.label(
                egui::RichText::new(&active.url)
                    .color(color.text_muted)
                    .size(self.theme.tokens.primitive.typography.body),
            );
            ui.add_space(self.theme.tokens.primitive.space.xl);
            ui.label(
                egui::RichText::new("Web renderer mount point")
                    .color(color.text_muted)
                    .size(self.theme.tokens.primitive.typography.caption),
            );
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "Wind Browser",
        options,
        Box::new(|_cc| Ok(Box::new(BrowserApp::default()))),
    )
}
