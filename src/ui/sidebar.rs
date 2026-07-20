use eframe::egui;

use crate::{
    browser::{BrowserState, SpaceColor, SpaceId, Tab, TabAction, TabActionKind, TabGroup, TabId},
    ds::{
        components::{DsButton, Icon, TabButton, divider},
        theming::Theme,
    },
    native_menu,
};

#[cfg(not(target_os = "macos"))]
use crate::ds::components::MenuItem;

use super::toolbar;

const HIGHLIGHT_COLUMNS: usize = 4;

#[derive(Clone, Copy, Debug)]
struct DraggedTab {
    id: TabId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DropTarget {
    group: TabGroup,
    index: usize,
}

#[derive(Clone, Copy, Debug)]
struct SpaceTransition {
    current: SpaceId,
    outgoing: Option<SpaceId>,
    direction: f32,
    started_at: f64,
}

pub fn show(
    ui: &mut egui::Ui,
    browser: &mut BrowserState,
    address_input: &mut String,
    theme: &Theme,
    sidebar_collapsed: &mut bool,
) -> bool {
    #[allow(unused_mut)]
    let mut toggle_theme = false;

    toolbar::show_compact(ui, browser, address_input, theme, sidebar_collapsed);

    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        space_switcher(ui, browser, address_input, theme);
        #[cfg(not(target_os = "macos"))]
        if DsButton::new("Toggle Theme")
            .ghost()
            .small()
            .width(ui.available_width())
            .show(ui, theme)
            .clicked()
        {
            toggle_theme = true;
        }

        let (pane_rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
        show_space_tabs(ui, pane_rect, browser, address_input, theme);
    });
    confirm_space_deletion(ui, browser, address_input, theme);
    rename_space_dialog(ui, browser, theme);

    toggle_theme
}

fn show_space_tabs(
    ui: &mut egui::Ui,
    pane_rect: egui::Rect,
    browser: &mut BrowserState,
    address_input: &mut String,
    theme: &Theme,
) {
    let (outgoing, direction, progress) = space_transition(ui, browser, theme);
    let (outgoing_x, incoming_x) = space_offsets(direction, progress, pane_rect.width());

    if let Some(outgoing_id) = outgoing {
        let mut old_browser = browser.clone();
        if old_browser.switch_space(outgoing_id) {
            let mut old_address = old_browser.active_url_for_input();
            show_tab_layer(
                ui,
                pane_rect,
                outgoing_x,
                &mut old_browser,
                &mut old_address,
                theme,
                false,
            );
        }
    }

    show_tab_layer(
        ui,
        pane_rect,
        incoming_x,
        browser,
        address_input,
        theme,
        outgoing.is_none(),
    );
}

#[allow(clippy::too_many_arguments)]
fn show_tab_layer(
    ui: &mut egui::Ui,
    pane_rect: egui::Rect,
    offset_x: f32,
    browser: &mut BrowserState,
    address_input: &mut String,
    theme: &Theme,
    interactive: bool,
) {
    let layer_rect = pane_rect.translate(egui::vec2(offset_x, 0.0));
    let mut layer = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(layer_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    layer.set_clip_rect(pane_rect.intersect(ui.clip_rect()));
    layer.add_enabled_ui(interactive, |ui| {
        ui.push_id(("space-tabs", browser.active_space().id()), |ui| {
            tabs_panel(ui, browser, address_input, theme, interactive);
        });
    });
}

fn tabs_panel(
    ui: &mut egui::Ui,
    browser: &mut BrowserState,
    address_input: &mut String,
    theme: &Theme,
    interactive: bool,
) {
    let space = &theme.tokens.primitive.space;

    let dragging = egui::DragAndDrop::payload::<DraggedTab>(ui.ctx()).map(|payload| *payload);
    let mut drop_target = None;
    let mut actions = Vec::new();
    if interactive {
        actions.extend(native_menu::take_tab_menu_requests());
    }

    ui.add_space(space.lg);
    highlighted_pinned_tabs(
        ui,
        browser,
        theme,
        dragging,
        &mut drop_target,
        &mut actions,
        interactive,
    );

    divider(ui, theme);
    tab_sections(
        ui,
        browser,
        theme,
        dragging,
        &mut drop_target,
        &mut actions,
        interactive,
    );

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

    if let Some(dragged) = egui::DragAndDrop::payload::<DraggedTab>(ui.ctx()).map(|item| *item) {
        if ui.input(|input| input.pointer.any_released()) {
            if let Some(target) = drop_target {
                let _ = egui::DragAndDrop::take_payload::<DraggedTab>(ui.ctx());
                actions.push(TabAction::new(
                    dragged.id,
                    TabActionKind::Place {
                        group: target.group,
                        index: target.index,
                    },
                ));
            }
        } else {
            floating_drag_preview(ui, browser, dragged, drop_target, theme);
            ui.ctx().request_repaint();
        }
    }

    apply_tab_actions(browser, address_input, actions);
}

fn space_transition(
    ui: &egui::Ui,
    browser: &BrowserState,
    theme: &Theme,
) -> (Option<SpaceId>, f32, f32) {
    let state_id = egui::Id::new("space-tab-transition");
    let active = browser.active_space().id();
    let now = ui.input(|input| input.time);
    let mut state = ui
        .ctx()
        .data_mut(|data| data.get_temp::<SpaceTransition>(state_id))
        .unwrap_or(SpaceTransition {
            current: active,
            outgoing: None,
            direction: 1.0,
            started_at: now,
        });

    if state.current != active {
        let previous_index = browser
            .spaces()
            .iter()
            .position(|space| space.id() == state.current);
        let active_index = browser
            .spaces()
            .iter()
            .position(|space| space.id() == active)
            .unwrap_or(0);
        state.direction = if previous_index.is_some_and(|index| active_index < index) {
            -1.0
        } else {
            1.0
        };
        state.outgoing = previous_index.map(|_| state.current);
        state.current = active;
        state.started_at = now;
    }

    let duration = f64::from(theme.tokens.primitive.motion.space_switch_seconds.max(0.01));
    let linear = ((now - state.started_at) / duration).clamp(0.0, 1.0) as f32;
    let progress = egui::emath::easing::sin_in_out(linear);
    if linear >= 1.0 {
        state.outgoing = None;
    } else if state.outgoing.is_some() {
        ui.ctx().request_repaint();
    }
    let result = (state.outgoing, state.direction, progress);
    ui.ctx().data_mut(|data| data.insert_temp(state_id, state));
    result
}

fn space_offsets(direction: f32, progress: f32, width: f32) -> (f32, f32) {
    (
        -direction * width * progress,
        direction * width * (1.0 - progress),
    )
}

fn space_switcher(
    ui: &mut egui::Ui,
    browser: &mut BrowserState,
    address_input: &mut String,
    theme: &Theme,
) {
    apply_native_space_menu_requests(ui, browser);
    let active = browser.active_space().id();
    let spaces = browser
        .spaces()
        .iter()
        .map(|space| (space.id(), space.name().to_owned(), space.color()))
        .collect::<Vec<_>>();

    let tokens = &theme.tokens;
    let space = &tokens.primitive.space;
    ui.add_space(space.sm);
    egui::Frame::new()
        .fill(tokens.semantic.color.chrome)
        .stroke(egui::Stroke::new(
            tokens.primitive.stroke.hairline,
            tokens.semantic.color.border,
        ))
        .corner_radius(tokens.primitive.radius.round)
        .inner_margin(egui::Margin::same(space.xxs as i8))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                for (index, (space_id, name, color)) in spaces.into_iter().enumerate() {
                    let response = space_bubble(ui, space_id, color, space_id == active, theme)
                        .on_hover_text(format!("{name} · Control+{}", index + 1));
                    if response.clicked() && browser.switch_space(space_id) {
                        *address_input = browser.active_url_for_input();
                    }
                    attach_space_context_menu(&response, browser, space_id, &name, color, theme);
                }

                if add_space_bubble(ui, theme)
                    .on_hover_text("Create a new space")
                    .clicked()
                {
                    let ordinal = browser.spaces().len() + 1;
                    let color = SpaceColor::ALL[(ordinal - 1) % SpaceColor::ALL.len()];
                    let id = browser.create_space(format!("Space {ordinal}"), color);
                    browser.switch_space(id);
                    *address_input = browser.active_url_for_input();
                }
            });
        });
}

