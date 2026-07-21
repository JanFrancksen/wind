# Wind Browser Feature Backlog

This file tracks the next browser features after the design-system and shell-layout work.

## Address and Command Behavior

- [x] Make the address bar handle URLs, search terms, and browser commands from one input.
- [x] Add command palette actions for new tab, close tab, duplicate tab, reopen closed tab, pin tab, reorder tab, back, forward, and reload.
- [x] Support empty input as a new-tab page state instead of navigating to a placeholder URL.
- [x] Add keyboard shortcuts for new tab, close tab, reopen closed tab, reload, back, and forward.
- [x] Add command palette actions for switching spaces once real spaces exist.

## Tab Model

- [x] Add pinned tabs.
- [x] Add tab reordering.
- [x] Add duplicate tab.
- [x] Add close active tab.
- [x] Add recently closed tab restore.
- [ ] Add tab loading state, favicon, and page title updates from the renderer.

## Spaces and Sidebar

- [x] Add real Arc-style spaces/workspaces with isolated login sessions.
- [x] Persist active space and per-space tabs.
- Add sidebar sections for pinned tabs, today tabs, and archived/recent tabs.
- Add sidebar collapse and compact mode.
- Add hover, focus, and active states for tab rows.

## Browser Surface

- [x] Add persistent, resizable side-by-side Split Views with pane focus and close controls.
- Build a proper new-tab page.
- Add loading and error states.
- Add a web renderer boundary that can be backed by Wry, CEF, or another renderer.
- Connect navigation state to the renderer once a renderer is selected.

## Visual Polish

- Replace text glyph controls with a real icon strategy.
- Tighten sidebar spacing and typography.
- Add focused address-bar styling.
- Improve empty states and webview placeholder states.
