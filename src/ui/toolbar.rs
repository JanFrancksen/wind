use eframe::egui;

use crate::{
    browser::{BrowserState, TabAction, TabActionKind},
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
            let outcome = apply_to_active(browser, TabActionKind::Back);
            sync_address_after(outcome, browser, address_input);
        }

        if DsButton::icon(Icon::ArrowRight)
            .ghost()
            .small()
            .width(control)
            .show(ui, theme)
            .clicked()
            && browser.can_go_forward()
        {
            let outcome = apply_to_active(browser, TabActionKind::Forward);
            sync_address_after(outcome, browser, address_input);
        }

        if DsButton::icon(Icon::Reload)
            .ghost()
            .small()
            .width(control)
            .show(ui, theme)
            .clicked()
        {
            let outcome = apply_to_active(browser, TabActionKind::Reload);
            sync_address_after(outcome, browser, address_input);
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
        let outcome = browser.submit_address_input(address_input);
        if outcome.active_page_changed() {
            *address_input = browser.active_url_for_input();
        }
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

    let outcome = if reopen_closed {
        let changed = browser.reopen_closed_tab().is_some();
        if changed {
            *address_input = browser.active_url_for_input();
        }
        return;
    } else if new_tab {
        super::open_new_tab(browser, address_input);
        return;
    } else if close_tab {
        apply_to_active(browser, TabActionKind::Close)
    } else if reload {
        apply_to_active(browser, TabActionKind::Reload)
    } else if back {
        apply_to_active(browser, TabActionKind::Back)
    } else if forward {
        apply_to_active(browser, TabActionKind::Forward)
    } else {
        return;
    };
    sync_address_after(outcome, browser, address_input);
}

fn apply_to_active(
    browser: &mut BrowserState,
    kind: TabActionKind,
) -> crate::browser::TabActionOutcome {
    let tab_id = browser.active_tab().id;
    browser.apply_tab_action(TabAction::new(tab_id, kind))
}

fn sync_address_after(
    outcome: crate::browser::TabActionOutcome,
    browser: &BrowserState,
    address_input: &mut String,
) {
    if outcome.active_page_changed() {
        *address_input = browser.active_url_for_input();
    }
}
