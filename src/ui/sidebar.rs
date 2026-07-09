use eframe::egui;

use crate::{
    browser::BrowserState,
    ds::{
        components::{DsButton, Icon, TabButton, divider},
        theming::Theme,
    },
};

use super::toolbar;

pub fn show(
    ui: &mut egui::Ui,
    browser: &mut BrowserState,
    address_input: &mut String,
    theme: &Theme,
) -> bool {
    let space = &theme.tokens.primitive.space;
    let color = &theme.tokens.semantic.color;
    let mut toggle_theme = false;

    toolbar::show_compact(ui, browser, address_input, theme);

    ui.add_space(space.lg);
    quick_tiles(ui, theme);

    ui.add_space(space.lg);
    workspace_header(ui, theme);

    ui.add_space(space.sm);
    if DsButton::new("New Tab")
        .leading_icon(Icon::Plus)
        .ghost()
        .width(ui.available_width())
        .show(ui, theme)
        .clicked()
    {
        browser.add_tab("arc://new-tab");
        *address_input = browser.active_url_for_input();
    }

    divider(ui, theme);
    tab_sections(ui, browser, address_input, theme);

    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        ui.add_space(space.sm);
        ui.horizontal(|ui| {
            DsButton::icon(Icon::X)
                .ghost()
                .small()
                .width(theme.tokens.component.tab.close_size)
                .show(ui, theme);
            if DsButton::new(theme.appearance_label())
                .leading_icon(Icon::Command)
                .ghost()
                .small()
                .width(110.0)
                .show(ui, theme)
                .clicked()
            {
                toggle_theme = true;
            }
        });
        ui.add_space(space.sm);
        divider(ui, theme);
        ui.add_space(space.sm);
        ui.label(
            egui::RichText::new("Wind Browser")
                .color(color.text_muted)
                .size(theme.tokens.primitive.typography.caption),
        );
    });

    toggle_theme
}

fn quick_tiles(ui: &mut egui::Ui, theme: &Theme) {
    let labels = ["TB", "M", "Run", "◆", "TB", "◎", "≡", "▥"];
    egui::Grid::new("quick_tiles")
        .num_columns(4)
        .spacing(egui::vec2(
            theme.tokens.primitive.space.sm,
            theme.tokens.primitive.space.sm,
        ))
        .show(ui, |ui| {
            for (index, label) in labels.iter().enumerate() {
                quick_tile(ui, label, theme);
                if (index + 1) % 4 == 0 {
                    ui.end_row();
                }
            }
        });
}

fn quick_tile(ui: &mut egui::Ui, label: &str, theme: &Theme) {
    let size = 48.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let color = &theme.tokens.semantic.color;
    let fill = if response.hovered() {
        color.tile_hover
    } else {
        color.tile
    };

    ui.painter()
        .rect_filled(rect, theme.tokens.primitive.radius.lg, fill);
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
        egui::FontId::proportional(theme.tokens.primitive.typography.body_strong),
        if label.contains('◆') {
            color.accent
        } else {
            color.text_strong
        },
    );
}

fn workspace_header(ui: &mut egui::Ui, theme: &Theme) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("TidBITS")
                .color(theme.tokens.semantic.color.text_strong)
                .size(theme.tokens.primitive.typography.body_strong)
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            DsButton::icon(Icon::Command)
                .ghost()
                .small()
                .width(theme.tokens.component.tab.close_size)
                .show(ui, theme);
        });
    });
}

fn tab_sections(
    ui: &mut egui::Ui,
    browser: &mut BrowserState,
    address_input: &mut String,
    theme: &Theme,
) {
    let space = &theme.tokens.primitive.space;
    let color = &theme.tokens.semantic.color;
    let tabs = browser.tabs().to_vec();
    let has_pinned_tabs = tabs.iter().any(|tab| tab.pinned);

    let mut selected_tab = None;
    let mut closed_tab = None;
    let mut pinned_tab = None;
    let mut moved_up_tab = None;
    let mut moved_down_tab = None;

    for (index, tab) in tabs.iter().enumerate() {
        if tab.pinned && index == 0 {
            section_label(ui, "Pinned", theme);
            ui.add_space(space.xs);
        } else if !tab.pinned && (index == 0 || tabs[index - 1].pinned) {
            section_label(ui, if has_pinned_tabs { "Today" } else { "Tabs" }, theme);
            ui.add_space(space.xs);
        }

        let is_active = browser.active_index() == index;
        let row_width = ui.available_width();
        let close_size = theme.tokens.component.tab.close_size;
        let spacing = theme.tokens.primitive.space.xs;
        let pin_size = theme.tokens.component.tab.close_size;
        let move_size = theme.tokens.component.tab.close_size;
        let actions_width = close_size + pin_size + (move_size * 2.0) + (spacing * 4.0);
        let tab_width = (row_width - actions_width).max(48.0);

        ui.push_id(format!("{:?}", tab.id), |ui| {
            ui.horizontal(|ui| {
                if TabButton::new(&tab.title)
                    .selected(is_active)
                    .width(tab_width)
                    .show(ui, theme)
                    .clicked()
                {
                    selected_tab = Some(index);
                }

                if DsButton::icon(Icon::Pin)
                    .ghost()
                    .small()
                    .selected(tab.pinned)
                    .width(pin_size)
                    .show(ui, theme)
                    .clicked()
                {
                    pinned_tab = Some(index);
                }

                if DsButton::icon(Icon::ChevronUp)
                    .ghost()
                    .small()
                    .width(move_size)
                    .show(ui, theme)
                    .clicked()
                {
                    moved_up_tab = Some(index);
                }

                if DsButton::icon(Icon::ChevronDown)
                    .ghost()
                    .small()
                    .width(move_size)
                    .show(ui, theme)
                    .clicked()
                {
                    moved_down_tab = Some(index);
                }

                if DsButton::icon(Icon::X)
                    .danger()
                    .small()
                    .width(close_size)
                    .show(ui, theme)
                    .clicked()
                {
                    closed_tab = Some(index);
                }
            });
        });
    }

    if let Some(index) = pinned_tab {
        browser.select_tab(index);
        browser.toggle_pin_active_tab();
        *address_input = browser.active_url_for_input();
    }

    if let Some(index) = moved_up_tab {
        browser.select_tab(index);
        browser.move_active_tab_up();
        *address_input = browser.active_url_for_input();
    }

    if let Some(index) = moved_down_tab {
        browser.select_tab(index);
        browser.move_active_tab_down();
        *address_input = browser.active_url_for_input();
    }

    if let Some(index) = selected_tab {
        browser.select_tab(index);
        *address_input = browser.active_url_for_input();
    }

    if let Some(index) = closed_tab {
        browser.close_tab(index);
        *address_input = browser.active_url_for_input();
    }

    ui.add_space(space.sm);
    ui.label(
        egui::RichText::new(format!("{} open", browser.tabs().len()))
            .color(color.text_muted)
            .size(theme.tokens.primitive.typography.caption),
    );
}

fn section_label(ui: &mut egui::Ui, label: &str, theme: &Theme) {
    ui.label(
        egui::RichText::new(label)
            .color(theme.tokens.semantic.color.text_muted)
            .size(theme.tokens.primitive.typography.caption),
    );
}
