//! Lane `collections` of the #14 burn-down: this lane's callable scenarios.
//!
//! Each scenario builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. Register one `ScenarioCase` per
//! `ScenarioId` variant from the `lane:collections` block in `../coverage.rs`. Edit only
//! this file, `../coverage/lane_collections.rs` and `../coverage_debt/collections.txt`.

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::{Part, WidgetId};
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::{Actions as _, KeyBinding, NavKeys};
use junie_tui::core::list::{TypeAheadPrecedence, route_key};
use junie_tui::core::pointer::MouseTarget as _;
use junie_tui::core::pointer::PointerEvent;
use junie_tui::core::refusal::{Control, Reason, clear_refusal};
use junie_tui::core::response::Response;
use junie_tui::core::scroll::TrackClick;
use junie_tui::core::selection::{ClickPolicy, SelectGesture};
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::theme::{InactiveSelection, Tone};
use junie_tui::ui::content::ContentState;
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::reveal::Reveal;
use junie_tui::widgets::empty::EmptyState;
use junie_tui::widgets::icon_button::IconButton;
use junie_tui::widgets::list::{ListBox, ListCmd, ListItem, SelectMode};
use junie_tui::widgets::nav_list::{NavAction, NavItem, NavList};
use junie_tui::widgets::tabs::{
    CloseReveal, DEFAULT_CLOSE_KEYS, TabAction, TabCmd, TabItem, TabRule, Tabs,
};
use junie_tui::widgets::tree::{
    BranchAction, FlatRow, Path, TreeAction, TreeCmd, TreeNode, TreeView,
};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};

use super::{ScenarioCase, key, text_of};
use crate::coverage::ScenarioId;

#[rustfmt::skip]
pub const CASES: &[ScenarioCase] = &[
    ScenarioCase { id: ScenarioId::CollectionsTabsLook, run: tabs_look },
    ScenarioCase { id: ScenarioId::CollectionsTabsInput, run: tabs_input },
    ScenarioCase { id: ScenarioId::CollectionsTabsOverflow, run: tabs_overflow },
    ScenarioCase { id: ScenarioId::CollectionsNavList, run: nav_list },
    ScenarioCase { id: ScenarioId::CollectionsListSingle, run: list_single },
    ScenarioCase { id: ScenarioId::CollectionsListMulti, run: list_multi },
    ScenarioCase { id: ScenarioId::CollectionsListOptions, run: list_options },
    ScenarioCase { id: ScenarioId::CollectionsTreeStructure, run: tree_structure },
    ScenarioCase { id: ScenarioId::CollectionsTreeInput, run: tree_input },
    ScenarioCase { id: ScenarioId::CollectionsTreeOptions, run: tree_options },
];

fn theme() -> Theme {
    Theme::for_level(ColorLevel::TrueColor)
}

fn ch(code: char) -> Key {
    key(KeyCode::Char(code))
}

/// One frame under an explicit interaction state (focus, hover) with the hit
/// regions and focus stops it registered.
struct Shot {
    buf: Buffer,
    hits: HitRegistry,
    ring: FocusRing,
}

impl Shot {
    fn event(&self, phase: Phase, owner: WidgetId, part: Part, at: Position) -> PointerEvent {
        let id = self
            .hits
            .regions()
            .iter()
            .rev()
            .find(|r| r.owner == owner && r.part.part == part && r.area.contains(at))
            .map(|r| r.id)
            .unwrap_or_else(|| panic!("no {part:?} region of {owner:?} at {at:?}"));
        PointerEvent::new(&self.hits, phase, id, at, 1, KeyModifiers::NONE)
    }

    /// The first cell of `part` of `owner`, by registered region.
    fn region(&self, owner: WidgetId, part: Part, index: usize) -> Rect {
        self.hits
            .regions()
            .iter()
            .filter(|r| r.owner == owner && r.part.part == part)
            .nth(index)
            .map(|r| r.area)
            .unwrap_or_else(|| panic!("no {part:?} region #{index} of {owner:?}"))
    }
}

fn frame(
    area: Rect,
    interaction: Interaction,
    draw: impl FnOnce(Rect, &mut Buffer, &mut RenderCtx),
) -> Shot {
    let theme = theme();
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        draw(area, &mut buf, &mut ctx);
        ctx.flush_overlays(&mut buf);
    }
    Shot { buf, hits, ring }
}

fn focus_on(id: WidgetId) -> Interaction {
    Interaction {
        focus: Some(id),
        ..Interaction::default()
    }
}

fn draw_tabs(tabs: &mut Tabs, width: u16, interaction: Interaction) -> Shot {
    frame(Rect::new(0, 0, width, 3), interaction, |area, buf, ctx| {
        let bg = ctx.theme.canvas;
        tabs.render(area, buf, ctx, bg)
    })
}

/// What a keyboard or pointer call decided: the outcome and every action.
fn acts<A>(response: Response<A>) -> (Outcome, Vec<A>) {
    let (outcome, actions) = response.into_parts();
    (outcome, actions.into_iter().collect())
}

