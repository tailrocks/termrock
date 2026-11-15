use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::core::refusal::Reason;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::popup::Placement;
use junie_tui::widgets::menu::{ContextMenu, MenuItem};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

fn menu(short: &str) -> ContextMenu {
    ContextMenu::new(
        WidgetId::of("t.menu"),
        vec![
            MenuItem::new("Reconnect").shortcut("Enter").reason(short),
            MenuItem::new("Stop").shortcut("t").disabled(true),
        ],
    )
    .anchor(Rect::new(2, 1, 1, 1), Placement::Below)
}

fn border_row(menu: &mut ContextMenu) -> String {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 12));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    {
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        menu.render(Rect::new(0, 0, 60, 12), &mut buf, &mut ctx);
        ctx.flush_overlays(&mut buf);
    }
    let (_, h) = menu.size();
    let bottom = 1 + h;
    (0..60)
        .map(|x| buf[(x, bottom)].symbol().to_owned())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

#[test]
fn a_reason_stays_within_the_status_and_menu_limits() {
    let text = |n: usize| "a".repeat(n);
    assert!(Reason::new(&text(60)).within_limits());
    assert!(!Reason::new(&text(61)).within_limits());
    assert!(Reason::new("x").short(&text(20)).within_limits());
    assert!(!Reason::new("x").short(&text(21)).within_limits());
    assert!(Reason::new(&"界".repeat(30)).within_limits());
    assert!(!Reason::new(&"界".repeat(31)).within_limits());
    assert!(Reason::new("x").short(&"界".repeat(10)).within_limits());
    assert!(!Reason::new("x").short(&"界".repeat(11)).within_limits());
}

#[test]
fn a_reason_is_measured_against_the_room_of_its_surface() {
    let reason = Reason::new("Always off · the host decides").short("no disabled state");
    assert!(reason.text_fits(46));
    assert!(reason.text_fits(29));
    assert!(!reason.text_fits(28));
    assert!(reason.short_fits(17));
    assert!(!reason.short_fits(16));
    assert!(!Reason::new("x").short_fits(80));
}

#[test]
fn the_menu_border_leaves_six_columns_less_than_the_menu_is_wide() {
    let menu = menu("env account");
    assert_eq!(menu.size().0, 23);
    assert_eq!(menu.reason_room(false), 17);
    assert_eq!(menu.reason_room(true), 15);
}

#[test]
fn a_short_reason_shows_whole_in_the_menu_border_up_to_its_room() {
    let fits = border_row(&mut menu("no disabled state"));
    assert!(fits.contains(" no disabled state "), "{fits}");
    let cut = border_row(&mut menu("no disabled state!!"));
    assert!(
        !cut.contains("!!") && cut.contains(" no disabled stat… "),
        "{cut}"
    );
}

fn scrolling_menu(short: &str) -> ContextMenu {
    let mut items = vec![MenuItem::new("Reconnect").shortcut("Enter").reason(short)];
    items.extend((1..31).map(|n| MenuItem::new(format!("Item {n}")).disabled(true)));
    ContextMenu::new(WidgetId::of("t.menu"), items)
        .max_rows(5)
        .anchor(Rect::new(2, 1, 1, 1), Placement::Below)
}

#[test]
fn a_scrolling_menu_keeps_its_marker_clear_of_the_reason() {
    let room = scrolling_menu("x").reason_room(true);
    assert_eq!(room, scrolling_menu("x").reason_room(false) - 2);
    let whole: String = ('a'..='z').take(room).collect();
    let fits = border_row(&mut scrolling_menu(&whole));
    assert!(
        fits.contains(&format!(" {whole} ")) && fits.contains("─▾─╯"),
        "{fits}"
    );
    let long: String = ('a'..='z').take(room + 1).collect();
    let cut = border_row(&mut scrolling_menu(&long));
    let kept: String = whole.chars().take(room - 1).collect();
    assert!(
        cut.contains(&format!(" {kept}… ")) && cut.contains("─▾─╯"),
        "{cut}"
    );
    assert!(!cut.contains(&long), "{cut}");
}

#[test]
fn a_narrow_place_shows_the_short_text_or_the_whole_reason() {
    assert_eq!(
        Reason::new("Always off · the host decides")
            .short("managed")
            .short_or_text(),
        "managed"
    );
    assert_eq!(
        Reason::new("Always off · the host decides").short_or_text(),
        "Always off · the host decides"
    );
}

mod item_eligibility {
    use junie_tui::core::event::{Key, Outcome};
    use junie_tui::core::focus::FocusRing;
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
}
