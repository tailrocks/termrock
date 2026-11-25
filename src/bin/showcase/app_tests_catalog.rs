//! Callable behavior scenarios for the capability gate (#14, layer 2).
//!
//! Each `ScenarioCase` builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. `coverage.rs` lists which APIs each
//! scenario proves; the source-only gate in `src/catalog_contract.rs` checks
//! that every referenced scenario has exactly one case here. One line per
//! case keeps a restack mechanical.

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::{Focus, FocusRing};
use junie_tui::core::hit::Axis;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::core::refusal::{Reason, clear_refusal, take_refusal};
use junie_tui::terminal::{TerminalState, TerminalViewport};
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::layout::Split;
use junie_tui::ui::popup::{Dismiss, Placement};
use junie_tui::widgets::chips::{Chip, ChipAction, ChipBar};
use junie_tui::widgets::choice::Checkbox;
use junie_tui::widgets::dialog::{Dialog, DialogResult};
use junie_tui::widgets::edit::{EditField, Validator};
use junie_tui::widgets::input::TextInput;
use junie_tui::widgets::list_detail::{Action, Event, Frame, Line, ListDetail, ListRow};
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::panel::ScrollPanel;
use junie_tui::widgets::picker::{Picker, PickerAction, PickerItemsError, VisibleKey};
use junie_tui::widgets::tabs::{TabItem, Tabs};
use junie_tui::widgets::toolbar::{ToolItem, Toolbar, ToolbarAction};
use junie_tui::widgets::viewport_ops::ViewportOps;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};

use super::super::coverage::ScenarioId;

pub struct ScenarioCase {
    pub id: ScenarioId,
    pub run: fn(),
}

#[rustfmt::skip]
pub const CASES: &[ScenarioCase] = &[
    ScenarioCase { id: ScenarioId::PickerStableSelection, run: picker_stable_selection },
    ScenarioCase { id: ScenarioId::RequiredCustomValidation, run: required_custom_validation },
    ScenarioCase { id: ScenarioId::DisabledInputRejection, run: disabled_input_rejection },
    ScenarioCase { id: ScenarioId::OverlayDismissPolicy, run: overlay_dismiss_policy },
    ScenarioCase { id: ScenarioId::CollectionReplacement, run: collection_replacement },
    ScenarioCase { id: ScenarioId::NarrowLayout, run: narrow_layout },
    ScenarioCase { id: ScenarioId::TerminalViewportLifecycle, run: terminal_viewport_lifecycle },
    ScenarioCase { id: ScenarioId::FixedControlsPolicy, run: fixed_controls_policy },
    ScenarioCase { id: ScenarioId::FixedDialogsPolicy, run: fixed_dialogs_policy },
    ScenarioCase { id: ScenarioId::ListDetailNavigation, run: list_detail_navigation },
    ScenarioCase { id: ScenarioId::ChipBarEditing, run: chip_bar_editing },
    ScenarioCase { id: ScenarioId::ToolbarEditing, run: toolbar_editing },
    ScenarioCase { id: ScenarioId::TabsEditing, run: tabs_editing },
    ScenarioCase { id: ScenarioId::ScrollPanelContent, run: scroll_panel_content },
];

fn key(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

fn rows(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_owned())
                .collect::<String>()
        })
        .collect()
}

fn text_of(buf: &Buffer) -> String {
    rows(buf).join("\n")
}

fn paint(area: Rect, draw: impl FnOnce(Rect, &mut Buffer, &mut RenderCtx)) -> Buffer {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    draw(area, &mut buf, &mut ctx);
    ctx.flush_overlays(&mut buf);
    buf
}

fn lettered(labels: &[&str]) -> Vec<Item> {
    labels
        .iter()
        .map(|label| Item::new(*label).key(ItemKey::text(&label.to_lowercase())))
        .collect()
}

