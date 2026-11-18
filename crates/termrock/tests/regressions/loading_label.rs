use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::content::draw_loading;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::common::{paint_with, row};

#[test]
fn a_wide_spinner_glyph_leaves_the_loading_label_whole() {
    let mut theme = Theme::for_level(ColorLevel::TrueColor);
    theme.glyphs.spinner = &["界"];
    let screen = Rect::new(0, 0, 20, 1);
    let p = paint_with(&theme, screen, None, |buf, ctx| {
        draw_loading(screen, buf, ctx, "ab", Color::Reset);
    });
    let text = row(&p.buf, 0);
    assert!(text.contains("ab") && !text.contains('…'), "{text:?}");
}