fn apply_native_space_menu_requests(ui: &egui::Ui, browser: &mut BrowserState) {
    for action in native_menu::take_space_menu_requests() {
        match action.kind {
            native_menu::SpaceMenuActionKind::Rename => {
                let name = browser
                    .space(action.space_id)
                    .map(|space| space.name().to_owned())
                    .unwrap_or_default();
                ui.ctx().data_mut(|data| {
                    data.insert_temp(egui::Id::new("space-rename-dialog"), action.space_id);
                    data.insert_temp(egui::Id::new(("space-rename-value", action.space_id)), name);
                });
            }
            native_menu::SpaceMenuActionKind::Recolor(color) => {
                browser.recolor_space(action.space_id, color);
            }
            native_menu::SpaceMenuActionKind::Delete => {
                #[cfg(target_os = "macos")]
                if let Some(space) = browser.space(action.space_id) {
                    native_menu::show_space_delete_confirmation(
                        action.space_id,
                        space.name().to_owned(),
                    );
                }
                #[cfg(not(target_os = "macos"))]
                ui.ctx().data_mut(|data| {
                    data.insert_temp(egui::Id::new("space-delete-confirm"), action.space_id);
                });
            }
        }
    }
}

fn attach_space_context_menu(
    response: &egui::Response,
    browser: &mut BrowserState,
    space_id: SpaceId,
    _name: &str,
    _color: SpaceColor,
    _theme: &Theme,
) {
    #[cfg(target_os = "macos")]
    if response.secondary_clicked() {
        native_menu::show_space_context_menu(space_id, _color, browser.spaces().len() > 1);
    }

    #[cfg(not(target_os = "macos"))]
    response.context_menu(|ui| space_context_menu(ui, browser, space_id, _name, _theme));
}

fn space_bubble(
    ui: &mut egui::Ui,
    space_id: SpaceId,
    color: SpaceColor,
    selected: bool,
    theme: &Theme,
) -> egui::Response {
    let tokens = &theme.tokens;
    let component = &tokens.component.space_switcher;
    let (rect, response) = ui
        .push_id(space_id, |ui| {
            ui.allocate_exact_size(
                egui::Vec2::splat(component.bubble_hit_size),
                egui::Sense::click(),
            )
        })
        .inner;
    let id = response.id;
    let hover = ui.ctx().animate_bool_with_time_and_easing(
        id.with("hover"),
        response.hovered(),
        tokens.primitive.motion.space_bubble_seconds,
        egui::emath::easing::cubic_out,
    );
    let active = ui.ctx().animate_bool_with_time_and_easing(
        id.with("active"),
        selected,
        tokens.primitive.motion.space_bubble_seconds,
        egui::emath::easing::cubic_out,
    );
    let center = rect.center();
    let base_radius = component.bubble_size * 0.5;
    let radius = base_radius + hover * 0.35 + active * 0.5;
    let color = space_color(color, theme);
    let painter = ui.painter();

    let selection_strength = (active * 0.9 + hover * 0.45).min(1.0);
    let selection = with_alpha(
        tokens.semantic.color.surface_active,
        (f32::from(tokens.semantic.color.surface_active.a()) * selection_strength) as u8,
    );
    painter.rect_filled(
        rect.shrink(tokens.primitive.space.xxs),
        tokens.primitive.radius.round,
        selection,
    );
    let seed = (id.value() & 0xffff) as f32 / 65_535.0 * std::f32::consts::TAU;
    let time = ui.input(|input| input.time) as f32;
    paint_energy_orb(
        painter,
        center,
        radius,
        color,
        seed + time * 0.22,
        active,
        hover,
    );
    if selected || response.hovered() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(40));
    }
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(tokens.primitive.space.xxs),
            tokens.primitive.radius.round,
            egui::Stroke::new(tokens.primitive.stroke.thin, tokens.semantic.color.focus),
            egui::StrokeKind::Inside,
        );
    }
    response
}

