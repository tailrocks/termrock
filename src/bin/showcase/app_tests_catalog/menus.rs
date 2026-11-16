//! Lane `menus` of the #14 burn-down: this lane's callable scenarios.
//!
//! Each scenario builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. Register one `ScenarioCase` per
//! `ScenarioId` variant from the `lane:menus` block in `../coverage.rs`. Edit only
//! this file, `../coverage/lane_menus.rs` and `../coverage_debt/menus.txt`.

use junie_tui::core::edit::{EditEntry, FieldKeys, TextCmd, clipboard};
use junie_tui::core::event::ClientPlatform;
use junie_tui::core::event::Outcome;
use junie_tui::core::event::{Input, Key};
use junie_tui::core::hints::Binding;
use junie_tui::core::hit::{Axis, ReleaseFrom};
use junie_tui::core::id::Part;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::DismissReason;
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::Actions as _;
use junie_tui::core::keymap::NavKeys;
use junie_tui::core::list::{TypeAheadPrecedence, route_key};
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::refusal::{Control, Reason, clear_refusal, take_refusal};
use junie_tui::core::scroll::TrackClick;
use junie_tui::theme::CursorStyle;
use junie_tui::theme::{BadgeKind, ColorLevel, HintAlign, HintBarStyle, StatusKind, Theme, Tone};
use junie_tui::ui::context::{
    ContextAnchor, ContextCmd, ContextConfig, ContextKeys, ContextRequest, ContextSource,
    command_items,
};
use junie_tui::ui::edit_menu::{
    EditCaps, EditMenu, EditMenuHost, edit_items, edit_menu_key, edit_menu_pointer,
    edit_menu_route, editable_target, point_editable, point_field,
};
use junie_tui::ui::popup::{
    self, Dismiss, Overflow, Place, Placement, RefuseFeedback, VAlign, backdrop_id,
};
use junie_tui::ui::tooltip::{self, Tip, Tooltips, tip_id};
use junie_tui::widgets::brand::Lockup;
use junie_tui::widgets::edit::EditField;
use junie_tui::widgets::help::{HelpCmd, HelpOverlay, HelpSection};
use junie_tui::widgets::hintbar::{HintBar, HintKeys, HintLayer};
use junie_tui::widgets::icon_button::IconButton;
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::keyhint::{self, Hint, HintPriority};
use junie_tui::widgets::list::{ListBox, ListItem, SelectMode};
use junie_tui::widgets::menu::{ContextMenu, MenuAction, MenuItem, UnknownKey, reason_room};
use junie_tui::widgets::menu::{MenuBar, MenuBarAction, OpenOn, Overflow as BarOverflow};
use junie_tui::widgets::option_list::{SelectArrows, WheelMode};
use junie_tui::widgets::progress::MeterTone;
use junie_tui::widgets::row_menu::{self, RowMenu};
use junie_tui::widgets::select::{Select, SelectAction, TabInOpen};
use junie_tui::widgets::statusbar::{
    Emphasis, Group, Placed, STATUS_METER_TRACK, StatusBar, StatusItem, reveal_id,
};
use junie_tui::widgets::widget_hints;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};

use super::{ScenarioCase, key, paint_hits, rows, text_of};
use crate::coverage::ScenarioId;

#[rustfmt::skip]
pub const CASES: &[ScenarioCase] = &[
    ScenarioCase { id: ScenarioId::MenusPopupDismissPolicy, run: popup_dismiss_policy },
    ScenarioCase { id: ScenarioId::MenusPopupPlacement, run: popup_placement },
    ScenarioCase { id: ScenarioId::MenusContextMenuKeys, run: context_menu_keys },
    ScenarioCase { id: ScenarioId::MenusContextMenuPointer, run: context_menu_pointer },
    ScenarioCase { id: ScenarioId::MenusContextMenuLayout, run: context_menu_layout },
    ScenarioCase { id: ScenarioId::MenusSelectKeys, run: select_keys },
    ScenarioCase { id: ScenarioId::MenusSelectPointer, run: select_pointer },
    ScenarioCase { id: ScenarioId::MenusSelectPopup, run: select_popup },
    ScenarioCase { id: ScenarioId::MenusBarKeys, run: menu_bar_keys },
    ScenarioCase { id: ScenarioId::MenusBarPointer, run: menu_bar_pointer },
    ScenarioCase { id: ScenarioId::MenusBarOverflow, run: menu_bar_overflow },
    ScenarioCase { id: ScenarioId::MenusRowMenu, run: row_menu_flow },
    ScenarioCase { id: ScenarioId::MenusContextRequests, run: context_requests },
    ScenarioCase { id: ScenarioId::MenusContextCommands, run: context_commands },
    ScenarioCase { id: ScenarioId::MenusEditMenuCaps, run: edit_menu_caps },
    ScenarioCase { id: ScenarioId::MenusEditMenuHost, run: edit_menu_host },
    ScenarioCase { id: ScenarioId::MenusTooltips, run: tooltips },
    ScenarioCase { id: ScenarioId::MenusKeyHintLabels, run: key_hint_labels },
    ScenarioCase { id: ScenarioId::MenusKeyHintRendering, run: key_hint_rendering },
    ScenarioCase { id: ScenarioId::MenusHintBar, run: hint_bar },
    ScenarioCase { id: ScenarioId::MenusStatusBar, run: status_bar },
    ScenarioCase { id: ScenarioId::MenusHelpOverlay, run: help_overlay },
];

fn popup_dismiss_policy() {
    let light = Dismiss::LIGHT;
    let escape = Dismiss::ESCAPE;
    let never = Dismiss::NEVER;
    let all = [
        DismissReason::Esc,
        DismissReason::CloseButton,
        DismissReason::OutsideClick,
        DismissReason::FocusOut,
        DismissReason::Programmatic,
    ];
    let expected = [
        (light, [true, true, true, true, true]),
        (escape, [true, true, false, false, true]),
        (never, [false, false, false, false, true]),
    ];
    for (policy, want) in expected {
        for (reason, want) in all.into_iter().zip(want) {
            assert_eq!(policy.allows(reason), want, "{policy:?} for {reason:?}");
        }
    }
    assert!(light.shows_close() && escape.shows_close() && !never.shows_close());
    assert!(
        !light.close_button(false).shows_close(),
        "a policy can drop the close button while Esc still dismisses"
    );
    assert!(light.close_button(false).allows(DismissReason::Esc));
    assert!(!light.focus_out(false).allows(DismissReason::FocusOut));
    assert!(never.focus_out(true).allows(DismissReason::FocusOut));
    assert_eq!(light.refuse_feedback, RefuseFeedback::Flash);
    assert_eq!(RefuseFeedback::default(), RefuseFeedback::Flash);
    assert_eq!(
        light.refuse_feedback(RefuseFeedback::None).refuse_feedback,
        RefuseFeedback::None
    );
    // The feedback decides what an ignored outside click does: flash the modal
    // surface, or nothing. A click the policy honours is never refused.
    let mut pointer = junie_tui::core::pointer::PointerState::default();
    assert!(!junie_tui::ui::modal::refuse_outside_click(
        &mut pointer,
        light,
        100
    ));
    assert!(!pointer.has_flash());
    assert!(junie_tui::ui::modal::refuse_outside_click(
        &mut pointer,
        escape,
        100
    ));
    assert_eq!(pointer.flashing(100), Some(junie_tui::ui::modal::SURFACE));
    let mut quiet = junie_tui::core::pointer::PointerState::default();
    assert!(!junie_tui::ui::modal::refuse_outside_click(
        &mut quiet,
        escape.refuse_feedback(RefuseFeedback::None),
        100
    ));
    assert!(!quiet.has_flash());
    assert!(!light.pass_through && light.pass_through(true).pass_through);

    // The policy decides what the popup registers behind itself: a backdrop
    // that swallows the click, or one that lets the click through.
    let owner = WidgetId::of("scenario.popup.owner");
    let screen = Rect::new(0, 0, 30, 10);
    for (policy, through) in [
        (light, false),
        (light.pass_through(true), true),
        (escape.pass_through(true), false),
    ] {
        let painted = paint_hits(screen, |area, _, ctx| {
            popup::backdrop(ctx, owner, policy, area)
        });
        let region = painted
            .hits
            .regions()
            .iter()
            .find(|r| r.id == backdrop_id(owner))
            .expect("the popup registers its backdrop under the owner");
        assert_eq!(region.owner, owner);
        assert_eq!(region.area, screen);
        assert_eq!(
            region.pass_through, through,
            "pass-through needs light dismiss and the pass_through option"
        );
    }
    let painted = paint_hits(screen, |area, _, ctx| {
        ctx.inert = true;
        popup::backdrop(ctx, owner, light, area);
    });
    assert!(
        painted.hits.regions().is_empty(),
        "an inert frame registers no backdrop"
    );
    assert_ne!(backdrop_id(owner), owner);
    assert_eq!(Dismiss::default(), Dismiss::LIGHT);
}

fn popup_placement() {
    let screen = Rect::new(0, 0, 40, 20);
    let (w, h) = (10, 4);
    let anchor = Rect::new(14, 8, 6, 1);
    assert_eq!(
        popup::place(screen, anchor, w, h, Placement::Below),
        Rect::new(14, 9, 10, 4)
    );
    assert_eq!(
        popup::place(screen, anchor, w, h, Placement::Above),
        Rect::new(14, 4, 10, 4)
    );
    assert_eq!(
        popup::place(screen, anchor, w, h, Placement::Right),
        Rect::new(20, 8, 10, 4)
    );
    assert_eq!(
        popup::place(screen, anchor, w, h, Placement::Left),
        Rect::new(4, 8, 10, 4)
    );
    assert_eq!(
        popup::place(screen, anchor, w, h, Placement::Center),
        Rect::new(15, 8, 10, 4)
    );
    // A side with no room flips to the other one instead of leaving the screen.
    let near_left = Rect::new(2, 8, 4, 1);
    let flipped = popup::place(screen, near_left, w, h, Placement::Left);
    assert_eq!(flipped.x, near_left.right(), "Left flips to the right edge");

    // Place options: margin, text inset, vertical alignment, overflow.
    assert_eq!(Place::default(), Place::DEFAULT);
    assert_eq!((Place::DEFAULT.margin, Place::FLUSH.margin), (1, 0));
    assert_eq!(Place::MENU.vertical_margin, Some(0));
    assert_eq!(Place::MENU.overflow, Overflow::Slide);
    assert_eq!(Place::DEFAULT.overflow, Overflow::Shrink);
    assert_eq!(
        popup::place_with(screen, anchor, w, h, Placement::Center, Place::FLUSH),
        Rect::new(15, 8, 10, 4)
    );
    let inset = Place::DEFAULT.text_inset(2);
    assert_eq!(inset.text_inset, 2);
    assert_eq!(
        popup::place_with(screen, anchor, w, h, Placement::Below, inset).x,
        12,
        "the text inset moves the popup left so its text lines up with the anchor"
    );
    let third = Place::DEFAULT.v_align(VAlign::Third);
    assert_eq!(third.v_align, VAlign::Third);
    assert_eq!(
        popup::place_with(screen, anchor, w, h, Placement::Center, third).y,
        5
    );
    assert_eq!(Place::DEFAULT.v_align, VAlign::Center);
    let wide = Place::DEFAULT.margin(4);
    assert_eq!(wide.margin, 4);
    let edge = Rect::new(38, 8, 2, 1);
    assert_eq!(
        popup::place_with(screen, edge, w, h, Placement::Below, wide).x,
        26,
        "the margin keeps the popup four columns off the screen edge"
    );
    assert_eq!(
        popup::place_with(screen, edge, w, h, Placement::Below, Place::FLUSH).x,
        30,
        "a flush fit may touch the edge"
    );
    let tall = Place::FLUSH.vertical_margin(Some(3));
    assert_eq!(tall.vertical_margin, Some(3));
    assert_eq!(
        popup::place_with(screen, anchor, w, 30, Placement::Center, tall).height,
        20,
        "a popup taller than the screen is clamped to it"
    );

    // Overflow: with no room on either side Shrink keeps the popup near the
    // anchor and cuts its height; Slide keeps the height and slides it.
    let low = Rect::new(0, 0, 40, 8);
    let low_anchor = Rect::new(5, 5, 4, 1);
    let shrunk = popup::place_with(
        low,
        low_anchor,
        w,
        8,
        Placement::Below,
        Place::DEFAULT.overflow(Overflow::Shrink),
    );
    let slid = popup::place_with(
        low,
        low_anchor,
        w,
        8,
        Placement::Below,
        Place::DEFAULT.overflow(Overflow::Slide),
    );
    assert!(shrunk.height < 8, "Shrink cuts the popup: {shrunk:?}");
    assert_eq!(slid.height, 8, "Slide keeps the popup whole: {slid:?}");
    assert!(shrunk.bottom() <= low.bottom() && slid.bottom() <= low.bottom());

    // The shared popup surface: a bordered elevated box plus its hit regions.
    let owner = WidgetId::of("scenario.popup.surface");
    let area = Rect::new(4, 2, 12, 5);
    let mut inner = Rect::ZERO;
    let painted = paint_hits(Rect::new(0, 0, 30, 10), |_, buf, ctx| {
        let t = ctx.theme;
        inner = popup::surface(area, buf, ctx, t);
    });
    assert_eq!(
        inner,
        Rect::new(5, 3, 10, 3),
        "the surface returns its inside"
    );
    let top = rows(&painted.buf)[2].clone();
    assert!(top.chars().nth(4).is_some_and(|c| c != ' '), "{top:?}");
    assert!(
        painted
            .hits
            .regions()
            .iter()
            .any(|r| r.id == WidgetId::of("popup.surface") && r.area == area),
        "the surface registers its own area"
    );
    let painted = paint_hits(Rect::new(0, 0, 30, 10), |_, buf, ctx| {
        let t = ctx.theme;
        popup::surface_for(area, buf, ctx, t, Some((owner, Dismiss::LIGHT)));
    });
    assert!(
        painted
            .hits
            .regions()
            .iter()
            .any(|r| r.id == backdrop_id(owner) && r.backdrop),
        "an owned surface registers the owner's backdrop"
    );
    let none = paint_hits(Rect::new(0, 0, 30, 10), |_, buf, ctx| {
        let t = ctx.theme;
        assert!(popup::surface(Rect::new(40, 40, 5, 5), buf, ctx, t).is_empty());
    });
    assert!(none.hits.regions().is_empty());
}

fn file_menu() -> Vec<MenuItem> {
    vec![
        MenuItem::new("Open").shortcut("o"),
        MenuItem::new("Rename").shortcut("F2"),
        MenuItem::new("Copy path").shortcut("c").separator(),
        MenuItem::new("Share")
            .disabled(true)
            .reason("Sign in first"),
        MenuItem::new("Delete").danger().shortcut("d"),
    ]
}

fn draw_menu(menu: &mut ContextMenu, screen: Rect) -> super::Painted {
    paint_hits(screen, |area, buf, ctx| menu.render(area, buf, ctx))
}

/// Finds `needle` on a painted row and returns the first cell of it.
fn find_text(buf: &ratatui::buffer::Buffer, needle: &str) -> Option<Position> {
    rows(buf).iter().enumerate().find_map(|(y, row)| {
        let col = row.find(needle)?;
        let x = row[..col].chars().count();
        Some(Position::new(x as u16, y as u16))
    })
}

