use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::common::*;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};
use termrock::core::hit::HitRegistry;
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::pointer::{MouseTarget, PointerEvent};
use termrock::widgets::chips::{Chip, ChipAction, ChipBar};
use termrock::widgets::toolbar::{ToolItem, Toolbar, ToolbarAction};

const TOOLBAR: WidgetId = WidgetId::of("lifecycle.toolbar");
const CHIPS: WidgetId = WidgetId::of("lifecycle.chips");
const NARROW: u16 = 8;
const CHIP_WIDTH: u16 = 24;

fn tools(labels: &[&str]) -> Vec<ToolItem> {
    labels.iter().map(|label| ToolItem::new(label)).collect()
}

fn render_toolbar(tb: &mut Toolbar, width: u16) -> HitRegistry {
    let mut hits = HitRegistry::default();
    draw(None, |buf, ctx| {
        tb.render(Rect::new(0, 0, width, 1), buf, ctx, ctx.theme.canvas);
        hits = ctx.hits.clone();
    });
    hits
}

fn render_chips(cb: &mut ChipBar, width: u16) -> HitRegistry {
    let mut hits = HitRegistry::default();
    draw(None, |buf, ctx| {
        cb.render(Rect::new(0, 0, width, 1), buf, ctx, ctx.theme.canvas);
        hits = ctx.hits.clone();
    });
    hits
}

fn click(hits: &HitRegistry, id: WidgetId) -> PointerEvent {
    PointerEvent::new(
        hits,
        Phase::Click,
        id,
        Position::new(0, 0),
        1,
        KeyModifiers::NONE,
    )
}

fn toolbar_click(tb: &mut Toolbar, hits: &HitRegistry, id: WidgetId) -> Option<ToolbarAction> {
    tb.route(&click(hits, id))
        .and_then(|response| response.into_actions().into_iter().next())
}

fn chip_click(cb: &mut ChipBar, hits: &HitRegistry, id: WidgetId) -> Option<ChipAction> {
    cb.route(&click(hits, id))
        .and_then(|response| response.into_actions().into_iter().next())
}

/// Renders a narrow toolbar and opens its overflow menu from the keyboard.
fn open_overflow_menu(tb: &mut Toolbar) {
    render_toolbar(tb, NARROW);
    tb.cursor = tb.items().len();
    tb.on_key(&key(KeyCode::Enter));
    assert!(tb.menu_open(), "Enter on More opens the overflow menu");
}

#[test]
fn toolbar_mutation_invalidates_hidden_geometry() {
    let mut tb = Toolbar::new(TOOLBAR, tools(&["A", "B", "C"]));
    let hits = render_toolbar(&mut tb, NARROW);
    assert_eq!(tb.hidden(), &[1, 2][..], "the narrow bar hides two items");

    let more = tb.more_id();
    tb.set_items(tools(&["A"]));
    let action = toolbar_click(&mut tb, &hits, more);

    assert_eq!(
        action, None,
        "a More click on stale geometry acts on nothing"
    );
    assert!(
        !tb.menu_open(),
        "a More click on stale geometry opens no menu"
    );
}

#[test]
fn toolbar_open_menu_cannot_retarget_after_reorder() {
    let mut tb = Toolbar::new(TOOLBAR, tools(&["A", "B", "C"]));
    open_overflow_menu(&mut tb);

    tb.update_item(1, |item| *item = ToolItem::new("Z"));
    assert!(!tb.menu_open(), "replacing a menu target closes the menu");
    let (_, action) = tb.on_key(&key(KeyCode::Enter));

    assert_ne!(
        action,
        Some(ToolbarAction::Activated(1)),
        "the old menu row must not activate the replacement at its index"
    );
}

#[test]
fn toolbar_disabled_update_closes_menu_and_rechecks_activation() {
    let mut tb = Toolbar::new(TOOLBAR, tools(&["A", "B", "C"]));
    open_overflow_menu(&mut tb);

    tb.update_item(1, |item| item.disabled = true);
    assert!(!tb.menu_open(), "disabling a menu target closes the menu");

    let hits = render_toolbar(&mut tb, NARROW);
    let row = tb.item_id(1);
    let action = toolbar_click(&mut tb, &hits, row);
    assert_eq!(action, None, "a disabled item does not activate by click");
}

#[test]
fn toolbar_overflow_policy_change_cancels_old_menu() {
    let mut tb = Toolbar::new(TOOLBAR, tools(&["A", "B", "C"]));
    open_overflow_menu(&mut tb);

    tb.overflow = false;
    let (_, action) = tb.on_key(&key(KeyCode::Enter));

    assert_eq!(
        action, None,
        "an overflow policy change cancels the old menu"
    );
    assert!(
        !tb.menu_open(),
        "the stale menu is closed on the next input"
    );
}

#[test]
fn toolbar_gap_policy_change_cancels_old_menu() {
    let mut tb = Toolbar::new(TOOLBAR, tools(&["A", "B", "C"]));
    open_overflow_menu(&mut tb);

    tb.gap = 2;
    let (_, action) = tb.on_key(&key(KeyCode::Enter));

    assert_eq!(action, None, "a spacing change cancels the old menu");
    assert!(
        !tb.menu_open(),
        "the stale menu is closed on the next input"
    );
}