fn tabs_look() {
    let t = theme();
    let id = WidgetId::of("collections.tabs.look");
    let mut tabs = Tabs::with_items(
        id,
        vec![
            TabItem::new("Build")
                .prefix("#")
                .suffix("main")
                .closable()
                .dirty(true),
            TabItem::new("Test").busy(true).closable(),
            TabItem::new("Lint").error(true),
            TabItem::new("Docs").done(true),
            TabItem::new("Plain").dirty(false).done(false).error(false),
        ],
    );
    let rendered = draw_tabs(&mut tabs, 90, Interaction::default());
    let text = text_of(&rendered.buf);
    let strip = text.lines().next().expect("a strip row").to_owned();
    assert!(strip.contains("# Build main"), "prefix and suffix:\n{text}");
    assert!(strip.contains(t.glyphs.dirty), "dirty mark:\n{text}");
    assert!(strip.contains(t.glyphs.spinner_frame(0)), "busy spinner");
    assert!(strip.contains("Lint !"), "error mark:\n{text}");
    assert!(strip.contains(t.glyphs.done), "done mark:\n{text}");
    assert_eq!(
        strip.matches(t.glyphs.close).count(),
        2,
        "only the closable tabs show a close button:\n{text}"
    );
    assert!(!tabs.items()[4].dirty && !tabs.items()[4].done && !tabs.items()[4].error);
    assert!(tabs.items()[0].closable && !tabs.items()[1].error);

    assert_eq!(Tabs::STRIP_H, 2);
    let area = Rect::new(2, 1, 40, 12);
    let (strip_area, body) = Tabs::layout(area);
    assert_eq!((strip_area.height, body.y, body.height), (2, 4, 9));
    let (_, flush) = Tabs::layout_gap(area, 0);
    assert_eq!((flush.y, flush.height), (3, 10), "no gap under the strip");
    let (_, roomy) = Tabs::layout_gap(area, 3);
    assert_eq!(roomy.y, 6, "a wider gap pushes the body down");

    let accent_rule = |rule: TabRule, interaction: Interaction| {
        let mut tabs = Tabs::new(id, &["one", "two"]).rule(rule);
        let p = draw_tabs(&mut tabs, 30, interaction);
        let x = tabs.areas[tabs.active].x;
        assert_eq!(p.buf[(x, 1)].symbol(), t.glyphs.bar_fill);
        p.buf[(x, 1)].fg
    };
    let focused = || focus_on(id);
    assert_eq!(accent_rule(TabRule::Auto, Interaction::default()), t.accent);
    assert_eq!(
        accent_rule(TabRule::Accent, Interaction::default()),
        t.accent
    );
    assert_eq!(
        accent_rule(TabRule::Neutral, focused()),
        t.border_strong,
        "a neutral rule stays grey even while focused"
    );
    assert_eq!(
        accent_rule(TabRule::FocusOnly, Interaction::default()),
        t.border_strong
    );
    assert_eq!(accent_rule(TabRule::FocusOnly, focused()), t.accent);

    let gutter = t.glyphs.focus_bar;
    let mut live = Tabs::new(id, &["one", "two"]);
    let p = draw_tabs(&mut live, 30, focused());
    assert_eq!(
        p.buf[(live.areas[0].x, 0)].symbol(),
        gutter,
        "focused gutter"
    );
    let mut passive = Tabs::new(id, &["one", "two"]).passive(true);
    let p = draw_tabs(&mut passive, 30, focused());
    assert!(
        !text_of(&p.buf).contains(gutter),
        "a passive strip ignores the live focus"
    );
    assert!(p.ring.is_empty(), "a passive strip is no focus stop");
    let mut live_stop = Tabs::new(id, &["one", "two"]);
    assert!(
        !draw_tabs(&mut live_stop, 30, Interaction::default())
            .ring
            .is_empty()
    );
    let mut borrowed = Tabs::new(id, &["one", "two"])
        .passive(true)
        .passive_focused(true);
    let p = draw_tabs(&mut borrowed, 30, Interaction::default());
    assert_eq!(
        p.buf[(borrowed.areas[0].x, 0)].symbol(),
        gutter,
        "the host draws the focus on a passive strip"
    );

    let strip_of = |reveal: CloseReveal, interaction: Interaction| {
        let mut tabs = Tabs::with_items(
            id,
            vec![
                TabItem::new("one").closable(),
                TabItem::new("two").closable(),
            ],
        )
        .close_reveal(reveal);
        let p = draw_tabs(&mut tabs, 30, interaction);
        let strip = text_of(&p.buf).lines().next().unwrap().to_owned();
        strip.matches(t.glyphs.close).count()
    };
    assert_eq!(strip_of(CloseReveal::Always, Interaction::default()), 2);
    assert_eq!(
        strip_of(CloseReveal::Hover, Interaction::default()),
        1,
        "hover reveal shows the close button on the active tab only"
    );
    let probe = Tabs::new(id, &[]);
    let hover_second = Interaction {
        hover: Some(probe.tab_id(1)),
        ..Interaction::default()
    };
    assert_eq!(
        strip_of(CloseReveal::Hover, hover_second),
        2,
        "pointing at a tab reveals its close button"
    );

    let close_width = |tabs: Tabs| {
        let mut tabs = tabs;
        let p = draw_tabs(&mut tabs, 30, Interaction::default());
        p.hits
            .regions()
            .iter()
            .find(|r| r.owner == id && r.part.part == Part::CLOSE)
            .map(|r| r.area.width)
            .expect("a close hit region")
    };
    let closable = || Tabs::with_items(id, vec![TabItem::new("one").closable()]);
    let default_w = close_width(closable());
    assert_eq!(
        close_width(closable().close_target_w(default_w + 2)),
        default_w + 2,
        "a wider close target widens the hit plate"
    );
    let custom = IconButton::new(WidgetId::of("collections.tabs.x"), "x");
    let mut swapped = closable().close_button(custom);
    let p = draw_tabs(&mut swapped, 30, Interaction::default());
    assert!(
        text_of(&p.buf).lines().next().unwrap().contains('x')
            && !text_of(&p.buf).contains(t.glyphs.close),
        "a custom close button replaces the default glyph"
    );

    let overflowing = || Tabs::new(id, &["alpha", "beta", "gamma", "delta", "epsilon", "zeta"]);
    let more = overflowing().more_id();
    let hovered = Interaction {
        hover: Some(more),
        ..Interaction::default()
    };
    let more_style = |tabs: &mut Tabs| {
        let p = draw_tabs(tabs, 36, hovered);
        let area = p
            .hits
            .regions()
            .iter()
            .find(|r| r.id == more)
            .map(|r| r.area)
            .expect("the more button registers under its own id");
        p.buf[(area.x + 1, 0)].style()
    };
    let default_hover = more_style(&mut overflowing());
    let danger = IconButton::new(WidgetId::of("collections.tabs.more"), "+").danger_hover(true);
    assert_ne!(
        more_style(&mut overflowing().more_button(danger)),
        default_hover,
        "the replacement button brings its own hover look"
    );
}

fn tabs_input() {
    let id = WidgetId::of("collections.tabs.input");
    let items = || {
        vec![
            TabItem::new("one").closable(),
            TabItem::new("two"),
            TabItem::new("three").closable(),
        ]
    };
    let ready = |tabs: Tabs| {
        let mut tabs = tabs;
        draw_tabs(&mut tabs, 60, Interaction::default());
        tabs
    };
    let mut tabs = ready(Tabs::with_items(id, items()).allow_new(true));

    assert_eq!(
        tabs.on_key(&key(KeyCode::Right)),
        (Outcome::Changed, Some(TabAction::Activated(1)))
    );
    assert_eq!(tabs.active, 1);
    assert_eq!(
        tabs.on_key(&ch('3')),
        (Outcome::Changed, Some(TabAction::Activated(2))),
        "a digit jumps to that tab"
    );
    assert_eq!(tabs.on_key(&ch('9')), (Outcome::Ignored, None));
    assert_eq!(TabCmd::digit_keys().len(), 9);
    assert!(matches!(
        TabCmd::default_keymap()
            .iter()
            .find(|(b, _)| *b == KeyBinding::char('2')),
        Some((_, TabCmd::Jump(1)))
    ));
    assert_eq!(TabCmd::Jump(0).label(), "Jump to tab");
    assert_eq!(TabCmd::ShowHidden.label(), "Show hidden");

    assert_eq!(DEFAULT_CLOSE_KEYS, &[KeyCode::Char('x'), KeyCode::Delete]);
    for code in DEFAULT_CLOSE_KEYS {
        assert_eq!(
            tabs.on_key(&key(*code)),
            (Outcome::Changed, Some(TabAction::Close(2))),
            "{code:?} closes the focused closable tab"
        );
    }
    tabs.set_active(1);
    assert_eq!(
        tabs.on_key(&ch('x')),
        (Outcome::Ignored, None),
        "a tab that is not closable stays open"
    );
    assert_eq!(
        tabs.on_key(&ch('n')),
        (Outcome::Changed, Some(TabAction::New))
    );
    assert_eq!(TabCmd::Close.label(), "Close tab");
    assert_eq!(TabCmd::New.label(), "New tab");

    let mut closed = ready(Tabs::with_items(id, items()).allow_new(false));
    assert_eq!(closed.on_key(&ch('n')), (Outcome::Ignored, None));

    let mut plain = ready(Tabs::with_items(id, items()).digit_jump(false));
    assert_eq!(plain.on_key(&ch('2')), (Outcome::Ignored, None));
    assert_eq!(plain.active, 0, "digits no longer jump");
    let mut digits = ready(Tabs::with_items(id, items()).digit_jump(true));
    assert_eq!(digits.on_key(&ch('2')).1, Some(TabAction::Activated(1)));

    let mut remapped = ready(Tabs::with_items(id, items()).close_keys(&[KeyCode::Char('d')]));
    assert_eq!(remapped.on_key(&ch('x')), (Outcome::Ignored, None));
    assert_eq!(remapped.on_key(&ch('d')).1, Some(TabAction::Close(0)));

    let mut fresh = ready(
        Tabs::with_items(id, items())
            .allow_new(true)
            .new_key(Some('t')),
    );
    assert_eq!(fresh.on_key(&ch('n')), (Outcome::Ignored, None));
    assert_eq!(fresh.on_key(&ch('t')).1, Some(TabAction::New));
    let mut unbound = ready(Tabs::with_items(id, items()).allow_new(true).new_key(None));
    assert_eq!(unbound.on_key(&ch('n')), (Outcome::Ignored, None));

    let mut custom =
        ready(Tabs::with_items(id, items()).keymap(vec![(KeyBinding::char('q'), TabCmd::Close)]));
    assert_eq!(custom.on_key(&ch('q')).1, Some(TabAction::Close(0)));
    assert_eq!(custom.on_key(&ch('x')), (Outcome::Ignored, None));

    let mut wrapping = ready(Tabs::with_items(id, items()).wrap(true));
    assert_eq!(
        wrapping.on_key(&key(KeyCode::Left)).1,
        Some(TabAction::Activated(2))
    );
    let mut bounded = ready(Tabs::with_items(id, items()).wrap(false));
    assert_eq!(
        bounded.on_key(&key(KeyCode::Left)).1,
        Some(TabAction::Activated(0))
    );
    assert_eq!(bounded.active, 0, "without wrap the strip stops at its end");

    let mut vim = ready(Tabs::with_items(id, items()).nav(NavKeys::VIM));
    assert_eq!(vim.on_key(&ch('l')).1, Some(TabAction::Activated(1)));
    let mut arrows = ready(Tabs::with_items(id, items()));
    assert_eq!(arrows.on_key(&ch('l')), (Outcome::Ignored, None));

    let mut empty = ready(Tabs::new(id, &[]).allow_new(true));
    assert_eq!(
        empty.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(TabAction::New)),
        "an empty strip offers a new tab"
    );

    let mut clicks = ready(Tabs::with_items(id, items()).allow_new(true));
    assert_eq!(
        clicks.on_click(clicks.tab_id(1)),
        (Outcome::Changed, Some(TabAction::Activated(1)))
    );
    assert_eq!(
        clicks.on_click(clicks.close_id(2)),
        (Outcome::Changed, Some(TabAction::Close(2)))
    );
    assert_eq!(
        clicks.on_click(clicks.new_id()),
        (Outcome::Changed, Some(TabAction::New))
    );
    assert_eq!(
        clicks.on_middle_click(clicks.tab_id(2)),
        Some(TabAction::Close(2)),
        "a middle click closes a closable tab"
    );
    assert_eq!(clicks.on_middle_click(clicks.tab_id(1)), None);
    let mut no_middle = ready(Tabs::with_items(id, items()).middle_click_closes(false));
    assert_eq!(no_middle.on_middle_click(no_middle.tab_id(2)), None);
    let _ = no_middle.on_key(&ch('x'));

    let mut edited = ready(Tabs::with_items(id, items()));
    edited.set_active(2);
    edited.remove(0);
    assert_eq!(edited.items().len(), 2);
    assert_eq!(edited.active, 1, "the active tab follows the removal");
    edited.remove(9);
    assert_eq!(edited.items().len(), 2);
    edited.set_items(vec![TabItem::new("solo")]);
    assert_eq!((edited.items().len(), edited.active), (1, 0));
    assert_eq!(
        edited.on_click(edited.tab_id(0)),
        (Outcome::Ignored, None),
        "pointer hits wait for the next render after the collection changed"
    );
}

