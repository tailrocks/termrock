use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::popup::backdrop_id;
use junie_tui::widgets::tabs::{TabAction, TabItem, Tabs};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};

const ID: WidgetId = WidgetId::of("t.tabs44");
const AREA: Rect = Rect::new(0, 0, 24, 3);

fn names(prefix: &str, n: usize) -> Vec<String> {
    (0..n).map(|i| format!("{prefix} {i}")).collect()
}

fn strip(names: &[String]) -> Tabs {
    Tabs::with_items(ID, names.iter().map(|n| TabItem::new(n)).collect())
}

fn key(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

/// Renders the strip at the narrow width and returns the frame and its hit regions.
fn render(tabs: &mut Tabs) -> (Buffer, HitRegistry) {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 12));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        tabs.render(AREA, &mut buf, &mut ctx, theme.canvas);
        ctx.flush_overlays(&mut buf);
    }
    (buf, hits)
}

fn screen_text(buf: &Buffer) -> String {
    (0..buf.area.height)
        .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
        .map(|(x, y)| buf[(x, y)].symbol().to_owned())
        .collect()
}

fn region(hits: &HitRegistry, id: WidgetId) -> Rect {
    hits.regions()
        .iter()
        .rev()
        .find(|r| r.id == id)
        .map(|r| r.area)
        .expect("a rendered hit region")
}

/// Routes a pointer press on the hit region `id` from the frame `hits`.
fn click(tabs: &mut Tabs, hits: &HitRegistry, id: WidgetId) -> (Outcome, Option<TabAction>) {
    let area = region(hits, id);
    let ev = PointerEvent::new(
        hits,
        Phase::Click,
        id,
        Position::new(area.x, area.y),
        1,
        KeyModifiers::NONE,
    );
    match tabs.route(&ev) {
        Some(response) => {
            let (outcome, actions) = response.into_parts();
            (outcome, actions.into_iter().next())
        }
        None => (Outcome::Ignored, None),
    }
}

/// The first row of the open overflow menu, as the frame drew it.
fn first_menu_row(tabs: &Tabs, hits: &HitRegistry) -> Option<WidgetId> {
    let menu = tabs.menu_id();
    hits.regions()
        .iter()
        .find(|r| r.owner == menu && r.id != menu && r.id != backdrop_id(menu))
        .map(|r| r.id)
}

#[test]
fn issue44_overflow_remove_before_closes_snapshot() {
    let mut labels = names("Tab", 10);
    let mut tabs = strip(&labels);
    render(&mut tabs);

    tabs.on_key(&key(KeyCode::Down));
    assert!(tabs.menu_open(), "Down opens the hidden-tabs menu");

    tabs.remove(0);
    labels.remove(0);
    assert!(
        !tabs.menu_open(),
        "removing a tab closes the menu that was built from the old positions"
    );
    render(&mut tabs);

    let (_, action) = tabs.on_key(&key(KeyCode::Enter));
    let TabAction::Activated(i) = action.expect("Enter on the strip activates a tab") else {
        panic!("expected an activation");
    };
    assert_eq!(
        labels[i], "Tab 1",
        "Enter activates the strip cursor, not the tab the old menu row named"
    );
}

#[test]
fn issue44_overflow_replace_empty_and_same_length() {
    let old = names("Tab", 10);
    let mut tabs = strip(&old);
    render(&mut tabs);
    tabs.on_key(&key(KeyCode::Down));
    let (_, open_hits) = render(&mut tabs);
    let row = first_menu_row(&tabs, &open_hits).expect("an open menu row");

    tabs.set_items(vec![]);
    assert!(!tabs.menu_open(), "an emptied strip closes its menu");
    let (_, stale) = click(&mut tabs, &open_hits, row);
    assert_eq!(stale, None, "a stale menu row cannot activate anything");
    render(&mut tabs);
    assert_eq!(
        tabs.on_key(&key(KeyCode::Enter)).1,
        None,
        "an empty strip emits no activation"
    );

    let replacement = names("Other", 10);
    let mut tabs = strip(&old);
    render(&mut tabs);
    tabs.on_key(&key(KeyCode::Down));
    let (_, open_hits) = render(&mut tabs);
    let row = first_menu_row(&tabs, &open_hits).expect("an open menu row");

    tabs.set_items(replacement.iter().map(|n| TabItem::new(n)).collect());
    assert!(
        !tabs.menu_open(),
        "a same-length replacement closes the menu"
    );
    let (_, stale) = click(&mut tabs, &open_hits, row);
    assert_eq!(
        stale, None,
        "a stale row cannot activate a replacement tab at the old position"
    );
}