#[test]
fn chip_context_remove_rechecks_current_eligibility() {
    let mut cb = ChipBar::new(CHIPS);
    cb.set_chips(vec![Chip::new("a"), Chip::new("b")]);
    render_chips(&mut cb, CHIP_WIDTH);
    cb.on_key(&key(KeyCode::Menu));
    assert!(cb.menu_open(), "the Menu key opens the chip context menu");

    cb.update_chip(0, |chip| chip.removable = false);
    assert!(!cb.menu_open(), "a fixed chip closes its open context menu");
    cb.on_key(&key(KeyCode::Down));
    let (_, action) = cb.on_key(&key(KeyCode::Enter));

    assert!(
        !matches!(action, Some(ChipAction::Remove(_))),
        "Remove never emits for a chip that is no longer removable"
    );
}

#[test]
fn chip_hidden_menu_mutation_requires_fresh_layout() {
    let mut cb = ChipBar::new(CHIPS);
    cb.set_chips((0..6).map(|i| Chip::new(&format!("chip{i}"))).collect());
    render_chips(&mut cb, CHIP_WIDTH);
    cb.on_key(&key(KeyCode::Down));
    assert!(cb.menu_open(), "Down opens the hidden-chip menu");

    cb.set_chips(vec![Chip::new("chip0")]);
    let (_, action) = cb.on_key(&key(KeyCode::Enter));

    assert!(
        !matches!(action, Some(ChipAction::Activate(i)) if i >= 1),
        "a hidden-chip menu row must not activate an index the collection no longer has"
    );
}

#[test]
fn chip_add_label_change_invalidates_old_add_hit() {
    let mut cb = ChipBar::new(CHIPS);
    cb.set_chips(vec![Chip::new("a")]);
    let hits = render_chips(&mut cb, CHIP_WIDTH);

    let add = cb.add_id();
    assert!(cb.set_add_label(None), "removing the add label changes it");
    let action = chip_click(&mut cb, &hits, add);

    assert_eq!(
        action, None,
        "a removed add label leaves no add target to click"
    );
}

#[test]
fn collection_equal_updates_preserve_menu() {
    let mut tb = Toolbar::new(TOOLBAR, tools(&["A", "B", "C"]));
    open_overflow_menu(&mut tb);
    let hidden = tb.hidden().to_vec();

    assert!(
        !tb.set_items(tools(&["A", "B", "C"])),
        "an equal set is a no-op"
    );
    assert!(!tb.update_item(1, |_| {}), "an identity update is a no-op");
    assert!(
        !tb.update_item(9, |item| item.disabled = true),
        "a bad index is a no-op"
    );

    assert!(tb.menu_open(), "no-op updates keep the open menu");
    assert_eq!(
        tb.hidden(),
        hidden.as_slice(),
        "no-op updates keep the geometry"
    );
}

#[test]
fn chip_equal_updates_preserve_menu() {
    let mut cb = ChipBar::new(CHIPS);
    cb.set_chips(vec![Chip::new("a"), Chip::new("b")]);
    render_chips(&mut cb, CHIP_WIDTH);
    cb.on_key(&key(KeyCode::Menu));
    assert!(cb.menu_open(), "the Menu key opens the chip context menu");

    assert!(!cb.set_chips(vec![Chip::new("a"), Chip::new("b")]));
    assert!(!cb.update_chip(0, |_| {}));
    assert!(!cb.set_add_label(Some("+ Add filter".into())));
    assert!(!cb.set_lead(None));

    assert!(cb.menu_open(), "no-op updates keep the open chip menu");
}

#[test]
fn collection_update_panic_does_not_partially_commit() {
    let mut tb = Toolbar::new(TOOLBAR, tools(&["A", "B", "C"]));
    open_overflow_menu(&mut tb);

    let panicked = catch_unwind(AssertUnwindSafe(|| {
        tb.update_item(1, |item| {
            item.label = "Changed".into();
            panic!("the update fails midway");
        });
    }));

    assert!(panicked.is_err(), "the panic reaches the caller");
    assert_eq!(tb.items()[1].label, "B", "the source item keeps its label");
    assert!(tb.menu_open(), "a failed update leaves the menu alone");
}

#[test]
fn collection_checked_inserts_and_removes_reject_out_of_range() {
    let mut tb = Toolbar::new(TOOLBAR, tools(&["A", "B"]));
    assert!(
        !tb.insert_item(3, ToolItem::new("C")),
        "past-the-end insert fails"
    );
    assert_eq!(tb.remove_item(2), None, "out-of-range remove fails");
    assert!(tb.insert_item(2, ToolItem::new("C")), "append is allowed");
    assert_eq!(tb.items().len(), 3);

    let mut cb = ChipBar::new(CHIPS);
    assert!(
        !cb.insert_chip(1, Chip::new("a")),
        "past-the-end insert fails"
    );
    assert_eq!(cb.remove_chip(0), None, "out-of-range remove fails");
    assert!(
        cb.insert_chip(0, Chip::new("a")),
        "insert at the front is allowed"
    );
    assert_eq!(cb.chips().len(), 1);
}

