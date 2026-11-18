//! The foundation layers own their types (#441), and every old path still
//! names the same type.

use junie_tui::core::hints::{HasHints, Hint, HintCx, HintPriority};
use junie_tui::core::selection::TriState;
use junie_tui::theme::{CardHeading, Glyphs, HintBarStyle, Theme, VisualState};

struct Source;

impl HasHints for Source {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        vec![
            Hint::new("Enter", "Open"),
            Hint::new("Esc", "Close").priority(HintPriority::Normal),
        ]
    }
}

#[test]
fn core_hints_build_a_hint_list_without_the_widgets() {
    let hints = Source.hints(&HintCx::default());
    assert_eq!(hints.len(), 2);
    assert_eq!(junie_tui::core::hints::normalize_key("Del"), "Delete");
    assert_eq!(junie_tui::core::hints::range(1, 9), "1–9");
}

#[test]
fn hint_pinning_reads_the_hint_bar_style() {
    let style = HintBarStyle::DEFAULT;
    assert!(Hint::new("Esc", "Close").is_pinned(&style));
    assert!(!Hint::new("Enter", "Open").is_pinned(&style));
    assert!(
        !Hint::new("Esc", "Close")
            .priority(HintPriority::Normal)
            .is_pinned(&style)
    );
    assert!(Hint::new("Enter", "Open").pinned().is_pinned(&style));
}

#[test]
fn old_paths_name_the_moved_types() {
    let hint: junie_tui::widgets::keyhint::Hint = Hint::new("a", "b");
    let _: junie_tui::core::hints::Hint = hint;
    let _: junie_tui::widgets::keyhint::HintPriority = HintPriority::Pinned;

    let state: junie_tui::ui::ctx::VisualState = VisualState::default();
    let _: junie_tui::theme::VisualState = state;

    let heading: junie_tui::widgets::panel::CardHeading = CardHeading::Band;
    let _: junie_tui::theme::CardHeading = heading;
    assert_eq!(Theme::junie().card_heading, CardHeading::Rule);
    assert_eq!(
        junie_tui::widgets::panel::CardHeading::ALL,
        CardHeading::ALL
    );
}

#[test]
fn tri_state_glyphs_follow_the_glyph_set() {
    assert_eq!(TriState::All.glyph(), Glyphs::UNICODE.check_on);
    assert_eq!(TriState::Mixed.glyph(), Glyphs::UNICODE.check_mixed);
    assert_eq!(TriState::None.glyph(), Glyphs::UNICODE.check_off);
    assert_eq!(
        TriState::All.glyph_in(&Glyphs::ASCII),
        Glyphs::ASCII.check_on
    );
    assert_eq!(
        TriState::Mixed.glyph_in(&Glyphs::ASCII),
        Glyphs::ASCII.check_mixed
    );
    assert_eq!(
        TriState::None.glyph_in(&Glyphs::ASCII),
        Glyphs::ASCII.check_off
    );
}