fn tabs_overflow() {
    let id = WidgetId::of("collections.tabs.overflow");
    let labels = ["alpha", "beta", "gamma", "delta", "epsilon", "zeta"];
    let mut tabs = Tabs::new(id, &labels);
    let p = draw_tabs(&mut tabs, 34, Interaction::default());
    let (before, after) = tabs.hidden();
    assert_eq!(before, 0);
    assert!(after > 0, "a narrow strip hides tabs:\n{}", text_of(&p.buf));
    assert_eq!(tabs.on_wheel(0, 0), Outcome::Ignored);
    assert_eq!(tabs.on_wheel(1, 0), Outcome::Changed);
    assert_eq!(tabs.first, 1);
    assert_eq!(
        tabs.on_wheel(0, -1),
        Outcome::Changed,
        "vertical wheel scrolls too"
    );
    assert_eq!(tabs.first, 0);
    assert_eq!(tabs.on_wheel(100, 0), Outcome::Changed);
    assert_eq!(tabs.on_wheel(1, 0), Outcome::Consumed, "already at the end");
    tabs.first = 0;

    let _ = draw_tabs(&mut tabs, 34, Interaction::default());
    assert_eq!(tabs.on_click(tabs.right_id()), (Outcome::Changed, None));
    assert_eq!(tabs.first, 1, "the right arrow scrolls one tab");
    let _ = draw_tabs(&mut tabs, 34, Interaction::default());
    assert_eq!(tabs.hidden().0, 1);
    assert_eq!(tabs.on_click(tabs.left_id()), (Outcome::Changed, None));
    assert_eq!(tabs.first, 0, "the left arrow scrolls back");

    let p = draw_tabs(&mut tabs, 34, Interaction::default());
    assert!(!tabs.menu_open());
    assert!(p.hits.regions().iter().any(|r| r.id == tabs.more_id()));
    assert_eq!(tabs.on_click(tabs.more_id()), (Outcome::Changed, None));
    assert!(tabs.menu_open(), "the +N button opens the overflow menu");
    let p = draw_tabs(&mut tabs, 34, Interaction::default());
    assert!(
        p.hits
            .regions()
            .iter()
            .any(|r| r.owner == tabs.menu_id() || r.id == tabs.menu_id()),
        "the menu registers under its own id"
    );
    let (_, chosen) = tabs.on_key(&key(KeyCode::Enter));
    let Some(TabAction::Activated(index)) = chosen else {
        panic!("choosing a hidden tab activates it: {chosen:?}");
    };
    assert!(index >= 2 && index == tabs.active && !tabs.menu_open());

    let _ = draw_tabs(&mut tabs, 34, Interaction::default());
    assert_eq!(tabs.on_key(&key(KeyCode::Down)).0, Outcome::Changed);
    assert!(
        tabs.menu_open(),
        "ShowHidden opens the menu from the keyboard"
    );
    let _ = tabs.on_key(&key(KeyCode::Esc));
    assert!(!tabs.menu_open(), "Esc dismisses the menu");

    let mut flat = Tabs::new(id, &labels).overflow_menu(false);
    let p = draw_tabs(&mut flat, 34, Interaction::default());
    assert!(p.hits.regions().iter().all(|r| r.id != flat.more_id()));
    let _ = flat.on_key(&key(KeyCode::Down));
    assert!(!flat.menu_open(), "no overflow menu to open");
}

/// Where `needle` first appears in the frame, as a cell position.
fn locate(buf: &Buffer, needle: &str) -> Option<Position> {
    for y in 0..buf.area.height {
        let mut line = String::new();
        let mut cols = Vec::new();
        for x in 0..buf.area.width {
            cols.push((line.len(), x));
            line.push_str(buf[(x, y)].symbol());
        }
        if let Some(at) = line.find(needle) {
            let x = cols.iter().find(|(byte, _)| *byte == at)?.1;
            return Some(Position::new(x, y));
        }
    }
    None
}

fn at(buf: &Buffer, needle: &str) -> Position {
    locate(buf, needle).unwrap_or_else(|| panic!("{needle:?} is not on screen:\n{}", text_of(buf)))
}

fn press(code: char) -> Key {
    ch(code)
}

// ---------------------------------------------------------------- NavList

fn nav_items() -> Vec<NavItem> {
    vec![
        NavItem::new("Home").icon("H").section("Main"),
        NavItem::new("Inbox").icon("I").section("Main").badge("3"),
        NavItem::new("Archive")
            .icon("A")
            .section("Main")
            .disabled(true),
        NavItem::new("Settings").icon("S").section("System"),
    ]
}

fn draw_nav(nav: &mut NavList, width: u16, height: u16, interaction: Interaction) -> Shot {
    frame(
        Rect::new(0, 0, width, height),
        interaction,
        |area, buf, ctx| {
            let bg = ctx.theme.canvas;
            nav.render(area, buf, ctx, bg)
        },
    )
}