#[allow(clippy::too_many_arguments)]
fn paint_energy_orb(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    color: egui::Color32,
    phase: f32,
    active: f32,
    hover: f32,
) {
    let energy = active.max(hover);
    painter.circle_filled(
        center,
        radius + 5.0,
        with_alpha(color, (10.0 + energy * 18.0) as u8),
    );
    painter.circle_filled(
        center,
        radius + 2.5,
        with_alpha(color, (24.0 + energy * 22.0) as u8),
    );
    painter.circle_filled(center, radius, mix_color(color, egui::Color32::BLACK, 0.12));
    painter.circle_filled(
        center,
        radius * 0.88,
        with_alpha(mix_color(color, egui::Color32::WHITE, 0.28), 80),
    );

    let strand_color = mix_color(color, egui::Color32::WHITE, 0.72);
    let point_count = 28;
    for strand in 0..9 {
        let strand = strand as f32;
        let mut points = Vec::with_capacity(point_count);
        for point in 0..point_count {
            let angle = std::f32::consts::TAU * point as f32 / point_count as f32;
            let ripple = (angle * 3.0 + phase * (0.7 + strand * 0.035) + strand * 1.31).sin()
                + 0.5 * (angle * 7.0 - phase * 0.45 + strand * 0.83).sin();
            let strand_radius = radius * (0.82 + strand * 0.015 + ripple * 0.035);
            points.push(center + egui::vec2(angle.cos(), angle.sin()) * strand_radius);
        }
        painter.add(egui::Shape::closed_line(
            points,
            egui::Stroke::new(
                0.35 + energy * 0.12,
                with_alpha(strand_color, (32.0 + energy * 26.0) as u8),
            ),
        ));
    }

    for strand in 0..6 {
        let strand = strand as f32;
        let rotation = phase * (0.12 + strand * 0.014) + strand * 0.91;
        let (sin_rotation, cos_rotation) = rotation.sin_cos();
        let mut points = Vec::with_capacity(point_count);
        for point in 0..point_count {
            let angle = std::f32::consts::TAU * point as f32 / point_count as f32;
            let x = angle.cos() * radius * (0.72 + 0.025 * strand);
            let y = angle.sin()
                * radius
                * (0.18 + 0.035 * strand + 0.025 * (angle * 4.0 + phase).sin());
            points.push(
                center
                    + egui::vec2(
                        x * cos_rotation - y * sin_rotation,
                        x * sin_rotation + y * cos_rotation,
                    ),
            );
        }
        painter.add(egui::Shape::closed_line(
            points,
            egui::Stroke::new(0.35, with_alpha(strand_color, (18.0 + energy * 18.0) as u8)),
        ));
    }

    for (offset, strength) in [(0.35, 1.0), (3.42, 0.75)] {
        let angle = phase * 0.55 + offset;
        let hotspot = center + egui::vec2(angle.cos(), angle.sin()) * radius * 0.86;
        painter.circle_filled(
            hotspot,
            2.8 + energy,
            with_alpha(color, (18.0 + energy * 22.0) as u8),
        );
        painter.circle_filled(
            hotspot,
            1.1 + energy * strength,
            with_alpha(egui::Color32::WHITE, (115.0 + energy * 90.0) as u8),
        );
    }
}

fn add_space_bubble(ui: &mut egui::Ui, theme: &Theme) -> egui::Response {
    let tokens = &theme.tokens;
    let component = &tokens.component.space_switcher;
    let (rect, response) = ui.allocate_exact_size(
        egui::Vec2::splat(component.bubble_hit_size),
        egui::Sense::click(),
    );
    let hover = ui.ctx().animate_bool_with_time_and_easing(
        response.id.with("hover"),
        response.hovered(),
        tokens.primitive.motion.space_bubble_seconds,
        egui::emath::easing::cubic_out,
    );
    let center = rect.center();
    let painter = ui.painter();
    painter.rect_filled(
        rect.shrink(tokens.primitive.space.xxs),
        tokens.primitive.radius.round,
        with_alpha(tokens.semantic.color.surface_active, (120.0 * hover) as u8),
    );
    let arm = tokens.primitive.space.xs + hover;
    let stroke = egui::Stroke::new(
        tokens.primitive.stroke.thin,
        mix_color(
            tokens.semantic.color.text_muted,
            tokens.semantic.color.text_strong,
            hover,
        ),
    );
    painter.line_segment(
        [center - egui::vec2(arm, 0.0), center + egui::vec2(arm, 0.0)],
        stroke,
    );
    painter.line_segment(
        [center - egui::vec2(0.0, arm), center + egui::vec2(0.0, arm)],
        stroke,
    );
    response
}

#[cfg(not(target_os = "macos"))]
fn space_context_menu(
    ui: &mut egui::Ui,
    browser: &mut BrowserState,
    space_id: SpaceId,
    current_name: &str,
    theme: &Theme,
) {
    ui.set_min_width(190.0);
    let rename_id = egui::Id::new(("space-rename", space_id));
    let mut name = ui
        .ctx()
        .data_mut(|data| data.get_temp::<String>(rename_id))
        .unwrap_or_else(|| current_name.to_owned());
    ui.label("Space name");
    let rename = ui.text_edit_singleline(&mut name);
    ui.ctx()
        .data_mut(|data| data.insert_temp(rename_id, name.clone()));
    if (rename.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)))
        || ui.button("Rename").clicked()
    {
        browser.rename_space(space_id, name);
        ui.close();
    }

    ui.separator();
    ui.label("Color");
    ui.horizontal_wrapped(|ui| {
        for color in SpaceColor::ALL {
            if ui
                .add(
                    egui::Button::new("")
                        .fill(space_color(color, theme))
                        .min_size(egui::Vec2::splat(22.0))
                        .corner_radius(11.0),
                )
                .clicked()
            {
                browser.recolor_space(space_id, color);
                ui.close();
            }
        }
    });

    ui.separator();
    let can_delete = browser.spaces().len() > 1;
    if ui
        .add_enabled(
            can_delete,
            egui::Button::new(
                egui::RichText::new("Delete space…").color(theme.tokens.semantic.color.danger),
            ),
        )
        .clicked()
    {
        ui.ctx()
            .data_mut(|data| data.insert_temp(egui::Id::new("space-delete-confirm"), space_id));
        ui.close();
    }
}

fn rename_space_dialog(ui: &mut egui::Ui, browser: &mut BrowserState, theme: &Theme) {
    let dialog_id = egui::Id::new("space-rename-dialog");
    let Some(space_id) = ui
        .ctx()
        .data_mut(|data| data.get_temp::<SpaceId>(dialog_id))
    else {
        return;
    };
    if browser.space(space_id).is_none() {
        ui.ctx().data_mut(|data| data.remove::<SpaceId>(dialog_id));
        return;
    }

    let value_id = egui::Id::new(("space-rename-value", space_id));
    let mut name = ui
        .ctx()
        .data_mut(|data| data.get_temp::<String>(value_id))
        .unwrap_or_default();
    egui::Window::new("Rename Space")
        .id(egui::Id::new("rename-space-window"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ui.ctx(), |ui| {
            let response = ui.text_edit_singleline(&mut name);
            response.request_focus();
            let submit =
                response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                if DsButton::new("Cancel").show(ui, theme).clicked() {
                    close_space_rename_dialog(ui, dialog_id, value_id);
                }
                if (DsButton::new("Rename").show(ui, theme).clicked() || submit)
                    && !name.trim().is_empty()
                {
                    browser.rename_space(space_id, &name);
                    close_space_rename_dialog(ui, dialog_id, value_id);
                }
            });
        });
    ui.ctx().data_mut(|data| data.insert_temp(value_id, name));
}

