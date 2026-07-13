use eframe::egui;

use crate::{
    browser::{BrowserState, Tab},
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
    sidebar_collapsed: &mut bool,
) -> bool {
    let space = &theme.tokens.primitive.space;
    let color = &theme.tokens.semantic.color;
    let mut toggle_theme = false;

    toolbar::show_compact(ui, browser, address_input, theme, sidebar_collapsed);

    ui.add_space(space.lg);
    highlighted_pinned_tabs(ui, browser, address_input, theme);

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

fn highlighted_pinned_tabs(
    ui: &mut egui::Ui,
    browser: &mut BrowserState,
    address_input: &mut String,
    theme: &Theme,
) {
    let tabs = browser.tabs().to_vec();
    let highlighted_tabs = tabs
        .iter()
        .enumerate()
        .filter(|(_, tab)| tab.pinned && tab.highlighted)
        .collect::<Vec<_>>();

    if highlighted_tabs.is_empty() {
        return;
    }

    let columns = 4;
    let spacing = theme.tokens.primitive.space.sm;
    let total_spacing = spacing * (columns - 1) as f32;
    let tile_size = ((ui.available_width() - total_spacing) / columns as f32).max(1.0);
    let mut selected_tab = None;
    let mut demoted_tab = None;
    let mut unpinned_tab = None;

    egui::Grid::new("highlighted_pinned_tabs")
        .num_columns(columns)
        .spacing(egui::vec2(spacing, spacing))
        .show(ui, |ui| {
            for (tile_index, (tab_index, tab)) in highlighted_tabs.iter().enumerate() {
                let is_active = browser.active_index() == *tab_index;
                let response = highlighted_pin_tile(ui, tab, is_active, tile_size, theme);

                if response.clicked() {
                    selected_tab = Some(*tab_index);
                }

                response.context_menu(|ui| {
                    if ui.button("Demote from highlight").clicked() {
                        demoted_tab = Some(*tab_index);
                        ui.close();
                    }

                    if ui.button("Unpin tab").clicked() {
                        unpinned_tab = Some(*tab_index);
                        ui.close();
                    }
                });

                if (tile_index + 1) % columns == 0 {
                    ui.end_row();
                }
            }
        });

    if let Some(index) = selected_tab {
        browser.select_tab(index);
        *address_input = browser.active_url_for_input();
    }

    if let Some(index) = demoted_tab {
        browser.select_tab(index);
        browser.demote_active_highlighted_tab();
        *address_input = browser.active_url_for_input();
    }

    if let Some(index) = unpinned_tab {
        browser.select_tab(index);
        browser.toggle_pin_active_tab();
        *address_input = browser.active_url_for_input();
    }
}

fn highlighted_pin_tile(
    ui: &mut egui::Ui,
    tab: &Tab,
    selected: bool,
    size: f32,
    theme: &Theme,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let color = &theme.tokens.semantic.color;
    let fill = if selected {
        color.surface_active
    } else if response.hovered() {
        color.tile_hover
    } else {
        color.tile
    };
    let stroke_color = if selected { color.focus } else { color.border };

    ui.painter()
        .rect_filled(rect, theme.tokens.primitive.radius.lg, fill);
    ui.painter().rect_stroke(
        rect,
        theme.tokens.primitive.radius.lg,
        egui::Stroke::new(theme.tokens.primitive.stroke.hairline, stroke_color),
        egui::StrokeKind::Inside,
    );
    if let Some(texture) = favicon_texture(ui, tab) {
        let image_size = (size * 0.5).clamp(20.0, 36.0);
        let image_rect = egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(image_size));
        egui::Image::new(&texture)
            .corner_radius(theme.tokens.primitive.radius.sm)
            .paint_at(ui, image_rect);
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            tile_label(&tab.title),
            egui::FontId::proportional(theme.tokens.primitive.typography.body_strong),
            color.text_strong,
        );
    }
    response
}

#[derive(Clone)]
struct CachedFavicon {
    revision: u64,
    texture: egui::TextureHandle,
}

