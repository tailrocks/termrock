use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::FocusRing;
use junie_tui::core::hints::{HasHints, HintCx};
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::core::text::TextBuffer;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::widgets::completion::{Completion, CompletionAction};
use junie_tui::widgets::option_editor::{Affordance, ColumnEditor, EditorConfig, Options};
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::table::{Cell, Column, DataTable, TableAction};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Position, Rect};

const SCREEN: Rect = Rect::new(0, 0, 60, 14);

fn key(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

fn render_with(draw: impl FnOnce(Rect, &mut Buffer, &mut RenderCtx)) -> HitRegistry {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(SCREEN);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        draw(SCREEN, &mut buf, &mut ctx);
        ctx.flush_overlays(&mut buf);
    }
    hits
}

fn at(hits: &HitRegistry, id: WidgetId) -> Position {
    let region = hits
        .regions()
        .iter()
        .rev()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("{id:?} has no hit region"));
    Position::new(region.area.x, region.area.y)
}

fn click(hits: &HitRegistry, id: WidgetId, count: u8) -> PointerEvent {
    PointerEvent::new(
        hits,
        Phase::Click,
        id,
        at(hits, id),
        count,
        KeyModifiers::NONE,
    )
}

fn parts<A>(r: junie_tui::core::response::Response<A>) -> (Outcome, Option<A>) {
    let (out, actions) = r.into_parts();
    (out, actions.into_iter().next())
}

fn completion(items: Vec<Item>) -> Completion {
    let mut c = Completion::new(WidgetId::of("t.completion"));
    c.open(items, Rect::new(4, 2, 1, 1), 2);
    c
}

fn mixed() -> Vec<Item> {
    vec![
        Item::new("blocked").disabled(true),
        Item::new("beta").reason("needs a license"),
        Item::new("bravo"),
    ]
}

fn render_completion(c: &mut Completion) -> HitRegistry {
    render_with(|screen, buf, ctx| c.render(screen, buf, ctx))
}

#[test]
fn issue36_completion_skips_disabled() {
    let c = completion(mixed());
    assert_eq!(
        c.list.cursor.index, 2,
        "open lands on the first eligible item"
    );

    for code in [KeyCode::Enter, KeyCode::Tab] {
        let mut c = completion(mixed());
        assert_eq!(
            c.on_key(&key(code)).1,
            Some(CompletionAction::Accept(2)),
            "{code:?} accepts the cursor item"
        );
        render_completion(&mut c);
        assert_eq!(
            c.on_key(&key(code)).1,
            Some(CompletionAction::Accept(2)),
            "{code:?} after a render accepts the same item"
        );
    }

    for row in [0, 1] {
        let mut c = completion(mixed());
        let hits = render_completion(&mut c);
        let ev = click(&hits, c.row_id(row), 1);
        let (out, action) = parts(c.route(&ev).expect("popup row is hit"));
        assert!(out.consumed(), "a click on a disabled row is consumed");
        assert_eq!(action, None, "row {row} is disabled and never accepts");
        assert_eq!(
            c.list.cursor.index, 2,
            "the cursor stays on an eligible item"
        );
    }

    let mut c = completion(mixed());
    let hits = render_completion(&mut c);
    let ev = click(&hits, c.row_id(2), 1);
    assert_eq!(
        c.route(&ev)
            .and_then(|r| r.into_actions().into_iter().next()),
        Some(CompletionAction::Accept(2))
    );
}

#[test]
fn issue36_completion_with_no_eligible_item_is_inert_and_dismissible() {
    let all_disabled = || {
        vec![
            Item::new("blocked").disabled(true),
            Item::new("nope").reason("why"),
        ]
    };
    for code in [KeyCode::Enter, KeyCode::Tab] {
        let mut c = completion(all_disabled());
        let (out, action) = c.on_key(&key(code));
        assert_eq!(action, None, "{code:?} cannot accept a disabled item");
        assert!(
            out.consumed(),
            "{code:?} is consumed, not leaked to the editor"
        );
        assert!(c.is_open(), "the popup stays up to explain the list");
    }
    let mut c = completion(all_disabled());
    let (out, action) = c.on_key(&key(KeyCode::Esc));
    assert_ne!(out, Outcome::Ignored);
    assert_eq!(action, Some(CompletionAction::Dismiss));
    assert!(!c.is_open(), "Esc still dismisses");
}

