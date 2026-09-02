// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

use termrock::keymap::glyph;
use termrock::{
    input::KeyCode,
    keymap::KeyChord,
    style::{ColorCapability, DesignSystem},
};

use crate::{
    PREVIEW_KEYMAP, PreviewAction, SIDEBAR_KEYMAP, SidebarAction,
    svg::render_story_to_buffer_with_system,
};
use termrock_lookbook::stories::stories;

#[test]
fn list_story_visibly_uses_the_selected_theme() {
    let story = stories()
        .into_iter()
        .find(|story| story.id == "list/selection")
        .expect("list story exists");
    // One palette ships. Visible contrast is the capability rung: truecolor
    // tokens against their ANSI-16 projection of the same junie system.
    let truecolor = render_story_to_buffer_with_system(story, &DesignSystem::junie());
    let ansi = render_story_to_buffer_with_system(
        story,
        &DesignSystem::junie().quantize(ColorCapability::Ansi16),
    );

    assert_eq!(truecolor.area, ansi.area);
    assert!(
        truecolor
            .content()
            .iter()
            .zip(ansi.content())
            .any(|(left, right)| left.symbol() == right.symbol()
                && !left.symbol().trim().is_empty()
                && (left.fg, left.bg, left.modifier) != (right.fg, right.bg, right.modifier)),
        "list cells must visibly differ between themes"
    );
}

#[test]
fn every_preset_survives_capability_ladder() {
    let story_set = stories();
    let representatives: Vec<_> = ["surface/basic", "list/selection", "button/variants"]
        .into_iter()
        .map(|id| {
            story_set
                .iter()
                .copied()
                .find(|story| story.id == id)
                .unwrap_or_else(|| panic!("representative story missing: {id}"))
        })
        .collect();
    // junie is the only shipped preset; the ladder question is per rung now.
    let presets = [DesignSystem::junie()];
    for preset in presets {
        for capability in [
            ColorCapability::Truecolor,
            ColorCapability::Indexed256,
            ColorCapability::Ansi16,
            ColorCapability::Monochrome,
        ] {
            let system = preset.clone().quantize(capability);
            for story in &representatives {
                let buffer = render_story_to_buffer_with_system(*story, &system);
                assert!(
                    buffer
                        .content()
                        .iter()
                        .any(|cell| !cell.symbol().trim().is_empty()),
                    "{} rendered empty at {capability:?}",
                    story.id
                );
            }
        }
        let no_color = preset.no_color();
        for story in &representatives {
            let buffer = render_story_to_buffer_with_system(*story, &no_color);
            assert!(
                buffer
                    .content()
                    .iter()
                    .any(|cell| !cell.symbol().trim().is_empty())
            );
        }
    }
}

// ── SIDEBAR ───────────────────────────────────────────────────────────────────

#[test]
fn sidebar_down_up_dispatch_navigate() {
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::Down)),
        Some(SidebarAction::Navigate)
    );
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::Up)),
        Some(SidebarAction::Navigate)
    );
}

#[test]
fn runtime_remap_demo_updates_sidebar_dispatch_and_hint_from_one_table() {
    let mut keymap = SIDEBAR_KEYMAP.clone();
    assert!(keymap.remap(
        SidebarAction::Quit,
        vec![KeyChord::ctrl(KeyCode::Char('c'))]
    ));

    assert_eq!(keymap.dispatch(KeyChord::plain(KeyCode::Char('q'))), None);
    assert_eq!(
        keymap.dispatch(KeyChord::ctrl(KeyCode::Char('c'))),
        Some(SidebarAction::Quit)
    );
    assert_eq!(keymap.glyph_for(SidebarAction::Quit), "Ctrl-C");
}

#[test]
fn sidebar_vim_aliases_dispatch_navigate() {
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::Char('j'))),
        Some(SidebarAction::Navigate)
    );
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::Char('k'))),
        Some(SidebarAction::Navigate)
    );
}

#[test]
fn sidebar_home_end_dispatch_go_to_edge() {
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::Home)),
        Some(SidebarAction::GoToEdge)
    );
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::End)),
        Some(SidebarAction::GoToEdge)
    );
}

#[test]
fn sidebar_tab_dispatches_focus_preview() {
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::Tab)),
        Some(SidebarAction::FocusPreview)
    );
}

#[test]
fn sidebar_q_esc_dispatch_quit() {
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::Char('q'))),
        Some(SidebarAction::Quit)
    );
    assert_eq!(
        SIDEBAR_KEYMAP.dispatch(KeyChord::plain(KeyCode::Esc)),
        Some(SidebarAction::Quit)
    );
}

#[test]
fn sidebar_non_registered_keys_return_none() {
    for chord in [
        KeyChord::plain(KeyCode::Enter),
        KeyChord::plain(KeyCode::Char('a')),
        KeyChord::plain(KeyCode::Char('Q')),
        KeyChord::ctrl(KeyCode::Char('c')),
        KeyChord::plain(KeyCode::PageUp),
        KeyChord::plain(KeyCode::PageDown),
        KeyChord::plain(KeyCode::BackTab),
    ] {
        assert_eq!(
            SIDEBAR_KEYMAP.dispatch(chord),
            None,
            "sidebar must not dispatch {chord:?}"
        );
    }
}

