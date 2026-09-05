// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Bridge Keymap chord resolution to InteractionScene action availability.
use crate::{
    input::{KeyEvent, KeyEventKind},
    interaction::{InteractionOutcome, InteractionScene},
    keymap::{KeyChord, Keymap},
};

/// Resolve a key through `map`, then dispatch only if the scene advertises it.
///
/// Release events are ignored. Availability is the same set used by
/// [`InteractionScene::available_actions`] for hints and palettes.
pub fn dispatch_keymap_action<Id, LayerId, Action>(
    scene: &InteractionScene<Id, LayerId, Action>,
    map: &Keymap<Action>,
    key: KeyEvent,
) -> InteractionOutcome<Id, LayerId, Action>
where
    Id: Clone + PartialEq,
    LayerId: PartialEq,
    Action: Clone + Copy + PartialEq + 'static,
{
    if key.is_release() {
        return InteractionOutcome::Ignored;
    }
    let chord = KeyChord::from(key);
    let Some(action) = map.dispatch(chord) else {
        return InteractionOutcome::Ignored;
    };
    if !scene.action_available(&action) {
        return InteractionOutcome::Ignored;
    }
    scene.dispatch_action(action)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        input::{KeyCode, KeyModifiers},
        interaction::{
            InteractionElement, InteractionLayer, InteractionScene, LayerDismissPolicy, LayerKind,
        },
        keymap::{KeyBinding, Visibility},
    };
    use ratatui_core::layout::Rect;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Act {
        Confirm,
        Cancel,
    }

    const ENTER: &[KeyChord] = &[KeyChord::plain(KeyCode::Enter)];
    const ESC: &[KeyChord] = &[KeyChord::plain(KeyCode::Esc)];
    const BINDINGS: &[KeyBinding<Act>] = &[
        KeyBinding::borrowed(
            ENTER,
            Act::Confirm,
            Some("confirm"),
            Visibility::Shown,
            None,
        ),
        KeyBinding::borrowed(ESC, Act::Cancel, Some("cancel"), Visibility::Shown, None),
    ];

    #[test]
    fn disabled_action_disappears_from_dispatch() {
        let mut scene = InteractionScene::<&str, u8, Act>::new();
        scene.ensure_root(InteractionLayer {
            id: 0,
            kind: LayerKind::Root,
            owns_input: true,
            esc: LayerDismissPolicy::Ignore,
            outside: LayerDismissPolicy::Ignore,
            focus_return: None,
        });
        scene
            .register(
                InteractionElement::control("ok", 0, Rect::new(0, 0, 1, 1))
                    .actions(vec![Act::Confirm]),
            )
            .unwrap();
        scene.reconcile();
        let map = Keymap::from_static(BINDINGS);
        assert!(matches!(
            dispatch_keymap_action(
                &scene,
                &map,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            ),
            InteractionOutcome::Action {
                action: Act::Confirm,
                ..
            }
        ));
        assert_eq!(
            dispatch_keymap_action(
                &scene,
                &map,
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
            ),
            InteractionOutcome::Ignored
        );
    }
}