fn picker_stable_selection() {
    let mut picker = Picker::new(WidgetId::of("scenario.picker"), "Choose")
        .multi_select(true)
        .expect("an empty picker accepts multi-selection");
    picker
        .set_items(lettered(&["Alpha", "Beta", "Gamma"]))
        .expect("keyed items are accepted");
    assert!(picker.visible_item(2).is_some() && picker.visible_item(3).is_none());
    picker.set_cursor(0);
    let (outcome, _) = picker.on_key(&key(KeyCode::Insert));
    assert_eq!(outcome, Outcome::Changed);
    assert_eq!(picker.checked(), &[ItemKey::text("alpha")]);

    picker
        .set_visible_keys(vec![
            VisibleKey::new(ItemKey::text("beta")).matched(vec![0]),
        ])
        .expect("a filtered view keeps hidden choices");
    assert_eq!(
        picker.visible_item(0).map(|row| row.matched.to_vec()),
        Some(vec![0])
    );
    assert_eq!(picker.checked(), &[ItemKey::text("alpha")]);
    picker
        .replace_items(lettered(&["Gamma", "Beta", "Alpha"]))
        .expect("reordering keeps identity");
    assert_eq!(picker.checked(), &[ItemKey::text("alpha")]);

    let (_, action) = picker.on_key(&key(KeyCode::Enter));
    assert_eq!(
        action,
        Some(PickerAction::ChosenMany(vec![ItemKey::text("alpha")]))
    );
    let buf = paint(Rect::new(0, 0, 80, 24), |area, buf, ctx| {
        picker.draw(area, buf, ctx)
    });
    assert!(text_of(&buf).contains("Alpha"));
}

fn required_custom_validation() {
    let mut required = TextInput::new(WidgetId::of("scenario.required"), "Name").required(true);
    assert!(!required.validate(), "an empty required field is invalid");
    assert!(required.error().is_some());
    required = required.value("filled");
    assert!(required.validate());
    assert_eq!(required.error(), None);

    let mut custom = TextInput::new(WidgetId::of("scenario.custom"), "Name")
        .validator(|text| (text == "bad").then(|| "not allowed".to_owned()));
    custom = custom.value("bad");
    assert!(!custom.validate());
    assert_eq!(custom.error(), Some("not allowed"));
    custom = custom.value("good");
    assert!(custom.validate());
    assert_eq!(custom.error(), None);

    let mut field = EditField::single("bad").validator(Validator::check(|text| {
        (text == "bad").then(|| "not allowed".to_owned())
    }));
    assert!(!field.validate());
    assert_eq!(field.error_message(), Some("not allowed"));
    field.set_text("good");
    assert!(field.validate());
    assert_eq!(field.error_message(), None);
}

fn disabled_input_rejection() {
    let mut input = TextInput::new(WidgetId::of("scenario.disabled"), "Name")
        .value("fixed")
        .disabled(true);
    let (outcome, action) = input.on_key(&key(KeyCode::Char('x')));
    assert_eq!(outcome, Outcome::Ignored);
    assert_eq!(action, None);
    assert_eq!(input.on_paste("pasted"), Outcome::Ignored);
    assert_eq!(input.text(), "fixed");

    let mut live = TextInput::new(WidgetId::of("scenario.live"), "Name").value("ok");
    live.begin_edit();
    let (outcome, _) = live.on_key(&key(KeyCode::Char('!')));
    assert_eq!(outcome, Outcome::Changed);
    assert_eq!(live.text(), "ok!");
}

fn overlay_dismiss_policy() {
    for (policy, outside, escape) in [
        (Dismiss::LIGHT, true, true),
        (Dismiss::ESCAPE, false, true),
        (Dismiss::NEVER, false, false),
    ] {
        assert_eq!(policy.outside_click, outside);
        assert_eq!(policy.escape, escape);
        let mut picker = Picker::new(WidgetId::of("scenario.dismiss"), "Choose").dismiss(policy);
        assert_eq!(
            picker.on_click_outside(),
            outside.then_some(PickerAction::Cancelled)
        );

        let mut dialog = Dialog::confirm(WidgetId::of("scenario.dialog"), "Close", "Sure?", "Yes")
            .dismiss(policy);
        dialog.on_click_outside();
        assert_eq!(
            dialog.result,
            outside.then_some(DialogResult::Action(0)),
            "an outside click resolves to the cancel action only when the policy allows it"
        );
        let mut dialog = Dialog::confirm(WidgetId::of("scenario.dialog"), "Close", "Sure?", "Yes")
            .dismiss(policy);
        let mut focus = Focus::default();
        dialog.on_key(&key(KeyCode::Esc), &mut focus, &FocusRing::default());
        assert_eq!(
            dialog.result.is_some(),
            escape,
            "Esc follows the escape policy"
        );
    }
}

