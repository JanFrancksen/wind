use cef::{CefString, ImplBrowser, ImplBrowserHost, ImplDictionaryValue, dictionary_value_create};

use std::collections::HashSet;

use crate::browser::TabId;

const ENTER_PICTURE_IN_PICTURE: &str = r#"
(async () => {
  if (!document.pictureInPictureEnabled || document.pictureInPictureElement) return false;

  const key = "__windFloatingVideoState";
  const state = globalThis[key] ??= {
    bound: new WeakSet(),
    dismissed: new WeakSet(),
    pendingDismissal: new WeakMap(),
    lastVideo: null,
    returning: null,
  };
  const videos = Array.from(document.querySelectorAll("video"));

  for (const video of videos) {
    if (state.bound.has(video)) continue;
    state.bound.add(video);
    video.addEventListener("play", () => {
      const pending = state.pendingDismissal.get(video);
      if (pending !== undefined) clearTimeout(pending);
      state.pendingDismissal.delete(video);
      state.dismissed.delete(video);
    });
    video.addEventListener("leavepictureinpicture", () => {
      if (state.returning === video) {
        state.returning = null;
      } else {
        const pending = setTimeout(() => {
          state.pendingDismissal.delete(video);
          state.dismissed.add(video);
        }, 750);
        state.pendingDismissal.set(video, pending);
      }
    });
  }

  const candidates = videos
    .filter((video) =>
      !video.paused &&
      !video.ended &&
      video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA &&
      video.videoWidth > 0 &&
      video.videoHeight > 0 &&
      !video.disablePictureInPicture &&
      !state.dismissed.has(video))
    .map((video) => ({
      video,
      area: video.getBoundingClientRect().width * video.getBoundingClientRect().height,
      audible: !video.muted && video.volume > 0,
    }))
    .sort((left, right) =>
      Number(right.audible) - Number(left.audible) || right.area - left.area);

  const selected = candidates[0]?.video;
  if (!selected) return false;

  state.lastVideo = selected;
  await selected.requestPictureInPicture();
  return true;
})()
"#;

const EXIT_PICTURE_IN_PICTURE: &str = r#"
(async () => {
  const video = document.pictureInPictureElement;
  if (!video) return false;

  const state = globalThis.__windFloatingVideoState;
  if (state) state.returning = video;
  try {
    await document.exitPictureInPicture();
    return true;
  } catch (error) {
    if (state?.returning === video) state.returning = null;
    throw error;
  }
})()
"#;

const CONFIRM_RETURN_TO_TAB: &str = r#"
(() => {
  const state = globalThis.__windFloatingVideoState;
  const video = state?.lastVideo;
  if (!video) return false;

  const pending = state.pendingDismissal.get(video);
  if (pending === undefined) return false;

  clearTimeout(pending);
  state.pendingDismissal.delete(video);
  return true;
})()
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FloatingVideoCommand {
    Exit(TabId),
    Enter(TabId),
    ConfirmReturn(TabId),
}

impl FloatingVideoCommand {
    pub(super) fn tab_id(self) -> TabId {
        match self {
            Self::Exit(tab_id) | Self::Enter(tab_id) | Self::ConfirmReturn(tab_id) => tab_id,
        }
    }

    pub(super) fn owner_after_success(self, current: Option<TabId>) -> Option<TabId> {
        match self {
            Self::Exit(_) => None,
            Self::Enter(tab_id) => Some(tab_id),
            Self::ConfirmReturn(_) => current,
        }
    }
}

pub(super) fn commands_for_presentation_change(
    previous_focus: Option<TabId>,
    next_visible: &HashSet<TabId>,
    floating_owner: Option<TabId>,
) -> Vec<FloatingVideoCommand> {
    let leaving_focus = previous_focus.filter(|tab_id| !next_visible.contains(tab_id));
    let returning_owner =
        floating_owner.filter(|owner| leaving_focus.is_some() || next_visible.contains(owner));

    returning_owner
        .map(FloatingVideoCommand::Exit)
        .into_iter()
        .chain(leaving_focus.map(FloatingVideoCommand::Enter))
        .collect()
}

pub(super) fn execute(command: FloatingVideoCommand, browser: &cef::Browser) -> bool {
    match command {
        FloatingVideoCommand::Exit(_) => evaluate(browser, EXIT_PICTURE_IN_PICTURE, false),
        FloatingVideoCommand::Enter(_) => evaluate(browser, ENTER_PICTURE_IN_PICTURE, true),
        FloatingVideoCommand::ConfirmReturn(_) => evaluate(browser, CONFIRM_RETURN_TO_TAB, false),
    }
}

fn evaluate(browser: &cef::Browser, expression: &str, user_gesture: bool) -> bool {
    let Some(host) = browser.host() else {
        return false;
    };
    let Some(mut params) = dictionary_value_create() else {
        return false;
    };

    params.set_string(
        Some(&CefString::from("expression")),
        Some(&CefString::from(expression)),
    );
    params.set_bool(Some(&CefString::from("awaitPromise")), 1);
    params.set_bool(
        Some(&CefString::from("userGesture")),
        i32::from(user_gesture),
    );

    host.execute_dev_tools_method(
        0,
        Some(&CefString::from("Runtime.evaluate")),
        Some(&mut params),
    ) != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::BrowserState;

    #[test]
    fn selecting_another_tab_returns_its_video_then_floats_the_tab_being_left() {
        let mut browser = BrowserState::with_initial_url("youtube.com");
        let source = browser.active_page().tab_id;
        browser.add_tab("theverge.com");
        let destination = browser.active_page().tab_id;

        assert_eq!(
            commands_for_presentation_change(
                Some(source),
                &HashSet::from([destination]),
                Some(destination)
            ),
            vec![
                FloatingVideoCommand::Exit(destination),
                FloatingVideoCommand::Enter(source),
            ]
        );
    }

    #[test]
    fn replacing_a_floating_video_explicitly_returns_the_previous_owner_first() {
        let mut browser = BrowserState::with_initial_url("youtube.com");
        let first_source = browser.active_page().tab_id;
        browser.add_tab("example.com");
        let current = browser.active_page().tab_id;
        browser.add_tab("theverge.com");
        let destination = browser.active_page().tab_id;

        assert_eq!(
            commands_for_presentation_change(
                Some(current),
                &HashSet::from([destination]),
                Some(first_source)
            ),
            vec![
                FloatingVideoCommand::Exit(first_source),
                FloatingVideoCommand::Enter(current),
            ]
        );
    }

    #[test]
    fn moving_focus_inside_a_split_does_not_float_the_other_visible_pane() {
        let mut browser = BrowserState::with_initial_url("youtube.com");
        let first = browser.active_page().tab_id;
        let second = browser.add_tab("example.com");

        assert!(
            commands_for_presentation_change(Some(first), &HashSet::from([first, second]), None,)
                .is_empty()
        );
    }
}