#[test]
fn issue36_completion_apply_rechecks_eligibility() {
    let mut c = completion(mixed());
    let accepted = 2;
    let mut items = c.items().to_vec();
    items[accepted].disabled = true;
    c.set_items(items);
    for stale in [0, 1, accepted, 9] {
        let mut buffer = TextBuffer::single("ab");
        buffer.select_range(0, 1);
        let before = (
            buffer.text().to_owned(),
            buffer.cursor_offset(),
            buffer.selection(),
        );
        assert!(
            !c.apply(stale, &mut buffer),
            "stale index {stale} is refused"
        );
        let after = (
            buffer.text().to_owned(),
            buffer.cursor_offset(),
            buffer.selection(),
        );
        assert_eq!(after, before, "stale index {stale} leaves the buffer alone");
    }

    let mut c = completion(mixed());
    let mut items = c.items().to_vec();
    items.truncate(2);
    c.set_items(items);
    let mut buffer = TextBuffer::single("ab");
    assert!(!c.apply(2, &mut buffer));
    assert_eq!(buffer.text(), "ab", "a removed item cannot be applied");

    let c = completion(mixed());
    let mut buffer = TextBuffer::single("ab");
    c.apply(2, &mut buffer);
    assert_eq!(
        buffer.text(),
        "bravo",
        "an eligible item still replaces the prefix"
    );
}

fn table() -> DataTable {
    let flag = Options::values(&["off", "on"]);
    let mode = Options::values(&["a", "b", "c"]);
    let columns = vec![
        Column::new("Name", Constraint::Length(10)).editable(),
        Column::new("Mode", Constraint::Length(8)).editor(ColumnEditor::Select(mode)),
        Column::new("Flag", Constraint::Length(8)).editor(ColumnEditor::Checkbox(flag)),
    ];
    let rows = (0..2)
        .map(|_| vec![Cell::new("one"), Cell::new("a"), Cell::new("off")])
        .collect();
    let mut t = DataTable::try_new(WidgetId::of("t.table"), columns, rows)
        .unwrap()
        .cell_nav(true)
        .editor_config(EditorConfig {
            affordance: Affordance::Always,
            ..EditorConfig::default()
        });
    t.set_row_disabled(0, true);
    t
}

fn render_table(t: &mut DataTable) -> HitRegistry {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    render_with(|screen, buf, ctx| t.render(screen, buf, ctx, theme.canvas))
}

fn cells(t: &DataTable) -> Vec<Vec<String>> {
    t.rows()
        .iter()
        .map(|r| r.iter().map(|c| c.text.clone()).collect())
        .collect()
}

#[test]
fn issue36_disabled_table_row_rejects_all_editors() {
    let pristine = cells(&table());
    for col in 0..3 {
        for code in [KeyCode::F(2), KeyCode::Enter] {
            let mut t = table();
            t.cursor_row = 0;
            t.cursor_col = col;
            let (out, action) = parts(t.on_key(&key(code)));
            assert!(out.consumed(), "col {col} {code:?} is consumed");
            assert_eq!(action, None, "col {col} {code:?} claims no change");
            assert!(!t.is_editing(), "col {col} {code:?} opens no editor");
            assert_eq!(cells(&t), pristine, "col {col} {code:?} leaves the data");
        }

        let mut t = table();
        t.cursor_col = col;
        let hits = render_table(&mut t);
        let ev = click(&hits, t.cell_id(0, col), 2);
        let (_, action) = parts(t.route(&ev).expect("cell is hit"));
        assert_eq!(action, None, "col {col} double-click claims no change");
        assert!(!t.is_editing(), "col {col} double-click opens no editor");
        assert_eq!(
            cells(&t),
            pristine,
            "col {col} double-click leaves the data"
        );

        assert_eq!(t.on_paste("pasted"), Outcome::Ignored);
        assert_eq!(cells(&t), pristine, "col {col} paste leaves the data");
    }

    let mut t = table();
    let hits = render_table(&mut t);
    let affordance = t.open_id(0, 1);
    let ev = click(&hits, affordance, 1);
    let (_, action) = parts(t.route(&ev).expect("affordance is hit"));
    assert_eq!(action, None);
    assert!(!t.is_editing(), "the popup affordance opens nothing");

    t.cursor_row = 1;
    t.cursor_col = 1;
    let _ = t.on_key(&key(KeyCode::F(2)));
    assert!(t.is_editing(), "an enabled row still edits");
}

fn editing(col: usize) -> DataTable {
    let mut t = table();
    t.cursor_row = 1;
    t.cursor_col = col;
    let _ = t.on_key(&key(KeyCode::F(2)));
    assert!(t.is_editing(), "col {col} editor opens on the enabled row");
    if col == 0 {
        let _ = t.on_key(&key(KeyCode::Char('x')));
    } else {
        render_table(&mut t);
        let _ = t.on_key(&key(KeyCode::Down));
    }
    t
}

fn commits(action: Option<TableAction>) -> bool {
    matches!(action, Some(TableAction::Committed { .. }))
}