fn collection_replacement() {
    let mut picker = Picker::new(WidgetId::of("scenario.replace"), "Choose");
    picker
        .set_items(lettered(&["Alpha", "Beta", "Gamma"]))
        .expect("keyed items are accepted");
    picker.set_cursor(1);
    assert_eq!(
        picker.current().map(|item| item.label.as_str()),
        Some("Beta")
    );
    picker
        .replace_items(lettered(&["Gamma", "Beta", "Alpha"]))
        .expect("reordering keeps identity");
    assert_eq!(
        picker.current().map(|item| item.label.as_str()),
        Some("Beta"),
        "the cursor follows the item, not the index"
    );
    let duplicate = lettered(&["Alpha", "Alpha"]);
    assert_eq!(
        picker.replace_items(duplicate),
        Err(PickerItemsError::DuplicateKey(ItemKey::text("alpha")))
    );
    assert!(
        picker.visible_item(2).is_some() && picker.visible_item(3).is_none(),
        "a rejected replacement changes nothing"
    );
}

fn narrow_layout() {
    let mut picker = Picker::new(WidgetId::of("scenario.narrow"), "Choose")
        .width(64)
        .max_rows(3)
        .placement(Placement::Center);
    picker
        .set_items(lettered(&["Alpha", "Beta", "Gamma", "Delta", "Epsilon"]))
        .expect("keyed items are accepted");
    let area = Rect::new(0, 0, 28, 12);
    let buf = paint(area, |area, buf, ctx| picker.draw(area, buf, ctx));
    let text = text_of(&buf);
    assert!(
        text.contains("Choose"),
        "the title survives a 28 column screen:\n{text}"
    );
    assert!(text.contains("Alpha"));
    assert!(
        !text.contains("Epsilon"),
        "max_rows bounds the visible rows"
    );
}

fn terminal_viewport_lifecycle() {
    let id = WidgetId::of("scenario.terminal");
    let mut viewport = TerminalViewport::new(id, TerminalState::new(10, 10));
    viewport.set_area(Rect::new(0, 0, 11, 10));
    let lines = (1..=100)
        .map(|n| format!("line {n:03}"))
        .collect::<Vec<_>>()
        .join("\r\n");
    viewport.feed(lines.as_bytes());
    assert!(viewport.follow());
    assert_eq!(
        viewport.position_label(),
        "",
        "the live tail shows no position"
    );
    assert_eq!(viewport.wheel(Axis::Vertical, -3), Outcome::Changed);
    assert!(!viewport.follow());
    assert_eq!(viewport.offset(), 3);
    assert_eq!(viewport.position_label(), "88–97 of 100");
    viewport.feed(b"\r\nline 101");
    assert_eq!(
        viewport.offset(),
        4,
        "new output does not move what is read"
    );
    viewport.reset();
    assert_eq!(viewport.offset(), 0);
}

fn fixed_controls_policy() {
    clear_refusal();
    let reason = || Reason::new("Always off, the host decides").short("managed");
    let mut fixed =
        Checkbox::new(WidgetId::of("scenario.fixed"), "Keep awake", false).fixed(reason());
    assert_eq!(fixed.on_key(&key(KeyCode::Char(' '))), Outcome::Consumed);
    assert_eq!(
        take_refusal().map(|r| r.text).as_deref(),
        Some("Always off, the host decides")
    );
    assert_eq!(fixed.on_click(), Outcome::Consumed);
    assert!(take_refusal().is_some());
    assert!(!fixed.checked);
    assert_eq!(
        take_refusal().map(|r| r.text),
        None,
        "taking the refusal clears it"
    );

    let mut live = Checkbox::new(WidgetId::of("scenario.live"), "Keep awake", false);
    assert_eq!(live.on_key(&key(KeyCode::Char(' '))), Outcome::Changed);
    assert!(live.checked);
    assert_eq!(take_refusal().map(|r| r.text), None);
}