fn nav_list() {
    let id = WidgetId::of("collections.nav");
    let mut nav = NavList::new(id, nav_items());
    assert_eq!((nav.current, nav.cursor), (0, 0));
    assert_eq!(nav.width(), 24, "the open rail is 24 cells by default");
    let shot = draw_nav(&mut nav, 24, 8, Interaction::default());
    let text = text_of(&shot.buf);
    for want in ["Main", "System", "Home", "Inbox", "Archive", "Settings"] {
        assert!(text.contains(want), "{want}:\n{text}");
    }
    let badge = at(&shot.buf, "3");
    assert_eq!(
        badge.y,
        at(&shot.buf, "Inbox").y,
        "the badge sits on its row"
    );
    assert!(shot.hits.regions().iter().any(|r| r.id == nav.item_id(1)));
    assert!(
        shot.hits.regions().iter().all(|r| r.id != nav.item_id(2)),
        "a disabled item is not clickable"
    );

    assert_eq!(nav.on_key(&key(KeyCode::Down)), (Outcome::Changed, None));
    assert_eq!(nav.cursor, 1);
    nav.on_key(&key(KeyCode::Down));
    assert_eq!(nav.cursor, 3, "the cursor skips the disabled item");
    assert_eq!(nav.on_key(&key(KeyCode::Home)), (Outcome::Changed, None));
    assert_eq!(nav.cursor, 0);
    assert_eq!(nav.on_key(&key(KeyCode::End)).0, Outcome::Changed);
    assert_eq!(nav.cursor, 3);
    assert_eq!(nav.on_key(&key(KeyCode::End)).0, Outcome::Consumed);
    assert_eq!(nav.on_key(&key(KeyCode::Left)), (Outcome::Ignored, None));
    assert_eq!(
        nav.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(NavAction::Activated(3)))
    );
    assert_eq!(nav.current, 3);
    assert_eq!(
        nav.on_click(1),
        (Outcome::Changed, Some(NavAction::Activated(1)))
    );
    assert_eq!(nav.on_click(2), (Outcome::Changed, None), "disabled");
    assert_eq!((nav.current, nav.cursor), (1, 1));
    assert_eq!(nav.on_key(&press('j')), (Outcome::Ignored, None));
    let mut vim = NavList::new(id, nav_items()).nav(NavKeys::VIM);
    assert_eq!(vim.on_key(&press('j')), (Outcome::Changed, None));
    assert_eq!(vim.cursor, 1);

    let mut picked = NavList::new(id, nav_items()).current(1);
    assert_eq!((picked.current, picked.cursor), (1, 1));
    let _ = draw_nav(&mut picked, 24, 8, Interaction::default());

    let mut swapped = NavList::new(id, nav_items());
    assert!(
        !swapped.set_items(nav_items()),
        "an equal list changes nothing"
    );
    assert!(swapped.set_items(vec![NavItem::new("Only")]));
    assert_eq!(swapped.items.len(), 1);
    let mut filled = NavList::new(id, Vec::new());
    assert!(filled.set_items(vec![NavItem::new("off").disabled(true), NavItem::new("on")]));
    assert_eq!(filled.cursor, 1, "filling starts on the first enabled item");

    let wide = NavList::new(id, nav_items()).widths(30, 8);
    assert_eq!(wide.width(), 30);
    let slim = NavList::new(id, nav_items()).widths(30, 8).collapsed(true);
    assert!(slim.collapsed);
    assert_eq!(slim.width(), 8, "collapsed uses the narrow width");
    let mut slim = slim;
    let shot = draw_nav(&mut slim, 8, 8, Interaction::default());
    let text = text_of(&shot.buf);
    assert!(text.contains("····"), "sections shrink to a rule:\n{text}");
    assert!(!text.contains("Inbox"), "labels are hidden when collapsed");
    assert!(text.contains('I'), "icons stay");

    let current_bg = |look: InactiveSelection| {
        let mut nav = NavList::new(id, nav_items()).inactive_selection(look);
        let shot = draw_nav(&mut nav, 24, 8, Interaction::default());
        let home = at(&shot.buf, "Home");
        shot.buf[(home.x + 8, home.y)].bg
    };
    let t = theme();
    assert_eq!(current_bg(InactiveSelection::Accent), t.accent_bg);
    assert_eq!(current_bg(InactiveSelection::Neutral), t.selection_inactive);
}

// ---------------------------------------------------------------- ListBox

fn list_items() -> Vec<ListItem> {
    vec![
        ListItem::new("alpha").meta("first"),
        ListItem::new("beta").meta("ok").meta_tone(Tone::Success),
        ListItem::new("gamma").disabled(true),
        ListItem::new("delta").fixed(Reason::new("locked by policy")),
        ListItem::new("epsilon").control(Control::Live),
    ]
}

fn draw_list(list: &mut ListBox, width: u16, height: u16, interaction: Interaction) -> Shot {
    frame(
        Rect::new(0, 0, width, height),
        interaction,
        |area, buf, ctx| {
            let bg = ctx.theme.canvas;
            list.render(area, buf, ctx, bg)
        },
    )
}

fn list_single() {
    let id = WidgetId::of("collections.list.single");
    let mut list = ListBox::new(id, list_items(), SelectMode::Single);
    assert_eq!(list.mode, SelectMode::Single);
    let shot = draw_list(&mut list, 44, 6, Interaction::default());
    let text = text_of(&shot.buf);
    for want in ["alpha", "first", "beta", "ok", "gamma", "delta", "epsilon"] {
        assert!(text.contains(want), "{want}:\n{text}");
    }
    let (plain, toned) = (at(&shot.buf, "first"), at(&shot.buf, "ok"));
    assert_ne!(
        shot.buf[(plain.x, plain.y)].fg,
        shot.buf[(toned.x, toned.y)].fg,
        "a toned meta column is not the muted one"
    );
    assert!(shot.hits.regions().iter().any(|r| r.id == list.row_id(1)));

    assert_eq!(list.on_key(&key(KeyCode::Down)), Outcome::Changed);
    assert_eq!(list.cursor, 1);
    list.on_key(&key(KeyCode::Down));
    assert_eq!(list.cursor, 3, "the cursor skips the disabled row");
    list.on_key(&key(KeyCode::Up));
    assert_eq!(list.cursor, 1);
    assert_eq!(list.on_key(&key(KeyCode::Enter)), Outcome::Changed);
    assert_eq!(list.chosen, Some(1));
    assert_eq!(
        list.activate(2),
        Outcome::Consumed,
        "a disabled row stays put"
    );
    assert_eq!(list.activate(99), Outcome::Consumed);
    assert_eq!(list.chosen, Some(1));
    assert_eq!(list.activate(3), Outcome::Consumed, "a fixed row refuses");
    clear_refusal();
    assert_eq!(list.chosen, Some(1));
    assert_eq!(list.activate(4), Outcome::Changed);
    assert_eq!(list.chosen, Some(4));

    let mut every_row = ListBox::new(id, list_items(), SelectMode::Single).skip_disabled(false);
    every_row.on_key(&key(KeyCode::Down));
    every_row.on_key(&key(KeyCode::Down));
    assert_eq!(
        every_row.cursor, 2,
        "without skipping, the disabled row is reachable"
    );

    let mut wrapping = ListBox::new(id, list_items(), SelectMode::Single).wrap(true);
    wrapping.on_key(&key(KeyCode::Up));
    assert_eq!(
        wrapping.cursor, 4,
        "wrap goes from the first row to the last"
    );
    let mut bounded = ListBox::new(id, list_items(), SelectMode::Single).wrap(false);
    bounded.on_key(&key(KeyCode::Up));
    assert_eq!(bounded.cursor, 0);

    let mut vim = ListBox::new(id, list_items(), SelectMode::Single)
        .nav(NavKeys::VIM)
        .type_ahead(false);
    let _ = vim.on_key(&press('j'));
    assert_eq!(vim.cursor, 1, "the vim profile adds j");

    let mut slow = ListBox::new(id, list_items(), SelectMode::Single).click(ClickPolicy::DATA);
    assert_eq!(slow.on_click(1), Outcome::Changed);
    assert_eq!(
        (slow.cursor, slow.chosen),
        (1, None),
        "one click only moves"
    );
    assert_eq!(slow.on_click_count(1, 2), Outcome::Changed);
    assert_eq!(slow.chosen, Some(1), "a double click chooses");
    let mut quick =
        ListBox::new(id, list_items(), SelectMode::Single).click(ClickPolicy::NAVIGATION);
    quick.on_click_at(4, 1, KeyModifiers::NONE, false);
    assert_eq!(
        quick.chosen,
        Some(4),
        "navigation lists choose on one click"
    );
    assert_eq!(
        quick.on_click(40),
        Outcome::Consumed,
        "a click past the end"
    );

    let mut pointed =
        ListBox::new(id, list_items(), SelectMode::Single).click(ClickPolicy::NAVIGATION);
    let shot = draw_list(&mut pointed, 44, 6, Interaction::default());
    let beta = at(&shot.buf, "beta");
    let click = shot.event(Phase::Click, id, Part::ROW, beta);
    let (outcome, _) = acts(pointed.pointer(&click));
    assert_eq!((outcome, pointed.chosen), (Outcome::Changed, Some(1)));

    let mut removing = ListBox::new(id, list_items(), SelectMode::Single);
    removing.activate(4);
    removing.cursor = 4;
    assert_eq!(removing.remove_rows(&[0, 40]), 1);
    assert_eq!(removing.items.len(), 4);
    assert_eq!(
        removing.chosen,
        Some(3),
        "the chosen row follows the removal"
    );
    assert_eq!(removing.cursor, 3);
    assert_eq!(removing.remove_rows(&[40]), 0);
}

