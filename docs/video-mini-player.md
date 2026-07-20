# Floating video player exploration

## Goal

When a user leaves a tab with a playing video, keep that video visible in a
small floating player with play/pause, return-to-tab, and dismiss controls. The
first version should work for ordinary HTML `<video>` players, including
YouTube, without a site-specific YouTube integration.

## Recommendation

Wind uses **CEF's native video Picture-in-Picture window** with the current
windowed renderer. It already owns video presentation and supplies the basic
controls, so it does not require copying decoded frames or mutating a site's
player DOM.

Treat an Arc-style player embedded inside Wind's content area as a second step.
That exact presentation requires either hosting the PiP popup ourselves or
compositing video frames into an egui overlay; both are materially larger than
letting Chromium own the PiP window.

## Why this fits the current code

- `CefRenderer` already keeps one `CefTab` per visited open tab. Switching tabs
  hides the old native child window instead of destroying its browser, so media
  playback can continue (`src/renderer/cef.rs`, `sync_visibility`).
- CEF is currently embedded as native child windows. Egui paints behind those
  windows, so an egui card alone cannot cover the visible webview.
- CEF supports Picture-in-Picture and exposes explicit BrowserView delegate
  hooks for frameless PiP windows, movement, and opening Document PiP without
  user activation. Wind does not use `CefBrowserView` today, so those
  customization hooks are not immediately available.
- CEF's off-screen rendering path would expose pixel buffers through
  `CefRenderHandler::OnPaint`, but it also makes Wind responsible for browser
  input, IME, focus, popup, scaling, and repaint integration. That is too large
  a prerequisite for this feature alone.

CEF references:

- [BrowserView Picture-in-Picture hooks](https://cef-builds.spotifycdn.com/docs/149.0/classCefBrowserViewDelegate.html)
- [CEF off-screen rendering responsibilities](https://chromiumembedded.github.io/cef/general_usage.html#off-screen-rendering)
- [Browser host windowless APIs](https://cef-builds.spotifycdn.com/docs/150.0/classCefBrowserHost.html)

## Proposed behavior

The first implementation uses this policy:

1. At most one floating player is visible.
2. When the user selects another tab, it floats the best playing video from the
   tab being left.
3. Returning to the source tab hides the floating player; playback continues in
   the page.
4. Native play/pause controls operate on the page's existing video.
5. Dismiss hides the player but does not pause the page. That video remains
   dismissed until its playback stops and starts again.
6. Closing or discarding the source tab closes the floating player.

## Implementation

### 1. Select playable media

On tab selection, Wind evaluates a small script in the tab being left. It
selects the largest audible playing `<video>`, falling back to the largest muted
video, and asks Chromium to present it in native PiP. The script keeps only
page-local weak references for dismissal state; no playback state is persisted.

### 2. Enter native PiP

Wind calls CEF's DevTools `Runtime.evaluate` with `userGesture: true`. That lets
the browser shell invoke `requestPictureInPicture()` without relying on a stale
transient activation from the page. Native PiP was validated against a live
YouTube video in the bundled macOS app.

### 3. Follow Wind tab selection

Every Active Tab change first exits the last requested PiP owner, then asks the
previous tab to float a playing video. This also runs for Wind's native new-tab
page and unsupported internal URLs, which bypass normal CEF page rendering.

CEF's system focus-request callback maps the native PiP “return to tab” action
back to a normal Wind `TabAction::Select`, preserving the browser's tab model
and address state. Navigation-origin focus requests are ignored so background
pages cannot select themselves.
Wind tracks the last requested PiP owner so replacing one source explicitly
exits it before opening another. A short deferred-dismissal window lets the
native return action confirm its source focus without being mistaken for the
player's close action.

### 4. Remaining polish

- Add a preference: automatic floating video on/off.
- Surface a small media badge on the source tab.
- Persist only the preference and last player size/position, never playback
  state.

## If native PiP cannot match the desired presentation

| Approach | Fidelity to the reference | Cost | Main risk |
| --- | --- | --- | --- |
| Chromium native video PiP | Good controls; separate floating window | Low | Automatic entry and window customization |
| CEF BrowserView + Document PiP | High; Wind can own popup content | Medium/high | Requires changing the current embedding seam |
| Reuse the source native child window as an in-app overlay | High visually | Medium | Requires invasive DOM isolation and can break complex players |
| Off-screen rendering and crop/composite frames | Highest | Very high | Renderer-wide input, performance, GPU, and protected-video work |

The source-child-window option is tempting because Wind already resizes and
hides CEF children. It is not the default recommendation: the child renders the
entire page, not a video surface. Making only the video fill that child means
injecting CSS/DOM changes into arbitrary sites, then restoring them without
breaking the site's framework.

## Acceptance criteria for the first real slice

- Start a YouTube video, select another Wind tab, and see a floating player
  without restarting or duplicating playback.
- Play/pause, close, and return-to-source work.
- Returning to the source tab removes the duplicate floating presentation.
- No player appears for paused, ended, audio-only, or never-played media.
- Dismissal is respected until that playback session stops.
- Closing the source tab or deleting its Space cannot leave a native window or
  CEF callback alive.
- Existing tab/session isolation remains unchanged.

## Implemented policy

The active source is the most recently left tab with a suitable playing video.
Closing native PiP dismisses that video until playback starts again. Returning
to the source tab exits PiP without marking the video dismissed.