fn fixed_dialogs_policy() {
    clear_refusal();
    let route = |fixed: bool| {
        let id = WidgetId::of("scenario.fixed.dialog");
        let mut dialog = Dialog::confirm(id, "Disable all", "Every item stays on.", "Disable");
        if fixed {
            dialog.actions[1] = dialog.actions[1]
                .clone()
                .fixed(Reason::new("Always on, the host decides"));
        }
        let mut focus = Focus::default();
        focus.set(Some(id.sub("ok")));
        let outcome = dialog.on_key(&key(KeyCode::Enter), &mut focus, &FocusRing::default());
        (outcome, dialog.result)
    };
    let (outcome, result) = route(false);
    assert_eq!(outcome, Outcome::Changed);
    assert_eq!(result, Some(DialogResult::Action(1)));
    assert!(take_refusal().is_none());
    let (outcome, result) = route(true);
    assert!(outcome.consumed());
    assert_eq!(result, None, "a fixed action never resolves the dialog");
    assert_eq!(
        take_refusal().map(|r| r.text).as_deref(),
        Some("Always on, the host decides")
    );
}

fn list_detail_navigation() {
    const MASTER: WidgetId = WidgetId::of("scenario.ld.master");
    const DETAIL: WidgetId = WidgetId::of("scenario.ld.detail");
    let area = Rect::new(0, 0, 120, 20);
    let rows = vec![ListRow::new("first"), ListRow::new("second")];
    let mut list = ListDetail::new(MASTER, DETAIL, Split::new(42, 36, 44));
    let mut focus = Focus::default();
    focus.focus(DETAIL);
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let render = |list: &mut ListDetail, focus: &Focus| {
        let mut buf = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let interaction = Interaction {
            focus: focus.current(),
            ..Default::default()
        };
        let mut ctx = RenderCtx::new(&theme, interaction, &mut hits, &mut ring);
        let items = (0..4).map(|n| Line::Item {
            mark: None,
            cols: [format!("item{n:02}"), "agent".into(), "meta".into()],
            tone: junie_tui::theme::Tone::Secondary,
        });
        let frame = Frame {
            title: "List",
            meta: String::new(),
            head: vec![],
            rows: &rows,
            under: None,
            detail_title: "Detail".into(),
            detail_meta: String::new(),
            lines: items.collect(),
            actions: vec![Action::new(
                "go",
                "go the whole selection",
                junie_tui::theme::ButtonKind::Secondary,
            )],
        };
        list.render(area, &mut buf, &mut ctx, &frame);
        buf
    };
    let buf = render(&mut list, &focus);
    assert!(text_of(&buf).contains("second"), "both rows are listed");
    for _ in 0..3 {
        list.on_key(&key(KeyCode::Down), &mut focus, rows.len());
    }
    let (_, event) = list.on_key(&key(KeyCode::Enter), &mut focus, rows.len());
    assert_eq!(
        event,
        Some(Event::Item(3)),
        "Enter reports the item under the cursor"
    );
    list.select(1);
    assert_eq!(
        (list.cursor, list.item_cursor),
        (1, 0),
        "selecting a row resets the item cursor"
    );
    list.clamp(1);
    assert_eq!(
        list.cursor, 0,
        "clamp keeps the cursor inside a shorter list"
    );
    list.open_detail(&mut focus);
    list.close_detail(&mut focus);
    let _ = Position::new(0, 0);
}

