//! Nested scroll views: the inner view takes a scroll key first when it has
//! something to scroll, and hands back a key it cannot use, so the outer view
//! scrolls only then.
//!
//! `cargo run -p termrock --example nested_scroll`

mod common;

use common::{frame, key};
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Rect, Size};
use ratatui::widgets::Widget;
use termrock::core::event::Outcome;
use termrock::core::id::WidgetId;
use termrock::ui::scroll_view::{Axes, ScrollView};

const OUTER: Rect = Rect::new(0, 0, 24, 8);
const INNER: Rect = Rect::new(2, 1, 20, 3);

struct Page {
    outer: ScrollView,
    inner: ScrollView,
    /// Rows of inner content. The inner window shows 3.
    inner_rows: u16,
}

impl Page {
    fn new(inner_rows: u16) -> Self {
        let view = |name| ScrollView::new(WidgetId::of(name)).axes(Axes::VERTICAL);
        Self {
            outer: view("example.outer"),
            inner: view("example.inner"),
            inner_rows,
        }
    }

    /// The inner view (3-row window) sits inside the outer view (30 rows of
    /// content in an 8-row window).
    fn screen(&mut self) -> String {
        let (inner, rows) = (&mut self.inner, self.inner_rows);
        let (screen, ()) = frame(OUTER.width, OUTER.height, |buf, ctx| {
            self.outer.render(
                OUTER,
                Size::new(OUTER.width, 30),
                buf,
                ctx,
                |_, buf, ctx| {
                    inner.render(
                        INNER,
                        Size::new(INNER.width, rows),
                        buf,
                        ctx,
                        |area, buf, _| {
                            for y in 0..rows {
                                let line = ratatui::text::Line::raw(format!("inner row {y}"));
                                line.render(Rect::new(area.x, area.y + y, area.width, 1), buf);
                            }
                        },
                    );
                },
            );
        });
        screen
    }

    /// Routes a key to the inner view first. `None` from it means "not mine".
    fn press(&mut self, code: KeyCode) -> Option<Outcome> {
        let key = key(code);
        self.inner.on_key(&key).or_else(|| self.outer.on_key(&key))
    }
}

fn main() {
    for rows in [6, 2] {
        let mut page = Page::new(rows);
        println!("-- inner content: {rows} rows --\n{}", page.screen());
        page.press(KeyCode::PageDown);
        page.screen();
        println!(
            "PageDown: inner {:?}, outer {:?}",
            page.inner.offset(),
            page.outer.offset()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_inner_view_with_more_content_scrolls_before_the_outer_view() {
        let mut page = Page::new(6);
        page.screen();
        assert!(page.screen().contains("inner row 0"));

        assert_eq!(page.press(KeyCode::PageDown), Some(Outcome::Changed));
        page.screen();
        assert_ne!(page.inner.offset(), (0, 0), "the inner view scrolled");
        assert_eq!(page.outer.offset(), (0, 0), "the outer view did not move");
    }

    #[test]
    fn an_inner_view_that_fits_hands_the_key_to_the_outer_view() {
        let mut page = Page::new(2);
        page.screen();

        assert_eq!(page.press(KeyCode::PageDown), Some(Outcome::Changed));
        assert_eq!(page.inner.offset(), (0, 0), "nothing to scroll inside");
        assert_ne!(page.outer.offset(), (0, 0), "the outer view took the key");
    }
}