fn context_menu_keys() {
    // Item options become fields the menu reads.
    let items = file_menu();
    assert_eq!(items[0].shortcut.as_deref(), Some("o"));
    assert!(items[2].separator_after && !items[1].separator_after);
    assert!(items[3].item.disabled);
    assert_eq!(items[3].item.reason.as_deref(), Some("Sign in first"));
    assert!(items[4].danger && !items[0].danger);
    assert!(!MenuItem::new("x").danger().danger_if(false).danger);
    assert!(MenuItem::new("x").danger_if(true).danger);
    assert!(MenuItem::new("x").separator_if(true).separator_after);
    assert!(
        !MenuItem::new("x")
            .separator()
            .separator_if(false)
            .separator_after
    );

    // Enter chooses the highlighted row; an accelerator chooses its own row.
    let mut menu = ContextMenu::new(WidgetId::of("scenario.menu.keys"), file_menu());
    assert_eq!(menu.highlighted(), Some(0));
    assert_eq!(
        menu.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(MenuAction::Chosen(0)))
    );
    assert_eq!(
        menu.on_key(&key(KeyCode::Char('d'))),
        (Outcome::Changed, Some(MenuAction::Chosen(4)))
    );
    assert_eq!(
        menu.highlighted(),
        Some(4),
        "the accelerator moves the cursor"
    );
    // F2 is a label, not an accelerator: the disabled row never answers either.
    assert_eq!(menu.on_key(&key(KeyCode::Down)).0, Outcome::Changed);

    // Wrapping and disabled rows: the default cursor rests on a disabled row
    // and refuses to choose it; skip_disabled steps over it instead.
    let mut menu = ContextMenu::new(WidgetId::of("scenario.menu.nav"), file_menu());
    assert_eq!(menu.on_key(&key(KeyCode::Up)).0, Outcome::Changed);
    assert_eq!(
        menu.highlighted(),
        Some(4),
        "Up wraps from the first row to the last"
    );
    for _ in 0..2 {
        menu.on_key(&key(KeyCode::Up));
    }
    assert_eq!(menu.highlighted(), Some(2));
    menu.on_key(&key(KeyCode::Down));
    assert_eq!(
        menu.highlighted(),
        Some(3),
        "a disabled row can hold the cursor"
    );
    assert_eq!(
        menu.on_key(&key(KeyCode::Enter)),
        (Outcome::Consumed, None),
        "Enter on a disabled row chooses nothing"
    );
    let mut skipping =
        ContextMenu::new(WidgetId::of("scenario.menu.skip"), file_menu()).skip_disabled(true);
    skipping.on_key(&key(KeyCode::Down));
    skipping.on_key(&key(KeyCode::Down));
    skipping.on_key(&key(KeyCode::Down));
    assert_eq!(
        skipping.highlighted(),
        Some(4),
        "skip_disabled steps over Share"
    );
    let mut fixed = ContextMenu::new(WidgetId::of("scenario.menu.nowrap"), file_menu()).wrap(false);
    assert_eq!(fixed.on_key(&key(KeyCode::Up)), (Outcome::Consumed, None));
    assert_eq!(
        fixed.highlighted(),
        Some(0),
        "without wrap Up stops at the first row"
    );

    // Dismissal: Esc follows the Dismiss policy.
    let mut menu = ContextMenu::new(WidgetId::of("scenario.menu.esc"), file_menu());
    assert_eq!(
        menu.on_key(&key(KeyCode::Esc)),
        (Outcome::Changed, Some(MenuAction::Dismissed))
    );
    let mut sticky =
        ContextMenu::new(WidgetId::of("scenario.menu.esc"), file_menu()).dismiss(Dismiss::NEVER);
    assert_eq!(sticky.on_key(&key(KeyCode::Esc)), (Outcome::Consumed, None));
    assert_eq!(sticky.on_click_outside(), None);
    assert_eq!(menu.on_click_outside(), Some(MenuAction::Dismissed));
    let mut escape_only =
        ContextMenu::new(WidgetId::of("scenario.menu.esc"), file_menu()).dismiss(Dismiss::ESCAPE);
    assert_eq!(escape_only.on_click_outside(), None);

    // A key the menu does not know closes it and passes on, or is swallowed.
    let mut passing = ContextMenu::new(WidgetId::of("scenario.menu.unknown"), file_menu());
    assert_eq!(
        passing.on_key(&key(KeyCode::F(5))),
        (Outcome::Ignored, Some(MenuAction::Dismissed))
    );
    let mut swallowing = ContextMenu::new(WidgetId::of("scenario.menu.unknown"), file_menu())
        .unknown_key(UnknownKey::Swallow);
    assert_eq!(
        swallowing.on_key(&key(KeyCode::F(5))),
        (Outcome::Consumed, None)
    );
    assert_eq!(UnknownKey::default(), UnknownKey::CloseAndPass);

    // Navigation profile and type-ahead.
    let words = || {
        vec![
            MenuItem::new("Alpha"),
            MenuItem::new("Beta"),
            MenuItem::new("Bravo"),
            MenuItem::new("Gamma"),
        ]
    };
    let id = WidgetId::of("scenario.menu.type");
    let mut vim = ContextMenu::new(id, words())
        .nav(NavKeys::VIM)
        .type_ahead(false);
    assert_eq!(vim.on_key(&key(KeyCode::Char('j'))).0, Outcome::Changed);
    assert_eq!(
        vim.highlighted(),
        Some(1),
        "j moves down in the Vim profile"
    );
    let mut desktop = ContextMenu::new(id, words()).type_ahead(false);
    assert_eq!(
        desktop.on_key(&key(KeyCode::Char('j'))),
        (Outcome::Ignored, Some(MenuAction::Dismissed)),
        "j is just an unknown key on the desktop profile"
    );
    let mut typing = ContextMenu::new(id, words()).type_ahead(true);
    typing.on_key(&key(KeyCode::Char('g')));
    assert_eq!(typing.highlighted(), Some(3), "type-ahead jumps to Gamma");
    let mut prefix = ContextMenu::new(id, words())
        .type_ahead(true)
        .type_ahead_timeout(std::time::Duration::from_secs(60));
    prefix.on_key(&key(KeyCode::Char('b')));
    prefix.on_key(&key(KeyCode::Char('r')));
    assert_eq!(
        prefix.highlighted(),
        Some(2),
        "b then r keeps the prefix: Bravo"
    );
    let mut restart = ContextMenu::new(id, words())
        .type_ahead(true)
        .type_ahead_timeout(std::time::Duration::ZERO);
    restart.on_key(&key(KeyCode::Char('b')));
    std::thread::sleep(std::time::Duration::from_millis(2));
    restart.on_key(&key(KeyCode::Char('r')));
    assert_eq!(
        restart.highlighted(),
        Some(1),
        "an expired prefix starts over, so r finds nothing and the cursor stays"
    );
}

fn context_menu_pointer() {
    let screen = Rect::new(0, 0, 50, 20);
    let mut menu = ContextMenu::new(WidgetId::of("scenario.menu.pointer"), file_menu())
        .at(Position::new(4, 2))
        .title("File");
    let painted = draw_menu(&mut menu, screen);
    let text = text_of(&painted.buf);
    for label in ["File", "Open", "Rename", "Copy path", "Share", "Delete"] {
        assert!(text.contains(label), "{label} missing:\n{text}");
    }
    assert!(text.contains("F2"), "a deliverable shortcut label is drawn");

    assert_eq!(menu.on_click(menu.row_id(2)), Some(MenuAction::Chosen(2)));
    assert_eq!(
        menu.highlighted(),
        Some(2),
        "a click moves the cursor to its row"
    );
    assert_eq!(
        menu.on_click(menu.row_id(3)),
        None,
        "a disabled row ignores a click"
    );
    assert_eq!(
        menu.on_click(menu.id),
        None,
        "the body itself is not a choice"
    );
    assert_eq!(
        menu.on_click(WidgetId::of("scenario.elsewhere")),
        Some(MenuAction::Dismissed),
        "any other id counts as an outside click"
    );

    // The reason of a disabled row shows while the cursor rests on it.
    let mut menu =
        ContextMenu::new(WidgetId::of("scenario.menu.reason"), file_menu()).at(Position::new(4, 2));
    for _ in 0..3 {
        menu.on_key(&key(KeyCode::Down));
    }
    let text = text_of(&draw_menu(&mut menu, screen).buf);
    assert!(text.contains("Sign in first"), "{text}");
    let text = text_of(&draw_menu(&mut menu, Rect::new(0, 0, 50, 20)).buf);
    assert!(text.contains("Sign in first"));

    // Danger rows are drawn in a different ink from plain rows.
    let mut menu =
        ContextMenu::new(WidgetId::of("scenario.menu.danger"), file_menu()).at(Position::new(4, 2));
    let painted = draw_menu(&mut menu, screen);
    let open = find_text(&painted.buf, "Open").expect("Open row");
    let delete = find_text(&painted.buf, "Delete").expect("Delete row");
    assert_ne!(
        painted.buf[(delete.x, delete.y)].fg,
        painted.buf[(open.x, open.y)].fg,
        "a danger row does not look like an ordinary one"
    );

    // Release selects: dragging out of the opener and releasing on a row picks it.
    let id = WidgetId::of("scenario.menu.release");
    let mut on = ContextMenu::new(id, file_menu()).at(Position::new(4, 2));
    let painted = draw_menu(&mut on, screen);
    assert!(
        painted
            .hits
            .accepts_release(on.row_id(0), ReleaseFrom::Secondary)
    );
    assert!(
        !painted
            .hits
            .accepts_release(on.row_id(3), ReleaseFrom::Secondary)
    );
    let mut off = ContextMenu::new(id, file_menu())
        .at(Position::new(4, 2))
        .release_selects(false);
    let painted = draw_menu(&mut off, screen);
    assert!(
        !painted
            .hits
            .accepts_release(off.row_id(0), ReleaseFrom::Secondary)
    );

    // Wheel: the viewport scrolls by default, the cursor moves in Cursor mode.
    let tall = || {
        (0..10)
            .map(|i| MenuItem::new(format!("Entry {i}")))
            .collect::<Vec<_>>()
    };
    let wheel = |menu: &mut ContextMenu, painted: &super::Painted| {
        let at = find_text(&painted.buf, "Entry 1").expect("a row to wheel over");
        let ev = PointerEvent::wheel_on(
            &painted.hits,
            menu.id,
            Axis::Vertical,
            1,
            at,
            KeyModifiers::NONE,
        );
        menu.pointer(&ev).into_parts().0
    };
    let mut view = ContextMenu::new(WidgetId::of("scenario.menu.wheel"), tall())
        .at(Position::new(4, 2))
        .max_rows(4);
    let painted = draw_menu(&mut view, screen);
    assert_eq!(wheel(&mut view, &painted), Outcome::Changed);
    assert_eq!(
        view.highlighted(),
        Some(0),
        "the viewport wheel leaves the cursor"
    );
    let scrolled = text_of(&draw_menu(&mut view, screen).buf);
    assert!(
        !scrolled.contains("Entry 0"),
        "the wheel scrolled the list:\n{scrolled}"
    );
    let mut cursor = ContextMenu::new(WidgetId::of("scenario.menu.wheel"), tall())
        .at(Position::new(4, 2))
        .max_rows(4)
        .wheel_mode(WheelMode::Cursor);
    let painted = draw_menu(&mut cursor, screen);
    wheel(&mut cursor, &painted);
    assert_eq!(
        cursor.highlighted(),
        Some(1),
        "Cursor mode moves the highlight"
    );

    // Overflow markers page the list.
    let mut paged = ContextMenu::new(WidgetId::of("scenario.menu.page"), tall())
        .at(Position::new(4, 2))
        .max_rows(4);
    let painted = draw_menu(&mut paged, screen);
    assert!(painted.hits.area_of(paged.more_below_id()).is_some());
    assert!(painted.hits.area_of(paged.more_above_id()).is_none());
    assert_eq!(paged.on_click(paged.more_below_id()), None);
    let text = text_of(&draw_menu(&mut paged, screen).buf);
    assert!(
        !text.contains("Entry 0") && text.contains("Entry 4"),
        "{text}"
    );
    let painted = draw_menu(&mut paged, screen);
    assert!(painted.hits.area_of(paged.more_above_id()).is_some());
    assert_eq!(paged.on_click(paged.more_above_id()), None);
    let text = text_of(&draw_menu(&mut paged, screen).buf);
    assert!(
        text.contains("Entry 0"),
        "the upper marker pages back:\n{text}"
    );
}

fn context_menu_layout() {
    let screen = Rect::new(0, 0, 40, 20);
    // size: the widest label plus its shortcut sets the width, rows set the height.
    let menu = ContextMenu::new(WidgetId::of("scenario.menu.size"), file_menu());
    assert_eq!(
        menu.size(),
        (19, 8),
        "6 lines (one separator) plus the frame"
    );
    assert_eq!(menu.clone().min_width(30).size().0, 36);
    assert_eq!(menu.clone().max_rows(3).size().1, 5);
    assert_eq!(menu.clone().title("A fairly long menu title").size().0, 30);
    assert_eq!(
        menu.clone().title("Files").size().1,
        9,
        "a title adds a row"
    );
    assert_eq!(menu.reason_room(false), 13);
    assert_eq!(menu.reason_room(true), 11);
    assert_eq!(reason_room(19, false), 13);
    assert_eq!(reason_room(19, true), 11);
    assert_eq!(reason_room(3, true), 0, "a tiny menu leaves no room");

    // Anchoring: a point opens below-right of the pointer, an anchor rect
    // opens on the requested side.
    let mut at =
        ContextMenu::new(WidgetId::of("scenario.menu.at"), file_menu()).at(Position::new(5, 3));
    draw_menu(&mut at, screen);
    assert_eq!((at.area.x, at.area.y), (5, 4));
    assert_eq!(at.area.width, 19);
    let mut above = ContextMenu::new(WidgetId::of("scenario.menu.above"), file_menu())
        .anchor(Rect::new(5, 12, 6, 1), Placement::Above);
    draw_menu(&mut above, screen);
    assert_eq!(
        above.area.bottom(),
        12,
        "Above ends where the anchor starts"
    );
    let mut right = ContextMenu::new(WidgetId::of("scenario.menu.right"), file_menu())
        .anchor(Rect::new(5, 5, 6, 1), Placement::Right);
    draw_menu(&mut right, screen);
    assert_eq!(right.area.x, 11);

    // Anchoring to a control follows where the control really is drawn.
    let control = WidgetId::of("scenario.menu.control");
    let mut follow = ContextMenu::new(WidgetId::of("scenario.menu.follow"), file_menu())
        .anchor_control(control, Rect::new(1, 1, 3, 1));
    paint_hits(screen, |area, buf, ctx| {
        ctx.clickable(control, Rect::new(20, 6, 8, 1));
        follow.render(area, buf, ctx);
    });
    assert_eq!(follow.anchor, Rect::new(20, 6, 8, 1));
    assert_eq!((follow.area.x, follow.area.y), (20, 7));
    let mut missing = ContextMenu::new(WidgetId::of("scenario.menu.follow"), file_menu())
        .anchor_control(control, Rect::new(1, 1, 3, 1));
    draw_menu(&mut missing, screen);
    assert_eq!(
        missing.anchor,
        Rect::new(1, 1, 3, 1),
        "the fallback is used"
    );

    // Edge margin and fit decide how close to the screen edge the menu may sit.
    let edge = Position::new(39, 3);
    let mut padded = ContextMenu::new(WidgetId::of("scenario.menu.edge"), file_menu()).at(edge);
    draw_menu(&mut padded, screen);
    assert_eq!(
        padded.area.right(),
        39,
        "the default margin keeps one column free"
    );
    let mut flush = ContextMenu::new(WidgetId::of("scenario.menu.edge"), file_menu())
        .edge_margin(0)
        .at(edge);
    draw_menu(&mut flush, screen);
    assert_eq!(flush.area.right(), 40);
    let mut fitted = ContextMenu::new(WidgetId::of("scenario.menu.edge"), file_menu())
        .fit(Place::FLUSH)
        .at(edge);
    draw_menu(&mut fitted, screen);
    assert_eq!(fitted.area.right(), 40);

    // Owner focus: an open menu hides the page focus unless told otherwise.
    let menu = ContextMenu::new(WidgetId::of("scenario.menu.focus"), file_menu());
    assert!(menu.hides_page_focus());
    assert!(!menu.hide_owner_focus(false).hides_page_focus());
}

const CHANNELS: [&str; 4] = ["Alpha", "Beta", "Gamma", "Delta"];