#[test]
fn issue36_disable_during_edit_cancels_commit() {
    let pristine = cells(&table());
    for col in [0, 1] {
        let mut t = editing(col);
        t.set_row_disabled(1, true);
        assert!(!t.is_editing(), "col {col}: disabling closes the editor");
        let hits = render_table(&mut t);
        assert!(
            hits.regions().iter().all(|r| r.owner != t.choice_id()),
            "col {col}: no popup region survives a redraw"
        );
        for code in [KeyCode::Enter, KeyCode::Tab] {
            let (_, action) = parts(t.on_key(&key(code)));
            assert!(!commits(action), "col {col}: {code:?} commits nothing");
        }
        assert!(!commits(t.on_blur().into_actions().into_iter().next()));
        assert_eq!(cells(&t), pristine, "col {col}: the cell is unchanged");
        t.set_row_disabled(1, false);
        assert!(
            !t.is_editing(),
            "col {col}: re-enabling does not resurrect it"
        );

        for finish in 0..3 {
            let mut t = editing(col);
            t.disabled.insert(1);
            let (out, action) = match finish {
                0 => parts(t.on_key(&key(KeyCode::Enter))),
                1 => parts(t.on_key(&key(KeyCode::Tab))),
                _ => (
                    Outcome::Changed,
                    t.on_blur().into_actions().into_iter().next(),
                ),
            };
            assert!(out.consumed());
            assert!(
                matches!(action, None | Some(TableAction::Cancelled)),
                "col {col} finish {finish}: no commit, at most a cancellation: {action:?}"
            );
            assert!(!t.is_editing(), "col {col} finish {finish}: session closed");
            assert_eq!(cells(&t), pristine, "col {col} finish {finish}: unchanged");
            let hits = render_table(&mut t);
            assert!(hits.regions().iter().all(|r| r.owner != t.choice_id()));
            let (_, again) = parts(t.on_key(&key(KeyCode::Enter)));
            assert!(!commits(again), "no second event on a later key");
        }
    }
}

#[test]
fn issue36_completion_advertises_accept_only_with_an_eligible_item() {
    let accepts = |c: &Completion| {
        c.hints(&HintCx::new(None))
            .iter()
            .any(|h| h.action == "Accept")
    };
    let blocked = completion(vec![Item::new("blocked").disabled(true)]);
    assert!(!accepts(&blocked), "nothing can be accepted");
    assert!(!blocked.has_eligible());

    let mixed = completion(mixed());
    assert!(accepts(&mixed));
    assert!(mixed.has_eligible());
}

fn drawn(c: &mut Completion) -> String {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(SCREEN);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        c.render(SCREEN, &mut buf, &mut ctx);
        ctx.flush_overlays(&mut buf);
    }
    (0..SCREEN.height)
        .map(|y| {
            (0..SCREEN.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn issue281_completion_draws_why_the_highlighted_entry_is_disabled() {
    let reason = "Moved into the request above";
    let mut blocked = completion(vec![Item::new("url").reason(reason)]);
    assert!(!blocked.has_eligible());
    assert!(
        drawn(&mut blocked).contains(reason),
        "the highlighted disabled entry shows its reason:\n{}",
        drawn(&mut blocked)
    );

    let mut mixed = completion(vec![Item::new("url").reason(reason), Item::new("fetch(")]);
    assert_eq!(mixed.list.cursor.index, 1, "open skips the disabled entry");
    assert!(
        !drawn(&mut mixed).contains(reason),
        "an eligible highlighted entry shows no reason"
    );
}

fn drawn_on(c: &mut Completion, screen: Rect) -> Vec<String> {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(screen);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        c.render(screen, &mut buf, &mut ctx);
        ctx.flush_overlays(&mut buf);
    }
    (0..screen.height)
        .map(|y| {
            (0..screen.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

#[test]
fn issue294_completion_reason_wider_than_the_popup_cap_fits_whole_when_the_screen_allows() {
    // 60 columns is `Reason::TEXT_MAX`, the longest reason a caller may set.
    let reason = "Moved into the request above because the request builder own";
    assert_eq!(reason.chars().count(), 60);
    let build = || {
        let mut c = Completion::new(WidgetId::of("t.completion294"));
        c.open(
            vec![Item::new("url").reason(reason), Item::new("fetch(")],
            Rect::new(2, 2, 1, 1),
            0,
        );
        c.list.cursor.index = 0;
        c
    };

    let wide = drawn_on(&mut build(), Rect::new(0, 0, 100, 20));
    assert!(
        wide.iter().any(|l| l.contains(reason)),
        "a 100-column screen shows the 60-column reason whole:\n{}",
        wide.join("\n")
    );

    let narrow = Rect::new(0, 0, 40, 20);
    let rows = drawn_on(&mut build(), narrow);
    let border = rows
        .iter()
        .find(|l| l.contains('╰'))
        .unwrap_or_else(|| panic!("no bottom border:\n{}", rows.join("\n")));
    assert!(
        border.contains('…') && !border.contains(reason),
        "a 40-column screen clamps the popup and ellipsizes the reason: {border:?}"
    );
    assert!(
        border.contains('╯'),
        "the popup stays on the screen: {border:?}"
    );
}