fn favicon_texture(ui: &egui::Ui, tab: &Tab) -> Option<egui::TextureHandle> {
    let favicon = tab.favicon.as_ref()?;
    let cache_id = egui::Id::new(("favicon", tab.id));

    if let Some(cached) = ui
        .ctx()
        .data(|data| data.get_temp::<CachedFavicon>(cache_id))
        && cached.revision == tab.favicon_revision
    {
        return Some(cached.texture);
    }

    let image = egui::ColorImage::from_rgba_unmultiplied(favicon.size(), favicon.rgba());
    let texture = ui.ctx().load_texture(
        format!("favicon-{:?}", tab.id),
        image,
        egui::TextureOptions::LINEAR,
    );
    ui.ctx().data_mut(|data| {
        data.insert_temp(
            cache_id,
            CachedFavicon {
                revision: tab.favicon_revision,
                texture: texture.clone(),
            },
        );
    });

    Some(texture)
}

fn tile_label(title: &str) -> String {
    let words = title
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();

    if words.is_empty() {
        return "?".to_string();
    }

    if words.len() == 1 {
        return words[0].chars().take(3).collect::<String>();
    }

    words
        .iter()
        .take(3)
        .filter_map(|word| word.chars().next())
        .collect::<String>()
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
    let mut highlighted_tab = None;
    let mut moved_up_tab = None;
    let mut moved_down_tab = None;

    for (index, tab) in tabs.iter().enumerate() {
        if tab.pinned && !tab.highlighted && (index == 0 || tabs[index - 1].highlighted) {
            section_label(ui, "Pinned", theme);
            ui.add_space(space.xs);
        } else if !tab.pinned && (index == 0 || tabs[index - 1].pinned) {
            section_label(ui, if has_pinned_tabs { "Today" } else { "Tabs" }, theme);
            ui.add_space(space.xs);
        }

        if tab.pinned && tab.highlighted {
            continue;
        }

        let is_active = browser.active_index() == index;
        let row_width = ui.available_width().max(0.0);
        let action_size = theme
            .tokens
            .component
            .tab
            .close_size
            .max(theme.tokens.component.button.height_sm);
        let spacing = theme.tokens.primitive.space.xs;
        let actions_width = (action_size * 4.0) + (spacing * 4.0);
        let tab_width = (row_width - actions_width).max(0.0);
        let row_height = theme.tokens.component.tab.height;

        ui.push_id(format!("{:?}", tab.id), |ui| {
            let favicon = favicon_texture(ui, tab);
            let (row_rect, _) =
                ui.allocate_exact_size(egui::vec2(row_width, row_height), egui::Sense::hover());
            let mut row_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(row_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            row_ui.set_clip_rect(row_rect);
            row_ui.spacing_mut().item_spacing.x = spacing;

            let tab_response = TabButton::new(&tab.title)
                .favicon(favicon.as_ref())
                .selected(is_active)
                .width(tab_width)
                .show(&mut row_ui, theme);

            if tab_response.clicked() {
                selected_tab = Some(index);
            }

            tab_response.context_menu(|ui| {
                if tab.pinned && !tab.highlighted {
                    if ui.button("Promote to highlight").clicked() {
                        highlighted_tab = Some(index);
                        ui.close();
                    }
                } else if !tab.pinned && ui.button("Pin tab").clicked() {
                    pinned_tab = Some(index);
                    ui.close();
                }
            });

            if DsButton::icon(Icon::Pin)
                .ghost()
                .small()
                .selected(tab.pinned)
                .width(action_size)
                .show(&mut row_ui, theme)
                .clicked()
            {
                pinned_tab = Some(index);
            }

            if DsButton::icon(Icon::ChevronUp)
                .ghost()
                .small()
                .width(action_size)
                .show(&mut row_ui, theme)
                .clicked()
            {
                moved_up_tab = Some(index);
            }

            if DsButton::icon(Icon::ChevronDown)
                .ghost()
                .small()
                .width(action_size)
                .show(&mut row_ui, theme)
                .clicked()
            {
                moved_down_tab = Some(index);
            }

            if DsButton::icon(Icon::X)
                .danger()
                .small()
                .width(action_size)
                .show(&mut row_ui, theme)
                .clicked()
            {
                closed_tab = Some(index);
            }
        });
    }

    if let Some(index) = pinned_tab {
        browser.select_tab(index);
        browser.toggle_pin_active_tab();
        *address_input = browser.active_url_for_input();
    }

    if let Some(index) = highlighted_tab {
        browser.select_tab(index);
        browser.promote_active_pinned_tab();
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