fn channel_select(selected: usize) -> Select {
    Select::new(
        WidgetId::of("scenario.select"),
        "Channel",
        &CHANNELS,
        selected,
    )
}

fn draw_select(select: &mut Select, area: Rect, screen: Rect) -> super::Painted {
    paint_hits(screen, |_, buf, ctx| {
        let bg = ctx.theme.canvas;
        select.render(area, buf, ctx, bg);
    })
}

fn select_keys() {
    assert_eq!(Select::HEIGHT, 3);
    assert_eq!(Select::MAX_ROWS, 8);
    assert_eq!(Select::POPUP_WIDTH, (12, 40));

    // Enter opens the list, arrows move inside it, Enter chooses.
    let mut select = channel_select(1).help("Where updates come from");
    assert_eq!(select.value(), "Beta");
    let painted = draw_select(&mut select, Rect::new(0, 0, 30, 3), Rect::new(0, 0, 30, 12));
    let text = text_of(&painted.buf);
    assert!(text.contains("Channel") && text.contains("Beta") && text.contains("Where updates"));
    assert_eq!(
        select.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, None)
    );
    assert!(select.open);
    assert_eq!(select.on_key(&key(KeyCode::Down)).0, Outcome::Changed);
    assert_eq!(
        select.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(SelectAction::Changed(2)))
    );
    assert!(!select.open);
    assert_eq!(select.value(), "Gamma");
    // Choosing the value that is already selected reports no change.
    select.on_key(&key(KeyCode::Enter));
    assert_eq!(
        select.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, None)
    );
    // Esc closes without choosing; Never keeps it open.
    select.on_key(&key(KeyCode::Enter));
    select.on_key(&key(KeyCode::Down));
    assert_eq!(select.on_key(&key(KeyCode::Esc)), (Outcome::Changed, None));
    assert!(!select.open && select.value() == "Gamma");
    let mut sticky = channel_select(0).dismiss(Dismiss::NEVER);
    sticky.on_key(&key(KeyCode::Enter));
    assert_eq!(sticky.on_key(&key(KeyCode::Esc)), (Outcome::Consumed, None));
    assert!(
        sticky.open,
        "Esc does not close a popup whose policy forbids it"
    );

    // Arrows: Open opens the list, Step changes the value in place.
    let mut opens = channel_select(1);
    assert_eq!(opens.on_key(&key(KeyCode::Down)), (Outcome::Changed, None));
    assert!(opens.open);
    let mut steps = channel_select(1).arrows(SelectArrows::Step);
    assert_eq!(
        steps.on_key(&key(KeyCode::Down)),
        (Outcome::Changed, Some(SelectAction::Changed(2)))
    );
    assert!(!steps.open);
    assert_eq!(
        steps.on_key(&key(KeyCode::Left)),
        (Outcome::Changed, Some(SelectAction::Changed(1)))
    );
    assert_eq!(opens.arrows, SelectArrows::Open);

    // Wrap decides what Up does on the first value.
    let mut wraps = channel_select(0).arrows(SelectArrows::Step).wrap(true);
    assert_eq!(
        wraps.on_key(&key(KeyCode::Up)),
        (Outcome::Changed, Some(SelectAction::Changed(3)))
    );
    let mut stops = channel_select(0).arrows(SelectArrows::Step);
    assert_eq!(stops.on_key(&key(KeyCode::Up)), (Outcome::Changed, None));
    assert_eq!(
        stops.value(),
        "Alpha",
        "without wrap Up stays on the first value"
    );

    // Tab in an open list: choose, close, or swallow.
    for (policy, outcome, action, stays_open) in [
        (
            TabInOpen::Choose,
            Outcome::Ignored,
            Some(SelectAction::Changed(2)),
            false,
        ),
        (TabInOpen::Close, Outcome::Ignored, None, false),
        (TabInOpen::Swallow, Outcome::Consumed, None, true),
    ] {
        let mut select = channel_select(0).tab_in_open(policy);
        select.on_key(&key(KeyCode::Enter));
        select.on_key(&key(KeyCode::Down));
        select.on_key(&key(KeyCode::Down));
        assert_eq!(
            select.on_key(&key(KeyCode::Tab)),
            (outcome, action),
            "{policy:?}"
        );
        assert_eq!(select.open, stays_open, "{policy:?}");
    }
    assert_eq!(TabInOpen::default(), TabInOpen::Choose);

    // Type-ahead picks a value without opening; it can be turned off.
    let mut typing = channel_select(0);
    assert_eq!(
        typing.on_key(&key(KeyCode::Char('g'))),
        (Outcome::Changed, Some(SelectAction::Changed(2)))
    );
    let mut deaf = channel_select(0).type_ahead(false);
    assert_eq!(
        deaf.on_key(&key(KeyCode::Char('g'))),
        (Outcome::Ignored, None)
    );
    let mut prefix = channel_select(0).type_ahead_timeout(std::time::Duration::from_secs(60));
    prefix.on_key(&key(KeyCode::Char('d')));
    assert_eq!(prefix.value(), "Delta");
    // With the Vim profile a shortcut key navigates by default; the
    // type-ahead precedence option lets the same key type instead.
    let jade = ["Alpha", "Jade", "Kilo"];
    let vim = |p: Option<TypeAheadPrecedence>| {
        let s = Select::new(WidgetId::of("scenario.select.vim"), "Gem", &jade, 0)
            .nav(NavKeys::VIM)
            .type_ahead(true);
        match p {
            Some(p) => s.type_ahead_precedence(p),
            None => s,
        }
    };
    // With Shortcuts precedence the first routing pass leaves a typed key to
    // the app's shortcuts; with TypeAhead the select takes it at once.
    let passes = |p: TypeAheadPrecedence| {
        let mut select = vim(Some(p));
        let mut seen = vec![];
        route_key(&key(KeyCode::Char('j')), || {
            let (outcome, _) = select.on_key(&key(KeyCode::Char('j')));
            seen.push(outcome);
            outcome
        });
        (seen, select.value().to_owned())
    };
    assert_eq!(
        passes(TypeAheadPrecedence::Shortcuts),
        (vec![Outcome::Ignored, Outcome::Changed], "Jade".to_owned())
    );
    assert_eq!(
        passes(TypeAheadPrecedence::TypeAhead),
        (vec![Outcome::Changed], "Jade".to_owned())
    );
    assert_eq!(vim(None).type_ahead_precedence, None);

    // Mixed values show a placeholder and any choice resolves them.
    let mut mixed = channel_select(1).mixed("Multiple values");
    let text =
        text_of(&draw_select(&mut mixed, Rect::new(0, 0, 30, 3), Rect::new(0, 0, 30, 6)).buf);
    assert!(text.contains("Multiple values"), "{text}");
    mixed.on_key(&key(KeyCode::Enter));
    assert_eq!(
        mixed.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(SelectAction::Changed(1))),
        "choosing the current value still resolves a mixed field"
    );
    assert!(mixed.mixed.is_none());
    mixed.set_mixed(Some("Again"));
    assert_eq!(mixed.mixed.as_deref(), Some("Again"));
    mixed.set_mixed(None);
    assert!(mixed.mixed.is_none());

    // Disabled and fixed controls do not answer.
    let mut disabled = channel_select(0).disabled(true);
    assert_eq!(
        disabled.on_key(&key(KeyCode::Enter)),
        (Outcome::Ignored, None)
    );
    clear_refusal();
    let mut fixed = channel_select(0).fixed(Reason::new("Managed by policy"));
    assert_eq!(
        fixed.on_key(&key(KeyCode::Enter)),
        (Outcome::Consumed, None)
    );
    assert!(!fixed.open);
    assert_eq!(
        take_refusal().map(|r| r.text),
        Some("Managed by policy".to_owned())
    );
    let mut live = channel_select(0).control(Control::Live);
    assert_eq!(live.on_key(&key(KeyCode::Enter)), (Outcome::Changed, None));
    assert_eq!(take_refusal(), None);
    let mut locked = channel_select(0).control(Control::fixed(Reason::new("Locked")));
    assert_eq!(
        locked.on_key(&key(KeyCode::Down)),
        (Outcome::Consumed, None)
    );
    clear_refusal();

    // Closing: close(), blur and the close-on-blur option.
    let mut select = channel_select(0);
    assert_eq!(select.close(), Outcome::Ignored);
    select.on_key(&key(KeyCode::Enter));
    assert_eq!(
        select.on_blur(),
        Outcome::Changed,
        "blur closes a light popup"
    );
    assert!(!select.open);
    let mut keeps = channel_select(0).close_on_blur(false);
    keeps.on_key(&key(KeyCode::Enter));
    assert_eq!(keeps.on_blur(), Outcome::Ignored);
    assert!(keeps.open);
    assert_eq!(keeps.close(), Outcome::Changed);
    assert!(!keeps.open);
}

fn select_pointer() {
    let area = Rect::new(2, 1, 30, 3);
    let screen = Rect::new(0, 0, 40, 14);
    let mut select = channel_select(1);
    draw_select(&mut select, area, screen);

    // A click on the field opens it, a second click closes it.
    assert_eq!(select.on_click(select.id), (Outcome::Changed, None));
    assert!(select.open);
    assert_eq!(select.on_click(select.id), (Outcome::Changed, None));
    assert!(!select.open);
    select.on_click(select.id);
    // A click on an option chooses it.
    assert_eq!(
        select.on_click(select.option_id(3)),
        (Outcome::Changed, Some(SelectAction::Changed(3)))
    );
    assert_eq!((select.value(), select.open), ("Delta", false));
    assert_eq!(
        select.on_click(WidgetId::of("scenario.unrelated")),
        (Outcome::Ignored, None)
    );

    // Outside clicks follow the Dismiss policy.
    for (policy, closes, outcome) in [
        (Dismiss::LIGHT, true, Outcome::Changed),
        (Dismiss::ESCAPE, false, Outcome::Consumed),
    ] {
        let mut select = channel_select(0).dismiss(policy);
        assert_eq!(
            select.on_click_outside(),
            Outcome::Ignored,
            "closed: nothing to dismiss"
        );
        select.on_click(select.id);
        assert_eq!(select.on_click_outside(), outcome);
        assert_eq!(!select.open, closes);
    }
    let mut via_backdrop = channel_select(0);
    via_backdrop.on_click(via_backdrop.id);
    assert_eq!(
        via_backdrop.on_click(popup::backdrop_id(via_backdrop.id)),
        (Outcome::Changed, None)
    );
    assert!(!via_backdrop.open);

    // Press, drag and release picks an option in one gesture.
    let mut drag = channel_select(0);
    draw_select(&mut drag, area, screen);
    let field = Position::new(area.x + 6, area.y + 1);
    assert_eq!(
        drag.on_press(drag.id, field),
        Outcome::Changed,
        "press opens the list"
    );
    assert!(drag.open);
    let painted = draw_select(&mut drag, area, screen);
    let gamma = find_text(&painted.buf, "Gamma").expect("the Gamma option");
    assert_eq!(drag.on_drag(drag.id, gamma), Outcome::Changed);
    assert_eq!(
        drag.on_release(gamma),
        (Outcome::Changed, Some(SelectAction::Changed(2)))
    );
    assert!(!drag.open);
    // A release without movement keeps the list open for the click that follows.
    let mut still = channel_select(0);
    draw_select(&mut still, area, screen);
    still.on_press(still.id, field);
    assert_eq!(still.on_release(field), (Outcome::Consumed, None));
    assert!(still.open, "a release without movement keeps the list open");
    let mut clicked = channel_select(0);
    draw_select(&mut clicked, area, screen);
    clicked.on_press(clicked.id, field);
    assert_eq!(clicked.on_click(clicked.id), (Outcome::Changed, None));
    assert!(
        clicked.open,
        "the click that ends the opening press does not close it"
    );
    // press_drag(false): the press does not open, the click does.
    let mut plain = channel_select(0).press_drag(false);
    draw_select(&mut plain, area, screen);
    assert_eq!(plain.on_press(plain.id, field), Outcome::Consumed);
    assert!(!plain.open);
    plain.on_click(plain.id);
    assert!(plain.open);
    assert_eq!(plain.on_drag(plain.id, field), Outcome::Ignored);
    // The label is its own hit area unless label_hit is turned off.
    let label = Position::new(area.x + 1, area.y);
    let mut with_label = channel_select(0);
    draw_select(&mut with_label, area, screen);
    assert_eq!(with_label.on_press(with_label.id, label), Outcome::Consumed);
    assert!(
        !with_label.open,
        "a press on the label does not open on press"
    );
    let mut without = channel_select(0).label_hit(false);
    draw_select(&mut without, area, screen);
    assert_eq!(
        without.on_press(without.id, label),
        Outcome::Changed,
        "without a label hit area the label opens the list like the field"
    );
    assert!(without.open);
    // Disabled controls ignore every pointer phase.
    let mut off = channel_select(0).disabled(true);
    draw_select(&mut off, area, screen);
    assert_eq!(off.on_click(off.id), (Outcome::Consumed, None));
    assert_eq!(off.on_press(off.id, field), Outcome::Consumed);
    assert_eq!(off.on_drag(off.id, field), Outcome::Ignored);
    // A fixed control refuses and says why.
    clear_refusal();
    let mut fixed = channel_select(0).fixed(Reason::new("Managed by policy"));
    draw_select(&mut fixed, area, screen);
    assert_eq!(fixed.on_click(fixed.id), (Outcome::Consumed, None));
    assert!(take_refusal().is_some());
    assert_eq!(fixed.on_press(fixed.id, field), Outcome::Consumed);
    clear_refusal();

    // The wheel scrolls a long open list; the scrollbar accepts a press.
    let long: Vec<String> = (0..20).map(|i| format!("Option {i}")).collect();
    let refs: Vec<&str> = long.iter().map(String::as_str).collect();
    let mut tall = Select::new(WidgetId::of("scenario.select.tall"), "Many", &refs, 0)
        .max_rows(4)
        .track_click(TrackClick::Jump)
        .wheel(WheelMode::Viewport);
    tall.on_click(tall.id);
    let painted = draw_select(&mut tall, area, screen);
    assert!(
        painted.hits.area_of(tall.bar_id()).is_some(),
        "a long list has a scrollbar"
    );
    let at = find_text(&painted.buf, "Option 1").expect("a visible option");
    let ev = PointerEvent::wheel_on(
        &painted.hits,
        tall.id,
        Axis::Vertical,
        2,
        at,
        KeyModifiers::NONE,
    );
    assert_eq!(tall.pointer(&ev).into_parts().0, Outcome::Changed);
    let text = text_of(&draw_select(&mut tall, area, screen).buf);
    assert!(
        text.contains("Option 2") && !text.contains("Option 1"),
        "the wheel scrolled the options:\n{text}"
    );
    assert_eq!(tall.on_click(tall.bar_id()), (Outcome::Consumed, None));
}