#[test]
fn issue44_item_mutation_noop_keeps_menu() {
    let mut labels = names("Tab", 10);
    let mut tabs = strip(&labels);
    render(&mut tabs);
    tabs.on_key(&key(KeyCode::Down));
    assert!(tabs.menu_open());

    tabs.set_items(labels.iter().map(|n| TabItem::new(n)).collect());
    assert!(tabs.menu_open(), "an equal replacement keeps the menu");
    tabs.remove(99);
    assert!(
        tabs.menu_open(),
        "removing an out-of-range tab keeps the menu"
    );
    assert!(
        !tabs.update_item(0, |_| {}),
        "an unchanged metadata edit reports no change"
    );
    assert!(
        tabs.menu_open(),
        "an unchanged metadata edit keeps the menu"
    );

    assert!(tabs.update_item(5, |it| it.label = "Renamed 5".into()));
    assert!(!tabs.menu_open(), "a label change closes the menu");
    labels[5] = "Renamed 5".into();

    tabs.on_key(&key(KeyCode::Down));
    let (buf, _) = render(&mut tabs);
    assert!(
        screen_text(&buf).contains("Renamed 5"),
        "the reopened menu shows the current label"
    );
}

#[test]
fn issue44_overflow_hit_regions_reconcile() {
    let mut labels = names("Tab", 10);
    let mut tabs = strip(&labels);
    let (_, hits) = render(&mut tabs);

    let more = tabs.more_id();
    let (out, _) = click(&mut tabs, &hits, more);
    assert_eq!(out, Outcome::Changed, "a click on More opens the menu");
    let (_, open_hits) = render(&mut tabs);
    assert!(first_menu_row(&tabs, &open_hits).is_some());

    tabs.remove(0);
    labels.remove(0);
    let (_, hits) = render(&mut tabs);
    assert!(
        hits.regions().iter().all(|r| r.owner != tabs.menu_id()),
        "no stale menu row or backdrop region survives the mutation"
    );
    assert!(
        hits.regions()
            .iter()
            .all(|r| r.id != backdrop_id(tabs.menu_id())),
        "no stale backdrop region survives the mutation"
    );

    click(&mut tabs, &hits, more);
    let (buf, open_hits) = render(&mut tabs);
    let row = first_menu_row(&tabs, &open_hits).expect("reopened menu row");
    let (_, action) = click(&mut tabs, &open_hits, row);
    let TabAction::Activated(i) = action.expect("the reopened row activates") else {
        panic!("expected an activation");
    };
    assert!(
        screen_text(&buf).contains(&labels[i]),
        "the activated tab is the one the reopened menu shows"
    );
}

#[test]
fn issue44_stale_more_click_does_not_open() {
    let labels = names("Tab", 10);
    let mut tabs = strip(&labels);
    let (_, hits) = render(&mut tabs);

    tabs.set_items(names("Other", 10).iter().map(|n| TabItem::new(n)).collect());
    let more = tabs.more_id();
    let (out, _) = click(&mut tabs, &hits, more);
    assert_eq!(out, Outcome::Ignored);
    assert!(
        !tabs.menu_open(),
        "a More click from the previous frame does not open the menu"
    );
}

#[test]
fn issue44_panicking_item_update_is_atomic() {
    let labels = names("Tab", 10);
    let mut tabs = strip(&labels);
    render(&mut tabs);
    tabs.on_key(&key(KeyCode::Down));
    let (before_items, before_areas) = (tabs.items().to_vec(), tabs.areas.clone());
    let before_active = (tabs.active, tabs.cursor);

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        tabs.update_item(0, |it| {
            it.busy = true;
            panic!("edit failed");
        })
    }));
    assert!(outcome.is_err());

    assert_eq!(tabs.items(), before_items.as_slice());
    assert_eq!(tabs.areas, before_areas);
    assert_eq!((tabs.active, tabs.cursor), before_active);
    assert!(tabs.menu_open(), "a failed edit leaves the menu alone");
}

#[test]
fn issue44_reorder_moves_active_with_its_tab() {
    let labels = names("Tab", 6);
    let mut tabs = strip(&labels);
    render(&mut tabs);
    tabs.set_active(2);

    assert!(!tabs.reorder(2, 2), "a same-position move is a no-op");
    assert!(!tabs.reorder(2, 9), "an out-of-range move is a no-op");
    assert!(tabs.reorder(2, 0));
    assert_eq!(tabs.items()[0].label, "Tab 2");
    tabs.on_key(&key(KeyCode::Enter));
    assert_eq!(tabs.items()[tabs.active].label, "Tab 2");
}