fn chip_bar_editing() {
    let mut bar = ChipBar::new(WidgetId::of("scenario.chips"));
    assert!(bar.chips().is_empty());
    assert!(bar.set_chips(vec![Chip::new("alpha"), Chip::new("beta")]));
    assert!(bar.insert_chip(2, Chip::new("gamma")));
    assert!(!bar.insert_chip(9, Chip::new("past the end")));
    assert!(bar.update_chip(1, |chip| chip.enabled = false));
    assert!(!bar.update_chip(1, |chip| chip.enabled = false));
    assert_eq!(
        bar.remove_chip(0).map(|chip| chip.label),
        Some("alpha".to_owned())
    );
    assert!(bar.remove_chip(9).is_none());
    let labels: Vec<&str> = bar.chips().iter().map(|chip| chip.label.as_str()).collect();
    assert_eq!(labels, ["beta", "gamma"]);
    assert!(!bar.chips()[0].enabled);

    assert_eq!(bar.lead(), None);
    assert!(bar.set_lead(Some("match all".to_owned())));
    assert_eq!(bar.lead(), Some("match all"));
    assert_eq!(bar.add_label(), Some("+ Add filter"));
    assert!(bar.set_add_label(Some("+ New".to_owned())));
    assert!(!bar.set_add_label(Some("+ New".to_owned())));
    let draw = |bar: &mut ChipBar| {
        paint(Rect::new(0, 0, 70, 1), |area, buf, ctx| {
            bar.render(area, buf, ctx, ctx.theme.canvas)
        })
    };
    let text = text_of(&draw(&mut bar));
    for shown in ["match all", "beta", "gamma", "+ New"] {
        assert!(text.contains(shown), "{shown} is drawn:\n{text}");
    }
    assert!(!text.contains("+ Add filter"));
    let (_, action) = bar.on_key(&key(KeyCode::Char('+')));
    assert_eq!(action, Some(ChipAction::Add));

    assert!(bar.set_add_label(None));
    let text = text_of(&draw(&mut bar));
    assert!(!text.contains("+ New"), "the add action is gone:\n{text}");
    let (_, action) = bar.on_key(&key(KeyCode::Char('+')));
    assert_eq!(action, None, "a bar without an add label cannot add");
}

fn toolbar_editing() {
    let items = || {
        vec![
            ToolItem::new("Run"),
            ToolItem::new("Stop").reason("No query is running"),
        ]
    };
    let mut bar = Toolbar::new(WidgetId::of("scenario.toolbar"), items());
    assert_eq!(bar.items().len(), 2);
    assert!(bar.insert_item(1, ToolItem::new("Run all")));
    assert!(!bar.insert_item(9, ToolItem::new("past the end")));
    assert!(bar.update_item(2, |item| {
        item.disabled = false;
        item.reason = None;
    }));
    assert!(!bar.update_item(2, |item| item.disabled = false));
    assert_eq!(
        bar.remove_item(0).map(|item| item.label),
        Some("Run".to_owned())
    );
    assert!(bar.remove_item(9).is_none());
    let labels: Vec<&str> = bar.items().iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels, ["Run all", "Stop"]);
    assert!(!bar.items()[1].disabled, "Stop was enabled in place");

    assert!(bar.set_items(items()));
    assert!(
        !bar.set_items(items()),
        "an equal collection changes nothing"
    );
    let text = text_of(&paint(Rect::new(0, 0, 40, 1), |area, buf, ctx| {
        bar.render(area, buf, ctx, ctx.theme.canvas)
    }));
    assert!(text.contains("Run") && text.contains("Stop"), "{text}");
    let (_, action) = bar.on_key(&key(KeyCode::Enter));
    assert_eq!(action, Some(ToolbarAction::Activated(0)));
}

fn tabs_editing() {
    let mut tabs = Tabs::new(WidgetId::of("scenario.tabs"), &["one", "two"]);
    tabs.push(TabItem::new("three").closable());
    let labels = |tabs: &Tabs| -> Vec<String> {
        tabs.items().iter().map(|item| item.label.clone()).collect()
    };
    assert_eq!(labels(&tabs), ["one", "two", "three"]);
    tabs.set_active(2);
    assert!(tabs.reorder(2, 0), "the last tab moves to the front");
    assert!(!tabs.reorder(1, 1) && !tabs.reorder(0, 9));
    assert_eq!(labels(&tabs), ["three", "one", "two"]);
    assert_eq!(tabs.active, 0, "the active mark follows the moved tab");
    assert!(tabs.update_item(1, |item| item.dirty = true));
    assert!(!tabs.update_item(1, |item| item.dirty = true));
    assert!(!tabs.update_item(9, |item| item.dirty = true));
    assert!(tabs.items()[1].dirty);
    let text = text_of(&paint(Rect::new(0, 0, 60, 3), |area, buf, ctx| {
        tabs.render(area, buf, ctx, ctx.theme.canvas)
    }));
    let at = |label: &str| {
        text.find(label)
            .unwrap_or_else(|| panic!("{label}:\n{text}"))
    };
    assert!(at("three") < at("one") && at("one") < at("two"), "{text}");
}