fn select_popup() {
    let area = Rect::new(2, 6, 20, 3);
    let screen = Rect::new(0, 0, 40, 16);
    let surface = |painted: &super::Painted| {
        painted
            .hits
            .regions()
            .iter()
            .find(|r| r.id == WidgetId::of("popup.surface"))
            .map(|r| r.area)
            .expect("an open select draws a popup surface")
    };
    let open = |mut select: Select| {
        select.on_click(select.id);
        let painted = draw_select(&mut select, area, screen);
        (select, painted)
    };

    // Placement: below by default, above on request.
    let (_, painted) = open(channel_select(0));
    let below = surface(&painted);
    assert!(below.y >= area.y + 2, "below the field: {below:?}");
    let (_, painted) = open(channel_select(0).placement(Placement::Above));
    let above = surface(&painted);
    assert!(above.bottom() <= area.y + 1, "above the field: {above:?}");
    let (_, painted) = open(channel_select(0).placement(Placement::Right));
    assert!(surface(&painted).x >= area.right());

    // Rows and width.
    assert_eq!(below.height, 6, "four options plus the frame");
    let (_, painted) = open(channel_select(0).max_rows(2));
    assert_eq!(surface(&painted).height, 4, "max_rows caps the popup");
    assert_eq!(
        channel_select(0).max_rows(0).list.max_rows,
        1,
        "at least one row"
    );
    let (_, painted) = open(channel_select(0).popup_width(30, 36));
    assert_eq!(
        surface(&painted).width,
        30,
        "the popup is at least popup_width.0 wide"
    );
    let (_, painted) = open(channel_select(0).popup_width(5, 8));
    assert_eq!(surface(&painted).width, 8, "and at most popup_width.1 wide");
    assert_eq!(below.width, 20, "otherwise as wide as the field");
    let tight = channel_select(0).popup_width(9, 3);
    assert_eq!(
        tight.list.width,
        (9, 9),
        "an inverted range collapses to its minimum"
    );

    // Fit and cursor style.
    let (_, painted) = open(
        channel_select(0)
            .popup_fit(Place::FLUSH)
            .placement(Placement::Center),
    );
    assert_eq!(surface(&painted).x, (screen.width - 20) / 2);
    let styled = [CursorStyle::Fill, CursorStyle::Bar].map(|style| {
        let mut select = channel_select(0).cursor_style(style);
        select.on_click(select.id);
        let focus = select.id;
        paint_hits(screen, |_, buf, ctx| {
            ctx.interaction.focus = Some(focus);
            let bg = ctx.theme.canvas;
            select.render(area, buf, ctx, bg);
        })
        .buf
    });
    assert_ne!(
        styled[0], styled[1],
        "the cursor row looks different per style"
    );

    // Hover can move the cursor.
    let hover = |on: bool| {
        let mut select = channel_select(0).hover_moves_cursor(on);
        select.on_click(select.id);
        let t = junie_tui::theme::Theme::for_level(junie_tui::theme::ColorLevel::TrueColor);
        let mut buf = ratatui::buffer::Buffer::empty(screen);
        let mut hits = junie_tui::core::hit::HitRegistry::default();
        let mut ring = junie_tui::core::focus::FocusRing::default();
        let hovered = select.option_id(2);
        let interaction = junie_tui::ui::ctx::Interaction {
            hover: Some(hovered),
            ..Default::default()
        };
        let mut ctx = junie_tui::ui::ctx::RenderCtx::new(&t, interaction, &mut hits, &mut ring);
        select.render(area, &mut buf, &mut ctx, t.canvas);
        ctx.flush_overlays(&mut buf);
        select.list.cursor.index
    };
    assert_eq!(hover(true), 2, "a hovered row takes the cursor");
    assert_eq!(hover(false), 0, "unless hover_moves_cursor is off");

    // Row ids and scrollbar id are distinct and stable.
    let select = channel_select(0);
    assert_ne!(select.option_id(0), select.option_id(1));
    assert_ne!(select.bar_id(), select.id);
    assert_eq!(select.nav(NavKeys::VIM).list.nav, NavKeys::VIM);
}

fn app_bar() -> MenuBar {
    MenuBar::new(
        WidgetId::of("scenario.bar"),
        vec![
            (
                "File",
                vec![
                    MenuItem::new("New").shortcut("n"),
                    MenuItem::new("Open"),
                    MenuItem::new("Quit").danger(),
                ],
            ),
            (
                "Edit",
                vec![
                    MenuItem::new("Undo"),
                    MenuItem::new("Redo"),
                    MenuItem::new("Repeat"),
                ],
            ),
            ("View", vec![MenuItem::new("Zoom")]),
        ],
    )
}

fn draw_bar(bar: &mut MenuBar, area: Rect, screen: Rect) -> super::Painted {
    paint_hits(screen, |_, buf, ctx| {
        let bg = ctx.theme.canvas;
        bar.render(area, buf, ctx, bg);
    })
}

fn menu_bar_keys() {
    let area = Rect::new(0, 0, 60, 1);
    let screen = Rect::new(0, 0, 60, 12);
    let mut bar = app_bar();
    let painted = draw_bar(&mut bar, area, screen);
    let text = text_of(&painted.buf);
    assert!(text.contains(" File ") && text.contains(" Edit ") && text.contains(" View "));
    assert!(!bar.is_open() && bar.open_index().is_none());
    assert!(!bar.hides_page_focus());

    // Arrows move between titles while closed; Down or Enter opens one.
    assert_eq!(bar.on_key(&key(KeyCode::Right)), (Outcome::Changed, None));
    assert_eq!(bar.cursor, 1);
    assert_eq!(
        bar.on_key(&key(KeyCode::Down)),
        (Outcome::Changed, Some(MenuBarAction::Opened(1)))
    );
    assert_eq!((bar.is_open(), bar.open_index()), (true, Some(1)));
    assert!(bar.hides_page_focus(), "an open menu hides the page focus");
    draw_bar(&mut bar, area, screen);
    // Left and Right switch the open menu; wrap goes round.
    assert_eq!(
        bar.on_key(&key(KeyCode::Right)),
        (Outcome::Changed, Some(MenuBarAction::Opened(2)))
    );
    assert_eq!(
        bar.on_key(&key(KeyCode::Right)),
        (Outcome::Changed, Some(MenuBarAction::Opened(0))),
        "Right wraps from the last title to the first"
    );
    assert_eq!(
        bar.on_key(&key(KeyCode::Left)),
        (Outcome::Changed, Some(MenuBarAction::Opened(2)))
    );
    // Enter chooses an item and closes.
    bar.on_key(&key(KeyCode::Left));
    assert_eq!(bar.open_index(), Some(1));
    bar.on_key(&key(KeyCode::Down));
    assert_eq!(
        bar.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(MenuBarAction::Chosen(1, 1)))
    );
    assert!(!bar.is_open());
    // An accelerator chooses in one key.
    bar.open_menu(0);
    assert_eq!(
        bar.on_key(&key(KeyCode::Char('n'))),
        (Outcome::Changed, Some(MenuBarAction::Chosen(0, 0)))
    );
    // Esc closes; an unknown key closes and passes through.
    bar.open_menu(0);
    assert_eq!(
        bar.on_key(&key(KeyCode::Esc)),
        (Outcome::Changed, Some(MenuBarAction::Closed))
    );
    bar.open_menu(0);
    assert_eq!(
        bar.on_key(&key(KeyCode::F(5))),
        (Outcome::Ignored, Some(MenuBarAction::Closed))
    );
    assert!(!bar.is_open());
    // Home and End jump to the first and last title.
    bar.on_key(&key(KeyCode::End));
    assert_eq!(bar.cursor, 2);
    bar.on_key(&key(KeyCode::Home));
    assert_eq!(bar.cursor, 0);
    // A key the bar does not use is left alone.
    assert_eq!(
        bar.on_key(&key(KeyCode::Char('x'))),
        (Outcome::Ignored, None)
    );

    // Programmatic open and close; an index past the last title is ignored.
    bar.open_menu(1);
    assert_eq!(bar.open_index(), Some(1));
    bar.close();
    assert!(!bar.is_open());
    bar.open_menu(9);
    assert!(!bar.is_open());

    // The dismiss policy governs Esc.
    let mut sticky = app_bar().dismiss(Dismiss::NEVER);
    sticky.open_menu(0);
    assert_eq!(sticky.on_key(&key(KeyCode::Esc)), (Outcome::Consumed, None));
    assert!(sticky.is_open());
    assert_eq!(sticky.dismiss, Dismiss::NEVER);

    // Wrap off stops at the ends.
    let mut flat = app_bar().wrap(false);
    flat.open_menu(0);
    assert_eq!(
        flat.on_key(&key(KeyCode::Left)),
        (Outcome::Changed, Some(MenuBarAction::Opened(0)))
    );
    // Nav profile: l moves right in the Vim profile.
    let mut vim = app_bar().nav(NavKeys::VIM).type_ahead(false);
    assert_eq!(
        vim.on_key(&key(KeyCode::Char('l'))),
        (Outcome::Changed, None)
    );
    assert_eq!(vim.cursor, 1);
    let mut desktop = app_bar().type_ahead(false);
    assert_eq!(
        desktop.on_key(&key(KeyCode::Char('l'))),
        (Outcome::Ignored, None)
    );
    assert_eq!(desktop.nav, NavKeys::DEFAULT);
    assert_eq!(vim.nav, NavKeys::VIM);

    // Type-ahead inside an open menu, with its timeout.
    let typed = |bar: &mut MenuBar, keys: &str, pause: bool| {
        bar.open_menu(1);
        for c in keys.chars() {
            if pause {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            bar.on_key(&key(KeyCode::Char(c)));
        }
        bar.open.as_ref().and_then(ContextMenu::highlighted)
    };
    let mut patient = app_bar()
        .type_ahead(true)
        .type_ahead_timeout(std::time::Duration::from_secs(60));
    assert_eq!(
        typed(&mut patient, "rep", true),
        Some(2),
        "r, e, p builds the prefix: Repeat"
    );
    let mut hasty = app_bar()
        .type_ahead(true)
        .type_ahead_timeout(std::time::Duration::ZERO);
    assert_eq!(
        typed(&mut hasty, "rep", true),
        Some(1),
        "each key starts over, so e and p find nothing"
    );
    assert_eq!(hasty.type_ahead_timeout, std::time::Duration::ZERO);
    let mut off = app_bar().type_ahead(false);
    off.open_menu(1);
    assert_eq!(
        off.on_key(&key(KeyCode::Char('r'))),
        (Outcome::Ignored, Some(MenuBarAction::Closed)),
        "without type-ahead r is an unknown key"
    );
    assert!(!off.type_ahead);

    // The owner keeps its focus when asked.
    let mut keeps = app_bar().hide_owner_focus(false);
    keeps.open_menu(0);
    assert!(!keeps.hides_page_focus());
}

fn menu_bar_pointer() {
    let area = Rect::new(0, 0, 60, 1);
    let screen = Rect::new(0, 0, 60, 12);
    let mut bar = app_bar().brand(Lockup::new("Acme"));
    let painted = draw_bar(&mut bar, area, screen);
    assert!(text_of(&painted.buf).contains("Acme"), "the brand is drawn");
    assert!(painted.hits.area_of(bar.brand_id()).is_some());
    assert!(painted.hits.area_of(bar.label_id(1)).is_some());
    assert_ne!(bar.label_id(0), bar.label_id(1));

    // Brand click.
    assert_eq!(
        bar.on_click(bar.brand_id()),
        (Outcome::Changed, Some(MenuBarAction::Brand))
    );
    assert_eq!(
        bar.on_click(WidgetId::of("scenario.elsewhere")),
        (Outcome::Ignored, None)
    );

    // Press opens at once (the default), and the click that follows is spent.
    assert_eq!(
        bar.on_press(bar.label_id(1)),
        (Outcome::Changed, Some(MenuBarAction::Opened(1)))
    );
    assert_eq!(bar.on_click(bar.label_id(1)), (Outcome::Consumed, None));
    assert_eq!(bar.open_index(), Some(1));
    // Pressing the open title again closes it.
    assert_eq!(
        bar.on_press(bar.label_id(1)),
        (Outcome::Changed, Some(MenuBarAction::Closed))
    );
    assert_eq!(
        bar.on_press(WidgetId::of("scenario.elsewhere")),
        (Outcome::Ignored, None)
    );
    // Release mode opens on the click instead.
    let mut late = app_bar().open_on(OpenOn::Release);
    draw_bar(&mut late, area, screen);
    assert_eq!(late.on_press(late.label_id(0)), (Outcome::Ignored, None));
    assert!(!late.is_open());
    assert_eq!(
        late.on_click(late.label_id(0)),
        (Outcome::Changed, Some(MenuBarAction::Opened(0)))
    );
    assert_eq!(
        late.on_click(late.label_id(0)),
        (Outcome::Changed, Some(MenuBarAction::Closed)),
        "a click on the open title closes it"
    );
    assert_eq!(OpenOn::default(), OpenOn::Press);

    // Clicking an item chooses it; the backdrop closes.
    let mut bar = app_bar();
    draw_bar(&mut bar, area, screen);
    bar.open_menu(0);
    let painted = draw_bar(&mut bar, area, screen);
    let item = bar.open.as_ref().expect("open menu").row_id(1);
    assert!(painted.hits.area_of(item).is_some());
    assert_eq!(
        bar.on_click(item),
        (Outcome::Changed, Some(MenuBarAction::Chosen(0, 1)))
    );
    bar.open_menu(0);
    let backdrop = popup::backdrop_id(bar.id);
    assert_eq!(
        bar.on_click(backdrop),
        (Outcome::Changed, Some(MenuBarAction::Closed))
    );
    let mut sticky = app_bar().dismiss(Dismiss::ESCAPE);
    sticky.open_menu(0);
    assert_eq!(
        sticky.on_click(backdrop_id(sticky.id)),
        (Outcome::Consumed, None)
    );
    assert!(sticky.is_open());

    // Hover switches the open menu to the title under the pointer.
    let mut hover = app_bar();
    draw_bar(&mut hover, area, screen);
    assert_eq!(
        hover.on_hover(Some(hover.label_id(1))),
        Outcome::Ignored,
        "closed bars do not switch"
    );
    hover.open_menu(0);
    assert_eq!(hover.on_hover(Some(hover.label_id(2))), Outcome::Changed);
    assert_eq!(hover.open_index(), Some(2));
    assert_eq!(hover.on_hover(Some(hover.label_id(2))), Outcome::Ignored);
    assert_eq!(hover.on_hover(None), Outcome::Ignored);
    let mut still = app_bar().hover_switch(false);
    still.open_menu(0);
    assert_eq!(still.on_hover(Some(still.label_id(2))), Outcome::Ignored);
    assert_eq!(still.open_index(), Some(0));
    assert!(!still.hover_switch);

    // Press-drag: releasing over an item of the open menu may pick it.
    let mut drag = app_bar();
    draw_bar(&mut drag, area, screen);
    drag.open_menu(0);
    let painted = draw_bar(&mut drag, area, screen);
    let row = drag.open.as_ref().expect("open").row_id(0);
    assert!(
        painted
            .hits
            .accepts_release(row, ReleaseFrom::Press(drag.label_id(0)))
    );
    let mut no_drag = app_bar().press_drag(false);
    draw_bar(&mut no_drag, area, screen);
    no_drag.open_menu(0);
    let painted = draw_bar(&mut no_drag, area, screen);
    let row = no_drag.open.as_ref().expect("open").row_id(0);
    assert!(
        !painted
            .hits
            .accepts_release(row, ReleaseFrom::Press(no_drag.label_id(0)))
    );
    assert!(!no_drag.press_drag);
}

fn menu_bar_overflow() {
    let narrow = Rect::new(0, 0, 16, 1);
    let screen = Rect::new(0, 0, 40, 10);
    let mut hide = app_bar().overflow(BarOverflow::Hide);
    draw_bar(&mut hide, narrow, screen);
    assert!(
        hide.hidden().is_empty(),
        "Hide just drops titles that do not fit"
    );
    assert!(hide.more_area.is_empty());
    assert_eq!(BarOverflow::default(), BarOverflow::Hide);

    let mut bar = app_bar().overflow(BarOverflow::Chevron);
    let painted = draw_bar(&mut bar, narrow, screen);
    assert_eq!(
        bar.hidden(),
        vec![1, 2],
        "Edit and View collapse into the chevron"
    );
    assert!(!bar.more_area.is_empty());
    assert!(painted.hits.area_of(bar.more_id()).is_some());
    let text = text_of(&painted.buf);
    assert!(
        text.contains(" File ") && !text.contains(" Edit "),
        "{text}"
    );

    // Opening the chevron lists the hidden titles; choosing one opens it.
    bar.open_menu(3);
    assert_eq!(bar.open_index(), Some(3));
    let painted = draw_bar(&mut bar, narrow, screen);
    let text = text_of(&painted.buf);
    assert!(text.contains("Edit") && text.contains("View"), "{text}");
    assert_eq!(
        bar.on_key(&key(KeyCode::Enter)),
        (Outcome::Changed, Some(MenuBarAction::Opened(1)))
    );
    assert_eq!(bar.open_index(), Some(1));
    // Clicking the chevron title opens and closes the hidden list.
    bar.close();
    draw_bar(&mut bar, narrow, screen);
    assert_eq!(
        bar.on_click(bar.more_id()),
        (Outcome::Changed, Some(MenuBarAction::Opened(3)))
    );
    assert_eq!(
        bar.on_click(bar.more_id()),
        (Outcome::Changed, Some(MenuBarAction::Closed))
    );
    // The cursor reaches the chevron with the arrows.
    bar.close();
    bar.cursor = 0;
    assert_eq!(bar.on_key(&key(KeyCode::Right)), (Outcome::Changed, None));
    assert_eq!(
        bar.cursor, 3,
        "Right steps from the last shown title to the chevron"
    );
}