fn list_multi() {
    let id = WidgetId::of("collections.list.multi");
    let mut list = ListBox::new(id, list_items(), SelectMode::Single).mode(SelectMode::Multi);
    assert_eq!(list.mode, SelectMode::Multi);
    assert_eq!(list.checked_count(), 0);
    list.set_checked(0, true);
    list.set_checked(40, true);
    assert!(list.is_checked(0) && !list.is_checked(1));
    assert_eq!(list.checked_count(), 1);
    list.set_checked(0, false);
    assert_eq!(list.checked_count(), 0);

    assert_eq!(list.on_key(&press('a')), Outcome::Changed);
    assert_eq!(
        list.checked_count(),
        3,
        "ToggleAll checks every row that can be checked"
    );
    assert!(
        !list.is_checked(2) && !list.is_checked(3),
        "disabled and fixed rows stay out"
    );
    list.on_key(&press('a'));
    assert_eq!(list.checked_count(), 0);
    let ctrl_a = Key {
        code: KeyCode::Char('a'),
        mods: KeyModifiers::CONTROL,
    };
    assert_eq!(list.on_key(&ctrl_a), Outcome::Changed);
    assert_eq!(list.checked_count(), 3, "SelectAll checks them too");
    assert_eq!(ListCmd::SelectAll.label(), "Select all");
    assert_eq!(ListCmd::ToggleAll.label(), "Toggle all");

    let mut single = ListBox::new(id, list_items(), SelectMode::Single);
    assert_eq!(
        single.on_key(&ctrl_a),
        Outcome::Ignored,
        "no select-all in a single list"
    );

    let mut remapped = ListBox::new(id, list_items(), SelectMode::Multi)
        .keymap(vec![(KeyBinding::char('t'), ListCmd::ToggleAll)]);
    assert_eq!(remapped.on_key(&press('t')), Outcome::Changed);
    assert_eq!(remapped.checked_count(), 3);
    remapped.on_key(&press('a'));
    assert_eq!(
        remapped.checked_count(),
        3,
        "'a' is type-ahead now, not ToggleAll"
    );

    let mut gestures = ListBox::new(id, list_items(), SelectMode::Multi);
    assert!(gestures.select(SelectGesture::Only, 1));
    assert!(gestures.is_checked(1) && gestures.checked_count() == 1);
    assert!(gestures.select(SelectGesture::Toggle, 4));
    assert_eq!(gestures.checked_count(), 2);
    assert!(gestures.select(SelectGesture::All, 0));
    assert_eq!(gestures.checked_count(), 3);
    assert!(gestures.select(SelectGesture::Only, 4));
    assert!(gestures.select(SelectGesture::Extend { from: 4 }, 0));
    assert!(
        gestures.is_checked(0) && gestures.is_checked(1) && gestures.is_checked(4),
        "a range selects every row it can between the ends"
    );
    assert!(!gestures.is_checked(2), "except the disabled one");

    let toggle = SelectGesture::toggle_mods();
    let mut clicks = ListBox::new(id, list_items(), SelectMode::Multi);
    assert_eq!(clicks.on_click(0), Outcome::Changed);
    assert!(
        clicks.is_checked(0) && clicks.checked_count() == 1,
        "a plain click selects one row"
    );
    clicks.on_click_at(4, 1, toggle, false);
    assert_eq!(clicks.checked_count(), 2, "the toggle modifier adds a row");
    clicks.on_click_at(4, 1, KeyModifiers::NONE, true);
    assert!(!clicks.is_checked(4), "a click on the box toggles it");
    let mut alt = ListBox::new(id, list_items(), SelectMode::Multi).toggle_mods(KeyModifiers::ALT);
    alt.on_click(0);
    alt.on_click_at(1, 1, KeyModifiers::ALT, false);
    assert_eq!((alt.is_checked(0), alt.is_checked(1)), (true, true));
    let mut gated = ListBox::new(id, list_items(), SelectMode::Multi);
    assert_eq!(
        gated.on_click_at(3, 1, KeyModifiers::NONE, true),
        Outcome::Changed
    );
    clear_refusal();
    assert!(!gated.is_checked(3), "a fixed row refuses the check");

    let shot = draw_list(&mut list, 44, 6, Interaction::default());
    assert!(shot.hits.regions().iter().any(|r| r.id == list.mark_id(1)));
    let theme = theme();
    let row = at(&shot.buf, "alpha").y;
    let marks: usize = (0..6)
        .map(|x| shot.buf[(x, row)].symbol().trim().len())
        .sum();
    assert!(
        marks > 0,
        "a checked row shows its mark:\n{}",
        text_of(&shot.buf)
    );
    let _ = theme;
}

fn list_options() {
    let id = WidgetId::of("collections.list.options");
    let t = theme();

    let slow_ahead = |timeout: std::time::Duration| {
        let items = ["alpha", "beta", "bravo", "brown"]
            .map(|label| ListItem::new(label))
            .to_vec();
        let mut list = ListBox::new(id, items, SelectMode::Single)
            .type_ahead(true)
            .type_ahead_timeout(timeout);
        list.on_key(&press('b'));
        assert_eq!(list.cursor, 1);
        std::thread::sleep(std::time::Duration::from_millis(30));
        list.on_key(&press('r'));
        list.cursor
    };
    assert_eq!(
        slow_ahead(std::time::Duration::from_secs(60)),
        2,
        "b then r finds bravo"
    );
    assert_eq!(
        slow_ahead(std::time::Duration::from_millis(1)),
        1,
        "a slow second letter starts a new search"
    );
    let mut off = ListBox::new(id, list_items(), SelectMode::Single).type_ahead(false);
    assert_eq!(off.on_key(&press('e')), Outcome::Ignored);
    let mut on = ListBox::new(id, list_items(), SelectMode::Single).type_ahead(true);
    assert_eq!(on.on_key(&press('e')), Outcome::Changed);
    assert_eq!(on.cursor, 4);

    let vim_items = || {
        ["alpha", "beta", "gamma", "juliet"]
            .map(|label| ListItem::new(label))
            .to_vec()
    };
    let routed = |precedence: TypeAheadPrecedence| {
        let mut list = ListBox::new(id, vim_items(), SelectMode::Single)
            .nav(NavKeys::VIM)
            .type_ahead(true)
            .type_ahead_precedence(precedence);
        let k = press('j');
        route_key(&k, || list.on_key(&k));
        list.cursor
    };
    assert_eq!(
        routed(TypeAheadPrecedence::Shortcuts),
        1,
        "j navigates first"
    );
    assert_eq!(routed(TypeAheadPrecedence::TypeAhead), 3, "j types first");

    let mut empty = ListBox::new(id, Vec::new(), SelectMode::Single);
    let shot = draw_list(&mut empty, 30, 3, Interaction::default());
    assert!(text_of(&shot.buf).contains(t.strings.empty_list));
    assert!(shot.ring.is_empty(), "an empty list is skipped by Tab");
    let mut stop = ListBox::new(id, Vec::new(), SelectMode::Single).skip_when_empty(false);
    assert_eq!(
        draw_list(&mut stop, 30, 3, Interaction::default())
            .ring
            .len(),
        1
    );
    let mut says = ListBox::new(id, Vec::new(), SelectMode::Single).empty_text("Nothing yet");
    assert!(
        text_of(&draw_list(&mut says, 30, 3, Interaction::default()).buf).contains("Nothing yet")
    );
    let mut titled =
        ListBox::new(id, Vec::new(), SelectMode::Single).empty(EmptyState::new("No results"));
    assert!(
        text_of(&draw_list(&mut titled, 30, 3, Interaction::default()).buf).contains("No results")
    );
    let mut loading = ListBox::new(id, list_items(), SelectMode::Single)
        .content(ContentState::loading("Fetching rows"));
    let text = text_of(&draw_list(&mut loading, 30, 3, Interaction::default()).buf);
    assert!(
        text.contains("Fetching rows") && !text.contains("alpha"),
        "{text}"
    );

    let chosen_bg = |look: InactiveSelection| {
        let mut list = ListBox::new(id, list_items(), SelectMode::Single).inactive_selection(look);
        list.activate(4);
        let shot = draw_list(&mut list, 44, 6, Interaction::default());
        let row = at(&shot.buf, "epsilon");
        shot.buf[(row.x + 12, row.y)].bg
    };
    assert_eq!(chosen_bg(InactiveSelection::Accent), t.accent_bg);
    assert_eq!(chosen_bg(InactiveSelection::Neutral), t.selection_inactive);
    assert_ne!(chosen_bg(InactiveSelection::None), t.accent_bg);

    let meta_x = |gap: u16| {
        let mut list = ListBox::new(id, list_items(), SelectMode::Single).scrollbar_gap(gap);
        let shot = draw_list(&mut list, 44, 3, Interaction::default());
        at(&shot.buf, "first").x
    };
    assert_ne!(meta_x(0), meta_x(3), "the scrollbar gap narrows the rows");

    let cut = |reveal: Reveal| {
        let mut list = ListBox::new(
            id,
            vec![ListItem::new("a label far too long to fit in this row")],
            SelectMode::Single,
        )
        .reveal(reveal);
        let _ = draw_list(&mut list, 14, 3, focus_on(id));
        list.reveal_hint()
    };
    assert_eq!(
        cut(Reveal::OFF),
        None,
        "a list with reveal off offers nothing"
    );
    assert_eq!(
        cut(Reveal::default().keyboard(Some(KeyCode::Tab))).map(|(key, _)| key),
        Some("Tab"),
        "a cut label advertises the configured key"
    );

    let tall = || {
        let items = (0..30)
            .map(|n| ListItem::new(&format!("row {n:02}")))
            .collect();
        ListBox::new(id, items, SelectMode::Single)
    };
    let offset_after_track_press = |click: TrackClick| {
        let mut list = tall().track_click(click);
        let shot = draw_list(&mut list, 20, 5, Interaction::default());
        let track = shot.region(id, Part::TRACK, 0);
        let bottom = Position::new(track.x, track.bottom() - 1);
        let ev = shot.event(Phase::Press, id, Part::TRACK, bottom);
        let _ = acts(list.pointer(&ev));
        list.scroll.visible_range().start
    };
    let (jumped, paged) = (
        offset_after_track_press(TrackClick::Jump),
        offset_after_track_press(TrackClick::Page),
    );
    assert!(paged > 0 && jumped > paged, "jump {jumped}, page {paged}");
}