fn scroll_panel_content() {
    fn plain(theme: &Theme, _line: &str) -> ratatui::style::Style {
        theme.secondary()
    }
    let source = |n: usize| (1..=n).map(|i| format!("line {i}")).collect::<Vec<_>>();
    let mut panel = ScrollPanel::new(WidgetId::of("scenario.panel"), source(5));
    let draw = |panel: &mut ScrollPanel| {
        text_of(&paint(Rect::new(0, 0, 30, 8), |area, buf, ctx| {
            panel.render(area, buf, ctx, ctx.theme.canvas, plain)
        }))
    };
    assert!(draw(&mut panel).contains("line 5"));
    assert_eq!(panel.lines().len(), 5);
    assert_eq!(panel.replace_line(1, "edited".to_owned()), Ok(true));
    assert_eq!(panel.replace_line(1, "edited".to_owned()), Ok(false));
    assert_eq!(panel.replace_line(9, "x".to_owned()), Err(9));
    assert!(panel.extend_lines(["appended".to_owned()]));
    assert!(!panel.extend_lines(Vec::new()));
    let text = draw(&mut panel);
    assert!(
        text.contains("edited") && text.contains("appended"),
        "{text}"
    );
    assert!(!text.contains("line 2"), "the old text is gone:\n{text}");

    panel.select_all();
    assert_eq!(
        panel.source_selection(),
        Some(((0, 0), (5, "appended".len()))),
        "the selection is in source lines, not wrapped rows"
    );
    assert!(panel.extend_lines(["more".to_owned()]));
    assert!(panel.source_selection().is_some(), "an append keeps it");
    assert!(panel.set_lines(source(2)));
    assert_eq!(panel.source_selection(), None, "a replacement clears it");
    assert_eq!(panel.lines(), ["line 1", "line 2"]);
    assert!(panel.clear());
    assert!(!panel.clear());
    assert!(panel.lines().is_empty());
    assert!(!draw(&mut panel).contains("line 1"));
}

#[test]
fn catalog_scenarios_execute() {
    for case in CASES {
        (case.run)();
    }
}

#[test]
fn catalog_scenario_ids_are_registered_once() {
    let mut seen = std::collections::BTreeSet::new();
    for case in CASES {
        assert!(
            seen.insert(case.id),
            "scenario {:?} is registered twice",
            case.id
        );
    }
}

#[test]
fn catalog_proofs_have_callable_cases() {
    use super::super::coverage::PROOFS;
    let registered: std::collections::BTreeSet<_> = CASES.iter().map(|case| case.id).collect();
    let routes: std::collections::BTreeSet<_> =
        super::NAV_ENTRIES.iter().map(|e| e.id as u8).collect();
    let mut apis = std::collections::BTreeSet::new();
    for proof in PROOFS {
        assert!(apis.insert(proof.api), "duplicate proof: {}", proof.api);
        assert!(
            !proof.scenarios.is_empty(),
            "{} names no scenario",
            proof.api
        );
        assert!(
            routes.contains(&(proof.route as u8)),
            "{} names a route that is not in the navigation",
            proof.api
        );
        for scenario in proof.scenarios {
            assert!(
                registered.contains(scenario),
                "{} names scenario {scenario:?} with no callable case",
                proof.api
            );
        }
    }
    for case in CASES {
        assert!(
            PROOFS
                .iter()
                .any(|proof| proof.scenarios.contains(&case.id)),
            "scenario {:?} proves no API",
            case.id
        );
    }
}