fn row_menu_flow() {
    let id = WidgetId::of("scenario.rowmenu");
    let screen = Rect::new(0, 0, 40, 12);
    let items = || {
        vec![
            (MenuItem::new("Open"), "open"),
            (MenuItem::new("Delete").danger(), "delete"),
        ]
    };
    let mut menu: RowMenu<usize, &str> = RowMenu::new(id);
    assert!(!menu.is_open() && !menu.hides_page_focus());
    assert_eq!(menu.on_key(&key(KeyCode::Enter)), (Outcome::Ignored, None));
    assert_eq!(menu.on_click(id), (Outcome::Ignored, None));

    menu.open(
        7,
        ContextAnchor::Pointer(Position::new(3, 2)),
        "Row 7",
        items(),
    );
    assert!(menu.is_open() && menu.hides_page_focus());
    let painted = paint_hits(screen, |area, buf, ctx| menu.render(area, buf, ctx));
    let text = text_of(&painted.buf);
    assert!(text.contains("Row 7") && text.contains("Open") && text.contains("Delete"));
    // Enter chooses the highlighted row and returns the row it was opened for.
    assert_eq!(menu.on_key(&key(KeyCode::Down)).0, Outcome::Changed);
    let (out, chosen) = menu.on_key(&key(KeyCode::Enter));
    assert_eq!((out, chosen), (Outcome::Changed, Some((7, "delete"))));
    assert!(!menu.is_open());
    // Esc closes without a choice.
    menu.open(
        8,
        ContextAnchor::Pointer(Position::new(3, 2)),
        "Row 8",
        items(),
    );
    assert_eq!(menu.on_key(&key(KeyCode::Esc)), (Outcome::Changed, None));
    assert!(!menu.is_open());
    // An unknown key closes it and is passed on.
    menu.open(
        9,
        ContextAnchor::Pointer(Position::new(3, 2)),
        "Row 9",
        items(),
    );
    assert_eq!(menu.on_key(&key(KeyCode::F(5))), (Outcome::Ignored, None));
    assert!(!menu.is_open());
    // A click on a row chooses it.
    menu.open(
        10,
        ContextAnchor::Pointer(Position::new(3, 2)),
        "Row 10",
        items(),
    );
    let row = ContextMenu::new(id, vec![MenuItem::new("a"), MenuItem::new("b")]).row_id(0);
    assert_eq!(menu.on_click(row), (Outcome::Changed, Some((10, "open"))));
    // An anchor on a control follows it.
    menu.open(
        11,
        ContextAnchor::Control {
            id: WidgetId::of("scenario.row"),
            fallback: Rect::new(5, 5, 4, 1),
        },
        "Row 11",
        items(),
    );
    assert!(menu.is_open());
    paint_hits(screen, |area, buf, ctx| menu.render(area, buf, ctx));

    // Context keys: Shift+F10 and the Menu key by default, configurable.
    let shift_f10 = ratatui::crossterm::event::KeyEvent::new(KeyCode::F(10), KeyModifiers::SHIFT);
    let shift_f10 = junie_tui::core::event::Key {
        code: shift_f10.code,
        mods: shift_f10.modifiers,
    };
    let menu_key = key(KeyCode::Menu);
    let fresh: RowMenu<usize, &str> = RowMenu::new(id);
    assert!(fresh.is_context_key(&shift_f10) && fresh.is_context_key(&menu_key));
    assert!(!fresh.is_context_key(&key(KeyCode::F(10))));
    let mut custom: RowMenu<usize, &str> = RowMenu::new(id);
    custom.config = ContextConfig::DEFAULT.keys(ContextKeys::NONE.extra(Some(key(KeyCode::F(9)))));
    assert!(custom.is_context_key(&key(KeyCode::F(9))) && !custom.is_context_key(&shift_f10));

    // The footer hints only name keys a terminal delivers.
    let hints = row_menu::hints();
    assert_eq!(
        hints
            .iter()
            .map(|h| (h.key.as_ref(), h.action.as_ref()))
            .collect::<Vec<_>>(),
        [("↑↓", "Move"), ("Enter", "Choose"), ("Esc", "Close")]
    );
    assert!(
        hints
            .iter()
            .all(|h| junie_tui::core::keyboard::label_deliverable(&h.key))
    );
}

/// A minimal context source: the pointer row is the target.
struct Rows {
    id: WidgetId,
    cursor: usize,
    len: usize,
}

impl ContextSource for Rows {
    type Target = usize;

    fn context_target(&self, ev: &PointerEvent) -> Option<usize> {
        ev.on(self.id).then(|| ev.index_of(Part::ROW))?
    }

    fn context_select(&mut self, target: usize) {
        self.cursor = target;
    }

    fn context_cursor(&self) -> Option<ContextRequest<usize>> {
        (self.cursor < self.len).then(|| ContextRequest {
            target: self.cursor,
            anchor: ContextAnchor::Pointer(Position::new(1, self.cursor as u16)),
        })
    }
}

fn shift_f10() -> Key {
    Key {
        code: KeyCode::F(10),
        mods: KeyModifiers::SHIFT,
    }
}

fn context_requests() {
    // Keys: Shift+F10 and the Menu key by default, nothing when switched off.
    assert!(ContextKeys::DEFAULT.matches(&shift_f10()));
    assert!(ContextKeys::default().matches(&key(KeyCode::Menu)));
    assert!(!ContextKeys::DEFAULT.matches(&key(KeyCode::F(10))));
    assert!(
        !ContextKeys::NONE.matches(&shift_f10()) && !ContextKeys::NONE.matches(&key(KeyCode::Menu))
    );
    assert!(!ContextKeys::DEFAULT.shift_f10(false).matches(&shift_f10()));
    assert!(
        ContextKeys::DEFAULT
            .shift_f10(false)
            .matches(&key(KeyCode::Menu))
    );
    assert!(
        !ContextKeys::DEFAULT
            .menu_key(false)
            .matches(&key(KeyCode::Menu))
    );
    let extra = ContextKeys::NONE.extra(Some(key(KeyCode::F(9))));
    assert!(extra.matches(&key(KeyCode::F(9))) && !extra.matches(&key(KeyCode::F(8))));
    assert!(!extra.extra(None).matches(&key(KeyCode::F(9))));
    assert_eq!(extra.extra, Some(key(KeyCode::F(9))));

    // A real list is a context source: a secondary press over a row asks for
    // a menu on that row and moves the cursor there.
    let id = WidgetId::of("scenario.context.list");
    let items = ["Alpha", "Beta", "Gamma"].map(ListItem::new).to_vec();
    let mut list = ListBox::new(id, items.clone(), SelectMode::Single);
    let screen = Rect::new(0, 0, 30, 8);
    let draw = |list: &mut ListBox| {
        paint_hits(screen, |area, buf, ctx| {
            let bg = ctx.theme.canvas;
            list.render(area, buf, ctx, bg);
        })
    };
    let painted = draw(&mut list);
    let at = find_text(&painted.buf, "Beta").expect("Beta row");
    let ev = painted.event(Phase::Secondary, id, Part::ROW, at);
    let config = ContextConfig::DEFAULT;
    assert_eq!(config, ContextConfig::default());
    let request = config
        .on_secondary(&mut list, &ev)
        .expect("a row under the pointer");
    assert_eq!(request.target, 1);
    assert_eq!(request.anchor, ContextAnchor::Pointer(ev.pos()));
    assert_eq!(list.cursor, 1, "the secondary press selects its row");
    // moves_cursor(false) leaves the cursor where it is.
    let mut still = ListBox::new(id, items.clone(), SelectMode::Single);
    let painted = draw(&mut still);
    let ev = painted.event(Phase::Secondary, id, Part::ROW, at);
    let request = config
        .moves_cursor(false)
        .on_secondary(&mut still, &ev)
        .expect("a row");
    assert_eq!(request.target, 1);
    assert_eq!(still.cursor, 0);
    assert!(!config.moves_cursor(false).moves_cursor);

    // The keyboard asks for a menu on the cursor row, anchored to the row.
    let key_request = config
        .on_key(&list, &shift_f10())
        .expect("Shift+F10 on a list");
    assert_eq!(key_request.target, 1);
    assert!(matches!(
        key_request.anchor,
        ContextAnchor::Control { id: row, .. } if row == list.row_id(1)
    ));
    assert!(config.on_key(&list, &key(KeyCode::Enter)).is_none());
    let silent = config.keys(ContextKeys::NONE);
    assert!(silent.on_key(&list, &shift_f10()).is_none());
    assert_eq!(silent.keys, ContextKeys::NONE);
    let empty = ListBox::new(id, vec![], SelectMode::Single);
    assert!(
        config.on_key(&empty, &shift_f10()).is_none(),
        "an empty list has no row to ask for"
    );

    // A custom source plugs into the same configuration.
    let mut rows = Rows {
        id,
        cursor: 2,
        len: 3,
    };
    let painted = draw(&mut list);
    let ev = painted.event(Phase::Secondary, id, Part::ROW, Position::new(at.x, at.y));
    let request = config.on_secondary(&mut rows, &ev).expect("a row");
    assert_eq!((request.target, rows.cursor), (1, 1));
    assert!(config.on_key(&rows, &key(KeyCode::Menu)).is_some());
    rows.len = 0;
    assert!(config.on_key(&rows, &key(KeyCode::Menu)).is_none());

    // Requests turn into anchored menus and map their target.
    let menu_items = || vec![MenuItem::new("Open"), MenuItem::new("Delete").danger()];
    let request = ContextRequest {
        target: 3usize,
        anchor: ContextAnchor::Pointer(Position::new(6, 2)),
    };
    let mut menu = request.menu(WidgetId::of("scenario.context.menu"), menu_items());
    draw_menu(&mut menu, Rect::new(0, 0, 40, 12));
    assert_eq!(
        (menu.area.x, menu.area.y),
        (6, 3),
        "a pointer anchor opens below the pointer"
    );
    let mapped = request.map(|n| n * 2);
    assert_eq!(mapped.target, 6);
    assert_eq!(mapped.anchor, request.anchor);
    let applied =
        ContextAnchor::Pointer(Position::new(2, 2)).apply(ContextMenu::new(id, menu_items()));
    assert_eq!(applied.anchor, Rect::new(2, 2, 1, 1));
    let control = WidgetId::of("scenario.context.control");
    let applied = ContextAnchor::Control {
        id: control,
        fallback: Rect::new(9, 4, 5, 1),
    }
    .apply(ContextMenu::new(id, menu_items()));
    assert_eq!(applied.anchor_to, Some(control));
    assert_eq!(applied.anchor, Rect::new(9, 4, 5, 1));
}

fn context_commands() {
    let map = FieldKeys::default().text_keymap();
    let cmds = [
        ContextCmd::new(TextCmd::Cut),
        ContextCmd::new(TextCmd::Copy).unless(Some("Nothing is selected")),
        ContextCmd::new(TextCmd::Paste).group_end(),
        ContextCmd::new(TextCmd::SelectAll).danger(),
    ];
    assert_eq!(cmds[0].cmd, TextCmd::Cut);
    assert!(!cmds[0].danger && !cmds[0].group_end && cmds[0].reason.is_none());
    assert_eq!(cmds[1].reason, Some("Nothing is selected"));
    assert!(cmds[2].group_end && cmds[3].danger);
    assert_eq!(ContextCmd::new(1).unless(None).reason, None);

    let items = command_items(&map, &cmds);
    assert_eq!(items.len(), 4);
    assert_eq!(
        items.iter().map(|(c, _)| *c).collect::<Vec<_>>(),
        [
            TextCmd::Cut,
            TextCmd::Copy,
            TextCmd::Paste,
            TextCmd::SelectAll
        ]
    );
    assert_eq!(items[0].1.item.label, TextCmd::Cut.label());
    assert!(
        items[0].1.shortcut.is_some(),
        "the keymap supplies the shortcut label"
    );
    assert!(
        items[1].1.item.disabled,
        "a command with a reason is disabled"
    );
    assert_eq!(
        items[1].1.item.reason.as_deref(),
        Some("Nothing is selected")
    );
    assert!(items[2].1.separator_after && !items[0].1.separator_after);
    assert!(items[3].1.danger && !items[0].1.danger);
    // The items make a working menu whose choice maps back to the command.
    let mut menu = ContextMenu::new(
        WidgetId::of("scenario.context.cmds"),
        items.iter().map(|(_, item)| item.clone()).collect(),
    )
    .at(Position::new(2, 1));
    let text = text_of(&draw_menu(&mut menu, Rect::new(0, 0, 40, 12)).buf);
    assert!(
        text.contains("Paste") && text.contains("Select all"),
        "{text}"
    );
    let (_, action) = menu.on_key(&key(KeyCode::Enter));
    let Some(MenuAction::Chosen(i)) = action else {
        panic!("Enter chooses a row: {action:?}")
    };
    assert_eq!(items[i].0, TextCmd::Cut);
}

fn edit_menu_caps() {
    // Capabilities come from the field.
    let mut field = EditField::single("hello world");
    assert_eq!(
        EditCaps::of(&field),
        EditCaps {
            editable: true,
            selection: false,
            text: true,
            secret: false
        }
    );
    field.buffer.select_range(0, 5);
    assert!(EditCaps::of(&field).selection);
    field.read_only = true;
    assert!(!EditCaps::of(&field).editable);
    let ro = EditCaps::read_only(true, false);
    assert_eq!(
        (ro.editable, ro.selection, ro.text, ro.secret),
        (false, true, false, false)
    );
    assert!(EditCaps::of(&field).secret(true).secret);
    assert!(!EditCaps::default().secret);

    // The menu entries follow the caps and say why one is unavailable.
    clipboard::copy("clip");
    let caps = EditCaps {
        editable: true,
        selection: true,
        text: true,
        secret: false,
    };
    let items = edit_items(caps, |cmd| {
        Some(format!("^{}", cmd.label().chars().next().unwrap_or('?')))
    });
    let cmds: Vec<TextCmd> = items.iter().map(|(c, _)| *c).collect();
    assert_eq!(
        cmds,
        [
            TextCmd::Cut,
            TextCmd::Copy,
            TextCmd::Paste,
            TextCmd::SelectAll
        ]
    );
    assert!(items.iter().all(|(_, item)| !item.item.disabled));
    assert_eq!(items[0].1.shortcut.as_deref(), Some("^C"));
    assert!(items[2].1.separator_after, "Paste ends the clipboard group");

    let bare = EditCaps {
        selection: false,
        ..caps
    };
    let items = edit_items(bare, |_| None);
    assert_eq!(
        items[0].1.item.reason.as_deref(),
        Some("Nothing is selected")
    );
    assert_eq!(
        items[1].1.item.reason.as_deref(),
        Some("Nothing is selected")
    );
    assert!(items[0].1.shortcut.is_none());
    let hidden = edit_items(caps.secret(true), |_| None);
    assert_eq!(
        hidden[1].1.item.reason.as_deref(),
        Some("Hidden text is not copied")
    );
    let nothing = edit_items(
        EditCaps {
            text: false,
            ..caps
        },
        |_| None,
    );
    assert_eq!(
        nothing[3].1.item.reason.as_deref(),
        Some("No text to select")
    );
    let read_only = edit_items(EditCaps::read_only(true, true), |_| None);
    assert_eq!(
        read_only.iter().map(|(c, _)| *c).collect::<Vec<_>>(),
        [TextCmd::Copy, TextCmd::SelectAll],
        "a read-only field cannot cut or paste"
    );
    assert!(read_only[0].1.separator_after);

    // Pointing a caret: it moves unless it lands inside the selection.
    let mut field = EditField::single("hello world");
    field.buffer.select_range(0, 5);
    point_field(&mut field, 3);
    assert_eq!(
        field.buffer.selected_text(),
        Some("hello"),
        "a point inside the selection keeps it"
    );
    point_field(&mut field, 8);
    assert_eq!(
        field.buffer.selected_text(),
        None,
        "a point outside clears it"
    );
}