// ---------------------------------------------------------------- TreeView

fn tree_nodes() -> Vec<TreeNode> {
    vec![
        TreeNode::dir(
            "src",
            vec![
                TreeNode::dir(
                    "core",
                    vec![
                        TreeNode::leaf("mod.rs"),
                        TreeNode::leaf_meta("lib.rs", "12 KB"),
                    ],
                ),
                TreeNode::leaf("main.rs")
                    .glyph("*")
                    .glyph_tone(Tone::Success)
                    .label_tone(Tone::Warning)
                    .meta("4 KB"),
            ],
        ),
        TreeNode::dir("docs", vec![TreeNode::note("no pages yet")]),
        TreeNode::lazy("remote"),
        TreeNode::leaf("README"),
    ]
}

fn draw_tree(tree: &mut TreeView, width: u16, height: u16, interaction: Interaction) -> Shot {
    frame(
        Rect::new(0, 0, width, height),
        interaction,
        |area, buf, ctx| {
            let bg = ctx.theme.canvas;
            tree.render(area, buf, ctx, bg)
        },
    )
}

fn paths(tree: &TreeView) -> Vec<Path> {
    tree.rows()
        .iter()
        .map(|row: &FlatRow| row.path.clone())
        .collect()
}

fn tree_structure() {
    let id = WidgetId::of("collections.tree");
    let t = theme();
    let mut tree = TreeView::new(id, tree_nodes());
    assert_eq!(
        paths(&tree),
        vec![
            vec![0],
            vec![0, 0],
            vec![0, 1],
            vec![1],
            vec![1, 0],
            vec![2],
            vec![3]
        ],
        "top level nodes start open"
    );
    {
        let rows = tree.rows();
        assert!(rows[0].expanded && rows[0].has_children);
        assert!(rows[1].has_children && !rows[1].expanded);
        assert!(rows[4].note && !rows[4].has_children);
        assert!(
            rows[5].has_children && rows[5].expanded,
            "a lazy node counts as a branch"
        );
        assert_eq!((rows[2].depth, rows[2].meta.as_deref()), (1, Some("4 KB")));
        assert_eq!(rows[2].glyph.as_deref(), Some("*"));
        assert_eq!(rows[2].glyph_tone, Tone::Success);
        assert_eq!(rows[2].label_tone, Some(Tone::Warning));
    }
    assert_eq!(tree.cursor_path(), Some(&[0usize][..]));
    assert_eq!(TreeView::MIN_LABEL, 12);
    assert_eq!(tree.node(&[0, 0]).map(|n| n.label.as_str()), Some("core"));
    assert!(tree.node(&[9]).is_none() && tree.node(&[]).is_none());
    tree.node_mut(&[3]).expect("README").label = "README.md".to_owned();
    assert_eq!(
        tree.rows()[6].label,
        "README",
        "rows are stale until flatten"
    );
    tree.flatten();
    assert_eq!(tree.rows()[6].label, "README.md");
    assert!(tree.node_mut(&[]).is_none());

    let shot = draw_tree(&mut tree, 40, 10, Interaction::default());
    let text = text_of(&shot.buf);
    for want in [
        "src",
        "core",
        "* main.rs",
        "4 KB",
        "docs",
        "no pages yet",
        "remote",
        "README.md",
    ] {
        assert!(text.contains(want), "{want}:\n{text}");
    }
    assert!(
        !text.contains("mod.rs"),
        "a closed branch hides its children"
    );
    let (glyph, plain) = (at(&shot.buf, "*"), at(&shot.buf, "README.md"));
    let label = at(&shot.buf, "main.rs");
    assert_ne!(
        shot.buf[(label.x, label.y)].fg,
        shot.buf[(plain.x, plain.y)].fg,
        "label_tone recolours the label"
    );
    let plain_glyph = {
        let mut other = TreeView::new(
            id,
            vec![TreeNode::dir("d", vec![TreeNode::leaf("x").glyph("*")])],
        );
        let p = draw_tree(&mut other, 40, 4, Interaction::default());
        let at = at(&p.buf, "*");
        p.buf[(at.x, at.y)].fg
    };
    assert_ne!(
        shot.buf[(glyph.x, glyph.y)].fg,
        plain_glyph,
        "glyph_tone recolours the glyph"
    );

    let meta_column = |meta: bool, indent: u16| {
        let mut tree = TreeView::new(id, tree_nodes())
            .show_meta(meta)
            .indent(indent);
        let p = draw_tree(&mut tree, 40, 10, Interaction::default());
        (locate(&p.buf, "4 KB").is_some(), at(&p.buf, "main.rs").x)
    };
    assert_eq!(meta_column(true, 2), (true, meta_column(true, 2).1));
    assert!(
        !meta_column(false, 2).0,
        "show_meta(false) hides the column"
    );
    assert_eq!(
        meta_column(true, 5).1,
        meta_column(true, 2).1 + 3,
        "indent widens each level"
    );

    tree.set_filter(Some("main"));
    assert_eq!(
        paths(&tree),
        vec![vec![0], vec![0, 1]],
        "a filter keeps matches and their ancestors"
    );
    assert_eq!(tree.cursor_path(), Some(&[0usize][..]));
    tree.set_filter(None);
    assert_eq!(tree.rows().len(), 7);

    tree.reveal(&[0, 0, 1]);
    assert_eq!(tree.cursor_path(), Some(&[0usize, 0, 1][..]));
    assert!(
        tree.rows().iter().any(|r| r.label == "lib.rs"),
        "reveal opens the ancestors"
    );

    tree.collapse_all();
    assert_eq!(tree.rows().len(), 4, "only the top level is left");
    let items = tree.context_items();
    assert_eq!(
        items.iter().map(|(cmd, _)| *cmd).collect::<Vec<_>>(),
        [TreeCmd::ExpandAll, TreeCmd::CollapseAll]
    );
    assert!(items[0].1.item.reason.is_none() && items[0].1.shortcut.is_some());
    assert_eq!(
        items[1].1.item.reason.as_deref(),
        Some("Every branch is closed")
    );
    tree.expand_all();
    assert_eq!(tree.rows().len(), 9, "every branch with children is open");
    let items = tree.context_items();
    assert_eq!(
        items[0].1.item.reason.as_deref(),
        Some("Every branch is open")
    );
    assert!(items[1].1.item.reason.is_none());
    let unbound = TreeView::new(id, tree_nodes()).keymap(Vec::new());
    assert!(
        unbound
            .context_items()
            .iter()
            .all(|(_, item)| item.shortcut.is_none())
    );
    assert_eq!(TreeCmd::ExpandAll.label(), "Expand all");
    assert_eq!(TreeCmd::CollapseAll.label(), "Collapse all");
    let (outcome, actions) = acts(tree.run(TreeCmd::CollapseAll));
    assert_eq!(
        (outcome, actions.len(), tree.rows().len()),
        (Outcome::Changed, 0, 4)
    );
    let (outcome, _) = acts(tree.run(TreeCmd::ExpandAll));
    assert_eq!((outcome, tree.rows().len()), (Outcome::Changed, 9));

    let mut lazy = TreeView::new(id, tree_nodes());
    assert!(lazy.node(&[2]).is_some_and(|n| n.lazy && !n.busy));
    lazy.set_busy(&[2], true);
    let p = draw_tree(&mut lazy, 40, 10, Interaction::default());
    assert!(
        text_of(&p.buf).contains(t.glyphs.spinner_frame(0)),
        "a busy node spins"
    );
    lazy.set_busy(&[2], false);
    lazy.set_children(&[2], vec![TreeNode::leaf("a.txt"), TreeNode::leaf("b.txt")]);
    assert_eq!(
        lazy.rows().len(),
        9,
        "the loaded children open under the node"
    );
    assert!(lazy.node(&[2]).is_some_and(|n| !n.lazy && !n.busy));
    lazy.set_busy(&[9], true);
}