fn close_space_rename_dialog(ui: &egui::Ui, dialog_id: egui::Id, value_id: egui::Id) {
    ui.ctx().data_mut(|data| {
        data.remove::<SpaceId>(dialog_id);
        data.remove::<String>(value_id);
    });
}

fn confirm_space_deletion(
    _ui: &mut egui::Ui,
    browser: &mut BrowserState,
    address_input: &mut String,
    _theme: &Theme,
) {
    #[cfg(target_os = "macos")]
    for space_id in native_menu::take_confirmed_space_deletions() {
        delete_space_and_refresh_address(browser, address_input, space_id);
    }

    #[cfg(not(target_os = "macos"))]
    show_space_delete_confirmation_fallback(_ui, browser, address_input, _theme);
}

fn delete_space_and_refresh_address(
    browser: &mut BrowserState,
    address_input: &mut String,
    space_id: SpaceId,
) {
    if browser.delete_space(space_id) {
        *address_input = browser.active_url_for_input();
    }
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
fn show_space_delete_confirmation_fallback(
    ui: &mut egui::Ui,
    browser: &mut BrowserState,
    address_input: &mut String,
    theme: &Theme,
) {
    let id = egui::Id::new("space-delete-confirm");
    let Some(space_id) = ui.ctx().data_mut(|data| data.get_temp::<SpaceId>(id)) else {
        return;
    };
    let name = browser
        .space(space_id)
        .map(|space| space.name().to_owned())
        .unwrap_or_else(|| "this space".to_owned());
    egui::Window::new("Delete space?")
        .id(egui::Id::new("delete-space-window"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ui.ctx(), |ui| {
            ui.label(format!(
                "Delete {name}, all of its tabs, and its local cookies and site data?"
            ));
            ui.horizontal(|ui| {
                if DsButton::new("Cancel").show(ui, theme).clicked() {
                    ui.ctx().data_mut(|data| data.remove::<SpaceId>(id));
                }
                if DsButton::new("Delete").danger().show(ui, theme).clicked() {
                    delete_space_and_refresh_address(browser, address_input, space_id);
                    ui.ctx().data_mut(|data| data.remove::<SpaceId>(id));
                }
            });
        });
}

pub(crate) fn has_modal_open(context: &egui::Context) -> bool {
    context.data_mut(|data| {
        data.get_temp::<SpaceId>(egui::Id::new("space-delete-confirm"))
            .is_some()
            || data
                .get_temp::<SpaceId>(egui::Id::new("space-rename-dialog"))
                .is_some()
    })
}

fn space_color(color: SpaceColor, theme: &Theme) -> egui::Color32 {
    let primitive = &theme.tokens.primitive.color;
    match color {
        SpaceColor::Violet => primitive.violet_400,
        SpaceColor::Blue => primitive.blue_500,
        SpaceColor::Green => primitive.green_400,
        SpaceColor::Amber => primitive.amber_400,
        SpaceColor::Rose => primitive.rose_400,
        SpaceColor::Slate => primitive.slate_400,
    }
}

fn with_alpha(color: egui::Color32, alpha: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

fn mix_color(a: egui::Color32, b: egui::Color32, amount: f32) -> egui::Color32 {
    let amount = amount.clamp(0.0, 1.0);
    let mix = |left: u8, right: u8| {
        (f32::from(left) + (f32::from(right) - f32::from(left)) * amount).round() as u8
    };
    egui::Color32::from_rgba_unmultiplied(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
        mix(a.a(), b.a()),
    )
}

fn highlighted_pinned_tabs(
    ui: &mut egui::Ui,
    browser: &BrowserState,
    theme: &Theme,
    dragging: Option<DraggedTab>,
    drop_target: &mut Option<DropTarget>,
    actions: &mut Vec<TabAction>,
    animate_reorder: bool,
) {
    let tabs = browser
        .tabs()
        .iter()
        .filter(|tab| tab.group() == TabGroup::Highlight)
        .filter(|tab| dragging.is_none_or(|dragged| tab.id != dragged.id))
        .cloned()
        .collect::<Vec<_>>();

    if tabs.is_empty() && dragging.is_none() {
        return;
    }

    let spacing = theme.tokens.primitive.space.sm;
    let total_spacing = spacing * (HIGHLIGHT_COLUMNS - 1) as f32;
    let available_width = ui.available_width().max(1.0);
    let tile_size = ((available_width - total_spacing) / HIGHLIGHT_COLUMNS as f32).max(1.0);
    let top_left = ui.cursor().min;
    let virtual_count = if dragging.is_some() {
        tabs.len() + 1
    } else {
        tabs.len()
    };
    let virtual_rows = virtual_count.max(1).div_ceil(HIGHLIGHT_COLUMNS);
    let accept_height =
        virtual_rows as f32 * tile_size + virtual_rows.saturating_sub(1) as f32 * spacing;
    let accept_rect = egui::Rect::from_min_size(
        top_left,
        egui::vec2(available_width, accept_height.max(tile_size)),
    );

    let hovered_index = dragging.and_then(|_| {
        let pointer = ui.ctx().pointer_interact_pos()?;
        accept_rect
            .expand2(egui::vec2(0.0, spacing * 0.5))
            .contains(pointer)
            .then(|| {
                grid_insertion_index(
                    pointer,
                    top_left,
                    tile_size,
                    spacing,
                    tabs.len(),
                    HIGHLIGHT_COLUMNS,
                )
            })
    });
    if let Some(index) = hovered_index {
        *drop_target = Some(DropTarget {
            group: TabGroup::Highlight,
            index,
        });
    }

    let source_placeholder =
        dragging.and_then(|dragged| group_index(browser.tabs(), dragged.id, TabGroup::Highlight));
    let placeholder = hovered_index
        .or(source_placeholder)
        .or_else(|| (dragging.is_some() && tabs.is_empty()).then_some(0));
    let mut slots = tabs.into_iter().map(Some).collect::<Vec<_>>();
    if let Some(index) = placeholder {
        slots.insert(index.min(slots.len()), None);
    }

    let row_count = slots.len().max(1).div_ceil(HIGHLIGHT_COLUMNS);
    let height = row_count as f32 * tile_size + row_count.saturating_sub(1) as f32 * spacing;
    let (block_rect, _) =
        ui.allocate_exact_size(egui::vec2(available_width, height), egui::Sense::hover());

    for (slot, tab) in slots.iter().enumerate() {
        let target_rect = highlight_slot_rect(block_rect.min, slot, tile_size, spacing);
        let Some(tab) = tab else {
            paint_drop_placeholder(ui, target_rect, "Highlight", theme);
            continue;
        };

        let rect = if animate_reorder {
            animated_tab_rect(ui, tab.id, target_rect, theme)
        } else {
            target_rect
        };
        let mut tile_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        let is_active = browser.tabs()[browser.active_index()].id == tab.id;
        let (response, return_clicked, close_clicked) =
            highlighted_pin_tile(&mut tile_ui, tab, is_active, rect.width(), theme);
        response.dnd_set_drag_payload(DraggedTab { id: tab.id });

        let available_actions = browser.context_actions(tab.id);
        if close_clicked {
            actions.push(TabAction::new(tab.id, TabActionKind::Close));
        } else if return_clicked {
            actions.push(TabAction::new(tab.id, TabActionKind::ReturnToPinned));
        } else if let Some(kind) =
            tab_action_for_pointer(&response, available_actions.contains(&TabActionKind::Close))
        {
            actions.push(TabAction::new(tab.id, kind));
        }

        attach_tab_context_menu(&response, tab, available_actions, actions, theme);
    }
}

fn highlighted_pin_tile(
    ui: &mut egui::Ui,
    tab: &Tab,
    selected: bool,
    size: f32,
    theme: &Theme,
) -> (egui::Response, bool, bool) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click_and_drag());
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
    let return_clicked = if tab.is_away_from_pinned() {
        let icon_size = 14.0;
        let icon_rect = egui::Rect::from_min_size(
            rect.left_top() + egui::vec2(6.0, 6.0),
            egui::Vec2::splat(icon_size),
        );
        let icon_response = ui
            .interact(
                icon_rect.expand(4.0),
                egui::Id::new(("return-to-pinned", tab.id)),
                egui::Sense::click(),
            )
            .on_hover_text("Return to pinned tab");
        Icon::ArrowLeft
            .image(icon_size, color.text_muted)
            .paint_at(ui, icon_rect);
        icon_response.clicked()
    } else {
        false
    };
    let close_clicked = if tab.is_open() && ui.rect_contains_pointer(rect) {
        let icon_size = 14.0;
        let icon_rect = egui::Rect::from_min_size(
            rect.right_top() + egui::vec2(-icon_size - 6.0, 6.0),
            egui::Vec2::splat(icon_size),
        );
        let icon_response = ui
            .interact(
                icon_rect.expand(4.0),
                egui::Id::new(("close-highlighted-tab", tab.id)),
                egui::Sense::click(),
            )
            .on_hover_text("Close tab");
        Icon::X
            .image(icon_size, color.text_muted)
            .paint_at(ui, icon_rect);
        close_control_clicked(&icon_response)
    } else {
        false
    };
    (response, return_clicked, close_clicked)
}

#[cfg(not(target_os = "macos"))]
fn tab_context_menu(
    ui: &mut egui::Ui,
    tab: &Tab,
    available: &[TabActionKind],
    actions: &mut Vec<TabAction>,
    theme: &Theme,
) {
    let menu = &theme.tokens.component.menu;
    ui.set_min_width(menu.width);
    ui.set_max_width(menu.width);

    let mut previous_section = None;
    for kind in available {
        let section = tab_action_section(kind);
        if previous_section.is_some_and(|previous| previous != section) {
            ui.separator();
        }
        let (label, icon) = tab_action_presentation(tab, kind);
        let item = MenuItem::new(&label, icon);
        let clicked = if matches!(kind, TabActionKind::Close) {
            item.danger().show(ui, theme).clicked()
        } else {
            item.show(ui, theme).clicked()
        };
        if clicked {
            actions.push(TabAction::new(tab.id, kind.clone()));
            ui.close();
        }
        previous_section = Some(section);
    }
}

#[cfg(not(target_os = "macos"))]
fn tab_action_presentation(tab: &Tab, kind: &TabActionKind) -> (String, Icon) {
    match kind {
        TabActionKind::ReturnToPinned => ("Return to pinned tab".to_owned(), Icon::ArrowLeft),
        TabActionKind::Demote => ("Demote from highlight".to_owned(), Icon::ChevronDown),
        TabActionKind::Promote => ("Promote to highlight".to_owned(), Icon::ChevronUp),
        TabActionKind::TogglePin if tab.is_organized() => ("Unpin tab".to_owned(), Icon::Pin),
        TabActionKind::TogglePin => ("Pin tab".to_owned(), Icon::Pin),
        TabActionKind::MoveUp => ("Move up".to_owned(), Icon::ChevronUp),
        TabActionKind::MoveDown => ("Move down".to_owned(), Icon::ChevronDown),
        TabActionKind::MoveToSpace { name, .. } => (format!("Move to {name}"), Icon::ArrowRight),
        TabActionKind::Close => ("Close tab".to_owned(), Icon::X),
        _ => ("Tab action".to_owned(), Icon::ArrowRight),
    }
}

#[cfg(not(target_os = "macos"))]
fn tab_action_section(kind: &TabActionKind) -> u8 {
    match kind {
        TabActionKind::MoveUp | TabActionKind::MoveDown => 1,
        TabActionKind::MoveToSpace { .. } => 2,
        TabActionKind::Close => 3,
        _ => 0,
    }
}

fn attach_tab_context_menu(
    response: &egui::Response,
    tab: &Tab,
    available: Vec<TabActionKind>,
    _actions: &mut Vec<TabAction>,
    _theme: &Theme,
) {
    #[cfg(target_os = "macos")]
    if response.secondary_clicked() {
        native_menu::show_tab_context_menu(tab, available);
    }

    #[cfg(not(target_os = "macos"))]
    response.context_menu(|ui| tab_context_menu(ui, tab, &available, _actions, _theme));
}

fn tab_action_for_pointer(response: &egui::Response, can_close: bool) -> Option<TabActionKind> {
    if response.clicked_by(egui::PointerButton::Middle) {
        tab_action_for_button(egui::PointerButton::Middle, can_close)
    } else if response.clicked() {
        tab_action_for_button(egui::PointerButton::Primary, can_close)
    } else {
        None
    }
}

fn tab_action_for_button(button: egui::PointerButton, can_close: bool) -> Option<TabActionKind> {
    match button {
        egui::PointerButton::Middle if can_close => Some(TabActionKind::Close),
        egui::PointerButton::Primary => Some(TabActionKind::Select),
        _ => None,
    }
}

fn close_control_clicked(response: &egui::Response) -> bool {
    response.clicked() || response.clicked_by(egui::PointerButton::Middle)
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

    let image = normalized_favicon_image(favicon);
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

fn normalized_favicon_image(favicon: &crate::browser::Favicon) -> egui::ColorImage {
    let [width, height] = favicon.size();
    let rgba = favicon.rgba();
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;

    for y in 0..height {
        for x in 0..width {
            if rgba[(y * width + x) * 4 + 3] > 8 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    if min_x > max_x || min_y > max_y {
        return egui::ColorImage::from_rgba_unmultiplied(favicon.size(), favicon.rgba());
    }

    let content_width = max_x - min_x + 1;
    let content_height = max_y - min_y + 1;
    let side = content_width.max(content_height);
    let offset_x = (side - content_width) / 2;
    let offset_y = (side - content_height) / 2;
    let mut normalized = vec![0; side * side * 4];

    for y in 0..content_height {
        for x in 0..content_width {
            let source = ((min_y + y) * width + min_x + x) * 4;
            let target = ((offset_y + y) * side + offset_x + x) * 4;
            normalized[target..target + 4].copy_from_slice(&rgba[source..source + 4]);
        }
    }

    egui::ColorImage::from_rgba_unmultiplied([side, side], &normalized)
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
    browser: &BrowserState,
    theme: &Theme,
    dragging: Option<DraggedTab>,
    drop_target: &mut Option<DropTarget>,
    actions: &mut Vec<TabAction>,
    animate_reorder: bool,
) {
    let has_pinned_tabs = browser
        .tabs()
        .iter()
        .any(|tab| tab.group() != TabGroup::Today);

    tab_row_group(
        ui,
        browser,
        TabGroup::Pinned,
        "Pinned",
        theme,
        dragging,
        drop_target,
        actions,
        animate_reorder,
    );
    tab_row_group(
        ui,
        browser,
        TabGroup::Today,
        if has_pinned_tabs || dragging.is_some() {
            "Today"
        } else {
            "Tabs"
        },
        theme,
        dragging,
        drop_target,
        actions,
        animate_reorder,
    );
}

#[allow(clippy::too_many_arguments)]
fn tab_row_group(
    ui: &mut egui::Ui,
    browser: &BrowserState,
    group: TabGroup,
    label: &str,
    theme: &Theme,
    dragging: Option<DraggedTab>,
    drop_target: &mut Option<DropTarget>,
    actions: &mut Vec<TabAction>,
    animate_reorder: bool,
) {
    let mut tabs = browser
        .tabs()
        .iter()
        .filter(|tab| tab.group() == group)
        .filter(|tab| dragging.is_none_or(|dragged| tab.id != dragged.id))
        .cloned()
        .collect::<Vec<_>>();

    if tabs.is_empty() && dragging.is_none() {
        return;
    }

    section_label(ui, label, theme);
    ui.add_space(theme.tokens.primitive.space.xs);

    let row_width = ui.available_width().max(1.0);
    let row_height = theme.tokens.component.tab.height;
    let spacing = theme.tokens.primitive.space.xs;
    let top_left = ui.cursor().min;
    let accept_rows = if dragging.is_some() && tabs.is_empty() {
        1
    } else if dragging.is_some() {
        tabs.len() + 1
    } else {
        tabs.len().max(1)
    };
    let accept_height =
        accept_rows as f32 * row_height + accept_rows.saturating_sub(1) as f32 * spacing;
    let accept_rect = egui::Rect::from_min_size(top_left, egui::vec2(row_width, accept_height));

    let hovered_index = dragging.and_then(|_| {
        let pointer = ui.ctx().pointer_interact_pos()?;
        accept_rect
            .contains(pointer)
            .then(|| row_insertion_index(pointer.y, top_left.y, row_height, spacing, tabs.len()))
    });
    if let Some(index) = hovered_index {
        *drop_target = Some(DropTarget { group, index });
    }

    let source_placeholder =
        dragging.and_then(|dragged| group_index(browser.tabs(), dragged.id, group));
    let placeholder = hovered_index
        .or(source_placeholder)
        .or_else(|| (dragging.is_some() && tabs.is_empty()).then_some(0));
    let mut slots = tabs.drain(..).map(Some).collect::<Vec<_>>();
    if let Some(index) = placeholder {
        slots.insert(index.min(slots.len()), None);
    }

    let block_rows = slots.len().max(1);
    let block_height =
        block_rows as f32 * row_height + block_rows.saturating_sub(1) as f32 * spacing;
    let (block_rect, _) =
        ui.allocate_exact_size(egui::vec2(row_width, block_height), egui::Sense::hover());

    for (slot, tab) in slots.iter().enumerate() {
        let target_rect = egui::Rect::from_min_size(
            block_rect.min + egui::vec2(0.0, slot as f32 * (row_height + spacing)),
            egui::vec2(row_width, row_height),
        );
        let Some(tab) = tab else {
            paint_drop_placeholder(ui, target_rect, label, theme);
            continue;
        };
        let rect = if animate_reorder {
            animated_tab_rect(ui, tab.id, target_rect, theme)
        } else {
            target_rect
        };
        paint_tab_row_at(
            ui,
            rect,
            tab,
            browser.active_tab().id == tab.id,
            browser.context_actions(tab.id),
            theme,
            actions,
        );
    }
}

fn paint_tab_row_at(
    ui: &mut egui::Ui,
    row_rect: egui::Rect,
    tab: &Tab,
    is_active: bool,
    available_actions: Vec<TabActionKind>,
    theme: &Theme,
    actions: &mut Vec<TabAction>,
) {
    ui.push_id(("tab-row", tab.id), |ui| {
        let favicon = favicon_texture(ui, tab);
        let action_size = theme
            .tokens
            .component
            .tab
            .close_size
            .max(theme.tokens.component.button.height_sm);
        let spacing = theme.tokens.primitive.space.xs;
        let is_away_from_pin = tab.is_away_from_pinned();
        let actions_width = if is_away_from_pin {
            action_size + spacing
        } else {
            0.0
        };
        let tab_width = (row_rect.width() - actions_width).max(0.0);
        let row_hovered = ui.rect_contains_pointer(row_rect);
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
            .close_visible(tab.is_open() && row_hovered)
            .width(tab_width)
            .show(&mut row_ui, theme);
        tab_response.dnd_set_drag_payload(DraggedTab { id: tab.id });
        if let Some(kind) = tab_action_for_pointer(
            &tab_response,
            available_actions.contains(&TabActionKind::Close),
        ) {
            actions.push(TabAction::new(tab.id, kind));
        }

        attach_tab_context_menu(&tab_response, tab, available_actions, actions, theme);

        if is_away_from_pin
            && DsButton::icon(Icon::ArrowLeft)
                .ghost()
                .small()
                .width(action_size)
                .show(&mut row_ui, theme)
                .on_hover_text("Return to pinned tab")
                .clicked()
        {
            actions.push(TabAction::new(tab.id, TabActionKind::ReturnToPinned));
        }
        if tab.is_open() && row_hovered {
            let close_rect = egui::Rect::from_center_size(
                egui::pos2(
                    tab_response.rect.right() - spacing - action_size * 0.5,
                    tab_response.rect.center().y,
                ),
                egui::Vec2::splat(action_size),
            );
            let mut close_ui = row_ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(close_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            let close_response = DsButton::icon(Icon::X)
                .ghost()
                .small()
                .width(action_size)
                .show(&mut close_ui, theme)
                .on_hover_text("Close tab");
            if close_control_clicked(&close_response) {
                actions.push(TabAction::new(tab.id, TabActionKind::Close));
            }
        }
    });
}

fn apply_tab_actions(
    browser: &mut BrowserState,
    address_input: &mut String,
    actions: Vec<TabAction>,
) {
    let mut active_page_changed = false;
    for action in actions {
        active_page_changed |= browser.apply_tab_action(action).active_page_changed();
    }
    if active_page_changed {
        *address_input = browser.active_url_for_input();
    }
}

fn group_index(tabs: &[Tab], tab_id: TabId, group: TabGroup) -> Option<usize> {
    tabs.iter()
        .filter(|tab| tab.group() == group)
        .position(|tab| tab.id == tab_id)
}

fn row_insertion_index(
    pointer_y: f32,
    top: f32,
    row_height: f32,
    spacing: f32,
    item_count: usize,
) -> usize {
    let step = row_height + spacing;
    (0..item_count)
        .position(|index| pointer_y < top + index as f32 * step + row_height * 0.5)
        .unwrap_or(item_count)
}

fn grid_insertion_index(
    pointer: egui::Pos2,
    origin: egui::Pos2,
    tile_size: f32,
    spacing: f32,
    item_count: usize,
    columns: usize,
) -> usize {
    if item_count == 0 || columns == 0 {
        return 0;
    }

    let step = tile_size + spacing;
    let row = ((pointer.y - origin.y) / step).floor().max(0.0) as usize;
    let column = ((pointer.x - origin.x) / step)
        .floor()
        .max(0.0)
        .min((columns - 1) as f32) as usize;
    let cell_left = origin.x + column as f32 * step;
    let after_cell = usize::from(pointer.x >= cell_left + tile_size * 0.5);

    (row * columns + column + after_cell).min(item_count)
}

fn highlight_slot_rect(
    origin: egui::Pos2,
    slot: usize,
    tile_size: f32,
    spacing: f32,
) -> egui::Rect {
    let column = slot % HIGHLIGHT_COLUMNS;
    let row = slot / HIGHLIGHT_COLUMNS;
    egui::Rect::from_min_size(
        origin
            + egui::vec2(
                column as f32 * (tile_size + spacing),
                row as f32 * (tile_size + spacing),
            ),
        egui::Vec2::splat(tile_size),
    )
}

fn animated_tab_rect(
    ui: &egui::Ui,
    tab_id: TabId,
    target: egui::Rect,
    theme: &Theme,
) -> egui::Rect {
    let duration = theme.tokens.primitive.motion.tab_reorder_seconds;
    let x = ui.ctx().animate_value_with_time(
        egui::Id::new(("tab-reorder-x", tab_id)),
        target.min.x,
        duration,
    );
    let y = ui.ctx().animate_value_with_time(
        egui::Id::new(("tab-reorder-y", tab_id)),
        target.min.y,
        duration,
    );
    let width = ui.ctx().animate_value_with_time(
        egui::Id::new(("tab-reorder-width", tab_id)),
        target.width(),
        duration,
    );
    let height = ui.ctx().animate_value_with_time(
        egui::Id::new(("tab-reorder-height", tab_id)),
        target.height(),
        duration,
    );

    egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(width, height))
}

fn paint_drop_placeholder(ui: &egui::Ui, rect: egui::Rect, label: &str, theme: &Theme) {
    let color = &theme.tokens.semantic.color;
    ui.painter().rect_filled(
        rect,
        theme.tokens.component.tab.radius,
        color.surface_active.gamma_multiply(0.55),
    );
    ui.painter().rect_stroke(
        rect,
        theme.tokens.component.tab.radius,
        egui::Stroke::new(theme.tokens.primitive.stroke.thin, color.focus),
        egui::StrokeKind::Inside,
    );
    if rect.width() >= 72.0 {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(theme.tokens.primitive.typography.caption),
            color.text_muted,
        );
    }
}

fn floating_drag_preview(
    ui: &egui::Ui,
    browser: &BrowserState,
    dragged: DraggedTab,
    target: Option<DropTarget>,
    theme: &Theme,
) {
    let Some(pointer) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    let Some(tab) = browser.tabs().iter().find(|tab| tab.id == dragged.id) else {
        return;
    };
    let group = target.map_or_else(|| tab.group(), |target| target.group);
    let spacing = theme.tokens.primitive.space.sm;
    let tile_size = ((ui.available_width() - spacing * (HIGHLIGHT_COLUMNS - 1) as f32)
        / HIGHLIGHT_COLUMNS as f32)
        .max(1.0);
    let desired = if group == TabGroup::Highlight {
        egui::Vec2::splat(tile_size)
    } else {
        egui::vec2(
            ui.available_width().max(120.0),
            theme.tokens.component.tab.height,
        )
    };
    let duration = theme.tokens.primitive.motion.tab_reorder_seconds;
    let width = ui.ctx().animate_value_with_time(
        egui::Id::new(("drag-preview-width", dragged.id)),
        desired.x,
        duration,
    );
    let height = ui.ctx().animate_value_with_time(
        egui::Id::new(("drag-preview-height", dragged.id)),
        desired.y,
        duration,
    );
    let rect =
        egui::Rect::from_center_size(pointer + egui::vec2(0.0, 6.0), egui::vec2(width, height));
    let layer = egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new(("drag-preview", dragged.id)),
    );
    let painter = ui.ctx().layer_painter(layer);
    let color = &theme.tokens.semantic.color;
    let radius = if group == TabGroup::Highlight {
        theme.tokens.primitive.radius.lg
    } else {
        theme.tokens.component.tab.radius
    };
    painter.rect_filled(rect.translate(egui::vec2(0.0, 5.0)), radius, color.shadow);
    painter.rect_filled(rect, radius, color.surface_overlay);
    painter.rect_stroke(
        rect,
        radius,
        egui::Stroke::new(theme.tokens.primitive.stroke.thin, color.focus),
        egui::StrokeKind::Inside,
    );

    let favicon = favicon_texture(ui, tab);
    if group == TabGroup::Highlight {
        if let Some(texture) = favicon {
            let image_size = (rect.height() * 0.5).clamp(20.0, 36.0);
            painter.image(
                texture.id(),
                egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(image_size)),
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                tile_label(&tab.title),
                egui::FontId::proportional(theme.tokens.primitive.typography.body_strong),
                color.text_strong,
            );
        }
    } else {
        let mut text_left = rect.left() + theme.tokens.primitive.space.md;
        if let Some(texture) = favicon {
            let image_rect = egui::Rect::from_center_size(
                egui::pos2(text_left + 9.0, rect.center().y),
                egui::Vec2::splat(18.0),
            );
            painter.image(
                texture.id(),
                image_rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            text_left += 18.0 + theme.tokens.primitive.space.xs;
        }
        painter.text(
            egui::pos2(text_left, rect.center().y),
            egui::Align2::LEFT_CENTER,
            tab.title.chars().take(36).collect::<String>(),
            egui::FontId::proportional(theme.tokens.primitive.typography.body),
            color.text,
        );
    }
}

fn section_label(ui: &mut egui::Ui, label: &str, theme: &Theme) {
    ui.label(
        egui::RichText::new(label)
            .color(theme.tokens.semantic.color.text_muted)
            .size(theme.tokens.primitive.typography.caption),
    );
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use crate::browser::{BrowserState, Favicon, SpaceId};

    use super::{
        grid_insertion_index, has_modal_open, normalized_favicon_image, row_insertion_index,
        space_offsets, tab_action_for_button,
    };

    #[test]
    fn middle_click_closes_a_closeable_tab() {
        assert_eq!(
            tab_action_for_button(egui::PointerButton::Middle, true),
            Some(crate::browser::TabActionKind::Close)
        );
    }

    #[test]
    fn middle_click_does_not_close_an_unavailable_tab() {
        assert_eq!(
            tab_action_for_button(egui::PointerButton::Middle, false),
            None
        );
    }

    #[test]
    fn primary_click_still_selects_a_tab() {
        assert_eq!(
            tab_action_for_button(egui::PointerButton::Primary, true),
            Some(crate::browser::TabActionKind::Select)
        );
    }

    #[test]
    fn forward_space_transition_moves_old_left_and_new_in_from_right() {
        assert_eq!(space_offsets(1.0, 0.0, 240.0), (0.0, 240.0));
        assert_eq!(space_offsets(1.0, 0.5, 240.0), (-120.0, 120.0));
        assert_eq!(space_offsets(1.0, 1.0, 240.0), (-240.0, 0.0));
    }

    #[test]
    fn backward_space_transition_mirrors_the_motion() {
        assert_eq!(space_offsets(-1.0, 0.0, 240.0), (0.0, -240.0));
        assert_eq!(space_offsets(-1.0, 0.5, 240.0), (120.0, -120.0));
        assert_eq!(space_offsets(-1.0, 1.0, 240.0), (240.0, 0.0));
    }

    #[test]
    fn deletion_confirmation_marks_the_native_surface_as_obscured() {
        let context = egui::Context::default();
        let space_id = BrowserState::default().active_space().id();
        let state_id = egui::Id::new("space-delete-confirm");

        context.data_mut(|data| data.insert_temp(state_id, space_id));
        assert!(has_modal_open(&context));

        context.data_mut(|data| data.remove::<SpaceId>(state_id));
        assert!(!has_modal_open(&context));
    }

    #[test]
    fn rename_dialog_marks_the_native_surface_as_obscured() {
        let context = egui::Context::default();
        let space_id = BrowserState::default().active_space().id();
        let state_id = egui::Id::new("space-rename-dialog");

        context.data_mut(|data| data.insert_temp(state_id, space_id));
        assert!(has_modal_open(&context));

        context.data_mut(|data| data.remove::<SpaceId>(state_id));
        assert!(!has_modal_open(&context));
    }

    #[test]
    fn favicon_normalization_crops_transparent_margins_to_a_square() {
        let mut rgba = vec![0; 4 * 4 * 4];
        for x in 1..=2 {
            let pixel = (2 * 4 + x) * 4;
            rgba[pixel..pixel + 4].copy_from_slice(&[255, 80, 20, 255]);
        }
        let favicon = Favicon::from_rgba(4, 4, rgba).unwrap();

        let normalized = normalized_favicon_image(&favicon);

        assert_eq!(normalized.size, [2, 2]);
    }

    #[test]
    fn row_drop_uses_each_rows_vertical_midpoint() {
        assert_eq!(row_insertion_index(5.0, 0.0, 20.0, 4.0, 3), 0);
        assert_eq!(row_insertion_index(12.0, 0.0, 20.0, 4.0, 3), 1);
        assert_eq!(row_insertion_index(37.0, 0.0, 20.0, 4.0, 3), 2);
        assert_eq!(row_insertion_index(100.0, 0.0, 20.0, 4.0, 3), 3);
    }

    #[test]
    fn grid_drop_uses_cell_halves_and_clamps_to_the_item_count() {
        let origin = egui::pos2(10.0, 20.0);

        assert_eq!(
            grid_insertion_index(egui::pos2(15.0, 25.0), origin, 40.0, 8.0, 5, 4),
            0
        );
        assert_eq!(
            grid_insertion_index(egui::pos2(35.0, 25.0), origin, 40.0, 8.0, 5, 4),
            1
        );
        assert_eq!(
            grid_insertion_index(egui::pos2(15.0, 75.0), origin, 40.0, 8.0, 5, 4),
            4
        );
        assert_eq!(
            grid_insertion_index(egui::pos2(500.0, 500.0), origin, 40.0, 8.0, 5, 4),
            5
        );
        assert_eq!(
            grid_insertion_index(egui::pos2(15.0, 25.0), origin, 40.0, 8.0, 0, 4),
            0
        );
    }
}