/// An editable that lets a scenario watch what the host hooks receive.
struct Host {
    id: WidgetId,
    menu: EditMenu,
    caps: EditCaps,
    ran: Vec<TextCmd>,
    pointed: Vec<Position>,
}

impl EditMenuHost for Host {
    fn edit_menu(&mut self) -> &mut EditMenu {
        &mut self.menu
    }
    fn edit_owner(&self) -> WidgetId {
        self.id
    }
    fn edit_caps(&self) -> EditCaps {
        self.caps
    }
    fn edit_shortcut(&self, _: TextCmd) -> Option<String> {
        None
    }
    fn edit_point(&mut self, pos: Position) {
        self.pointed.push(pos);
    }
    fn edit_run(&mut self, cmd: TextCmd) -> bool {
        self.ran.push(cmd);
        cmd == TextCmd::Paste
    }
}

fn edit_menu_host() {
    clipboard::copy("clip");
    let screen = Rect::new(0, 0, 40, 12);
    let id = WidgetId::of("scenario.edit.host");
    let host = || Host {
        id,
        menu: EditMenu::default(),
        caps: EditCaps {
            editable: true,
            selection: true,
            text: true,
            secret: false,
        },
        ran: vec![],
        pointed: vec![],
    };
    assert_eq!(EditMenu::id_for(id), id.sub("edit-menu"));

    // Keys: Shift+F10 opens, Down moves, Enter runs the command.
    let mut h = host();
    assert!(!h.menu.is_open() && h.menu.menu().is_none());
    assert!(h.menu.enabled && h.menu.keys == ContextKeys::DEFAULT);
    assert_eq!(
        edit_menu_key(&mut h, &key(KeyCode::Char('x'))),
        None,
        "closed: other keys pass"
    );
    assert_eq!(edit_menu_key(&mut h, &shift_f10()), Some(Outcome::Changed));
    assert!(h.menu.is_open());
    let painted = paint_hits(screen, |area, buf, ctx| h.menu.render(area, buf, ctx));
    let text = text_of(&painted.buf);
    assert!(
        text.contains("Cut") && text.contains("Copy") && text.contains("Paste"),
        "{text}"
    );
    assert_eq!(h.menu.menu().map(|m| m.id), Some(EditMenu::id_for(id)));
    assert_eq!(
        edit_menu_key(&mut h, &key(KeyCode::Down)),
        Some(Outcome::Changed)
    );
    assert_eq!(
        edit_menu_key(&mut h, &key(KeyCode::Enter)),
        Some(Outcome::Changed)
    );
    assert_eq!(h.ran, [TextCmd::Copy]);
    assert!(!h.menu.is_open());
    assert!(!h.menu.take_edited(), "Copy changed nothing");
    // Esc closes without running; an unknown key closes and passes through.
    edit_menu_key(&mut h, &shift_f10());
    assert_eq!(
        edit_menu_key(&mut h, &key(KeyCode::Esc)),
        Some(Outcome::Changed)
    );
    assert!(!h.menu.is_open());
    edit_menu_key(&mut h, &shift_f10());
    assert_eq!(edit_menu_key(&mut h, &key(KeyCode::F(5))), None);
    assert!(!h.menu.is_open());
    assert_eq!(h.menu.close(), Outcome::Ignored);
    edit_menu_key(&mut h, &shift_f10());
    assert_eq!(h.menu.close(), Outcome::Changed);
    // A command that edits is reported once.
    edit_menu_key(&mut h, &shift_f10());
    edit_menu_key(&mut h, &key(KeyCode::Down));
    edit_menu_key(&mut h, &key(KeyCode::Down));
    edit_menu_key(&mut h, &key(KeyCode::Enter));
    assert_eq!(h.ran.last(), Some(&TextCmd::Paste));
    assert!(h.menu.take_edited());
    assert!(!h.menu.take_edited(), "the flag is taken");
    // Disabled and re-keyed menus.
    let mut off = host();
    off.menu.enabled = false;
    assert_eq!(edit_menu_key(&mut off, &shift_f10()), None);
    let mut rekeyed = host();
    rekeyed.menu.keys = ContextKeys::NONE.extra(Some(key(KeyCode::F(9))));
    assert_eq!(edit_menu_key(&mut rekeyed, &shift_f10()), None);
    assert_eq!(
        edit_menu_key(&mut rekeyed, &key(KeyCode::F(9))),
        Some(Outcome::Changed)
    );

    // Where a keyboard-opened menu appears: at the caret, else on the control.
    let fresh = EditMenu::default();
    assert!(
        matches!(fresh.key_anchor(id), ContextAnchor::Control { id: owner, .. } if owner == id)
    );
    let mut with_caret = host();
    paint_hits(screen, |_, buf, ctx| {
        ctx.cursor = Some(Position::new(5, 2));
        with_caret.menu.render(Rect::new(0, 0, 20, 4), buf, ctx);
    });
    assert_eq!(
        with_caret.menu.key_anchor(id),
        ContextAnchor::Pointer(Position::new(5, 2))
    );

    // The menu itself answers keys and pointer events only while open.
    let mut direct = host();
    assert_eq!(direct.menu.on_key(&key(KeyCode::Down)), None);
    let anchor = ContextAnchor::Pointer(Position::new(2, 1));
    let caps = EditCaps {
        editable: true,
        selection: true,
        text: true,
        secret: false,
    };
    direct.menu.open(id, anchor, edit_items(caps, |_| None));
    assert_eq!(
        direct.menu.on_key(&key(KeyCode::Down)),
        Some((Outcome::Changed, None)),
        "moving inside the menu runs no command"
    );
    assert_eq!(
        direct.menu.on_key(&key(KeyCode::Enter)),
        Some((Outcome::Changed, Some(TextCmd::Copy)))
    );
    assert!(!direct.menu.is_open());
    direct.menu.open(id, anchor, edit_items(caps, |_| None));
    let painted = paint_hits(screen, |area, buf, ctx| direct.menu.render(area, buf, ctx));
    let first = direct.menu.menu().expect("open").row_id(0);
    let first_at = painted
        .hits
        .area_of(first)
        .expect("a painted row")
        .as_position();
    let elsewhere = PointerEvent::new(
        &painted.hits,
        Phase::Click,
        first,
        first_at,
        1,
        KeyModifiers::NONE,
    );
    assert_eq!(
        direct.menu.on_pointer(&elsewhere),
        Some((Outcome::Changed, Some(TextCmd::Cut)))
    );
    let stray = PointerEvent::new(
        &painted.hits,
        Phase::Click,
        first,
        first_at,
        1,
        KeyModifiers::NONE,
    );
    assert_eq!(
        direct.menu.on_pointer(&stray),
        None,
        "a closed menu owns no event"
    );

    // Pointer: a secondary press over the owner opens the menu at the pointer;
    // a click on a row runs its command.
    let mut h = host();
    let painted = paint_hits(screen, |area, buf, ctx| {
        ctx.clickable(id, Rect::new(0, 0, 30, 3));
        h.menu.render(area, buf, ctx);
    });
    let at = Position::new(4, 1);
    let ev = PointerEvent::new(
        &painted.hits,
        Phase::Secondary,
        id,
        at,
        1,
        KeyModifiers::NONE,
    );
    assert!(!h.menu.owns(&ev));
    assert_eq!(edit_menu_pointer(&mut h, &ev), Some(Outcome::Changed));
    assert_eq!(h.pointed, [at], "the caret is placed where the pointer was");
    assert!(h.menu.is_open());
    let painted = paint_hits(screen, |area, buf, ctx| {
        ctx.clickable(id, Rect::new(0, 0, 30, 3));
        h.menu.render(area, buf, ctx);
    });
    let menu = h.menu.menu().expect("open");
    let row = menu.row_id(2);
    let row_at = painted.hits.area_of(row).expect("a painted row");
    let click = PointerEvent::new(
        &painted.hits,
        Phase::Click,
        row,
        row_at.as_position(),
        1,
        KeyModifiers::NONE,
    );
    assert!(h.menu.owns(&click));
    assert_eq!(edit_menu_pointer(&mut h, &click), Some(Outcome::Changed));
    assert_eq!(h.ran.last(), Some(&TextCmd::Paste));
    assert!(!h.menu.is_open());
    let plain = PointerEvent::new(&painted.hits, Phase::Click, id, at, 1, KeyModifiers::NONE);
    assert_eq!(
        edit_menu_pointer(&mut h, &plain),
        None,
        "an ordinary click is not the menu's"
    );
    let mut disabled = host();
    disabled.menu.enabled = false;
    assert_eq!(edit_menu_pointer(&mut disabled, &ev), None);

    // A real text input wires the same hooks: the secondary press places the
    // caret, the label is not an editing target, and routing includes the menu.
    clipboard::copy("clip");
    let mut input =
        TextInput::new(WidgetId::of("scenario.edit.input"), "Name").value("hello world");
    input.begin_edit();
    let painted = paint_hits(Rect::new(0, 0, 40, 6), |area, buf, ctx| {
        let bg = ctx.theme.canvas;
        input.render(Rect::new(0, 0, 30, 3), buf, ctx, bg);
        let _ = area;
    });
    let field_at = find_text(&painted.buf, "hello").expect("the value");
    let ev = PointerEvent::new(
        &painted.hits,
        Phase::Secondary,
        input.id,
        field_at,
        1,
        KeyModifiers::NONE,
    );
    assert!(editable_target(&input, &ev));
    assert!(edit_menu_route(&mut input, &ev).is_some());
    point_editable(&mut input, field_at);
    assert_eq!(edit_menu_pointer(&mut input, &ev), Some(Outcome::Changed));
    assert!(input.edit_menu.is_open());
    let unrelated = PointerEvent::new(
        &painted.hits,
        Phase::Click,
        WidgetId::of("scenario.nothing"),
        field_at,
        1,
        KeyModifiers::NONE,
    );
    assert!(!input.edit_menu.owns(&unrelated));
    assert!(edit_menu_route(&mut input, &unrelated).is_none());
    assert_eq!(
        edit_menu_key(&mut input, &key(KeyCode::Esc)),
        Some(Outcome::Changed)
    );
    assert!(!input.edit_menu.is_open());
}

fn tooltip_frame(
    owner: WidgetId,
    hover: Option<WidgetId>,
    rested: Option<u64>,
    policy: Option<Tooltips>,
    tip: &Tip,
) -> Buffer {
    let t = junie_tui::theme::Theme::for_level(junie_tui::theme::ColorLevel::TrueColor);
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 10));
    let mut hits = junie_tui::core::hit::HitRegistry::default();
    let mut ring = junie_tui::core::focus::FocusRing::default();
    let interaction = junie_tui::ui::ctx::Interaction {
        hover,
        hover_ms: rested,
        ..Default::default()
    };
    let mut ctx = junie_tui::ui::ctx::RenderCtx::new(&t, interaction, &mut hits, &mut ring);
    tooltip::show(&mut ctx, owner, Rect::new(10, 2, 8, 1), tip, policy);
    ctx.flush_overlays(&mut buf);
    buf
}

fn tooltips() {
    let owner = WidgetId::of("scenario.tip.owner");
    let tip = Tip::new("Save the document").shortcut("Ctrl+S");
    assert_eq!(tip.text, "Save the document");
    assert_eq!(tip.shortcut.as_deref(), Some("Ctrl+S"));
    assert_eq!(Tip::default().text, "");
    assert_ne!(tip_id(owner), owner);

    // Hovering the owner long enough shows the tip below the anchor, with its key.
    let shown = text_of(&tooltip_frame(owner, Some(owner), Some(1000), None, &tip));
    assert!(
        shown.contains("Save the document") && shown.contains("Ctrl+S"),
        "{shown}"
    );
    // Not hovered, or not hovered long enough: nothing.
    assert!(!text_of(&tooltip_frame(owner, None, None, None, &tip)).contains("Save"));
    assert!(!text_of(&tooltip_frame(owner, Some(owner), Some(100), None, &tip)).contains("Save"));
    // A shorter delay shows it earlier.
    let quick = Tooltips::default().delay_ms(50);
    assert_eq!(quick.delay_ms, 50);
    assert!(
        text_of(&tooltip_frame(
            owner,
            Some(owner),
            Some(100),
            Some(quick),
            &tip
        ))
        .contains("Save")
    );
    // Tooltips can be switched off for a frame.
    assert!(!Tooltips::OFF.enabled && Tooltips::default().enabled);
    assert!(
        !text_of(&tooltip_frame(
            owner,
            Some(owner),
            Some(1000),
            Some(Tooltips::OFF),
            &tip
        ))
        .contains("Save")
    );
    let off = Tooltips::default().enabled(false);
    assert!(
        !text_of(&tooltip_frame(
            owner,
            Some(owner),
            Some(1000),
            Some(off),
            &tip
        ))
        .contains("Save")
    );
    assert!(
        text_of(&tooltip_frame(
            owner,
            Some(owner),
            Some(1000),
            Some(Tooltips::OFF.enabled(true)),
            &tip
        ))
        .contains("Save")
    );
    // An empty tip is never drawn.
    assert!(
        rows(&tooltip_frame(
            owner,
            Some(owner),
            Some(1000),
            None,
            &Tip::default()
        ))
        .iter()
        .all(|r| r.trim().is_empty())
    );
    // max_width wraps long text onto more lines.
    let long = Tip::new("Writes every open document to disk right now");
    let wide = tooltip_frame(owner, Some(owner), Some(1000), None, &long);
    let narrow = tooltip_frame(
        owner,
        Some(owner),
        Some(1000),
        Some(Tooltips::default().max_width(14)),
        &long,
    );
    let lines = |buf: &Buffer| rows(buf).iter().filter(|r| !r.trim().is_empty()).count();
    assert_eq!(Tooltips::default().max_width(14).max_width, 14);
    assert!(lines(&narrow) > lines(&wide), "a narrow tooltip wraps");
    // Hovering the tip itself keeps it open.
    assert!(
        text_of(&tooltip_frame(
            owner,
            Some(tip_id(owner)),
            Some(1000),
            None,
            &tip
        ))
        .contains("Save")
    );

    // Esc dismisses a shown tooltip until the pointer leaves.
    tooltip_frame(owner, None, None, None, &tip);
    assert_eq!(
        tooltip::on_input(&Input::Key(key(KeyCode::Esc))),
        None,
        "nothing shown yet"
    );
    tooltip_frame(owner, Some(owner), Some(1000), None, &tip);
    assert_eq!(
        tooltip::on_input(&Input::Key(key(KeyCode::Char('x')))),
        None
    );
    assert_eq!(tooltip::on_input(&Input::Tick), None);
    assert_eq!(
        tooltip::on_input(&Input::Key(key(KeyCode::Esc))),
        Some(Outcome::Changed)
    );
    assert!(!text_of(&tooltip_frame(owner, Some(owner), Some(1000), None, &tip)).contains("Save"));
    tooltip_frame(owner, None, None, None, &tip);
    assert!(text_of(&tooltip_frame(owner, Some(owner), Some(1000), None, &tip)).contains("Save"));
}

fn theme() -> Theme {
    Theme::for_level(ColorLevel::TrueColor)
}

fn bar_row(buf: &Buffer) -> String {
    rows(buf).remove(0)
}

