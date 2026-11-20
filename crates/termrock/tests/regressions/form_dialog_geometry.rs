use ratatui::layout::{Position, Rect};
use termrock::core::id::WidgetId;
use termrock::widgets::form_dialog::{FormDialog, FormField};
use termrock::widgets::input::TextInput;

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

#[test]
fn fields_scrolled_out_of_the_body_leave_no_hit_regions() {
    let screen = Rect::new(0, 0, 60, 14);
    let (mut form, ids) = form();
    let last = *ids.last().expect("fields");
    let p = paint_in(screen, None, |buf, ctx| form.render(screen, buf, ctx));
    let first_area = p
        .hits
        .regions()
        .iter()
        .find(|r| r.id == ids[0])
        .map(|r| r.area)
        .expect("the first field is drawn before scrolling");

    for _ in 0..2 {
        let p = paint_in(screen, Some(last), |buf, ctx| form.render(screen, buf, ctx));
        let body = form_body_hits(&p, &ids);
        assert!(
            body.iter().any(|(id, _)| *id == last),
            "the focused last field is scrolled into view"
        );
        for (id, area) in &body {
            let centre = Position::new(area.x, area.y);
            let top = p.hits.resolve(centre).map(|h| h.owner);
            assert_eq!(top, Some(*id), "{id:?} claims cells another field paints");
        }
        assert!(
            !body.iter().any(|(id, _)| *id == ids[0]),
            "the first field scrolled away yet still registers a hit at {first_area:?}"
        );
    }
}

fn form_body_hits(p: &crate::common::Painted, ids: &[WidgetId]) -> Vec<(WidgetId, Rect)> {
    p.hits
        .regions()
        .iter()
        .filter(|r| ids.contains(&r.id))
        .map(|r| (r.id, r.area))
        .collect()
}
