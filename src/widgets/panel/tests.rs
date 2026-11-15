use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;

use super::*;
use crate::core::focus::FocusRing;
use crate::core::hit::HitRegistry;
use crate::core::intent::Phase;
use crate::core::pointer::{MouseTarget, PointerEvent};
use crate::theme::{ColorLevel, Theme};
use crate::ui::ctx::Interaction;

const ID: WidgetId = WidgetId::of("t.scroll-panel");

struct Frame {
    buf: Buffer,
    hits: HitRegistry,
}

impl Frame {
    fn text(&self) -> String {
        (0..self.buf.area.height)
            .map(|y| {
                (0..self.buf.area.width)
                    .map(|x| self.buf[(x, y)].symbol().to_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn pointer(&self, panel: &mut ScrollPanel, phase: Phase, pos: Position) {
        let ev = PointerEvent::new(&self.hits, phase, ID, pos, 1, KeyModifiers::NONE);
        drop(panel.pointer(&ev));
    }
}

fn paint(panel: &mut ScrollPanel, width: u16, height: u16) -> Frame {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    panel.render(area, &mut buf, &mut ctx, Color::Reset, |t, _| t.secondary());
    Frame { buf, hits }
}

fn words(word: &str) -> String {
    format!("{word} {word} {word} {word} {word} {word}")
}

fn numbered(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("row {i:02}")).collect()
}

fn rows_of(panel: &ScrollPanel) -> Vec<String> {
    panel.rows().to_vec()
}

#[test]
fn issue39_same_width_replace_updates_wrapping() {
    let mut panel = ScrollPanel::new(ID, vec![words("old")]).wrap(true);
    let before = paint(&mut panel, 14, 6);
    assert!(before.text().contains("old old"), "{}", before.text());
    let old_rows = rows_of(&panel).len();

    assert!(panel.set_lines(vec![words("new")]));
    let after = paint(&mut panel, 14, 6);
    assert!(after.text().contains("new new"), "{}", after.text());
    assert!(!after.text().contains("old"), "{}", after.text());
    assert_eq!(rows_of(&panel).len(), old_rows);
    panel.select_all();
    assert_eq!(panel.selected_text().expect("selection"), words("new"));

    assert_eq!(panel.replace_line(0, words("rep")), Ok(true));
    let after = paint(&mut panel, 14, 6);
    assert!(after.text().contains("rep rep"), "{}", after.text());
    assert!(!after.text().contains("new"), "{}", after.text());

    assert!(panel.extend_lines(vec!["tail".to_owned()]));
    let after = paint(&mut panel, 14, 6);
    assert!(after.text().contains("tail"), "{}", after.text());
    assert!(after.text().contains("rep rep"), "{}", after.text());

    assert_eq!(panel.lines(), [words("rep"), "tail".to_owned()]);
}

#[test]
fn issue39_replace_line_rejects_a_missing_index() {
    let mut panel = ScrollPanel::new(ID, numbered(2));
    let revision = panel.content_revision;
    assert_eq!(panel.replace_line(2, "x".into()), Err(2));
    assert_eq!(panel.replace_line(usize::MAX, "x".into()), Err(usize::MAX));
    assert_eq!(panel.lines(), numbered(2));
    assert_eq!(panel.content_revision, revision);
}

#[test]
fn issue39_clear_and_zero_width_rebuild() {
    let mut panel = ScrollPanel::new(ID, vec![words("old")]).wrap(true);
    paint(&mut panel, 14, 6);
    assert!(panel.clear());
    let cleared = paint(&mut panel, 14, 6);
    assert!(!cleared.text().contains("old"), "{}", cleared.text());
    assert!(rows_of(&panel).is_empty());
    assert!(!panel.clear(), "clearing nothing is a no-op");

    panel.set_lines(vec![words("new")]);
    let builds = panel.work.wrap_builds.get();
    for width in [0u16, 1, 2, 0, 1, 2] {
        panel.measure(Rect::new(0, 0, width, 4));
    }
    assert_eq!(
        panel.work.wrap_builds.get() - builds,
        1,
        "widths 0..=2 share one zero-width projection and cache it"
    );
    assert!(panel.wrapped_cache.is_some());
    assert_eq!(panel.wrapped_cache.as_ref().map(|c| c.width), Some(0));

    let normal = paint(&mut panel, 14, 6);
    assert!(normal.text().contains("new new"), "{}", normal.text());
    assert!(!normal.text().contains("old"), "{}", normal.text());
    assert_eq!(panel.work.wrap_builds.get() - builds, 2);
}

#[test]
fn issue39_mutation_noop_preserves_selection() {
    let lines = vec![words("alpha"), words("beta")];
    let mut panel = ScrollPanel::new(ID, lines.clone()).wrap(true);
    let frame = paint(&mut panel, 14, 3);
    frame.pointer(&mut panel, Phase::Press, Position::new(1, 0));
    frame.pointer(&mut panel, Phase::Drag, Position::new(6, 1));
    panel.scroll.scroll_by(1);
    let selection = panel.selection().expect("dragged selection");
    let (revision, offset) = (panel.content_revision, panel.scroll.offset);
    let press = panel.select.press;
    assert!(press.is_some());
    paint(&mut panel, 14, 3);
    let builds = panel.work.wrap_builds.get();

    assert!(!panel.set_lines(lines.clone()));
    assert!(!panel.extend_lines(Vec::new()));
    assert_eq!(panel.replace_line(1, lines[1].clone()), Ok(false));

    assert_eq!(panel.content_revision, revision);
    assert_eq!(panel.selection(), Some(selection));
    assert_eq!(panel.select.press, press);
    assert_eq!(panel.scroll.offset, offset);
    paint(&mut panel, 14, 3);
    assert_eq!(panel.work.wrap_builds.get(), builds);
}

#[test]
fn issue39_mutation_clears_selection_and_drag() {
    let mut panel = ScrollPanel::new(ID, vec![words("alpha"), words("beta")]).wrap(true);
    let frame = paint(&mut panel, 14, 3);
    frame.pointer(&mut panel, Phase::Press, Position::new(1, 0));
    frame.pointer(&mut panel, Phase::Drag, Position::new(6, 1));
    assert!(panel.selection().is_some() && panel.select.press.is_some());

    assert_eq!(panel.replace_line(0, words("gamma")), Ok(true));
    assert!(panel.selection().is_none());
    assert!(panel.select.press.is_none());
    assert!(panel.selected_text().is_none());
}

#[test]
fn issue39_append_keeps_selection_and_drag() {
    for wrap in [true, false] {
        for extend in [false, true] {
            let mut panel = ScrollPanel::new(ID, vec![words("alpha"), words("beta")]).wrap(wrap);
            let frame = paint(&mut panel, 14, 4);
            frame.pointer(&mut panel, Phase::Press, Position::new(1, 0));
            frame.pointer(&mut panel, Phase::Drag, Position::new(6, 1));
            let selection = panel.selection().expect("dragged selection");
            let text = panel.selected_text().expect("selected text");
            assert!(panel.select.press.is_some());

            let label = format!("wrap={wrap} extend={extend}");
            if extend {
                assert!(
                    panel.extend_lines(vec![words("gamma"), words("delta")]),
                    "{label}"
                );
            } else {
                panel.push(words("gamma"));
            }
            let frame = paint(&mut panel, 14, 4);

            assert_eq!(panel.selection(), Some(selection), "{label}");
            assert_eq!(panel.selected_text(), Some(text), "{label}");
            assert!(panel.select.press.is_some(), "{label}");
            frame.pointer(&mut panel, Phase::Drag, Position::new(8, 1));
            assert_ne!(
                panel.selection(),
                Some(selection),
                "{label}: drag continues"
            );
        }
    }
}

#[test]
fn issue39_replacement_still_clears_selection_in_unwrapped_panels() {
    let mut panel = ScrollPanel::new(ID, vec![words("alpha"), words("beta")]);
    let frame = paint(&mut panel, 14, 4);
    frame.pointer(&mut panel, Phase::Press, Position::new(1, 0));
    frame.pointer(&mut panel, Phase::Drag, Position::new(6, 1));
    assert!(panel.selection().is_some());
    assert!(panel.set_lines(vec![words("gamma")]));
    assert!(panel.selection().is_none() && panel.select.press.is_none());

    panel.select_all();
    assert!(panel.clear());
    assert!(panel.selection().is_none());
}

#[test]
fn issue39_shrink_clamps_follow_and_scroll() {
    let mut tail = ScrollPanel::new(ID, numbered(30)).tail(true);
    paint(&mut tail, 20, 6);
    assert!(tail.follow);
    assert_eq!(tail.scroll.offset, 24);

    tail.set_lines(numbered(3));
    paint(&mut tail, 20, 6);
    assert_eq!(tail.scroll.offset, 0);
    assert!(
        tail.follow,
        "the explicit follow choice survives the shrink"
    );
    tail.extend_lines(numbered(20));
    paint(&mut tail, 20, 6);
    assert_eq!(tail.scroll.offset, 23 - 6);

    let mut manual = ScrollPanel::new(ID, numbered(30));
    paint(&mut manual, 20, 6);
    manual.scroll.scroll_to(18);
    manual.set_lines(numbered(8));
    paint(&mut manual, 20, 6);
    assert_eq!(manual.scroll.offset, 2);
    assert!(!manual.follow);

    let wide = "w".repeat(60);
    let mut sideways = ScrollPanel::new(ID, vec![wide, "short".into()]);
    paint(&mut sideways, 20, 6);
    sideways.scroll.h.scroll_to(30);
    assert_eq!(sideways.scroll.h.offset, 30);
    sideways.set_lines(vec!["w".repeat(24)]);
    paint(&mut sideways, 20, 6);
    assert_eq!(sideways.scroll.h.offset, 24 - 18);
    sideways.clear();
    let blank = paint(&mut sideways, 20, 6);
    assert_eq!(sideways.scroll.h.offset, 0);
    assert!(!blank.text().contains('w'), "{}", blank.text());
}

#[test]
fn issue39_unchanged_paint_does_no_content_scans() {
    let mut wrapped = ScrollPanel::new(ID, numbered(40)).wrap(true);
    paint(&mut wrapped, 20, 6);
    let builds = wrapped.work.wrap_builds.get();
    assert_eq!(builds, 1);
    for _ in 0..5 {
        paint(&mut wrapped, 20, 6);
    }
    assert_eq!(wrapped.work.wrap_builds.get(), builds);
    paint(&mut wrapped, 12, 6);
    assert_eq!(wrapped.work.wrap_builds.get(), builds + 1);

    let mut flat = ScrollPanel::new(ID, numbered(40));
    paint(&mut flat, 20, 6);
    let scans = flat.work.width_measurements.get();
    assert_eq!(scans, 40, "the first sideways measure scans each line once");
    for width in [20, 12, 30, 20] {
        paint(&mut flat, width, 6);
    }
    assert_eq!(
        flat.work.width_measurements.get(),
        scans,
        "width-only reflows keep the widest-line summary"
    );
}

#[test]
fn issue39_append_measures_only_new_lines() {
    let mut flat = ScrollPanel::new(ID, numbered(40));
    paint(&mut flat, 20, 6);
    let scans = flat.work.width_measurements.get();

    flat.push("a much longer line than the others".into());
    paint(&mut flat, 20, 6);
    assert_eq!(flat.work.width_measurements.get(), scans + 1);
    assert_eq!(flat.scroll.h.content_len, 34);

    flat.extend_lines(vec!["x".into(), "yy".into()]);
    paint(&mut flat, 20, 6);
    assert_eq!(flat.work.width_measurements.get(), scans + 3);
    assert_eq!(flat.scroll.h.content_len, 34);

    flat.replace_line(40, "short".into()).expect("line exists");
    paint(&mut flat, 20, 6);
    assert_eq!(
        flat.scroll.h.content_len, 6,
        "a replacement rescans the content"
    );
}

#[test]
fn issue39_extend_iterator_panic_keeps_previous_projection() {
    struct Boom(bool);
    impl Iterator for Boom {
        type Item = String;
        fn next(&mut self) -> Option<String> {
            if std::mem::replace(&mut self.0, true) {
                panic!("iterator failed");
            }
            Some("half".into())
        }
    }

    let mut panel = ScrollPanel::new(ID, vec![words("keep")]).wrap(true);
    paint(&mut panel, 14, 6);
    panel.select_all();
    let revision = panel.content_revision;
    let builds = panel.work.wrap_builds.get();

    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        panel.extend_lines(Boom(false))
    }));
    assert!(caught.is_err());

    assert_eq!(panel.lines(), [words("keep")]);
    assert_eq!(panel.content_revision, revision);
    assert!(panel.selection().is_some());
    let frame = paint(&mut panel, 14, 6);
    assert!(frame.text().contains("keep keep"), "{}", frame.text());
    assert!(!frame.text().contains("half"), "{}", frame.text());
    assert_eq!(panel.work.wrap_builds.get(), builds);
}

#[test]
fn issue39_revision_exhaustion_drops_every_cache() {
    let mut panel = ScrollPanel::new(ID, vec![words("old")]).wrap(true);
    panel.content_revision = u64::MAX;
    paint(&mut panel, 14, 6);
    assert_eq!(
        panel.wrapped_cache.as_ref().map(|c| c.revision),
        Some(u64::MAX)
    );

    assert!(panel.set_lines(vec![words("new")]));
    assert_eq!(panel.content_revision, 0);
    assert!(panel.wrapped_cache.is_none());
    let frame = paint(&mut panel, 14, 6);
    assert!(frame.text().contains("new new"), "{}", frame.text());
    assert!(!frame.text().contains("old"), "{}", frame.text());
}