fn tree_input() {
    let id = WidgetId::of("collections.tree.input");
    let mut tree = TreeView::new(id, tree_nodes());

    let (o, a) = acts(tree.on_key(&key(KeyCode::Down)));
    assert_eq!((o, a.len()), (Outcome::Changed, 0));
    assert_eq!(tree.cursor_path(), Some(&[0usize, 0][..]));
    let (o, _) = acts(tree.on_key(&key(KeyCode::Right)));
    assert_eq!(o, Outcome::Changed);
    assert_eq!(tree.rows().len(), 9, "Right opens a closed branch");
    let _ = tree.on_key(&key(KeyCode::Right));
    assert_eq!(
        tree.cursor_path(),
        Some(&[0usize, 0, 0][..]),
        "Right on an open branch enters it"
    );
    let _ = tree.on_key(&key(KeyCode::Left));
    assert_eq!(
        tree.cursor_path(),
        Some(&[0usize, 0][..]),
        "Left on a child goes to its parent"
    );
    let _ = tree.on_key(&key(KeyCode::Left));
    assert_eq!(tree.rows().len(), 7, "Left on an open branch closes it");
    let _ = tree.on_key(&key(KeyCode::Down));
    let (o, a) = acts(tree.on_key(&key(KeyCode::Enter)));
    assert_eq!(
        (o, a),
        (Outcome::Changed, vec![TreeAction::Activate(vec![0, 1])])
    );
    assert_eq!(tree.selected, Some(vec![0, 1]));
    let _ = tree.on_key(&key(KeyCode::Down));
    let (_, a) = acts(tree.on_key(&key(KeyCode::Enter)));
    assert!(a.is_empty(), "a branch toggles instead of activating");
    assert_eq!(tree.rows().len(), 6, "docs closed");
    let _ = tree.on_key(&key(KeyCode::Enter));
    assert_eq!(tree.rows().len(), 7);
    let _ = tree.on_key(&key(KeyCode::Down));
    assert_eq!(
        tree.cursor_path(),
        Some(&[2usize][..]),
        "the keyboard steps over a note"
    );
    assert_eq!(acts(tree.on_click_row(4)).0, Outcome::Consumed);
    let (o, a) = acts(tree.on_key(&key(KeyCode::Enter)));
    assert_eq!((o, a.len()), (Outcome::Consumed, 0), "a note is inert");
    let _ = tree.on_key(&key(KeyCode::Down));
    let (_, a) = acts(tree.on_key(&key(KeyCode::Enter)));
    assert!(a.is_empty(), "closing a lazy node asks for nothing");
    let (_, a) = acts(tree.on_key(&key(KeyCode::Enter)));
    assert_eq!(
        a,
        vec![TreeAction::Expand(vec![2])],
        "a lazy node asks for its children"
    );
    assert!(tree.node(&[2]).is_some_and(|n| n.busy));

    for (mode, collapses, actions) in [
        (BranchAction::Toggle, true, 0),
        (BranchAction::Activate, false, 1),
        (BranchAction::Both, true, 1),
    ] {
        let mut tree = TreeView::new(id, tree_nodes()).branch_activate(mode);
        let (_, a) = acts(tree.on_key(&key(KeyCode::Enter)));
        assert_eq!(tree.rows()[0].expanded, !collapses, "{mode:?}");
        assert_eq!(a.len(), actions, "{mode:?}");
        if actions == 1 {
            assert_eq!(a, vec![TreeAction::Activate(vec![0])]);
            assert_eq!(tree.selected, Some(vec![0]));
        }
    }

    let mut clicks = TreeView::new(id, tree_nodes());
    let (o, a) = acts(clicks.on_click_row(2));
    assert_eq!(
        (o, a.len(), clicks.cursor_path()),
        (Outcome::Changed, 0, Some(&[0usize, 1][..]))
    );
    let (_, a) = acts(clicks.on_click_row_count(2, 2));
    assert_eq!(
        a,
        vec![TreeAction::Activate(vec![0, 1])],
        "a double click activates"
    );
    let (_, a) = acts(clicks.on_click_row_count(0, 2));
    assert!(
        a.is_empty() && !clicks.rows()[0].expanded,
        "a double click on a branch toggles"
    );
    let _ = clicks.on_click_toggle(0);
    assert!(clicks.rows()[0].expanded, "the disclosure toggles");
    assert_eq!(acts(clicks.on_click_row(40)).0, Outcome::Consumed);
    assert_eq!(
        acts(clicks.on_click_row(4)).0,
        Outcome::Consumed,
        "a note ignores clicks"
    );
    let (_, a) = acts(clicks.on_click_with(2, false, KeyModifiers::NONE));
    assert!(a.is_empty());
    let (_, a) = acts(clicks.on_click_at(2, false, KeyModifiers::NONE, 2));
    assert_eq!(a, vec![TreeAction::Activate(vec![0, 1])]);
    let (_, a) = acts(clicks.on_click_at(0, true, KeyModifiers::NONE, 1));
    assert!(a.is_empty() && !clicks.rows()[0].expanded);
    let _ = clicks.on_click_toggle(0);

    let (o, _) = acts(clicks.on_click_with(0, false, KeyModifiers::ALT));
    assert_eq!(o, Outcome::Changed);
    assert!(
        !clicks.rows()[0].expanded,
        "alt-click closes the whole subtree"
    );
    let _ = clicks.on_click_with(0, false, KeyModifiers::ALT);
    assert!(
        clicks.rows().iter().any(|r| r.label == "mod.rs"),
        "alt-click opens every level"
    );
    let mut plain = TreeView::new(id, tree_nodes()).alt_click_recursive(false);
    let _ = plain.on_click_with(0, false, KeyModifiers::ALT);
    assert!(
        plain.rows()[0].expanded,
        "alt-click is an ordinary click when disabled"
    );
    assert_eq!(acts(plain.toggle(99)).0, Outcome::Consumed);
    assert_eq!(
        acts(plain.toggle(2)).0,
        Outcome::Consumed,
        "a leaf has nothing to toggle"
    );
    assert_eq!(acts(plain.toggle_recursive(99)).0, Outcome::Consumed);
    assert_eq!(acts(plain.toggle_recursive(2)).0, Outcome::Consumed);
    let _ = plain.toggle_recursive(5);
    let (_, a) = acts(plain.toggle_recursive(5));
    assert_eq!(
        a,
        vec![TreeAction::Expand(vec![2])],
        "recursive open also loads lazy nodes"
    );

    let mut quick = TreeView::new(id, tree_nodes()).click(ClickPolicy::NAVIGATION);
    let (_, a) = acts(quick.on_click_row(2));
    assert_eq!(
        a,
        vec![TreeAction::Activate(vec![0, 1])],
        "navigation trees activate on one click"
    );

    let mut pointed = TreeView::new(id, tree_nodes()).click(ClickPolicy::NAVIGATION);
    let shot = draw_tree(&mut pointed, 40, 10, Interaction::default());
    assert!(
        shot.hits
            .regions()
            .iter()
            .any(|r| r.id == pointed.row_id(1))
    );
    assert!(
        shot.hits
            .regions()
            .iter()
            .any(|r| r.id == pointed.toggle_id(0))
    );
    assert!(
        shot.hits
            .regions()
            .iter()
            .any(|r| r.id == pointed.glyph_id(2))
    );
    let glyph = at(&shot.buf, "*");
    let ev = shot.event(Phase::Click, id, Part::MARKER, glyph);
    assert_eq!(pointed.row_at(&ev), Some(2), "the glyph belongs to its row");
    let (_, a) = acts(pointed.pointer(&ev));
    assert_eq!(a, vec![TreeAction::Activate(vec![0, 1])]);
    let readme = at(&shot.buf, "README");
    let ev = shot.event(Phase::Click, id, Part::ROW, readme);
    assert_eq!(pointed.row_at(&ev), Some(6));
    let (_, a) = acts(pointed.pointer(&ev));
    assert_eq!(a, vec![TreeAction::Activate(vec![3])]);
    let icon = shot.region(id, Part::ICON, 0);
    let ev = shot.event(Phase::Click, id, Part::ICON, Position::new(icon.x, icon.y));
    assert_eq!(pointed.row_at(&ev), Some(0));
    let (o, a) = acts(pointed.pointer(&ev));
    assert_eq!((o, a.len()), (Outcome::Changed, 0));
    assert!(!pointed.rows()[0].expanded, "the disclosure icon toggles");
    let stray = Shot {
        buf: Buffer::empty(Rect::new(0, 0, 1, 1)),
        hits: HitRegistry::default(),
        ring: FocusRing::default(),
    };
    let _ = stray;
}