#[test]
fn sidebar_hints_advertise_navigate_and_quit() {
    let spans = SIDEBAR_KEYMAP.hint_spans();
    let text: String = spans
        .iter()
        .filter_map(|s| match s {
            termrock::widgets::HintSpan::Key(k) | termrock::widgets::HintSpan::Text(k) => Some(*k),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ");
    assert!(text.contains("↑↓"), "must advertise ↑↓ navigate: {text}");
    assert!(text.contains("navigate"), "must label navigate: {text}");
    assert!(text.contains("⇥"), "must advertise Tab: {text}");
    assert!(text.contains("quit"), "must advertise quit: {text}");
    assert!(text.contains("Home"), "must advertise Home: {text}");
    // HiddenAlias — j/k must NOT appear in hint bar
    assert!(
        !text.contains(" j ") && !text.contains("j/k"),
        "j/k alias must not appear in hint: {text}"
    );
}

// ── PREVIEW ───────────────────────────────────────────────────────────────────

#[test]
fn preview_esc_dispatches_back_to_list() {
    assert_eq!(
        PREVIEW_KEYMAP.dispatch(KeyChord::plain(KeyCode::Esc)),
        Some(PreviewAction::BackToList)
    );
}

#[test]
fn preview_component_keys_are_not_stolen_by_shell_keymap() {
    for chord in [
        KeyChord::plain(KeyCode::Up),
        KeyChord::plain(KeyCode::Down),
        KeyChord::plain(KeyCode::Left),
        KeyChord::plain(KeyCode::Right),
        KeyChord::plain(KeyCode::Tab),
        KeyChord::plain(KeyCode::BackTab),
    ] {
        assert_eq!(
            PREVIEW_KEYMAP.dispatch(chord),
            None,
            "preview shell must leave {chord:?} to mounted demo"
        );
    }
}

#[test]
fn preview_page_and_shift_keys_are_not_stolen_from_demo() {
    for chord in [
        KeyChord::plain(KeyCode::PageDown),
        KeyChord::plain(KeyCode::PageUp),
        KeyChord::plain(KeyCode::Char('J')),
        KeyChord::plain(KeyCode::Char('K')),
    ] {
        assert_eq!(PREVIEW_KEYMAP.dispatch(chord), None);
    }
}

#[test]
fn preview_non_registered_keys_return_none() {
    for chord in [
        KeyChord::plain(KeyCode::Enter),
        KeyChord::plain(KeyCode::Char('q')),
        KeyChord::plain(KeyCode::Char('j')),
        KeyChord::plain(KeyCode::Char('k')),
        KeyChord::ctrl(KeyCode::Char('q')),
    ] {
        assert_eq!(
            PREVIEW_KEYMAP.dispatch(chord),
            None,
            "preview must not dispatch {chord:?}"
        );
    }
}

#[test]
fn preview_shell_hints_advertise_only_back() {
    let spans = PREVIEW_KEYMAP.hint_spans();
    let text: String = spans
        .iter()
        .filter_map(|s| match s {
            termrock::widgets::HintSpan::Key(k) | termrock::widgets::HintSpan::Text(k) => Some(*k),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ");
    assert!(text.contains("Esc"), "must advertise Esc back: {text}");
    assert!(text.contains("back to list"), "must label back: {text}");
    assert!(!text.contains("↑↓"), "demo owns interaction hints: {text}");
    assert!(
        !text.contains("J/K"),
        "preview scrolling must not steal component keys: {text}"
    );
    assert!(
        !text.contains(glyph::PGUP_PGDN),
        "preview paging must not steal component keys: {text}"
    );
    // BackTab is HiddenAlias — must NOT appear in hint bar separately
    assert!(
        !text.contains("BackTab"),
        "BackTab alias must not appear in hint: {text}"
    );
}

#[test]
fn no_dual_agent_chrome_stories() {
    for story in stories() {
        assert!(
            !story.id.starts_with("approval-card/")
                && !story.id.starts_with("prompt-box/")
                && !story.id.starts_with("stream-view/"),
            "deleted dual story still registered: {}",
            story.id
        );
        assert!(
            story.component() != "ApprovalCard"
                && story.component() != "PromptBox"
                && story.component() != "StreamView",
            "deleted dual component label: {} ({})",
            story.component(),
            story.id
        );
    }
}

fn production_source(s: &str) -> String {
    let head = s.split("#[cfg(test)]").next().unwrap_or(s);
    head.lines()
        .filter(|l| {
            let t = l.trim_start();
            !t.starts_with("//") && !t.starts_with("//!")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn no_modal_stack_in_lookbook_sources() {
    let app_p = production_source(include_str!("app.rs"));
    let host_p = production_source(include_str!("host_frame.rs"));
    assert!(!app_p.contains("ModalStack"));
    assert!(
        !host_p.contains("ModalStack"),
        "host_frame must not import ModalStack"
    );
    assert!(
        app_p.contains("HostFrame") && host_p.contains("InteractionScene"),
        "lookbook chrome must use HostFrame/InteractionScene; demo overlays stay in DemoSession"
    );
}

#[test]
fn no_focus_ring_fork_in_lookbook() {
    let app = production_source(include_str!("app.rs"));
    let focus = production_source(include_str!("focus.rs"));
    let host = production_source(include_str!("host_frame.rs"));
    assert!(
        !app.contains("FocusRing") && !focus.contains("FocusRing") && !host.contains("FocusRing")
    );
    assert!(
        !std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/host_focus.rs")
            .exists(),
        "host_focus.rs must be deleted"
    );
    assert!(host.contains("InteractionScene"));
}
