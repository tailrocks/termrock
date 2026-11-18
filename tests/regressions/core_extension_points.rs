//! The three places where `termrock-core` meets a type of a higher crate keep
//! working for a downstream user (#444): a user row type that implements
//! `AsItem` is a `RowSource`, `ScrollPane::draw_*` needs no import and takes a
//! `RenderCtx`, and `Dismiss::allows` keeps its table.

use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::core::intent::DismissReason;
use junie_tui::core::list::RowSource;
use junie_tui::core::scroll::ScrollPane;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::popup::Dismiss;
use junie_tui::widgets::option_list::{AsItem, Item};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// A row type of a downstream crate: it owns the type, the trait is foreign.
struct Command {
    item: Item,
}

impl AsItem for Command {
    fn as_item(&self) -> &Item {
        &self.item
    }
}

#[test]
fn a_user_row_type_that_implements_as_item_is_a_row_source() {
    let rows = [
        Command {
            item: Item::new("open").key(ItemKey::text("o")),
        },
        Command {
            item: Item::new("close").reason("busy"),
        },
        Command {
            item: Item::new("quit"),
        },
    ];
    let source = &rows[..];
    assert_eq!(RowSource::len(source), 3);
    assert!(!RowSource::is_empty(source));
    assert_eq!(RowSource::label(source, 1), "close");
    assert!(RowSource::enabled(source, 0));
    assert!(!RowSource::enabled(source, 1));
    assert!(RowSource::enabled(source, 2));
    assert_eq!(RowSource::key(source, 0), ItemKey::text("o"));
    assert_eq!(RowSource::key(source, 2), ItemKey::index(2));
}

#[test]
fn scroll_pane_draws_with_a_render_context_and_no_import() {
    const OWNER: WidgetId = WidgetId::of("core-extension-points.scroll");
    let area = Rect::new(0, 0, 10, 4);
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut pane = ScrollPane::new(40);
    pane.set_viewport(4);
    {
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        pane.draw_v(Rect::new(9, 0, 1, 4), &mut buf, &mut ctx, OWNER, true);
    }
    let painted = (0..4).any(|y| buf[(9, y)].symbol() != " ");
    assert!(painted, "the vertical bar paints into its track");
}

#[test]
fn dismiss_allows_follows_the_reason_table() {
    for (dismiss, esc, outside, focus_out) in [
        (Dismiss::LIGHT, true, true, true),
        (Dismiss::ESCAPE, true, false, false),
        (Dismiss::NEVER, false, false, false),
    ] {
        assert_eq!(dismiss.allows(DismissReason::Esc), esc);
        assert_eq!(dismiss.allows(DismissReason::CloseButton), esc);
        assert_eq!(dismiss.allows(DismissReason::OutsideClick), outside);
        assert_eq!(dismiss.allows(DismissReason::FocusOut), focus_out);
        assert!(dismiss.allows(DismissReason::Programmatic));
    }
}
