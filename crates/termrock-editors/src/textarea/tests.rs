use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};
use ratatui::style::Color;

use super::TextArea;
use crate::test_alloc;
use termrock_core::focus::FocusRing;
use termrock_core::hit::HitRegistry;
use termrock_core::id::WidgetId;
use termrock_runtime::ctx::{Interaction, RenderCtx};
use termrock_runtime::measure::Measure;
use termrock_text::text_index::work;
use termrock_theme::{ColorLevel, Theme};

fn document(lines: usize) -> String {
    (0..lines)
        .map(|i| format!("note {i} 日本語 e\u{301} text"))
        .collect::<Vec<_>>()
        .join("\n")
}

struct Frame {
    theme: Theme,
    hits: HitRegistry,
    ring: FocusRing,
    buf: Buffer,
}

impl Frame {
    fn new() -> Self {
        Self {
            theme: Theme::for_level(ColorLevel::TrueColor),
            hits: HitRegistry::default(),
            ring: FocusRing::default(),
            buf: Buffer::empty(Rect::new(0, 0, 100, 14)),
        }
    }

    fn paint(&mut self, area: &mut TextArea, interaction: Interaction) {
        let rect = self.buf.area;
        let mut ctx = RenderCtx::new(&self.theme, interaction, &mut self.hits, &mut self.ring);
        area.render(rect, &mut self.buf, &mut ctx, Color::Reset);
    }

    fn row(&self, y: u16) -> String {
        (0..self.buf.area.width)
            .map(|x| self.buf[(x, y)].symbol())
            .collect::<String>()
            .replace(' ', "")
    }
}

#[test]
fn textarea_visits_only_visible_rows() {
    let id = WidgetId::of("area-test");
    let hover = Interaction {
        hover: Some(id),
        ..Default::default()
    };
    let mut per_size = Vec::new();
    for lines in [1_000usize, 10_000, 100_000] {
        let mut frame = Frame::new();
        let mut area = TextArea::new(id, "Notes", 10).value(&document(lines));
        let before = work::snapshot();
        frame.paint(&mut area, Interaction::default());
        let first = work::snapshot();
        assert_eq!(first.index_builds - before.index_builds, 1, "{lines} lines");
        assert!(
            first.grapheme_tables - before.grapheme_tables <= 10,
            "{lines} lines: only the 10 visible rows build tables: {first:?}"
        );
        assert!(frame.row(2).contains("note1日本語"));

        let mut worst = 0;
        for interaction in [Interaction::default(), hover, Interaction::default()] {
            let ((), bytes) = test_alloc::measure(|| frame.paint(&mut area, interaction));
            worst = worst.max(bytes);
        }
        assert_eq!(
            work::snapshot(),
            first,
            "{lines} lines: repaint adds no index work"
        );
        per_size.push((lines, worst));
    }
    let (_, small) = per_size[0];
    assert!(
        per_size.iter().all(|(_, bytes)| *bytes <= small + 1024),
        "per-frame allocation must not grow with hidden lines: {per_size:?}"
    );
}

#[test]
fn textarea_paints_the_scrolled_window_from_the_index() {
    let mut frame = Frame::new();
    let mut area = TextArea::new(WidgetId::of("area-test"), "Notes", 6).value(&document(500));
    frame.paint(&mut area, Interaction::default());
    area.scroll.offset = 300;
    frame.paint(&mut area, Interaction::default());
    for (i, y) in (1u16..7).enumerate() {
        assert!(
            frame.row(y).contains(&format!("note{}日本語", 300 + i)),
            "row {y}: {}",
            frame.row(y)
        );
    }
}

#[test]
fn textarea_measure_uses_the_line_index() {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    let area = TextArea::new(WidgetId::of("area-test"), "Notes", 6).value(&document(80_000));
    let size = area.measure(Size::new(80, 24), &ctx);
    assert!(size.width > 20);
    let warm = work::snapshot();
    let (_, bytes) = test_alloc::measure(|| area.measure(Size::new(80, 24), &ctx));
    assert!(bytes < 256, "measure allocates {bytes} bytes");
    assert_eq!(work::snapshot(), warm, "measure reuses the cached widths");
}