fn tree_options() {
    let id = WidgetId::of("collections.tree.options");
    let t = theme();
    let leaves = |labels: &[&str]| labels.iter().map(|l| TreeNode::leaf(l)).collect::<Vec<_>>();

    let mut wrapping = TreeView::new(id, tree_nodes()).wrap(true);
    wrapping.on_key(&key(KeyCode::Up));
    assert_eq!(
        wrapping.cursor_path(),
        Some(&[3usize][..]),
        "wrap goes to the last row"
    );
    let mut bounded = TreeView::new(id, tree_nodes()).wrap(false);
    bounded.on_key(&key(KeyCode::Up));
    assert_eq!(bounded.cursor_path(), Some(&[0usize][..]));

    let mut vim = TreeView::new(id, tree_nodes())
        .nav(NavKeys::VIM)
        .type_ahead(false);
    let _ = vim.on_key(&press('j'));
    assert_eq!(
        vim.cursor_path(),
        Some(&[0usize, 0][..]),
        "the vim profile adds j"
    );

    let mut remapped = TreeView::new(id, tree_nodes())
        .keymap(vec![(KeyBinding::char('e'), TreeCmd::ExpandAll)])
        .type_ahead(false);
    remapped.collapse_all();
    assert_eq!(acts(remapped.on_key(&press('*'))).0, Outcome::Ignored);
    assert_eq!(acts(remapped.on_key(&press('e'))).0, Outcome::Changed);
    assert_eq!(remapped.rows().len(), 9);

    let ahead = |timeout: std::time::Duration| {
        let mut tree = TreeView::new(id, leaves(&["alpha", "beta", "bravo", "brown"]))
            .type_ahead(true)
            .type_ahead_timeout(timeout);
        let _ = tree.on_key(&press('b'));
        assert_eq!(tree.cursor_path(), Some(&[1usize][..]));
        std::thread::sleep(std::time::Duration::from_millis(30));
        let _ = tree.on_key(&press('r'));
        tree.cursor
    };
    assert_eq!(ahead(std::time::Duration::from_secs(60)), 2);
    assert_eq!(ahead(std::time::Duration::from_millis(1)), 1);
    let mut off = TreeView::new(id, leaves(&["alpha", "beta"])).type_ahead(false);
    assert_eq!(acts(off.on_key(&press('b'))).0, Outcome::Ignored);
    let routed = |precedence: TypeAheadPrecedence| {
        let mut tree = TreeView::new(id, leaves(&["alpha", "beta", "gamma", "juliet"]))
            .nav(NavKeys::VIM)
            .type_ahead(true)
            .type_ahead_precedence(precedence);
        let k = press('j');
        route_key(&k, || tree.on_key(&k).outcome());
        tree.cursor
    };
    assert_eq!(routed(TypeAheadPrecedence::Shortcuts), 1);
    assert_eq!(routed(TypeAheadPrecedence::TypeAhead), 3);

    let mut none = TreeView::new(id, Vec::new());
    assert!(
        draw_tree(&mut none, 30, 3, Interaction::default())
            .ring
            .is_empty()
    );
    let mut stop = TreeView::new(id, Vec::new()).skip_when_empty(false);
    assert_eq!(
        draw_tree(&mut stop, 30, 3, Interaction::default())
            .ring
            .len(),
        1
    );
    let mut empty = TreeView::new(id, Vec::new()).empty(EmptyState::new("No files"));
    assert!(
        text_of(&draw_tree(&mut empty, 30, 3, Interaction::default()).buf).contains("No files")
    );
    let mut loading =
        TreeView::new(id, tree_nodes()).content(ContentState::loading("Reading tree"));
    let text = text_of(&draw_tree(&mut loading, 30, 3, Interaction::default()).buf);
    assert!(
        text.contains("Reading tree") && !text.contains("src"),
        "{text}"
    );

    let edge = |divider: bool| {
        let mut tree = TreeView::new(id, tree_nodes()).divider(divider);
        let p = draw_tree(&mut tree, 30, 8, Interaction::default());
        (0..8).any(|y| !p.buf[(29, y)].symbol().trim().is_empty())
    };
    assert!(
        edge(true) && !edge(false),
        "the divider paints the right edge"
    );

    let trailing = |cells: u16| {
        let mut tree = TreeView::new(id, tree_nodes()).reserve_right(cells);
        draw_tree(&mut tree, 40, 8, Interaction::default());
        tree.trailing()
    };
    assert_eq!(trailing(0), Rect::ZERO);
    assert_eq!((trailing(8).x, trailing(8).width), (32, 8));
    assert_eq!(
        trailing(30),
        Rect::ZERO,
        "no room is reserved when the label would starve"
    );

    let marker = |on: Option<&'static str>| {
        let mut tree = TreeView::new(id, tree_nodes()).selected_marker(on);
        let _ = tree.on_click_with(2, false, KeyModifiers::NONE);
        let _ = tree.on_key(&key(KeyCode::Enter));
        let p = draw_tree(&mut tree, 40, 10, Interaction::default());
        text_of(&p.buf).contains('●')
    };
    assert!(marker(Some("●")) && !marker(None));

    let selected_bg = |look: InactiveSelection| {
        let mut tree = TreeView::new(id, tree_nodes()).inactive_selection(look);
        tree.on_click_row(6);
        let _ = tree.on_key(&key(KeyCode::Enter));
        let p = draw_tree(&mut tree, 40, 10, Interaction::default());
        let row = at(&p.buf, "README");
        p.buf[(row.x + 12, row.y)].bg
    };
    assert_eq!(selected_bg(InactiveSelection::Accent), t.accent_bg);
    assert_eq!(
        selected_bg(InactiveSelection::Neutral),
        t.selection_inactive
    );

    let offset_after_track_press = |click: TrackClick| {
        let nodes = (0..30)
            .map(|n| TreeNode::leaf(&format!("row {n:02}")))
            .collect();
        let mut tree = TreeView::new(id, nodes).track_click(click);
        let shot = draw_tree(&mut tree, 20, 5, Interaction::default());
        let track = shot.region(id, Part::TRACK, 0);
        let ev = shot.event(
            Phase::Press,
            id,
            Part::TRACK,
            Position::new(track.x, track.bottom() - 1),
        );
        let _ = acts(tree.pointer(&ev));
        tree.core.scroll.visible_range().start
    };
    let (jumped, paged) = (
        offset_after_track_press(TrackClick::Jump),
        offset_after_track_press(TrackClick::Page),
    );
    assert!(paged > 0 && jumped > paged, "jump {jumped}, page {paged}");
}

#[test]
fn catalog_collections_scenarios_execute() {
    for case in CASES {
        (case.run)();
    }
}