#[test]
fn chip_cursor_clamps_to_current_stops_on_change() {
    let mut cb = ChipBar::new(CHIPS);
    cb.set_chips(vec![Chip::new("a"), Chip::new("b"), Chip::new("c")]);

    cb.cursor = 2;
    cb.remove_chip(2);
    assert_eq!(cb.cursor, 2, "the Add stop after the last chip stays valid");

    cb.cursor = 9;
    cb.set_chips(vec![Chip::new("a")]);
    assert_eq!(cb.cursor, 1, "a chip and the Add stop leave two stops");

    cb.set_add_label(None);
    assert_eq!(cb.cursor, 0, "without an add label the last chip is last");

    cb.set_chips(Vec::new());
    assert_eq!(cb.cursor, 0, "an empty bar has no stop past zero");
}

#[test]
fn toolbar_cursor_keeps_the_more_stop_and_clamps_past_it() {
    let mut tb = Toolbar::new(TOOLBAR, tools(&["A", "B", "C"]));

    tb.cursor = 3;
    tb.remove_item(0);
    assert_eq!(tb.cursor, 2, "the More stop follows the shrinking length");

    tb.cursor = 9;
    tb.remove_item(0);
    assert_eq!(tb.cursor, 1, "a cursor past the More stop lands on it");

    tb.cursor = 0;
    tb.insert_item(0, ToolItem::new("Z"));
    assert_eq!(tb.cursor, 0, "an in-range cursor is not moved");
}

#[test]
fn toolbar_policy_change_then_render_closes_the_open_menu() {
    let mut tb = Toolbar::new(TOOLBAR, tools(&["A", "B", "C"]));
    open_overflow_menu(&mut tb);

    tb.gap = 2;
    render_toolbar(&mut tb, NARROW);

    assert!(
        !tb.menu_open(),
        "a render after a spacing change must not re-validate the old menu rows"
    );
}

#[test]
fn toolbar_resize_that_changes_the_hidden_items_closes_the_open_menu() {
    let mut changed = false;
    for width in NARROW + 1..40 {
        let mut tb = Toolbar::new(TOOLBAR, tools(&["A", "B", "C", "D", "E"]));
        open_overflow_menu(&mut tb);
        let before = tb.hidden().to_vec();

        render_toolbar(&mut tb, width);

        if tb.hidden() != before && !tb.hidden().is_empty() {
            changed = true;
            assert!(
                !tb.menu_open(),
                "width {width} hides {:?} but the menu still lists {before:?}",
                tb.hidden()
            );
        }
    }
    assert!(
        changed,
        "no width changed the hidden items, so the case never ran"
    );
}

#[test]
fn chip_context_policy_change_then_render_closes_the_open_menu() {
    let mut cb = ChipBar::new(CHIPS);
    cb.set_chips(vec![Chip::new("a"), Chip::new("b")]);
    render_chips(&mut cb, CHIP_WIDTH);
    cb.on_key(&key(KeyCode::Menu));
    assert!(cb.menu_open(), "the Menu key opens the chip context menu");

    cb.context_menu = false;
    render_chips(&mut cb, CHIP_WIDTH);

    assert!(
        !cb.menu_open(),
        "a disabled context menu must not stay open across a render"
    );
}

/// The first chip the hidden-chip menu lists at `width`, if the bar overflows there.
fn first_hidden_chip(width: u16) -> Option<usize> {
    let mut cb = ChipBar::new(CHIPS);
    cb.set_chips((0..6).map(|i| Chip::new(&format!("chip{i}"))).collect());
    render_chips(&mut cb, width);
    cb.on_key(&key(KeyCode::Down));
    match cb.on_key(&key(KeyCode::Enter)).1 {
        Some(ChipAction::Activate(i)) if i >= 1 => Some(i),
        _ => None,
    }
}

#[test]
fn chip_resize_that_changes_the_hidden_chips_closes_the_open_menu() {
    let at_open = first_hidden_chip(CHIP_WIDTH).expect("the bar overflows when the menu opens");
    let mut changed = false;
    for width in CHIP_WIDTH + 1..80 {
        let Some(now) = first_hidden_chip(width).filter(|&now| now != at_open) else {
            continue;
        };
        let mut cb = ChipBar::new(CHIPS);
        cb.set_chips((0..6).map(|i| Chip::new(&format!("chip{i}"))).collect());
        render_chips(&mut cb, CHIP_WIDTH);
        cb.on_key(&key(KeyCode::Down));
        assert!(cb.menu_open(), "Down opens the hidden-chip menu");

        render_chips(&mut cb, width);

        changed = true;
        assert!(
            !cb.menu_open(),
            "width {width} hides from chip {now} but the menu still lists from chip {at_open}"
        );
    }
    assert!(
        changed,
        "no width changed the hidden chips, so the case never ran"
    );
}
