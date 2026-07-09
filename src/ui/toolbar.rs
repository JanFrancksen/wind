use eframe::egui;

use crate::{
    browser::BrowserState,
    ds::{
        components::{DsButton, Icon, TextField},
        theming::Theme,
    },
};

pub fn show(
    ui: &mut egui::Ui,
    browser: &mut BrowserState,
    address_input: &mut String,
    theme: &Theme,
) {
    handle_shortcuts(ui, browser, address_input);

    let control = theme.tokens.primitive.size.control_md;
    let gap = theme.tokens.primitive.space.sm;
    let nav_width = (control * 3.0) + (gap * 3.0);
    let go_width = 72.0;
    let address_width = (ui.available_width() - nav_width - go_width).max(control);

    ui.horizontal(|ui| {
        if DsButton::icon(Icon::ArrowLeft)
            .ghost()
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
            .width(control)
            .show(ui, theme)
            .clicked()
        {
            browser.reload();
        }

        let response = TextField::singleline(address_input)
            .desired_width(address_width)
            .show(ui, theme);

        let pressed_enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

        if DsButton::new("Go")
            .primary()
            .width(go_width)
            .show(ui, theme)
            .clicked()
            || pressed_enter
        {
            browser.submit_address_input(address_input);
            *address_input = browser.active_url_for_input();
        }
    });
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
        browser.add_tab("arc://new-tab");
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
