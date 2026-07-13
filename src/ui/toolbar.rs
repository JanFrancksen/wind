use eframe::egui;

use crate::{
    browser::BrowserState,
    ds::{
        components::{DsButton, Icon, SearchField},
        theming::Theme,
    },
};

pub fn show_compact(
    ui: &mut egui::Ui,
    browser: &mut BrowserState,
    address_input: &mut String,
    theme: &Theme,
    sidebar_collapsed: &mut bool,
) {
    handle_shortcuts(ui, browser, address_input);

    let control = theme.tokens.primitive.size.control_sm;
    let gap = theme.tokens.primitive.space.sm;

    ui.horizontal(|ui| {
        if DsButton::icon(Icon::ArrowLeft)
            .ghost()
            .small()
            .width(control)
            .show(ui, theme)
            .clicked()
            && browser.can_go_back()
        {
            browser.go_back();
            *address_input = browser.active_url_for_input();
        }

        if DsButton::icon(Icon::ArrowRight)
            .ghost()
            .small()
            .width(control)
            .show(ui, theme)
            .clicked()
            && browser.can_go_forward()
        {
            browser.go_forward();
            *address_input = browser.active_url_for_input();
        }

        if DsButton::icon(Icon::Reload)
            .ghost()
            .small()
            .width(control)
            .show(ui, theme)
            .clicked()
        {
            browser.reload();
        }

        if DsButton::icon(Icon::ArrowLeft)
            .ghost()
            .small()
            .width(control)
            .show(ui, theme)
            .on_hover_text("Collapse sidebar (Cmd+S)")
            .clicked()
        {
            *sidebar_collapsed = true;
        }
    });

    ui.add_space(gap);

    let response = SearchField::sidebar(address_input)
        .desired_width(ui.available_width().max(control))
        .show(ui, theme);

    let pressed_enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if pressed_enter {
        browser.submit_address_input(address_input);
        *address_input = browser.active_url_for_input();
    }
}

fn handle_shortcuts(ui: &mut egui::Ui, browser: &mut BrowserState, address_input: &mut String) {
    let (new_tab, close_tab, reload, back, forward, reopen_closed) = ui.input(|input| {
        let command = input.modifiers.command;
        let shift = input.modifiers.shift;

        (
            command && input.key_pressed(egui::Key::T),
            command && input.key_pressed(egui::Key::W),
            command && input.key_pressed(egui::Key::R),
            command && input.key_pressed(egui::Key::ArrowLeft),
            command && input.key_pressed(egui::Key::ArrowRight),
            command && shift && input.key_pressed(egui::Key::T),
        )
    });

    if reopen_closed {
        browser.reopen_closed_tab();
    } else if new_tab {
        super::open_new_tab(browser, address_input);
        return;
    } else if close_tab {
        browser.close_active_tab();
    } else if reload {
        browser.reload();
    } else if back {
        browser.go_back();
    } else if forward {
        browser.go_forward();
    } else {
        return;
    }

    *address_input = browser.active_url_for_input();
}
