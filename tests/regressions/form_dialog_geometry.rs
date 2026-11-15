use junie_tui::core::id::WidgetId;
use junie_tui::widgets::form_dialog::{FormDialog, FormField};
use junie_tui::widgets::input::TextInput;
use ratatui::layout::Rect;

use crate::common::paint_in;

fn form() -> (FormDialog, Vec<WidgetId>) {
    let id = WidgetId::of("t.form");
    let names = ["a", "b", "c", "d", "e", "f"];
    let fields = names
        .iter()
        .map(|n| FormField::input(n, TextInput::new(id.sub(n), n)))
        .collect();
    (
        FormDialog::new(id, "Account", fields).width(40),
        names.iter().map(|n| id.sub(n)).collect(),
    )
}

#[test]
fn a_form_with_no_room_for_its_body_keeps_its_fields_out_of_the_focus_ring() {
    let mut empty_bodies = 0;
    for h in 3..=16 {
        let screen = Rect::new(0, 0, 60, h);
        let (mut form, ids) = form();
        let p = paint_in(screen, None, |buf, ctx| form.render(screen, buf, ctx));
        if form.scroll.v.viewport_len > 0 {
            continue;
        }
        empty_bodies += 1;
        for id in &ids {
            assert!(
                !p.ring.contains(*id),
                "height {h}: unseen field is a tab stop"
            );
        }
    }
    assert!(empty_bodies > 0, "no screen height left the body empty");
}
