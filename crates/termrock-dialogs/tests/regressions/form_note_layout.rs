use ratatui::{buffer::Buffer, layout::Rect};
use termrock_core::{focus::FocusRing, hit::HitRegistry, id::WidgetId};
use termrock_dialogs::form_dialog::{FormDialog, FormField};
use termrock_runtime::ctx::{Interaction, RenderCtx};
use termrock_theme::{ColorLevel, Theme, Tone};

fn draw(form: &mut FormDialog, screen: Rect) -> Buffer {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    let mut buf = Buffer::empty(screen);
    form.render(screen, &mut buf, &mut ctx);
    buf
}

fn contains(buf: &Buffer, text: &str) -> bool {
    (buf.area.y..buf.area.bottom()).any(|y| {
        (buf.area.x..buf.area.right())
            .map(|x| buf[(x, y)].symbol())
            .collect::<String>()
            .contains(text)
    })
}

#[test]
fn long_wrapped_note_scrolls_past_the_terminal_coordinate_limit() {
    let mut text = "row\n".repeat(65_535);
    text.push_str("last row");
    let mut form = FormDialog::new(
        WidgetId::of("form"),
        "Title",
        vec![
            FormField::note("long", vec![(text, Tone::Normal)]).wrap(true),
            FormField::note("after", vec![("after note".into(), Tone::Muted)]),
        ],
    );
    let screen = Rect::new(0, 0, 40, 20);
    assert!(contains(&draw(&mut form, screen), "row"));
    assert_eq!(form.scroll.content_len, 65_539);
    form.reveal("after");
    let buf = draw(&mut form, screen);
    assert!(contains(&buf, "last row"));
    assert!(contains(&buf, "after note"));
}

#[test]
fn narrow_screens_keep_the_last_unwrapped_note_row_reachable() {
    let mut rows = vec![("A".into(), Tone::Normal); 65_536];
    rows.push(("Z".into(), Tone::Success));
    let mut form = FormDialog::new(
        WidgetId::of("form"),
        "Title",
        vec![FormField::note("rows", rows)],
    );
    for width in [0, 1, 3, 12] {
        let screen = Rect::new(0, 0, width, 24);
        draw(&mut form, screen);
        assert_eq!(form.scroll.content_len, 65_538);
        form.scroll.scroll_to(usize::MAX);
        let buf = draw(&mut form, screen);
        if width == 12 {
            assert!(contains(&buf, "Z"));
        }
    }
}
