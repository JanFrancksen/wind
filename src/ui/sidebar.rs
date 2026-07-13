use eframe::egui;

use crate::{
    browser::{BrowserState, Tab, TabGroup, TabId},
    ds::{
        components::{DsButton, Icon, TabButton, divider},
        theming::Theme,
    },
};

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

#[derive(Default)]
struct SidebarActions {
    selected: Option<TabId>,
    closed: Option<TabId>,
    toggled_pin: Option<TabId>,
    promoted: Option<TabId>,
    moved_up: Option<TabId>,
    moved_down: Option<TabId>,
    returned: Option<TabId>,
    demoted: Option<TabId>,
}

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

    let dragging = egui::DragAndDrop::payload::<DraggedTab>(ui.ctx()).map(|payload| *payload);
    let mut drop_target = None;
    let mut actions = SidebarActions::default();

    ui.add_space(space.lg);
    highlighted_pinned_tabs(ui, browser, theme, dragging, &mut drop_target, &mut actions);

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
    tab_sections(ui, browser, theme, dragging, &mut drop_target, &mut actions);

    apply_sidebar_actions(browser, address_input, actions);

    if let Some(dragged) = egui::DragAndDrop::payload::<DraggedTab>(ui.ctx()).map(|item| *item) {
        if ui.input(|input| input.pointer.any_released()) {
            if let Some(target) = drop_target {
                let _ = egui::DragAndDrop::take_payload::<DraggedTab>(ui.ctx());
                browser.place_tab(dragged.id, target.group, target.index);
            }
        } else {
            floating_drag_preview(ui, browser, dragged, drop_target, theme);
            ui.ctx().request_repaint();
        }
    }

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
    browser: &BrowserState,
    theme: &Theme,
    dragging: Option<DraggedTab>,
    drop_target: &mut Option<DropTarget>,
    actions: &mut SidebarActions,
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

        let rect = animated_tab_rect(ui, tab.id, target_rect, theme);
        let mut tile_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        let is_active = browser.tabs()[browser.active_index()].id == tab.id;
        let (response, return_clicked, close_clicked) =
            highlighted_pin_tile(&mut tile_ui, tab, is_active, rect.width(), theme);
        response.dnd_set_drag_payload(DraggedTab { id: tab.id });

        if close_clicked {
            actions.closed = Some(tab.id);
        } else if return_clicked {
            actions.returned = Some(tab.id);
        } else if response.clicked() {
            actions.selected = Some(tab.id);
        }

        response.context_menu(|ui| tab_context_menu(ui, tab, actions));
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
    let return_clicked = if tab_is_away_from_pin(tab) {
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
    let close_clicked = if ui.rect_contains_pointer(rect) {
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
        icon_response.clicked()
    } else {
        false
    };
    (response, return_clicked, close_clicked)
}

fn tab_is_away_from_pin(tab: &Tab) -> bool {
    tab.pinned_url
        .as_deref()
        .is_some_and(|pinned_url| pinned_url != tab.url)
}