fn key_hint_labels() {
    // The shared key vocabulary.
    assert_eq!(
        (
            keyhint::UP_DOWN,
            keyhint::LEFT_RIGHT,
            keyhint::ARROWS,
            keyhint::DELETE
        ),
        ("↑↓", "←→", "↑↓←→", "Delete")
    );
    assert_eq!(keyhint::range(1, 5), "1–5");
    let fields = keyhint::EDIT_FIELD;
    let edit_field: Vec<_> = fields.iter().map(|h| (&*h.key, &*h.action)).collect();
    assert_eq!(
        edit_field,
        [("Enter", "Apply"), ("Esc", "Cancel"), ("Tab", "Next field")]
    );
    assert_eq!(keyhint::EDIT_TEXT.len(), 2);
    assert_eq!(keyhint::EDIT_CODE[1].key, "F6");
    assert_eq!(keyhint::EDIT_CELL[2].action, "Next cell");
    assert_eq!(keyhint::CONTEXT_MENU, keyhint::hint("Shift+F10", "Menu"));
    assert_eq!(keyhint::hint("Esc", "Close"), Hint::new("Esc", "Close"));
    assert_eq!(
        keyhint::toggle_hint("F2", true, "Hide", "Show").action,
        "Hide"
    );
    assert_eq!(
        keyhint::toggle_hint("F2", false, "Hide", "Show").action,
        "Show"
    );

    // Normalization spells keys one way.
    assert_eq!(keyhint::normalize_key("Del"), "Delete");
    assert_eq!(keyhint::normalize_key("↑ ↓"), "↑↓");
    assert_eq!(keyhint::normalize_key("1-3"), "1–3");
    assert!(matches!(
        keyhint::normalize_key("Enter"),
        std::borrow::Cow::Borrowed(_)
    ));
    assert!(matches!(
        keyhint::normalize_key("a-b"),
        std::borrow::Cow::Borrowed(_)
    ));

    // Labels turn back into the key a click should send.
    let k = |label: &str| keyhint::key_of(label);
    assert_eq!(k("Esc"), Some(key(KeyCode::Esc)));
    assert_eq!(k("Enter"), Some(key(KeyCode::Enter)));
    assert_eq!(k("Space"), Some(key(KeyCode::Char(' '))));
    assert_eq!(k("Del"), Some(key(KeyCode::Delete)));
    assert_eq!(k("PgDn"), Some(key(KeyCode::PageDown)));
    assert_eq!(k("↑"), Some(key(KeyCode::Up)));
    assert_eq!(k("F5"), Some(key(KeyCode::F(5))));
    assert_eq!(k("Shift+Tab"), Some(key(KeyCode::BackTab)));
    assert_eq!(
        k("Ctrl+S"),
        Some(Key {
            code: KeyCode::Char('s'),
            mods: KeyModifiers::CONTROL
        })
    );
    assert_eq!(
        k("Shift+F10"),
        Some(Key {
            code: KeyCode::F(10),
            mods: KeyModifiers::SHIFT
        })
    );
    assert_eq!(k("Move"), None, "words are not keys");
    assert_eq!(k("↑↓"), None, "a pair of keys is not one key");
    assert_eq!(k("F13"), None);

    // Pointer modifiers are named per platform.
    use KeyModifiers as M;
    let mac = ClientPlatform::MacOs;
    let other = ClientPlatform::Other;
    assert_eq!(keyhint::modifier_click(M::ALT, mac), Some("Option+click"));
    assert_eq!(keyhint::modifier_click(M::ALT, other), Some("Alt+click"));
    assert_eq!(keyhint::modifier_click(M::SUPER, mac), Some("Cmd+click"));
    assert_eq!(
        keyhint::modifier_click(M::SUPER, other),
        Some("Super+click")
    );
    assert_eq!(
        keyhint::modifier_click(M::CONTROL, other),
        Some("Ctrl+click")
    );
    assert_eq!(
        keyhint::modifier_click(M::ALT | M::CONTROL, mac),
        Some("Option+click")
    );
    assert_eq!(
        keyhint::modifier_click(M::ALT | M::CONTROL, other),
        Some("Ctrl+click")
    );
    assert_eq!(keyhint::modifier_click(M::NONE, mac), None);

    // Hints never advertise keys the terminal cannot deliver.
    let line = "↑↓ Move · Cmd+S Save · Esc Close";
    assert_eq!(keyhint::deliverable_line(line), "↑↓ Move · Esc Close");
    let kept = "↑↓ Move · Esc Close";
    assert!(matches!(
        keyhint::deliverable_line(kept),
        std::borrow::Cow::Borrowed(_)
    ));

    // Priorities decide what survives a narrow bar.
    let style = HintBarStyle {
        pinned_keys: &["Esc"],
        ..theme().hintbar
    };
    let esc = Hint::new("Esc", "Close");
    assert_eq!(esc.priority, HintPriority::Auto);
    assert!(
        esc.is_pinned(&style),
        "Auto follows the style's pinned keys"
    );
    assert!(!Hint::new("Tab", "Next").is_pinned(&style));
    assert!(!esc.clone().priority(HintPriority::Normal).is_pinned(&style));
    assert!(
        Hint::new("Tab", "Next")
            .priority(HintPriority::Pinned)
            .is_pinned(&style)
    );
    assert!(Hint::new("Tab", "Next").pinned().is_pinned(&style));
    assert_eq!(
        Hint::new("Tab", "Next").pinned().priority,
        HintPriority::Pinned
    );
    let clicked = Hint::new("Ctrl+Alt+Q", "Quit").click_key(key(KeyCode::Char('q')));
    assert_eq!(clicked.click_key, Some(key(KeyCode::Char('q'))));
    assert_eq!(Hint::new("Esc", "Close").click_key, None);
}

fn key_hint_rendering() {
    let t = theme();
    let area = Rect::new(0, 0, 60, 1);
    let hints = [
        Hint::new("↑↓", "Move"),
        Hint::new("Enter", "Choose"),
        Hint::new("Esc", "Close"),
    ];
    let paint_row = |draw: &dyn Fn(Rect, &mut Buffer) -> usize| {
        let mut buf = Buffer::empty(area);
        let shown = draw(area, &mut buf);
        (shown, bar_row(&buf))
    };

    let (shown, row) = paint_row(&|a, b| keyhint::render(a, b, &t, &hints, None, None));
    assert_eq!(shown, 3);
    assert!(
        row.contains("↑↓ Move") && row.contains("Enter Choose") && row.contains("Esc Close"),
        "{row}"
    );
    // A badge and right-hand text.
    let (_, row) = paint_row(&|a, b| {
        keyhint::render(
            a,
            b,
            &t,
            &hints,
            Some(("EDIT", BadgeKind::Edit)),
            Some("3 rows"),
        )
    });
    assert!(
        row.contains(" EDIT ") && row.ends_with("3 rows "),
        "{row:?}"
    );
    let (_, row) = paint_row(&|a, b| {
        keyhint::render_toned(a, b, &t, &hints, None, Some(("Disk full", Tone::Error)))
    });
    assert!(
        row.contains("! Disk full"),
        "an error tone carries a mark: {row}"
    );
    let (_, row) = paint_row(&|a, b| {
        keyhint::render_status(
            a,
            b,
            &t,
            &hints,
            None,
            Some(("Saved", Some(StatusKind::Ok))),
        )
    });
    assert!(row.contains("Saved"), "{row}");
    let (_, plain) =
        paint_row(&|a, b| keyhint::render_status(a, b, &t, &hints, None, Some(("Idle", None))));
    assert!(plain.contains("Idle"));
    // Width is exactly what the hints need; one cell less drops a hint.
    let need = keyhint::width(&hints, None, None);
    let exact = Rect::new(0, 0, need, 1);
    let mut buf = Buffer::empty(exact);
    assert_eq!(keyhint::render(exact, &mut buf, &t, &hints, None, None), 3);
    let tight = Rect::new(0, 0, need - 1, 1);
    let mut buf = Buffer::empty(tight);
    assert!(keyhint::render(tight, &mut buf, &t, &hints, None, None) < 3);
    assert!(keyhint::width(&hints, Some("EDIT"), Some("3 rows")) > need);
    // Centered hints start away from the left edge.
    let (_, start) =
        paint_row(&|a, b| keyhint::render_aligned(a, b, &t, &hints, None, None, HintAlign::Start));
    let (_, centered) =
        paint_row(&|a, b| keyhint::render_aligned(a, b, &t, &hints, None, None, HintAlign::Center));
    assert!(
        centered.find("↑↓").unwrap_or(0) > start.find("↑↓").unwrap_or(0),
        "{centered:?} vs {start:?}"
    );
    // Compact style keeps only keys when everything cannot fit; pinned keys stay whole.
    let style = HintBarStyle {
        compact: true,
        pinned_keys: &["Esc"],
        ..t.hintbar
    };
    let narrow = Rect::new(0, 0, 24, 1);
    let mut buf = Buffer::empty(narrow);
    keyhint::render_styled(narrow, &mut buf, &t, &hints, None, None, &style);
    let row = bar_row(&buf);
    assert!(
        row.contains("Esc Close") && !row.contains("Choose"),
        "{row:?}"
    );
    // Parts of the right-hand text are toned separately; the spots locate each hint.
    let parts = [
        ("12 ".to_owned(), Tone::Success),
        ("errors".to_owned(), Tone::Error),
    ];
    let mut buf = Buffer::empty(area);
    let n = keyhint::render_parts(
        area,
        &mut buf,
        &t,
        &hints,
        None,
        Some(("12 errors", Tone::Normal)),
        &parts,
        &t.hintbar,
    );
    assert_eq!(n, 3);
    let at = find_text(&buf, "errors").expect("the status").x;
    assert_ne!(
        buf[(at, 0)].fg,
        buf[(at - 3, 0)].fg,
        "the parts have their own tone"
    );
    let mut buf = Buffer::empty(area);
    let (n, spots) =
        keyhint::render_parts_at(area, &mut buf, &t, &hints, None, None, &[], &t.hintbar);
    assert_eq!((n, spots.len()), (3, 3));
    assert!(
        spots.windows(2).all(|w| w[0].1.right() < w[1].1.x),
        "spots do not overlap"
    );
    let mut buf = Buffer::empty(area);
    let (n, spots) = keyhint::render_status_at(
        area,
        &mut buf,
        &t,
        &hints,
        None,
        Some(("OK", Some(StatusKind::Ok))),
    );
    assert_eq!((n, spots.len()), (3, 3));
    assert_eq!(spots[0].0, 0);
    let mut buf = Buffer::empty(area);
    let (n, spots) = keyhint::render_status_styled_at(
        area,
        &mut buf,
        &t,
        &hints,
        None,
        Some(("Waiting", Some(StatusKind::Pending))),
        &style,
    );
    assert_eq!((n, spots.len()), (3, 3));
    assert!(bar_row(&buf).contains("Waiting"));
}

fn hint_bar() {
    let t = theme();
    let area = Rect::new(0, 0, 60, 1);
    let hints = vec![
        Hint::new("Esc", "Close"),
        Hint::new("Ctrl+Alt+Q", "Quit").click_key(key(KeyCode::Char('q'))),
        Hint::new("Move", "Words"),
    ];
    let layer = HintLayer::new(hints.clone());
    assert!(layer.badge.is_none() && layer.status.is_none() && layer.align.is_none());
    let layer = layer
        .badge("EDIT", BadgeKind::Edit)
        .status("Saved", Tone::Success)
        .align(HintAlign::Start);
    assert_eq!(layer.badge, Some(("EDIT", BadgeKind::Edit)));
    assert_eq!(layer.status, Some(("Saved".to_owned(), Tone::Success)));
    assert_eq!(layer.align, Some(HintAlign::Start));
    let mut buf = Buffer::empty(area);
    assert_eq!(HintBar::render(area, &mut buf, &t, &layer), 3);
    let row = bar_row(&buf);
    assert!(
        row.contains(" EDIT ") && row.contains("Esc Close") && row.contains("Saved"),
        "{row}"
    );
    // Parts replace the plain status and keep its text.
    let parts = layer.clone().status_parts(
        vec![
            ("3 ".to_owned(), Tone::Success),
            ("failed".to_owned(), Tone::Error),
        ],
        Tone::Normal,
    );
    assert_eq!(
        parts.status.as_ref().map(|(s, _)| s.as_str()),
        Some("3 failed")
    );
    assert_eq!(parts.status_parts.len(), 2);
    assert!(
        layer
            .clone()
            .status("Again", Tone::Muted)
            .status_parts
            .is_empty()
    );
    let mut buf = Buffer::empty(area);
    HintBar::render(area, &mut buf, &t, &parts);
    assert!(bar_row(&buf).contains("3 failed"));
    // Alignment is per layer.
    let centered = HintLayer::new(hints.clone()).align(HintAlign::Center);
    let mut buf = Buffer::empty(area);
    let spots = HintBar::render_at(area, &mut buf, &t, &centered);
    assert_eq!(spots.len(), 3);
    assert!(
        spots[0].1.x > 3,
        "centered hints leave the left edge: {spots:?}"
    );

    // Layers: the first layer that exists wins.
    let top = HintLayer::new(vec![Hint::new("Esc", "Top")]);
    let low = HintLayer::new(vec![Hint::new("Esc", "Low")]);
    assert_eq!(HintBar::resolve(&[None, Some(top.clone()), Some(low)]), top);
    assert_eq!(HintBar::resolve(&[None, None]), HintLayer::default());
    assert_eq!(HintBar::resolve(&[]), HintLayer::default());

    // Rendered hints are clickable and send the key they name.
    let mut keys = HintKeys::default();
    let painted = paint_hits(Rect::new(0, 0, 60, 3), |_, buf, ctx| {
        keys = HintBar::render_ctx(area, buf, ctx, &layer);
    });
    // render_ctx registers through HintKeys::register; calling it directly
    // yields the same click map for the same spots.
    let mut direct = HintKeys::default();
    let direct_painted = paint_hits(Rect::new(0, 0, 60, 3), |_, buf, ctx| {
        let spots = HintBar::render_at(area, buf, ctx.theme, &layer);
        direct = HintKeys::register(ctx, buf, &layer.hints, spots);
    });
    assert_eq!(direct, keys);
    assert_eq!(
        direct_painted.hits.regions().len(),
        painted.hits.regions().len()
    );
    let at = |needle: &str| find_text(&painted.buf, needle).expect("a hint");
    let click = |needle: &str| {
        let ev = painted.event(Phase::Click, HintBar::ID, Part::KEY, at(needle));
        keys.clicked(&ev)
    };
    assert_eq!(click("Close"), Some(key(KeyCode::Esc)));
    assert_eq!(
        click("Quit"),
        Some(key(KeyCode::Char('q'))),
        "click_key overrides the label"
    );
    let words = at("Words");
    assert!(
        painted
            .hits
            .regions()
            .iter()
            .all(|r| !r.area.contains(words)),
        "a hint that names no key is not clickable"
    );
    let press = painted.event(Phase::Press, HintBar::ID, Part::KEY, at("Close"));
    assert_eq!(keys.clicked(&press), None, "only a click sends the key");
    assert_eq!(HintKeys::default().clicked(&press), None);
    // Hover underlines and press reverses the hint under the pointer.
    let region = painted
        .hits
        .regions()
        .iter()
        .find(|r| r.owner == HintBar::ID && r.area.contains(at("Close")))
        .map(|r| r.id)
        .expect("a region for Esc");
    let styled = |interaction: junie_tui::ui::ctx::Interaction| {
        let t = theme();
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 3));
        let mut hits = junie_tui::core::hit::HitRegistry::default();
        let mut ring = junie_tui::core::focus::FocusRing::default();
        let mut ctx = junie_tui::ui::ctx::RenderCtx::new(&t, interaction, &mut hits, &mut ring);
        HintBar::render_ctx(area, &mut buf, &mut ctx, &layer);
        buf[(at("Close").x, 0)].modifier
    };
    use ratatui::style::Modifier;
    let rest = styled(Default::default());
    let hover = styled(junie_tui::ui::ctx::Interaction {
        hover: Some(region),
        ..Default::default()
    });
    let press = styled(junie_tui::ui::ctx::Interaction {
        hover: Some(region),
        pressed: Some(region),
        ..Default::default()
    });
    assert!(!rest.contains(Modifier::UNDERLINED) && hover.contains(Modifier::UNDERLINED));
    assert!(press.contains(Modifier::REVERSED));

    // A field that is focused but not yet being edited advertises the shared
    // "Enter Edit" hint, and the hint bar shows it. A page that lists the
    // pair itself renders the same text.
    let input = TextInput::new(WidgetId::of("scenario.hints.input"), "Name")
        .edit_entry(EditEntry::Explicit)
        .value("hello");
    assert!(!input.wants_text_input());
    assert_eq!(input.hints(), vec![widget_hints::EDIT]);
    let mut buf = Buffer::empty(area);
    HintBar::render(area, &mut buf, &t, &HintLayer::new(input.hints()));
    assert!(bar_row(&buf).contains("Enter Edit"), "{}", bar_row(&buf));
    let mut editing = input.clone();
    editing.begin_edit();
    assert!(!editing.hints().contains(&widget_hints::EDIT));
    let paired = Hint::new(widget_hints::EDIT_PAIR.0, widget_hints::EDIT_PAIR.1);
    let mut buf = Buffer::empty(area);
    HintBar::render(area, &mut buf, &t, &HintLayer::new(vec![paired]));
    assert!(bar_row(&buf).contains("Enter Edit"), "{}", bar_row(&buf));
}

