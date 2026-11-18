use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;

use super::*;
use termrock_core::focus::FocusRing;
use termrock_core::hit::HitRegistry;
use termrock_core::intent::Phase;
use termrock_core::pointer::{MouseTarget, PointerEvent};
use termrock_runtime::ctx::Interaction;
use termrock_theme::{ColorLevel, Theme};

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
        self.pointer_n(panel, phase, pos, 1);
    }

    fn pointer_n(&self, panel: &mut ScrollPanel, phase: Phase, pos: Position, count: u8) {
        let ev = PointerEvent::new(&self.hits, phase, ID, pos, count, KeyModifiers::NONE);
        drop(panel.pointer(&ev));
    }

    fn drag(&self, panel: &mut ScrollPanel, from: (u16, u16), to: (u16, u16)) {
        self.pointer(panel, Phase::Press, Position::new(from.0, from.1));
        self.pointer(panel, Phase::Drag, Position::new(to.0, to.1));
    }

    fn highlighted(&self) -> String {
        let bg = Theme::for_level(ColorLevel::TrueColor).selection_bg;
        (0..self.buf.area.height)
            .flat_map(|y| (0..self.buf.area.width).map(move |x| (x, y)))
            .filter(|&pos| self.buf[pos].bg == bg)
            .map(|pos| self.buf[pos].symbol().to_owned())
            .collect::<String>()
            .replace(' ', "")
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

fn joined(lines: &[String]) -> String {
    lines.join("\n")
}

fn without_space(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn issue45_copy_long_token_has_no_soft_spaces() {
    let source = "abcdefghijkl".to_owned();
    for (width, expect_rows) in [
        (6u16, vec!["abcd", "efgh", "ijkl"]),
        (8, vec!["abcdef", "ghijkl"]),
    ] {
        let mut panel = ScrollPanel::new(ID, vec![source.clone()]).wrap(true);
        paint(&mut panel, width, 6);
        assert_eq!(rows_of(&panel), expect_rows, "width {width}");
        panel.select_all();
        assert_eq!(panel.selected_text().as_deref(), Some("abcdefghijkl"));
        let frame = paint(&mut panel, width, 6);
        assert_eq!(frame.highlighted(), "abcdefghijkl", "width {width}");
    }

    let mut panel = ScrollPanel::new(ID, vec!["abcdefghij".to_owned()]).wrap(true);
    let frame = paint(&mut panel, 6, 6);
    frame.drag(&mut panel, (1, 0), (3, 2));
    assert_eq!(panel.selected_text().as_deref(), Some("bcdefghij"));
    let frame = paint(&mut panel, 6, 6);
    assert_eq!(frame.highlighted(), "bcdefghij");
}

#[test]
fn issue45_copy_preserves_source_whitespace() {
    let lines: Vec<String> = [
        "  lead  and   repeated   gaps",
        "",
        "trailing   ",
        "hard\nbreak inside",
        "   ",
        "tail",
    ]
    .map(str::to_owned)
    .to_vec();
    for width in [8u16, 12, 20, 60] {
        let mut panel = ScrollPanel::new(ID, lines.clone()).wrap(true);
        paint(&mut panel, width, 30);
        panel.select_all();
        assert_eq!(
            panel.selected_text().as_deref(),
            Some(joined(&lines).as_str()),
            "width {width}"
        );
    }

    let mut panel = ScrollPanel::new(ID, lines.clone()).wrap(true);
    let frame = paint(&mut panel, 60, 12);
    frame.drag(&mut panel, (3, 0), (3, 2));
    assert_eq!(
        panel.selected_text().as_deref(),
        Some("ead  and   repeated   gaps\n\ntra")
    );
    frame.drag(&mut panel, (0, 2), (30, 2));
    assert_eq!(panel.selected_text().as_deref(), Some("trailing   "));
    assert_eq!(
        panel.source_selection(),
        Some(((2, 0), (2, "trailing   ".len())))
    );
}

#[test]
fn issue45_copy_unicode_grapheme_mapping() {
    let source = "日本語a😀e\u{301}🧑\u{200d}🤝\u{200d}🧑z".to_owned();
    for width in [3u16, 4, 5, 6, 12] {
        let mut panel = ScrollPanel::new(ID, vec![source.clone()]).wrap(true);
        paint(&mut panel, width, 30);
        panel.select_all();
        assert_eq!(
            panel.selected_text().as_deref(),
            Some(source.as_str()),
            "width {width}"
        );
    }

    let mut panel = ScrollPanel::new(ID, vec![source.clone()]).wrap(true);
    let frame = paint(&mut panel, 6, 30);
    assert_eq!(rows_of(&panel)[0], "日本");
    frame.drag(&mut panel, (1, 0), (3, 0));
    assert_eq!(
        panel.selected_text().as_deref(),
        Some("本"),
        "second halves"
    );
    frame.drag(&mut panel, (0, 0), (2, 0));
    assert_eq!(
        panel.selected_text().as_deref(),
        Some("日"),
        "leading cells"
    );

    let mut narrow = ScrollPanel::new(ID, vec![source.clone()]).wrap(true);
    let frame = paint(&mut narrow, 3, 30);
    assert!(frame.text().contains('…'), "{}", frame.text());
    frame.drag(&mut narrow, (0, 1), (0, 2));
    assert_eq!(narrow.selected_text().as_deref(), Some("本"));

    let accent = "abce\u{301}fgh".to_owned();
    let mut panel = ScrollPanel::new(ID, vec![accent]).wrap(true);
    let frame = paint(&mut panel, 5, 6);
    assert_eq!(rows_of(&panel)[1], "e\u{301}fg");
    frame.drag(&mut panel, (0, 1), (1, 1));
    assert_eq!(panel.selected_text().as_deref(), Some("e\u{301}"));
}

#[test]
fn issue45_selection_survives_resize_and_append() {
    let lines = vec![
        "alpha beta gamma delta epsilon zeta".to_owned(),
        "second line here".to_owned(),
    ];
    let mut panel = ScrollPanel::new(ID, lines.clone()).wrap(true);
    let frame = paint(&mut panel, 14, 12);
    frame.drag(&mut panel, (2, 0), (5, 3));
    let text = panel.selected_text().expect("selected");
    let source = panel.source_selection().expect("source");
    assert!(text.starts_with("pha"), "{text:?}");
    let mut visual = Vec::new();
    for width in [10u16, 30, 7, 14, 3, 14, 40] {
        let frame = paint(&mut panel, width, 60);
        assert_eq!(panel.selected_text().as_ref(), Some(&text), "width {width}");
        assert_eq!(panel.source_selection(), Some(source), "width {width}");
        assert_eq!(frame.highlighted(), without_space(&text), "width {width}");
        visual.push(panel.selection());
    }
    visual.dedup();
    assert!(visual.len() > 2, "the painted endpoints follow the width");

    let frame = paint(&mut panel, 14, 12);
    panel.push("appended tail".to_owned());
    assert_eq!(panel.selected_text().as_ref(), Some(&text));
    assert_eq!(panel.source_selection(), Some(source));
    let frame2 = paint(&mut panel, 14, 12);
    assert_eq!(frame2.highlighted(), frame.highlighted());
    panel.select_all();
    panel.push("later".to_owned());
    let all = format!("{}\nappended tail", joined(&lines));
    assert_eq!(
        panel.selected_text().as_deref(),
        Some(all.as_str()),
        "appended rows do not join an existing select-all range"
    );

    let frame3 = paint(&mut panel, 14, 12);
    frame3.drag(&mut panel, (1, 0), (4, 0));
    assert!(panel.select.press.is_some());
    assert_eq!(panel.replace_line(0, "new".to_owned()), Ok(true));
    assert!(panel.source_selection().is_none() && panel.selected_text().is_none());
    assert!(panel.select.press.is_none());
}

#[test]
fn issue45_copying_a_tiny_selection_reads_only_the_selected_source_lines() {
    use termrock_core::edit::{TextCmd, clipboard};

    let lines: Vec<String> = (0..10_000)
        .map(|i| format!("line {i:05} with some words to wrap around the panel"))
        .collect();
    let mut panel = ScrollPanel::new(ID, lines).wrap(true);
    let frame = paint(&mut panel, 20, 12);
    frame.drag(&mut panel, (0, 4), (3, 4));
    assert!(
        panel.source_selection().is_some(),
        "three cells are selected"
    );

    let builds = panel.work.wrap_builds.get();
    let measured = panel.work.width_measurements.get();
    clipboard::store("");
    #[cfg(feature = "work-metrics")]
    let (_, work) = termrock_metrics::measure(|| panel.edit_run(TextCmd::Copy));
    #[cfg(not(feature = "work-metrics"))]
    let _ = panel.edit_run(TextCmd::Copy);

    let copied = clipboard::text().expect("the selection was copied");
    assert_eq!(copied.chars().count(), 3, "{copied:?}");
    assert_eq!(
        panel.work.wrap_builds.get(),
        builds,
        "copying never rebuilds the wrap mapping"
    );
    assert_eq!(
        panel.work.width_measurements.get(),
        measured,
        "copying measures no line"
    );
    #[cfg(feature = "work-metrics")]
    assert_eq!(
        work.get(termrock_metrics::Metric::SourceRowsRead),
        1,
        "a selection inside one source line reads that line, not the 10k-line source: {:?}",
        work.nonzero().collect::<Vec<_>>()
    );
}

#[test]
fn issue45_unwrapped_hscroll_and_wrap_toggle() {
    let source = "0123456789日本語abcdef".to_owned();
    let mut panel = ScrollPanel::new(ID, vec![source.clone()]);
    paint(&mut panel, 10, 4);
    assert!(panel.scroll.h.scroll_to(11));
    let frame = paint(&mut panel, 10, 4);
    assert_eq!(panel.scroll.h.offset, 11);
    frame.drag(&mut panel, (0, 0), (5, 0));
    assert_eq!(
        panel.selected_text().as_deref(),
        Some("本語"),
        "the clipped half of 日 snaps to the whole grapheme boundary"
    );
    let source_range = panel.source_selection();
    let before = panel.selection();

    for wrap in [true, false, true, false] {
        panel.wrap = wrap;
        paint(&mut panel, 10, 8);
        if !wrap {
            panel.scroll.h.scroll_to(11);
        }
        let frame = paint(&mut panel, 10, 8);
        assert_eq!(
            panel.selected_text().as_deref(),
            Some("本語"),
            "wrap={wrap}"
        );
        assert_eq!(panel.source_selection(), source_range, "wrap={wrap}");
        assert_eq!(frame.highlighted(), "本語", "wrap={wrap}");
        if wrap {
            assert_ne!(panel.selection(), before, "the painted endpoints remap");
        }
    }

    panel.scroll.h.scroll_to(0);
    let frame = paint(&mut panel, 10, 4);
    assert_eq!(panel.selected_text().as_deref(), Some("本語"));
    assert_eq!(frame.highlighted(), "", "scrolled away from the selection");
}

#[test]
fn issue45_select_all_before_measure() {
    let lines = vec!["  one  two".to_owned(), String::new(), "three ".to_owned()];
    let mut panel = ScrollPanel::new(ID, lines.clone()).wrap(true);
    assert_eq!(panel.select_all(), Outcome::Changed);
    assert!(panel.selection().is_none(), "no projection exists yet");
    let all = joined(&lines);
    assert_eq!(panel.selected_text().as_deref(), Some(all.as_str()));
    assert_eq!(
        panel.source_selection(),
        Some(((0, 0), (2, "three ".len())))
    );

    panel.measure(Rect::new(0, 0, 2, 4));
    assert!(panel.selection().is_none(), "zero visible width");
    assert_eq!(panel.selected_text().as_deref(), Some(all.as_str()));
    assert_eq!(panel.select_all(), Outcome::Consumed);

    paint(&mut panel, 8, 6);
    assert!(panel.selection().is_some());
    assert_eq!(panel.selected_text().as_deref(), Some(all.as_str()));

    let mut empty = ScrollPanel::new(ID, Vec::new()).wrap(true);
    assert_eq!(empty.select_all(), Outcome::Consumed);
    assert!(empty.selected_text().is_none());
}

#[test]
fn issue45_visual_adapter_wrap_boundary_ties() {
    let mut panel = ScrollPanel::new(ID, vec!["abcdefgh".to_owned()]).wrap(true);
    let frame = paint(&mut panel, 6, 6);
    assert_eq!(rows_of(&panel), ["abcd", "efgh"]);

    frame.drag(&mut panel, (2, 0), (0, 1));
    assert_eq!(panel.source_selection(), Some(((0, 2), (0, 4))));
    assert_eq!(
        panel.selection(),
        Some((CellPos { line: 0, col: 2 }, CellPos { line: 0, col: 4 })),
        "an end endpoint on the shared boundary stays on the preceding row"
    );
    frame.drag(&mut panel, (0, 1), (3, 1));
    assert_eq!(
        panel.selection().map(|s| s.0),
        Some(CellPos { line: 1, col: 0 }),
        "a start endpoint on the shared boundary moves to the following row"
    );

    paint(&mut panel, 12, 6);
    assert_eq!(rows_of(&panel), ["abcdefgh"]);
    assert_eq!(
        panel.selection(),
        Some((CellPos { line: 0, col: 4 }, CellPos { line: 0, col: 7 }))
    );
    assert_eq!(panel.selected_text().as_deref(), Some("efg"));
}

#[test]
fn issue45_double_and_triple_click_select_logical_text() {
    let lines = vec![
        "see /usr/local/share/very-long-path now".to_owned(),
        "next entry".to_owned(),
    ];
    let mut panel = ScrollPanel::new(ID, lines.clone()).wrap(true);
    let frame = paint(&mut panel, 8, 20);
    assert!(rows_of(&panel).len() > 4, "{:?}", rows_of(&panel));

    frame.pointer_n(&mut panel, Phase::Press, Position::new(2, 2), 2);
    assert_eq!(
        panel.selected_text().as_deref(),
        Some("/usr/local/share/very-long-path")
    );

    frame.pointer_n(&mut panel, Phase::Press, Position::new(2, 2), 3);
    assert_eq!(panel.selected_text().as_deref(), Some(lines[0].as_str()));

    let last = rows_of(&panel).len() as u16 - 1;
    frame.pointer_n(&mut panel, Phase::Press, Position::new(1, last), 3);
    assert_eq!(panel.selected_text().as_deref(), Some("next entry"));
}

#[test]
fn issue45_keyboard_and_menu_copy_use_logical_text() {
    use ratatui::crossterm::event::KeyCode;
    use termrock_core::edit::{TextCmd, clipboard};

    let lines = vec!["abcdefghijkl".to_owned(), "mn  op".to_owned()];
    let all = joined(&lines);
    let mut panel = ScrollPanel::new(ID, lines).wrap(true);
    paint(&mut panel, 6, 8);
    panel.select_all();

    clipboard::store("");
    let ctrl_c = Key {
        code: KeyCode::Char('c'),
        mods: KeyModifiers::CONTROL,
    };
    assert_eq!(panel.on_key(&ctrl_c), Outcome::Consumed);
    assert_eq!(clipboard::text().as_deref(), Some(all.as_str()));

    clipboard::store("");
    panel.edit_run(TextCmd::Copy);
    assert_eq!(clipboard::text().as_deref(), Some(all.as_str()));
}

fn projection(panel: &ScrollPanel) -> (Vec<String>, String) {
    let cache = panel.wrapped_cache.as_ref().expect("wrapped cache");
    (cache.rows.clone(), format!("{:?}", cache.maps))
}

fn full_rewrap(panel: &ScrollPanel, width: u16) -> (Vec<String>, String) {
    let mut fresh = ScrollPanel::new(ID, panel.lines().to_vec()).wrap(true);
    paint(&mut fresh, width, 5);
    projection(&fresh)
}

#[test]
fn issue291_append_extends_wrapped_prefix_identically_to_a_full_rewrap() {
    let mut panel = ScrollPanel::new(ID, vec![words("alpha"), String::new()]).wrap(true);
    paint(&mut panel, 14, 5);
    let builds = panel.work.wrap_builds.get();
    assert_eq!(builds, 1);

    panel.push(words("beta"));
    paint(&mut panel, 14, 5);
    assert_eq!(projection(&panel), full_rewrap(&panel, 14));

    panel.extend_lines(vec![
        "wide \u{4e2d}\u{6587}\u{4e2d}\u{6587}\u{4e2d}\u{6587}\u{4e2d}\u{6587}".to_owned(),
        "unbrokenunbrokenunbroken".to_owned(),
        String::new(),
    ]);
    paint(&mut panel, 14, 5);
    assert_eq!(projection(&panel), full_rewrap(&panel, 14));
    assert_eq!(
        panel.work.wrap_builds.get(),
        builds,
        "appends at an unchanged width never rebuild the prefix"
    );

    // A width change rebuilds, and appends then extend the new width.
    paint(&mut panel, 9, 5);
    assert_eq!(panel.work.wrap_builds.get(), builds + 1);
    assert_eq!(projection(&panel), full_rewrap(&panel, 9));
    panel.push(words("gamma"));
    paint(&mut panel, 9, 5);
    assert_eq!(projection(&panel), full_rewrap(&panel, 9));
    assert_eq!(panel.work.wrap_builds.get(), builds + 1);

    // A non-append edit rebuilds from scratch.
    panel.replace_line(0, words("delta")).expect("line exists");
    paint(&mut panel, 9, 5);
    assert_eq!(panel.work.wrap_builds.get(), builds + 2);
    assert_eq!(projection(&panel), full_rewrap(&panel, 9));
    panel.push(words("eps"));
    panel.set_lines(vec![words("only")]);
    paint(&mut panel, 9, 5);
    assert_eq!(panel.work.wrap_builds.get(), builds + 3);
    assert_eq!(projection(&panel), full_rewrap(&panel, 9));
}

#[test]
fn issue291_several_appends_between_frames_and_unwrapped_gap() {
    let mut panel = ScrollPanel::new(ID, numbered(3)).wrap(true);
    paint(&mut panel, 20, 4);
    panel.push("one".into());
    panel.push("two".into());
    panel.extend_lines(vec!["three".into()]);
    paint(&mut panel, 20, 4);
    assert_eq!(projection(&panel), full_rewrap(&panel, 20));

    // Appends made while the panel is not wrapping are caught up on return.
    panel.wrap = false;
    panel.push("while flat".into());
    paint(&mut panel, 20, 4);
    panel.wrap = true;
    paint(&mut panel, 20, 4);
    assert_eq!(projection(&panel), full_rewrap(&panel, 20));
    assert_eq!(panel.work.wrap_builds.get(), 1);
}