fn tab_context_menu(ui: &mut egui::Ui, tab: &Tab, actions: &mut SidebarActions) {
    if tab_is_away_from_pin(tab) && ui.button("Return to pinned tab").clicked() {
        actions.returned = Some(tab.id);
        ui.close();
    }

    match tab.group() {
        TabGroup::Highlight => {
            if ui.button("Demote from highlight").clicked() {
                actions.demoted = Some(tab.id);
                ui.close();
            }
        }
        TabGroup::Pinned => {
            if ui.button("Promote to highlight").clicked() {
                actions.promoted = Some(tab.id);
                ui.close();
            }
        }
        TabGroup::Today => {}
    }

    let pin_label = if tab.pinned { "Unpin tab" } else { "Pin tab" };
    if ui.button(pin_label).clicked() {
        actions.toggled_pin = Some(tab.id);
        ui.close();
    }

    ui.separator();
    if ui.button("Move up").clicked() {
        actions.moved_up = Some(tab.id);
        ui.close();
    }
    if ui.button("Move down").clicked() {
        actions.moved_down = Some(tab.id);
        ui.close();
    }

    ui.separator();
    if ui.button("Close tab").clicked() {
        actions.closed = Some(tab.id);
        ui.close();
    }
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
    actions: &mut SidebarActions,
) {
    let space = &theme.tokens.primitive.space;
    let color = &theme.tokens.semantic.color;
    let has_pinned_tabs = browser.tabs().iter().any(|tab| tab.pinned);

    tab_row_group(
        ui,
        browser,
        TabGroup::Pinned,
        "Pinned",
        theme,
        dragging,
        drop_target,
        actions,
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
    );

    ui.add_space(space.sm);
    ui.label(
        egui::RichText::new(format!("{} open", browser.tabs().len()))
            .color(color.text_muted)
            .size(theme.tokens.primitive.typography.caption),
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
    actions: &mut SidebarActions,
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
        let rect = animated_tab_rect(ui, tab.id, target_rect, theme);
        paint_tab_row_at(
            ui,
            rect,
            tab,
            browser.active_tab().id == tab.id,
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
    theme: &Theme,
    actions: &mut SidebarActions,
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
        let is_away_from_pin = tab_is_away_from_pin(tab);
        let action_count = if is_away_from_pin { 5.0 } else { 4.0 };
        let actions_width = (action_size * action_count) + (spacing * action_count);
        let tab_width = (row_rect.width() - actions_width).max(0.0);
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
        tab_response.dnd_set_drag_payload(DraggedTab { id: tab.id });
        if tab_response.clicked() {
            actions.selected = Some(tab.id);
        }

        tab_response.context_menu(|ui| tab_context_menu(ui, tab, actions));

        if is_away_from_pin
            && DsButton::icon(Icon::ArrowLeft)
                .ghost()
                .small()
                .width(action_size)
                .show(&mut row_ui, theme)
                .on_hover_text("Return to pinned tab")
                .clicked()
        {
            actions.returned = Some(tab.id);
        }
        if DsButton::icon(Icon::Pin)
            .ghost()
            .small()
            .selected(tab.pinned)
            .width(action_size)
            .show(&mut row_ui, theme)
            .on_hover_text(if tab.pinned { "Unpin tab" } else { "Pin tab" })
            .clicked()
        {
            actions.toggled_pin = Some(tab.id);
        }
        if DsButton::icon(Icon::ChevronUp)
            .ghost()
            .small()
            .width(action_size)
            .show(&mut row_ui, theme)
            .on_hover_text("Move tab up")
            .clicked()
        {
            actions.moved_up = Some(tab.id);
        }
        if DsButton::icon(Icon::ChevronDown)
            .ghost()
            .small()
            .width(action_size)
            .show(&mut row_ui, theme)
            .on_hover_text("Move tab down")
            .clicked()
        {
            actions.moved_down = Some(tab.id);
        }
        if DsButton::icon(Icon::X)
            .danger()
            .small()
            .width(action_size)
            .show(&mut row_ui, theme)
            .on_hover_text("Close tab")
            .clicked()
        {
            actions.closed = Some(tab.id);
        }
    });
}

fn apply_sidebar_actions(
    browser: &mut BrowserState,
    address_input: &mut String,
    actions: SidebarActions,
) {
    let mut changed_selection = false;
    let select = |browser: &mut BrowserState, tab_id: TabId| {
        let Some(index) = browser.tabs().iter().position(|tab| tab.id == tab_id) else {
            return false;
        };
        browser.select_tab(index);
        true
    };

    if let Some(tab_id) = actions.toggled_pin
        && select(browser, tab_id)
    {
        browser.toggle_pin_active_tab();
        changed_selection = true;
    }
    if let Some(tab_id) = actions.returned
        && select(browser, tab_id)
    {
        browser.return_active_to_pinned_url();
        changed_selection = true;
    }
    if let Some(tab_id) = actions.promoted
        && select(browser, tab_id)
    {
        browser.promote_active_pinned_tab();
        changed_selection = true;
    }
    if let Some(tab_id) = actions.demoted
        && select(browser, tab_id)
    {
        browser.demote_active_highlighted_tab();
        changed_selection = true;
    }
    if let Some(tab_id) = actions.moved_up
        && select(browser, tab_id)
    {
        browser.move_active_tab_up();
        changed_selection = true;
    }
    if let Some(tab_id) = actions.moved_down
        && select(browser, tab_id)
    {
        browser.move_active_tab_down();
        changed_selection = true;
    }
    if let Some(tab_id) = actions.selected {
        changed_selection |= select(browser, tab_id);
    }
    if let Some(tab_id) = actions.closed
        && let Some(index) = browser.tabs().iter().position(|tab| tab.id == tab_id)
    {
        browser.close_tab(index);
        changed_selection = true;
    }

    if changed_selection {
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

    use crate::browser::Favicon;

    use super::{grid_insertion_index, normalized_favicon_image, row_insertion_index};

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