fn status_bar() {
    let screen = Rect::new(0, 0, 60, 4);
    let area = Rect::new(0, 0, 60, 1);
    let mut bar = StatusBar::new();
    bar.left
        .push(StatusItem::new("main", Tone::Normal).strong());
    bar.center
        .push(StatusItem::new("indexing", Tone::Info).busy());
    bar.right
        .push(StatusItem::new("3 errors", Tone::Error).chip());
    bar.right.push(StatusItem::new("UTF-8", Tone::Muted));

    // Layout puts each group where it belongs.
    let placed: Vec<Placed> = bar.layout(area);
    let by = |g: Group, i: usize| {
        placed
            .iter()
            .find(|p| p.group == g && p.index == i)
            .expect("placed")
    };
    assert_eq!(by(Group::Left, 0).x, 1);
    let center = by(Group::Center, 0);
    assert!(center.x > by(Group::Left, 0).x + 4 && center.x + center.width < by(Group::Right, 0).x);
    assert_eq!(
        by(Group::Right, 1).x + by(Group::Right, 1).width,
        59,
        "the right group ends at the edge"
    );
    assert!(by(Group::Right, 0).x < by(Group::Right, 1).x);
    assert_eq!(by(Group::Right, 1).text, "UTF-8");
    assert_eq!(placed.len(), 4);

    // Rendering draws text, chip padding, strong weight and a busy spinner.
    let painted = paint_hits(screen, |_, buf, ctx| bar.render(area, buf, ctx));
    let row = bar_row(&painted.buf);
    assert!(
        row.contains("main")
            && row.contains("indexing")
            && row.contains(" 3 errors ")
            && row.contains("UTF-8"),
        "{row:?}"
    );
    let main = find_text(&painted.buf, "main").expect("main").x;
    assert!(
        painted.buf[(main, 0)]
            .modifier
            .contains(ratatui::style::Modifier::BOLD),
        "strong is bold"
    );
    let busy = find_text(&painted.buf, "indexing").expect("indexing").x;
    assert_ne!(
        painted.buf[(busy - 2, 0)].symbol(),
        " ",
        "a busy item leads with a spinner"
    );
    assert_eq!(StatusItem::new("x", Tone::Normal).width(), 1);
    assert_eq!(StatusItem::new("x", Tone::Normal).busy().width(), 3);
    assert_eq!(StatusItem::new("x", Tone::Normal).chip().width(), 3);
    assert_eq!(Emphasis::default(), Emphasis::Plain);
    assert_eq!(
        StatusItem::new("x", Tone::Normal).chip().emphasis,
        Emphasis::Chip
    );
    assert_eq!(
        StatusItem::new("x", Tone::Normal).strong().emphasis,
        Emphasis::Strong
    );

    // A meter adds a gauge and its percentage after the text.
    let plain = StatusItem::new("CPU", Tone::Normal);
    let metered = StatusItem::new("CPU", Tone::Normal).meter(Some(40), MeterTone::Normal);
    assert_eq!(metered.width(), plain.width() + 1 + STATUS_METER_TRACK + 7);
    let mut gauge = StatusBar::new();
    gauge.left.push(metered.clone());
    let painted = paint_hits(screen, |_, buf, ctx| gauge.render(area, buf, ctx));
    assert!(
        bar_row(&painted.buf).contains("40%"),
        "{}",
        bar_row(&painted.buf)
    );
    let mut stale = StatusBar::new();
    stale.left.push(metered.clone().meter_stale(true));
    let stale_buf = paint_hits(screen, |_, buf, ctx| stale.render(area, buf, ctx)).buf;
    assert_ne!(
        rows(&stale_buf),
        rows(&painted.buf),
        "a stale meter is drawn differently"
    );
    assert!(stale.left[0].meter_stale && !metered.meter_stale);
    let mut unknown = StatusBar::new();
    unknown
        .left
        .push(StatusItem::new("CPU", Tone::Normal).meter(None, MeterTone::Normal));
    let painted = paint_hits(screen, |_, buf, ctx| unknown.render(area, buf, ctx));
    assert!(
        !bar_row(&painted.buf).contains('%'),
        "an unknown value shows no percentage"
    );

    // Clickable items register a hit region at their place.
    let click_id = WidgetId::of("scenario.status.branch");
    let mut clickable = StatusBar::new();
    clickable
        .left
        .push(StatusItem::new("branch", Tone::Normal).clickable(click_id));
    let painted = paint_hits(screen, |_, buf, ctx| clickable.render(area, buf, ctx));
    assert_eq!(painted.hits.area_of(click_id), Some(Rect::new(1, 0, 6, 1)));

    // Under pressure the lowest priority goes first and a More marker appears.
    let mut crowded = StatusBar::new();
    crowded
        .left
        .push(StatusItem::new("main", Tone::Normal).priority(9));
    crowded
        .right
        .push(StatusItem::new("UTF-8", Tone::Muted).priority(1));
    crowded
        .right
        .push(StatusItem::new("Ln 4, Col 2", Tone::Muted).priority(5));
    let kept = crowded.layout(Rect::new(0, 0, 24, 1));
    assert!(
        kept.iter().any(|p| p.text == "main"),
        "the highest priority stays"
    );
    assert!(
        !kept.iter().any(|p| p.text == "UTF-8"),
        "the lowest priority is dropped first"
    );
    let kept_all = crowded.layout(Rect::new(0, 0, 60, 1));
    assert_eq!(kept_all.len(), 3);

    // A tooltip shows over the hovered item; unclickable items still get one.
    let tip = Tip::new("Current branch");
    let hover_id = reveal_id(Group::Left, 0);
    assert_ne!(reveal_id(Group::Left, 0), reveal_id(Group::Right, 0));
    assert_ne!(reveal_id(Group::Left, 0), reveal_id(Group::Left, 1));
    let mut tipped = StatusBar::new();
    tipped
        .left
        .push(StatusItem::new("main", Tone::Normal).tooltip(tip.clone()));
    assert_eq!(tipped.left[0].tip, Some(tip));
    let t = theme();
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 6));
    let mut hits = junie_tui::core::hit::HitRegistry::default();
    let mut ring = junie_tui::core::focus::FocusRing::default();
    let interaction = junie_tui::ui::ctx::Interaction {
        hover: Some(hover_id),
        hover_ms: Some(2000),
        ..Default::default()
    };
    let mut ctx = junie_tui::ui::ctx::RenderCtx::new(&t, interaction, &mut hits, &mut ring);
    tipped.render(area, &mut buf, &mut ctx);
    ctx.flush_overlays(&mut buf);
    assert!(
        text_of(&buf).contains("Current branch"),
        "{}",
        text_of(&buf)
    );
}

fn help_overlay() {
    let screen = Rect::new(0, 0, 80, 24);
    let id = WidgetId::of("scenario.help");
    let sections = || {
        vec![
            (
                "Navigation",
                vec![("↑↓", "Move"), ("Cmd+K", "Palette"), ("Enter", "Open")],
            ),
            ("Editing", vec![("Ctrl+Z", "Undo")]),
        ]
    };
    let mut help = HelpOverlay::new(id, "Showcase", sections());
    assert_eq!(
        help.sections[0].1.len(),
        2,
        "a key the terminal cannot deliver is not listed"
    );
    let painted = paint_hits(screen, |area, buf, ctx| help.render(area, buf, ctx));
    let text = text_of(&painted.buf);
    for expected in ["Showcase", "Navigation", "Editing", "Move", "Undo"] {
        assert!(text.contains(expected), "{expected}:\n{text}");
    }
    assert!(!text.contains("Palette"));
    assert!(!help.closed);

    // Sections from keymaps merge rows that share an action, and drop empties.
    let section = HelpSection::new("Lists")
        .row("↑↓", "Move")
        .row("Cmd+K", "Palette")
        .bindings(&[
            Binding::group("j", "Move"),
            Binding::group("Enter", "Open"),
            Binding::group("", ""),
        ]);
    assert!(!section.is_empty());
    assert_eq!(section.title, "Lists");
    assert_eq!(
        section.rows,
        [
            ("↑↓ j".to_owned(), "Move".to_owned()),
            ("Enter".to_owned(), "Open".to_owned())
        ]
    );
    assert!(HelpSection::new("Nothing").is_empty());
    assert!(HelpSection::new("Nothing").row("Cmd+K", "x").is_empty());
    let from = HelpOverlay::from_keymaps(id, "Keys", vec![section, HelpSection::new("Empty")]);
    assert_eq!(from.sections.len(), 1);
    assert_eq!(from.scope, "Keys");

    // Closing: Esc, Enter, the Close command's keys, and the buttons.
    for code in [
        KeyCode::Esc,
        KeyCode::Enter,
        KeyCode::Char('?'),
        KeyCode::Char('q'),
    ] {
        let mut help = HelpOverlay::new(id, "Showcase", sections());
        assert_eq!(help.on_key(&key(code)), Outcome::Changed, "{code:?}");
        assert!(help.closed, "{code:?}");
    }
    let mut other = HelpOverlay::new(id, "Showcase", sections()).keymap(vec![(
        junie_tui::core::keymap::KeyBinding::char('x'),
        HelpCmd::Close,
    )]);
    assert_eq!(
        other.on_key(&key(KeyCode::Char('q'))),
        Outcome::Consumed,
        "a re-mapped overlay ignores the old key"
    );
    assert!(!other.closed);
    assert_eq!(other.on_key(&key(KeyCode::Char('x'))), Outcome::Changed);
    assert!(other.closed);
    let mut sticky = HelpOverlay::new(id, "Showcase", sections()).dismiss(Dismiss::NEVER);
    assert_eq!(sticky.on_key(&key(KeyCode::Esc)), Outcome::Consumed);
    assert!(!sticky.closed);
    assert_eq!(
        sticky.on_click(junie_tui::ui::modal::CLOSE),
        Outcome::Consumed,
        "no close button under Never"
    );
    let mut clicked = HelpOverlay::new(id, "Showcase", sections());
    assert_eq!(
        clicked.on_click(junie_tui::ui::modal::CLOSE),
        Outcome::Changed
    );
    assert!(clicked.closed);
    let mut done = HelpOverlay::new(id, "Showcase", sections());
    let done_id = done.done.id;
    paint_hits(screen, |area, buf, ctx| done.render(area, buf, ctx));
    assert_eq!(done.on_click(done_id), Outcome::Changed);
    assert!(done.closed);
    let mut idle = HelpOverlay::new(id, "Showcase", sections());
    assert_eq!(
        idle.on_click(WidgetId::of("scenario.elsewhere")),
        Outcome::Consumed
    );
    let mut custom = HelpOverlay::new(id, "Showcase", sections())
        .close_button(IconButton::close(WidgetId::of("scenario.help.x")).with_glyph("\u{3a9}"));
    assert_eq!(custom.close_button.id, WidgetId::of("scenario.help.x"));
    let plain = text_of(
        &paint_hits(screen, |area, buf, ctx| {
            HelpOverlay::new(id, "Showcase", sections()).render(area, buf, ctx)
        })
        .buf,
    );
    let painted = paint_hits(screen, |area, buf, ctx| custom.render(area, buf, ctx));
    assert!(
        text_of(&painted.buf).contains('\u{3a9}') && !plain.contains('\u{3a9}'),
        "the frame draws the replacement close button"
    );

    // Title and size are options; the footer lists only deliverable keys.
    let mut titled = HelpOverlay::new(id, "Showcase", sections()).title("Shortcuts");
    assert_eq!(titled.title, "Shortcuts");
    let text = text_of(&paint_hits(screen, |area, buf, ctx| titled.render(area, buf, ctx)).buf);
    assert!(
        text.contains("Shortcuts"),
        "the frame shows the title:\n{text}"
    );
    let width_of = |pct: u16| {
        let mut help = HelpOverlay::new(id, "Showcase", sections()).max_percent(pct);
        paint_hits(screen, |area, buf, ctx| help.render(area, buf, ctx));
        help.area.width
    };
    assert!(width_of(40) < width_of(100), "max_percent bounds the frame");
    assert_eq!(
        HelpOverlay::new(id, "S", vec![]).max_percent(5).max_percent,
        20
    );
    assert_eq!(
        HelpOverlay::new(id, "S", vec![])
            .max_percent(500)
            .max_percent,
        100
    );

    // Scrolling: arrows and paging move a long list.
    let long: Vec<(String, String)> = (0..60)
        .map(|i| (format!("F{}", i % 12 + 1), format!("Action {i}")))
        .collect();
    let rows_ref: Vec<(&str, &str)> = long.iter().map(|(k, a)| (k.as_str(), a.as_str())).collect();
    let mut tall = HelpOverlay::new(id, "Long", vec![("Everything", rows_ref.clone())]);
    paint_hits(screen, |area, buf, ctx| tall.render(area, buf, ctx));
    assert_eq!(tall.on_key(&key(KeyCode::Down)), Outcome::Changed);
    assert_eq!(tall.on_key(&key(KeyCode::PageDown)), Outcome::Changed);
    let text = text_of(&paint_hits(screen, |area, buf, ctx| tall.render(area, buf, ctx)).buf);
    assert!(
        !text.contains("Action 0\n") && !tall.closed,
        "paging scrolled the list:\n{text}"
    );
    assert_eq!(tall.on_key(&key(KeyCode::Home)), Outcome::Changed);
    assert_eq!(tall.on_key(&key(KeyCode::Left)), Outcome::Consumed);
    assert_eq!(
        tall.on_key(&key(KeyCode::Char('j'))),
        Outcome::Consumed,
        "j scrolls nothing in the default profile"
    );
    let mut vim = HelpOverlay::new(id, "Long", vec![("Everything", rows_ref)]).nav(NavKeys::VIM);
    paint_hits(screen, |area, buf, ctx| vim.render(area, buf, ctx));
    assert_eq!(vim.on_key(&key(KeyCode::Char('j'))), Outcome::Changed);
    assert_eq!(vim.nav, NavKeys::VIM);
}

#[test]
fn catalog_menus_widget_hint_constants() {
    // The shared "Enter Edit" hint is the pair rendered as a hint, and its
    // label is a key a terminal delivers.
    assert_eq!(widget_hints::EDIT_PAIR, ("Enter", "Edit"));
    assert_eq!(
        widget_hints::EDIT,
        Hint::new(widget_hints::EDIT_PAIR.0, widget_hints::EDIT_PAIR.1)
    );
    assert!(junie_tui::core::keyboard::label_deliverable(
        widget_hints::EDIT_PAIR.0
    ));
    assert_eq!(
        keyhint::key_of(&widget_hints::EDIT.key),
        Some(key(KeyCode::Enter))
    );
}