#[test]
fn catalog_all_routes_visible_once() {
    use super::{NAV_ENTRIES, NAV_GROUPS, PageId, entries, nav_roots};
    let file = syn::parse_file(include_str!("app.rs")).expect("app.rs parses");
    let declared = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Enum(e) if e.ident == "PageId" => Some(e.variants.len()),
            _ => None,
        })
        .expect("PageId is declared");
    assert_eq!(
        NAV_ENTRIES.len(),
        declared,
        "every PageId has one navigation entry"
    );
    assert_eq!(
        entries().count(),
        NAV_ENTRIES.len(),
        "entries() returns each entry once"
    );
    let mut seen = std::collections::BTreeSet::new();
    for entry in NAV_ENTRIES {
        assert!(
            seen.insert(entry.id as u8),
            "{} is listed twice",
            entry.label
        );
        assert!(
            entry.group.is_empty() || NAV_GROUPS.contains(&entry.group),
            "{} names an unknown group",
            entry.label
        );
    }
    for (name, id) in [
        ("terminalpane", PageId::TerminalPane),
        ("fixedcontrols", PageId::FixedControls),
        ("terminalviewport", PageId::TerminalViewport),
        ("listdetail", PageId::ListDetail),
        ("fixeddialogs", PageId::FixedDialogs),
    ] {
        assert_eq!(
            PageId::from_name(name),
            Some(id),
            "the CLI route {name} stays"
        );
        assert_eq!(
            id.index(),
            NAV_ENTRIES.iter().position(|e| e.id == id).expect("listed")
        );
        assert_eq!(
            PageId::from_nav_path(&id.nav_path()),
            Some(id),
            "{name} is reachable from the sidebar"
        );
        assert!(
            !id.entry().group.is_empty(),
            "{name} sits in a sidebar group like every other component page"
        );
    }
    for entry in NAV_ENTRIES {
        assert_eq!(
            PageId::from_nav_path(&entry.id.nav_path()),
            Some(entry.id),
            "{}",
            entry.label
        );
    }
    assert_eq!(
        nav_roots()
            .iter()
            .filter(|r| matches!(r, super::NavRoot::Group(_)))
            .count(),
        NAV_GROUPS.len()
    );
}

#[test]
fn catalog_picker_setters_and_accessors_behave() {
    use junie_tui::core::hints::{HasHints, HintCx};
    use junie_tui::core::hit::Axis;
    use junie_tui::core::keymap::KeyBinding;
    use junie_tui::widgets::picker::{DEFAULT_GLYPH, PickerCmd, PickerKeyProfile};

    assert_eq!(
        (Picker::MAX_ROWS, Picker::WIDTH, DEFAULT_GLYPH),
        (12, 64, "·")
    );
    let id = WidgetId::of("scenario.accessors");
    let mut picker = Picker::new(id, "Choose")
        .choose_hint("Pick")
        .keymap(vec![(KeyBinding::key(KeyCode::Insert), PickerCmd::Toggle)]);
    assert!(!picker.is_multi());
    picker
        .set_items(lettered(&["Alpha", "Beta", "Gamma"]))
        .expect("keyed items are accepted");
    picker.set_cursor(1);
    assert_eq!(picker.current_key(), Some(ItemKey::text("beta")));
    picker.set_chosen(Some(2));
    picker.set_chosen(Some(99));
    picker.set_width(40);
    picker.set_placement(Placement::Below);
    assert!(
        HasHints::hints(&picker, &HintCx::new(None))
            .iter()
            .any(|hint| hint.action == "Pick"),
        "choose_hint labels the choose key"
    );

    #[allow(deprecated)]
    let rendered = paint(Rect::new(0, 0, 80, 24), |area, buf, ctx| {
        picker.render(area, buf, ctx, "")
    });
    assert!(text_of(&rendered).contains("Choose"));
    assert_eq!(
        picker.on_click(id.sub("missing")),
        None,
        "an unknown id is no row"
    );
    assert_eq!(
        picker.on_click(picker.row_id(0)),
        Some(PickerAction::Chosen(0))
    );
    assert_eq!(
        picker.wheel(Axis::Vertical, 1),
        Outcome::Ignored,
        "three rows fit, so the wheel has nothing to scroll"
    );
    assert_eq!(picker.on_click_outside(), Some(PickerAction::Cancelled));

    let (outcome, action) = picker.on_paste("al");
    assert_eq!(
        (outcome, action),
        (Outcome::Changed, Some(PickerAction::QueryChanged))
    );
    assert_eq!(picker.caret(), 2, "the caret follows the pasted text");

    PickerKeyProfile::Vim.install();
    assert_eq!(PickerKeyProfile::current(), PickerKeyProfile::Vim);
    PickerKeyProfile::Desktop.install();
    assert_eq!(PickerKeyProfile::current(), PickerKeyProfile::Desktop);
}
