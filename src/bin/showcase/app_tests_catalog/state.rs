//! Lane `state` of the #14 burn-down: this lane's callable scenarios.
//!
//! Each scenario builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. Register one `ScenarioCase` per
//! `ScenarioId` variant from the `lane:state` block in `../coverage.rs`. Edit only
//! this file and `../coverage/lane_state.rs`.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};
use termrock::core::edit::{
    CMD_TEXT_KEYS, EditEntry, EditKind, FieldKeys, FocusSelect, TextCmd, UndoStack, clipboard,
};
use termrock::core::event::{ClientPlatform, Key, Outcome};
use termrock::core::focus::{
    BlurAction, BlurTarget, Focus, FocusChange, FocusFallback, FocusRing, cancel_focus_request,
    forward_blur, request_focus_next,
};
use termrock::core::hit::{Axis, HitRegistry};
use termrock::core::id::{ItemKey, Part, PartRef, WidgetId, custom_hash16};
use termrock::core::keymap::{KeyBinding, Nav, NavKeys, lookup};
use termrock::core::list::{
    Cursor, KeyStep, Moved, NavProfile, Navigable, RowFn, RowSource, TypeAheadPrecedence, route_key,
};
use termrock::core::nav::step_index;
use termrock::core::pointer::PointerState;
use termrock::core::response::{Actions, Flow, Invalidate, Response, StateFlags};
use termrock::core::selection::{
    Activate, CheckClick, ClickPolicy, RowSelection, SelectGesture, SelectionMode, TriState,
};
use termrock::core::shape::{
    CARET_RESET, CaretShape, PointerShape, ShapeSupport, ShapeSync, Shapes, reset_sequence,
    shapes_for, under,
};
use termrock::core::text::{CursorPos, TextBuffer};
use termrock::core::typeahead::TypeAhead;
use termrock::theme::{ColorLevel, Glyphs, Theme};
use termrock::ui::ctx::{Interaction, RenderCtx};
use termrock::widgets::edit::EditField;
use termrock::widgets::input::TextInput;
use termrock::widgets::list::{ListBox, ListItem, SelectMode};
use termrock::widgets::table::{DataTable, TableAction};

use super::ScenarioCase;
use super::{cells, paint, paint_hits, text_of, two_columns};
use crate::coverage::ScenarioId;

#[rustfmt::skip]
pub const CASES: &[ScenarioCase] = &[
    ScenarioCase { id: ScenarioId::StateIdentity, run: state_identity },
    ScenarioCase { id: ScenarioId::StateFocusRing, run: state_focus_ring },
    ScenarioCase { id: ScenarioId::StateFocusBlur, run: state_focus_blur },
    ScenarioCase { id: ScenarioId::StateResponseFlow, run: state_response_flow },
    ScenarioCase { id: ScenarioId::StatePointerShapes, run: state_pointer_shapes },
    ScenarioCase { id: ScenarioId::StateListCursor, run: state_list_cursor },
    ScenarioCase { id: ScenarioId::StateTypeAhead, run: state_type_ahead },
    ScenarioCase { id: ScenarioId::StateNavProfile, run: state_nav_profile },
    ScenarioCase { id: ScenarioId::StateRowSelection, run: state_row_selection },
    ScenarioCase { id: ScenarioId::StateClickPolicy, run: state_click_policy },
    ScenarioCase { id: ScenarioId::StateTextBuffer, run: state_text_buffer },
    ScenarioCase { id: ScenarioId::StateTextCommands, run: state_text_commands },
    ScenarioCase { id: ScenarioId::StateUndoHistory, run: state_undo_history },
    ScenarioCase { id: ScenarioId::StateEditEntry, run: state_edit_entry },
    ScenarioCase { id: ScenarioId::StatePartRegions, run: state_part_regions },
];

fn plain(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

fn chord(code: KeyCode, mods: KeyModifiers) -> Key {
    Key { code, mods }
}

fn letter(c: char) -> Key {
    plain(KeyCode::Char(c))
}

fn names(prefix: &str, names: &[&str]) -> Vec<WidgetId> {
    names
        .iter()
        .map(|n| WidgetId::of(&format!("{prefix}.{n}")))
        .collect()
}

// ---------------------------------------------------------------------------
// Identity: WidgetId, ItemKey, Part, PartRef
// ---------------------------------------------------------------------------

fn state_identity() {
    let by_row = ItemKey::index(3);
    assert_eq!(by_row, ItemKey::Index(3));
    assert_eq!(by_row.as_index(), Some(3));
    let by_number = ItemKey::num(7);
    assert_eq!(by_number, ItemKey::Num(7));
    assert_eq!(by_number.as_index(), None, "only an index key is a row");
    let alpha = ItemKey::text("alpha");
    assert!(matches!(alpha, ItemKey::Text(_)));
    assert_eq!(alpha, ItemKey::text("alpha"));
    assert_ne!(alpha, ItemKey::text("beta"));
    assert_eq!(alpha.as_index(), None);
    assert_eq!(ItemKey::pair(1, 2), ItemKey::pair(1, 2));
    assert_ne!(ItemKey::pair(1, 2), ItemKey::pair(2, 1), "order matters");
    assert_ne!(ItemKey::pair(1, 2), ItemKey::pair(1, 3));

    let tag = Part::custom("scenario.tag");
    assert_eq!(tag, Part::custom("scenario.tag"));
    assert_ne!(tag, Part::custom("scenario.other"));
    assert!(
        tag.raw() & 0x8000 != 0,
        "custom parts live above the built-in range"
    );
    assert_eq!(tag.raw(), custom_hash16("scenario.tag"));
    assert_eq!(Part::ROW.raw(), 15, "a built-in part keeps its number");
    assert_eq!(Part::ROW.name(), Some("ROW"));
    assert_eq!(tag.name(), None);

    let whole = PartRef::of(Part::GUTTER);
    assert_eq!((whole.part, whole.item), (Part::GUTTER, None));
    let row2 = PartRef::item(Part::ROW, ItemKey::Index(2));
    assert_eq!((row2.part, row2.item), (Part::ROW, Some(ItemKey::Index(2))));
    assert_eq!(
        (PartRef::OVERFLOW_BEFORE.part, PartRef::OVERFLOW_BEFORE.item),
        (Part::OVERFLOW, Some(ItemKey::Index(0)))
    );
    assert_eq!(
        (PartRef::OVERFLOW_AFTER.part, PartRef::OVERFLOW_AFTER.item),
        (Part::OVERFLOW, Some(ItemKey::Index(1)))
    );

    let root = WidgetId::of("scenario.identity");
    assert_eq!(root, WidgetId::of("scenario.identity"));
    assert_ne!(root, WidgetId::of("scenario.identity.other"));
    assert_ne!(root.child(0), root.child(1));
    assert_eq!(root.child(4), root.child(4));
    assert_ne!(root.child(0), root.sub("0"));
    assert_eq!(root.sub("menu"), root.sub("menu"));
    assert_ne!(root.sub("menu"), root.sub("list"));
    assert_eq!(
        root.part(PartRef::of(Part::CONTAINER)),
        root,
        "the container part is the widget itself"
    );
    assert_ne!(root.part(whole), root);
    assert_ne!(
        root.part(PartRef::of(Part::GUTTER)),
        root.part(PartRef::of(Part::MARKER))
    );
    assert_eq!(
        root.part(PartRef::item(Part::ROW, ItemKey::Index(2))),
        root.child(2),
        "an indexed row is the widget's child"
    );
    assert_ne!(
        root.part(PartRef::item(Part::ROW, ItemKey::num(2))),
        root.part(PartRef::item(Part::ROW, ItemKey::Index(2)))
    );
    assert_ne!(
        root.part(PartRef::item(Part::MARKER, ItemKey::Index(2))),
        root.part(PartRef::item(Part::ROW, ItemKey::Index(2)))
    );

    // The same identity round-trips through a real hit registry: a click on
    // the third row resolves back to its index.
    let mut list = ListBox::new(
        root,
        ["one", "two", "three", "four"]
            .into_iter()
            .map(ListItem::new)
            .collect(),
        SelectMode::Single,
    );
    let painted = paint_hits(Rect::new(0, 0, 24, 5), |area, buf, ctx| {
        list.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert!(text_of(&painted.buf).contains("three"));
    let hits = painted.hits;
    let at = Position::new(3, 2);
    let region = hits
        .regions()
        .iter()
        .rev()
        .find(|r| r.owner == root && r.part.part == Part::ROW && r.area.contains(at))
        .expect("the third row registers a region");
    assert_eq!(root.item_of(region.id, Part::ROW, 0..4), Some(2));
    assert_eq!(root.item_of(region.id, Part::ROW, 0..2), None);
    assert_eq!(root.item_of(region.id, Part::MARKER, 0..4), None);
    assert_eq!(list.row_id(2), region.id);
}

// ---------------------------------------------------------------------------
// Focus: FocusRing, Focus, blur forwarding
// ---------------------------------------------------------------------------

fn state_focus_ring() {
    let ids = names("scenario.focus", &["a", "b", "c", "d"]);
    let mut ring = FocusRing::default();
    assert!(ring.is_empty());
    assert_eq!(ring.len(), 0);
    assert_eq!(
        (ring.first(), ring.next(None), ring.prev(None)),
        (None, None, None)
    );

    ring.register(ids[0]);
    ring.register(ids[1]);
    ring.push_barrier();
    ring.register(ids[3]);
    ring.register(ids[2]);
    assert!(!ring.is_empty());
    assert_eq!(ring.len(), 4, "the barrier hides, it does not remove");
    assert_eq!(ring.reachable(), &[ids[3], ids[2]]);
    assert!(
        !ring.contains(ids[0]),
        "a widget behind the barrier is unreachable"
    );
    assert!(ring.contains(ids[2]));

    let position = |id: &WidgetId| ids.iter().position(|x| x == id).expect("known id");
    ring.sort_from(2, position);
    assert_eq!(
        ring.reachable(),
        &[ids[2], ids[3]],
        "the group sorts in place"
    );
    assert_eq!(ring.first(), Some(ids[2]));
    assert_eq!(ring.next(Some(ids[2])), Some(ids[3]));
    assert_eq!(ring.next(Some(ids[3])), Some(ids[2]), "next wraps");
    assert_eq!(ring.prev(Some(ids[3])), Some(ids[2]));
    assert_eq!(ring.prev(Some(ids[2])), Some(ids[3]), "prev wraps");
    assert_eq!(ring.next(None), Some(ids[2]));
    assert_eq!(ring.prev(None), Some(ids[3]));
    assert_eq!(
        ring.next(Some(ids[0])),
        Some(ids[2]),
        "an unreachable current restarts at the first"
    );

    assert!(!ring.has_selection(ids[2]));
    ring.mark_selection(ids[2]);
    ring.mark_selection(ids[2]);
    assert!(ring.has_selection(ids[2]) && !ring.has_selection(ids[3]));

    // The same ring drives a real list: a focus stop registered while it paints.
    let mut live = FocusRing::default();
    let mut list = ListBox::new(ids[0], vec![ListItem::new("only")], SelectMode::Single);
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
    let mut hits = HitRegistry::default();
    {
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut live);
        list.render(buf.area, &mut buf, &mut ctx, theme.canvas);
    }
    assert!(live.contains(ids[0]) && live.len() == 1);
    assert!(text_of(&buf).contains("only"));
}

fn state_focus_blur() {
    let ids = names("scenario.focus", &["a", "b", "c", "d"]);
    let mut ring = FocusRing::default();
    for id in &ids {
        ring.register(*id);
    }

    let mut focus = Focus::default();
    assert_eq!((focus.current(), focus.moves()), (None, 0));
    focus.ensure_valid(&ring);
    assert!(focus.is(ids[0]), "an empty focus lands on the first widget");
    focus.next(&ring);
    assert!(focus.is(ids[1]) && !focus.is(ids[0]));
    focus.next(&ring);
    focus.prev(&ring);
    assert_eq!(focus.current(), Some(ids[1]));
    focus.set(Some(ids[3]));
    focus.focus(ids[2]);
    focus.set(None);
    assert_eq!(focus.current(), None);
    focus.focus(ids[1]);
    assert_eq!(focus.moves(), 8, "every move is counted");

    let changes = focus.take_changes();
    assert!(
        changes.contains(&FocusChange {
            from: ids[2],
            to: Some(ids[1])
        }),
        "a widget that lost focus is reported with where focus went"
    );
    assert!(focus.take_changes().is_empty(), "reported once");
    focus.end_edit();
    assert_eq!(
        focus.take_changes(),
        vec![FocusChange {
            from: ids[1],
            to: Some(ids[1])
        }],
        "ending an edit re-reports the focused widget"
    );

    request_focus_next(false);
    assert!(focus.apply_request(&ring), "a pending request moves focus");
    assert_eq!(focus.current(), Some(ids[2]));
    assert!(!focus.apply_request(&ring), "a request is taken once");
    request_focus_next(true);
    cancel_focus_request();
    assert!(!focus.apply_request(&ring), "a cancelled request is gone");
    request_focus_next(true);
    let settled = focus.settle(&ring);
    assert_eq!(focus.current(), Some(ids[1]));
    assert_eq!(
        settled,
        vec![FocusChange {
            from: ids[2],
            to: Some(ids[1])
        }]
    );

    // A widget leaves the ring: Nearest keeps position, First restarts.
    focus.focus(ids[2]);
    focus.ensure_valid_with(&ring, FocusFallback::Nearest);
    let mut shrunk = FocusRing::default();
    for id in [ids[0], ids[1], ids[3]] {
        shrunk.register(id);
    }
    let mut nearest = focus.clone();
    nearest.ensure_valid_with(&shrunk, FocusFallback::Nearest);
    assert_eq!(
        nearest.current(),
        Some(ids[1]),
        "nearest stays beside the gap"
    );
    let mut first = focus.clone();
    first.ensure_valid_with(&shrunk, FocusFallback::First);
    assert_eq!(first.current(), Some(ids[0]));
    assert_eq!(FocusFallback::default(), FocusFallback::Nearest);
    focus.ensure_valid(&shrunk);
    assert_eq!(
        focus.current(),
        Some(ids[0]),
        "ensure_valid restarts at the first"
    );

    // Blur policy through a real field: each action ends the edit differently.
    let outcomes: Vec<(BlurAction, Outcome, String, bool)> =
        [BlurAction::Commit, BlurAction::Cancel, BlurAction::Keep]
            .into_iter()
            .map(|action| {
                let id = WidgetId::of("scenario.blur");
                let mut input = TextInput::new(id, "Name")
                    .value("abc")
                    .edit_entry(EditEntry::Explicit)
                    .blur_action(action);
                let target: &dyn BlurTarget = &input;
                assert_eq!(target.blur_id(), id);
                input.begin_edit();
                input.on_key(&letter('x'));
                let change = FocusChange {
                    from: id,
                    to: Some(ids[0]),
                };
                let outcome = forward_blur(change, &mut [&mut input]);
                (action, outcome, input.text().to_owned(), input.editing)
            })
            .collect();
    assert_eq!(
        outcomes,
        vec![
            (
                BlurAction::Commit,
                Outcome::Changed,
                "abcx".to_owned(),
                false
            ),
            (
                BlurAction::Cancel,
                Outcome::Changed,
                "abc".to_owned(),
                false
            ),
            (BlurAction::Keep, Outcome::Ignored, "abcx".to_owned(), true),
        ]
    );
    assert_eq!(BlurAction::default(), BlurAction::Commit);
    let mut other = TextInput::new(WidgetId::of("scenario.other"), "Other").value("z");
    other.begin_edit();
    let stray = FocusChange {
        from: ids[0],
        to: None,
    };
    assert_eq!(
        forward_blur(stray, &mut [&mut other]),
        Outcome::Ignored,
        "a blur for another widget reaches nobody"
    );
    assert!(other.editing);
}

// ---------------------------------------------------------------------------
// Response and its handler results
// ---------------------------------------------------------------------------

fn state_response_flow() {
    assert_eq!(
        (Flow::default(), Invalidate::default()),
        (Flow::Ignored, Invalidate::None)
    );
    assert!(Flow::Ignored < Flow::Consumed);
    assert!(Invalidate::None < Invalidate::Paint && Invalidate::Paint < Invalidate::Layout);

    let ignored: Response<&str> = Response::ignored();
    assert_eq!(
        (ignored.flow(), ignored.invalidate()),
        (Flow::Ignored, Invalidate::None)
    );
    assert!(!ignored.is_consumed() && !ignored.is_changed());
    assert_eq!(ignored.outcome(), Outcome::Ignored);

    let consumed: Response<&str> = Response::consumed();
    assert_eq!(
        (consumed.flow(), consumed.invalidate()),
        (Flow::Consumed, Invalidate::None)
    );
    assert!(consumed.is_consumed() && !consumed.is_changed());
    assert_eq!(consumed.outcome(), Outcome::Consumed);

    let changed: Response<&str> = Response::changed();
    assert_eq!(
        (changed.flow(), changed.invalidate()),
        (Flow::Consumed, Invalidate::Paint)
    );
    assert!(changed.is_consumed() && changed.is_changed());
    assert_eq!(changed.outcome(), Outcome::Changed);

    let emitted = Response::emit("open");
    assert_eq!(emitted.invalidate(), Invalidate::Paint);
    assert_eq!(emitted.actions().copied().collect::<Vec<_>>(), vec!["open"]);

    let repainted = Response::<&str>::consumed().repaint();
    assert_eq!(repainted.invalidate(), Invalidate::Paint);
    assert_eq!(repainted.outcome(), Outcome::Changed);
    let ignored_but_dirty = Response::<&str>::ignored().repaint();
    assert_eq!(
        (ignored_but_dirty.flow(), ignored_but_dirty.invalidate()),
        (Flow::Ignored, Invalidate::Paint),
        "a repaint does not claim the event"
    );
    let relaid = Response::<&str>::changed().relayout();
    assert_eq!(relaid.invalidate(), Invalidate::Layout);
    assert_eq!(
        relaid.repaint().invalidate(),
        Invalidate::Layout,
        "a repaint never weakens a relayout"
    );
    let quiet = Response::emit("kept").no_repaint();
    assert_eq!(
        (quiet.invalidate(), quiet.outcome(), quiet.has_actions()),
        (Invalidate::None, Outcome::Consumed, true)
    );

    let (outcome, actions) = Response::emit("one").into_parts();
    assert_eq!((outcome, actions), (Outcome::Changed, Actions::One("one")));

    let erased: Response<()> = Response::emit("gone").erase();
    assert_eq!(
        (erased.flow(), erased.invalidate(), erased.has_actions()),
        (Flow::Consumed, Invalidate::Paint, false),
        "erasing keeps the verdict and drops the payload"
    );
    let lifted: Response<&str> = Response::<()>::changed().lift();
    assert_eq!(lifted.outcome(), Outcome::Changed);
    assert!(!lifted.has_actions());

    // A real widget reports the same shape: moving a table's cursor repaints,
    // an unbound key is not consumed, a check box click emits its action.
    let mut table = DataTable::try_new(
        WidgetId::of("scenario.response.table"),
        two_columns(),
        vec![
            cells(&["lint", "ana"]),
            cells(&["build", "kai"]),
            cells(&["ship", "mira"]),
        ],
    )
    .expect("rectangular rows")
    .selection_mode(SelectionMode::Multi)
    .selection_column(true);
    let moved = table.on_key(&plain(KeyCode::Down));
    assert!(moved.is_consumed() && moved.is_changed());
    assert_eq!(moved.invalidate(), Invalidate::Paint);
    let stray = table.on_key(&plain(KeyCode::F(9)));
    assert!(!stray.is_consumed());
    assert_eq!(stray.outcome(), Outcome::Ignored);
    let (outcome, action) = table.on_click_check(2);
    assert_eq!(
        (outcome, action),
        (Outcome::Changed, Some(TableAction::SelectionChanged))
    );
    let reply: Response<TableAction> = Response::from((outcome, action));
    assert_eq!(
        reply.into_parts(),
        (
            Outcome::Changed,
            Actions::One(TableAction::SelectionChanged)
        )
    );
}

// ---------------------------------------------------------------------------
// Pointer and caret shapes
// ---------------------------------------------------------------------------

fn state_pointer_shapes() {
    assert_eq!(PointerShape::default(), PointerShape::Default);
    for (shape, name) in [
        (PointerShape::Default, "default"),
        (PointerShape::Text, "text"),
        (PointerShape::Pointer, "pointer"),
        (PointerShape::ColResize, "col-resize"),
        (PointerShape::RowResize, "row-resize"),
    ] {
        assert_eq!(shape.css_name(), name);
        assert_eq!(shape.osc22(), format!("\x1b]22;{name}\x1b\\"));
    }
    assert_eq!(CaretShape::default(), CaretShape::Bar);
    assert_eq!(CaretShape::Bar.decscusr(), Some("\x1b[6 q"));
    assert_eq!(CaretShape::Block.decscusr(), Some("\x1b[2 q"));
    assert_eq!(CaretShape::Underline.decscusr(), Some("\x1b[4 q"));
    assert_eq!(
        CaretShape::Terminal.decscusr(),
        None,
        "the terminal's own caret sends nothing"
    );
    assert_eq!(CARET_RESET, "\x1b[0 q");

    let env = |pairs: &'static [(&'static str, &'static str)]| {
        ShapeSupport::from_env(move |key| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_owned())
        })
    };
    assert_eq!(ShapeSupport::NONE, ShapeSupport::default());
    assert_eq!(
        env(&[("TERM", "xterm-kitty")]),
        ShapeSupport {
            pointer: true,
            caret: true
        }
    );
    assert_eq!(
        env(&[("TERM", "xterm-kitty"), ("TMUX", "/tmp/tmux-1/default")]),
        ShapeSupport {
            pointer: false,
            caret: true
        },
        "a multiplexer swallows the pointer sequence"
    );
    assert!(!env(&[("TERM", "xterm-256color")]).pointer);
    assert!(env(&[("TERM", "xterm-256color")]).caret);
    assert_eq!(env(&[("TERM", "dumb")]), ShapeSupport::NONE);
    assert_eq!(ShapeSupport::current(), ShapeSupport::current());

    let seam = WidgetId::of("scenario.seam");
    let bar = WidgetId::of("scenario.bar");
    let mut hits = HitRegistry::default();
    hits.register(seam, Rect::new(0, 0, 1, 4));
    hits.register(bar, Rect::new(3, 0, 6, 1));
    hits.set_shape(seam, PointerShape::ColResize);
    hits.set_shape(bar, PointerShape::Text);
    let mut pointer = PointerState::default();
    assert_eq!(under(&pointer, &hits), PointerShape::Default);
    pointer.hover = Some(seam);
    assert_eq!(under(&pointer, &hits), PointerShape::ColResize);
    pointer.pressed = Some(bar);
    assert_eq!(
        under(&pointer, &hits),
        PointerShape::Text,
        "a held press keeps its own shape while the pointer roams"
    );
    pointer.pressed = None;
    pointer.config.pointer_shapes = true;
    pointer.config.caret_shape = CaretShape::Block;
    assert_eq!(
        shapes_for(&pointer, &hits),
        Shapes {
            pointer: Some(PointerShape::ColResize),
            caret: Some(CaretShape::Block)
        }
    );
    pointer.config.pointer_shapes = false;
    assert_eq!(shapes_for(&pointer, &hits).pointer, None);

    let want = Shapes {
        pointer: Some(PointerShape::ColResize),
        caret: Some(CaretShape::Block),
    };
    let mut sync = ShapeSync::new(ShapeSupport {
        pointer: true,
        caret: true,
    });
    let mut out: Vec<u8> = Vec::new();
    assert!(sync.apply(want, &mut out).expect("a Vec accepts writes"));
    assert_eq!(out, b"\x1b]22;col-resize\x1b\\\x1b[2 q");
    out.clear();
    assert!(!sync.apply(want, &mut out).expect("a Vec accepts writes"));
    assert!(out.is_empty(), "an unchanged shape is not re-sent");
    sync.forget();
    assert!(sync.apply(want, &mut out).expect("a Vec accepts writes"));
    assert_eq!(
        out, b"\x1b]22;col-resize\x1b\\\x1b[2 q",
        "forgetting makes the next frame resend"
    );
    let mut deaf = ShapeSync::new(ShapeSupport::NONE);
    let mut none: Vec<u8> = Vec::new();
    assert!(!deaf.apply(want, &mut none).expect("a Vec accepts writes"));
    assert!(none.is_empty(), "an unsupporting terminal gets nothing");
    let mut native = ShapeSync::new(ShapeSupport {
        pointer: false,
        caret: true,
    });
    let own = Shapes {
        pointer: None,
        caret: Some(CaretShape::Terminal),
    };
    assert!(!native.apply(own, &mut none).expect("a Vec accepts writes"));
    assert!(none.is_empty());

    assert_eq!(
        reset_sequence(),
        format!("{}{CARET_RESET}", PointerShape::Default.osc22()),
        "quitting undoes exactly what was sent"
    );
    assert_eq!(reset_sequence(), "", "the reset is reported once");
}

// ---------------------------------------------------------------------------
// List navigation: RowSource, Cursor, Moved, KeyStep, Navigable, step_index
// ---------------------------------------------------------------------------

struct Roster {
    names: Vec<&'static str>,
    off: Vec<usize>,
    cursor: Cursor,
}

impl RowSource for Roster {
    fn len(&self) -> usize {
        self.names.len()
    }
    fn enabled(&self, i: usize) -> bool {
        !self.off.contains(&i)
    }
    fn label(&self, i: usize) -> &str {
        self.names[i]
    }
}

impl Navigable for Roster {
    fn cursor(&self) -> Cursor {
        self.cursor
    }
}

fn first_usable<N: Navigable + ?Sized>(rows: &N) -> Moved {
    let mut cursor = rows.cursor();
    cursor.first(rows)
}

fn state_list_cursor() {
    assert_eq!(step_index(0, 3, -1, false), 0);
    assert_eq!(step_index(0, 3, -1, true), 2);
    assert_eq!(step_index(2, 3, 1, false), 2);
    assert_eq!(step_index(2, 3, 1, true), 0);
    assert_eq!(step_index(1, 5, 3, false), 4, "a long step clamps");
    assert_eq!(
        step_index(4, 0, 1, true),
        0,
        "an empty list has no position"
    );

    let labels = ["Apple", "Avocado", "Banana", "Blueberry", "Cherry"];
    let src = &labels[..];
    let mut cur = Cursor::new(0);
    assert_eq!(
        (cur.index, cur.wrap, cur.axis, cur.skip_disabled),
        (0, false, Axis::Vertical, true)
    );
    assert_eq!(Cursor::default(), Cursor::new(0));
    let moved = cur.step(1, src);
    assert_eq!(moved, Moved::To(1));
    assert!(moved.moved());
    assert_eq!(moved.index(), Some(1));
    assert_eq!(moved.outcome(), Outcome::Changed);
    assert_eq!(cur.step(-1, src), Moved::To(0));
    let stayed = cur.step(-1, src);
    assert_eq!(stayed, Moved::Stayed(0), "the top edge holds");
    assert!(!stayed.moved());
    assert_eq!(stayed.index(), Some(0));
    assert_eq!(stayed.outcome(), Outcome::Consumed);
    let empty: &[&str] = &[];
    let nothing = Cursor::new(0).step(1, empty);
    assert_eq!(nothing, Moved::Ignored);
    assert_eq!((nothing.index(), nothing.moved()), (None, false));
    assert_eq!(nothing.outcome(), Outcome::Ignored);

    let mut ring = Cursor::new(0).wrap(true);
    assert_eq!(
        ring.step(-1, src),
        Moved::To(4),
        "wrapping leaves the top edge"
    );
    assert_eq!(ring.step(1, src), Moved::To(0));

    // Disabled rows are skipped unless the cursor is told otherwise.
    let off = |i: usize| i != 1;
    let label = |i: usize| labels[i];
    let rows = RowFn {
        len: labels.len(),
        enabled: &off,
        label: &label,
    };
    assert_eq!(rows.len(), 5);
    assert!(!rows.is_empty() && !rows.enabled(1) && rows.enabled(2));
    assert_eq!(rows.label(2), "Banana");
    assert_eq!(
        rows.key(3),
        ItemKey::index(3),
        "a plain source keys by position"
    );
    assert_eq!(Cursor::new(0).step(1, &rows), Moved::To(2));
    assert_eq!(
        Cursor::new(0).skip_disabled(false).step(1, &rows),
        Moved::To(1)
    );
    assert_eq!(Cursor::new(3).step(-1, &rows), Moved::To(2));
    assert_eq!(Cursor::new(2).step(-1, &rows), Moved::To(0));

    // Key-shaped navigation: arrows, paging, ends, and the axis it listens to.
    let mut page = Cursor::new(0);
    assert_eq!(page.apply(Nav::PageDown, src, 2), Moved::To(2));
    assert_eq!(
        page.apply(Nav::PageDown, src, 9),
        Moved::To(4),
        "a page clamps"
    );
    assert_eq!(page.apply(Nav::PageUp, src, 3), Moved::To(1));
    assert_eq!(page.apply(Nav::Home, src, 3), Moved::To(0));
    assert_eq!(page.apply(Nav::End, src, 3), Moved::To(4));
    assert_eq!(page.apply(Nav::Up, src, 3), Moved::To(3));
    assert_eq!(page.apply(Nav::Left, src, 3), Moved::Ignored);
    let mut across = Cursor::new(1).axis(Axis::Horizontal);
    assert_eq!(across.axis, Axis::Horizontal);
    assert_eq!(across.apply(Nav::Right, src, 3), Moved::To(2));
    assert_eq!(across.apply(Nav::Left, src, 3), Moved::To(1));
    assert_eq!(across.apply(Nav::Down, src, 3), Moved::Ignored);
    assert_eq!(across.apply(Nav::PageDown, src, 3), Moved::Ignored);
    let mut gaps = Cursor::new(0);
    assert_eq!(
        gaps.apply(Nav::PageDown, &rows, 1),
        Moved::To(2),
        "a page landing on a disabled row slides to the next usable one"
    );

    // first/last/select.
    let skipping = RowFn {
        len: 5,
        enabled: &|i| i != 0 && i != 4,
        label: &label,
    };
    assert_eq!(Cursor::new(3).first(&skipping), Moved::To(1));
    assert_eq!(Cursor::new(1).last(&skipping), Moved::To(3));
    assert_eq!(Cursor::new(0).first(empty), Moved::Ignored);
    assert_eq!(Cursor::new(0).last(empty), Moved::Ignored);
    let mut pick = Cursor::new(0);
    assert_eq!(pick.select(3, src), Moved::To(3));
    assert_eq!(pick.index, 3);
    assert_eq!(
        pick.select(9, src),
        Moved::Ignored,
        "past the end selects nothing"
    );
    assert_eq!(pick.index, 3);

    // A custom source plugs in through the same traits.
    let roster = Roster {
        names: vec!["ana", "kai", "mira"],
        off: vec![0],
        cursor: Cursor::new(0),
    };
    let as_rows: &dyn RowSource = &roster;
    assert_eq!((as_rows.len(), as_rows.label(2)), (3, "mira"));
    assert!(!as_rows.enabled(0) && as_rows.enabled(1));
    let as_nav: &dyn Navigable = &roster;
    assert_eq!(as_nav.cursor().index, 0);
    assert_eq!(first_usable(&roster), Moved::To(1));
    assert_eq!(roster.key(2), ItemKey::index(2));
    assert!(!roster.is_empty());

    // Keys map to steps the same way for every list.
    let nav = NavKeys::DESKTOP;
    assert_eq!(
        KeyStep::of(&plain(KeyCode::Down), &nav, true),
        Some(KeyStep::Nav(Nav::Down))
    );
    assert_eq!(
        KeyStep::of(&letter('a'), &nav, true),
        Some(KeyStep::Type('a'))
    );
    assert_eq!(KeyStep::of(&letter('a'), &nav, false), None);
    assert_eq!(
        KeyStep::ordered(
            &letter('j'),
            &NavKeys::VIM,
            false,
            TypeAheadPrecedence::TypeAhead
        ),
        Some(KeyStep::Nav(Nav::Down)),
        "with type-ahead off a vim letter navigates"
    );

    // Two-pass routing: shortcuts see a letter first, type-ahead gets what
    // they leave.
    TypeAheadPrecedence::Shortcuts.install();
    assert_eq!(
        TypeAheadPrecedence::current(),
        TypeAheadPrecedence::Shortcuts
    );
    let a = letter('a');
    let mut passes = Vec::new();
    let out = route_key(&a, || {
        let step = KeyStep::ordered(&a, &nav, true, TypeAheadPrecedence::Shortcuts);
        passes.push(step);
        match step {
            Some(KeyStep::Type(_)) => Outcome::Changed,
            _ => Outcome::Ignored,
        }
    });
    assert_eq!(out, Outcome::Changed);
    assert_eq!(
        passes,
        vec![None, Some(KeyStep::Type('a'))],
        "type-ahead is tried only after the shortcuts pass declines"
    );
    let mut calls = 0;
    let claimed = route_key(&a, || {
        calls += 1;
        Outcome::Changed
    });
    assert_eq!(
        (claimed, calls),
        (Outcome::Changed, 1),
        "a claimed key stops there"
    );
    let mut direct = 0;
    let _ = route_key(&plain(KeyCode::Enter), || {
        direct += 1;
        Outcome::Ignored
    });
    assert_eq!(direct, 1, "a non-letter key is routed once");
    TypeAheadPrecedence::TypeAhead.install();
    assert_eq!(
        TypeAheadPrecedence::current(),
        TypeAheadPrecedence::TypeAhead
    );
    TypeAheadPrecedence::default().install();
    assert_eq!(
        TypeAheadPrecedence::current(),
        TypeAheadPrecedence::Shortcuts
    );

    // The same cursor logic runs inside a real list.
    let mut list = ListBox::new(
        WidgetId::of("scenario.cursor.list"),
        ["ana", "kai", "mira"].map(ListItem::new).to_vec(),
        SelectMode::Single,
    )
    .wrap(true);
    assert_eq!(list.on_key(&plain(KeyCode::Up)), Outcome::Changed);
    assert_eq!(
        list.core.cursor, 2,
        "wrapping up from the top reaches the last row"
    );
    assert_eq!(list.on_key(&plain(KeyCode::Home)), Outcome::Changed);
    assert_eq!(list.core.cursor, 0);
    let buf = paint(Rect::new(0, 0, 20, 4), |area, buf, ctx| {
        list.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert!(text_of(&buf).contains("mira"));
}

fn state_type_ahead() {
    let mut ta = TypeAhead::with_timeout(Duration::from_secs(5));
    assert_eq!(ta.timeout, Duration::from_secs(5));
    assert_eq!(TypeAhead::DEFAULT_TIMEOUT, Duration::from_millis(1000));
    assert_eq!(TypeAhead::default().timeout, TypeAhead::DEFAULT_TIMEOUT);

    assert_eq!(TypeAhead::char_of(&letter('a')), Some('a'));
    assert_eq!(TypeAhead::char_of(&letter('7')), Some('7'));
    assert_eq!(
        TypeAhead::char_of(&letter(' ')),
        None,
        "a space is not a letter"
    );
    assert_eq!(
        TypeAhead::char_of(&chord(KeyCode::Char('a'), KeyModifiers::CONTROL)),
        None,
        "a chord is a shortcut, not text"
    );
    assert_eq!(TypeAhead::char_of(&plain(KeyCode::Down)), None);

    let mut slow = TypeAhead::default();
    let t0 = Instant::now();
    assert_eq!(slow.feed_at('A', t0), "a", "the buffer folds case");
    assert_eq!(slow.feed_at('v', t0 + Duration::from_millis(500)), "av");
    assert_eq!(
        slow.feed_at('x', t0 + Duration::from_secs(3)),
        "x",
        "a pause longer than the timeout starts a new word"
    );
    slow.reset();
    assert_eq!(slow.feed('q'), "q");
    assert_eq!(slow.feed('z'), "qz", "a quick follow-up joins the word");
    slow.reset();
    assert_eq!(slow.feed('w'), "w", "reset forgets the typed word");

    let labels = ["Apple", "Avocado", "Banana", "Blueberry", "Cherry"];
    let all = labels.iter().copied();
    assert_eq!(TypeAhead::find("b", all.clone(), 0, |_| true), Some(2));
    assert_eq!(TypeAhead::find("bl", all.clone(), 0, |_| true), Some(3));
    assert_eq!(
        TypeAhead::find("B", all.clone(), 0, |_| true),
        None,
        "the prefix is already folded"
    );
    assert_eq!(TypeAhead::find("", all.clone(), 0, |_| true), None);
    assert_eq!(TypeAhead::find("z", all.clone(), 0, |_| true), None);
    assert_eq!(
        TypeAhead::find("b", all.clone(), 0, |i| i != 2),
        Some(3),
        "an ineligible row is skipped"
    );
    assert_eq!(
        TypeAhead::find("aa", all.clone(), 0, |_| true),
        Some(1),
        "a repeated letter cycles through the rows that start with it"
    );
    assert_eq!(
        TypeAhead::find("a", all.clone(), 4, |_| true),
        Some(0),
        "the search wraps"
    );

    // The cursor feeds it and lands on the match.
    let src = &labels[..];
    let mut cur = Cursor::new(0);
    assert_eq!(cur.type_ahead('b', &mut ta, src), Moved::To(2));
    assert_eq!(cur.type_ahead('l', &mut ta, src), Moved::To(3));
    assert_eq!(cur.type_ahead('z', &mut ta, src), Moved::Stayed(3));
    ta.reset();
    assert_eq!(cur.type_ahead('c', &mut ta, src), Moved::To(4));
    assert_eq!(
        Cursor::new(0).type_ahead('a', &mut ta, &labels[..0]),
        Moved::Ignored
    );

    // And a real list does the same from key presses.
    let mut list = ListBox::new(
        WidgetId::of("scenario.typeahead"),
        labels.map(ListItem::new).to_vec(),
        SelectMode::Single,
    )
    .type_ahead_timeout(Duration::from_secs(5));
    assert_eq!(list.on_key(&letter('b')), Outcome::Changed);
    assert_eq!(list.core.cursor, 2);
    assert_eq!(list.on_key(&letter('l')), Outcome::Changed);
    assert_eq!(list.core.cursor, 3);
    assert_eq!(list.core.typeahead.timeout, Duration::from_secs(5));
}

fn state_nav_profile() {
    assert_eq!(NavProfile::default(), NavProfile::Desktop);
    assert_eq!(NavProfile::Desktop.nav(), NavKeys::DESKTOP);
    assert_eq!(NavProfile::Vim.nav(), NavKeys::VIM);
    assert!(NavProfile::Desktop.type_ahead());
    assert!(
        !NavProfile::Vim.type_ahead(),
        "vim letters and type-ahead cannot share keys"
    );
    assert_eq!(NavProfile::current(), NavProfile::Desktop);

    let items = || ["alpha", "bravo", "charlie"].map(ListItem::new).to_vec();
    let mut desktop = ListBox::new(
        WidgetId::of("scenario.profile.a"),
        items(),
        SelectMode::Single,
    );
    assert_eq!(
        desktop.on_key(&letter('j')),
        Outcome::Consumed,
        "under the desktop profile `j` types ahead and finds nothing"
    );
    assert_eq!(desktop.core.cursor, 0);

    NavProfile::Vim.install();
    assert_eq!(NavProfile::current(), NavProfile::Vim);
    assert_eq!(
        NavKeys::default(),
        NavKeys::VIM,
        "new lists follow the installed profile"
    );
    let mut vim = ListBox::new(
        WidgetId::of("scenario.profile.b"),
        items(),
        SelectMode::Single,
    );
    assert_eq!(vim.on_key(&letter('j')), Outcome::Changed);
    assert_eq!(vim.core.cursor, 1, "`j` steps down");
    assert_eq!(vim.on_key(&letter('k')), Outcome::Changed);
    assert_eq!(vim.core.cursor, 0, "`k` steps back up");
    NavProfile::Desktop.install();
    assert_eq!(NavProfile::current(), NavProfile::Desktop);
    assert_eq!(NavKeys::default(), NavKeys::DESKTOP);
}

// ---------------------------------------------------------------------------
// Row selection and click policy
// ---------------------------------------------------------------------------

fn state_row_selection() {
    assert_eq!(SelectionMode::default(), SelectionMode::Single);

    let all = |_: usize| true;
    let order: Vec<usize> = (0..6).collect();
    let mut sel = RowSelection::new();
    assert!(sel.is_empty() && sel.anchor().is_none());
    assert_eq!(sel.len(), 0);
    assert!(sel.select_only(2, all));
    assert_eq!((sel.to_vec(), sel.anchor()), (vec![2], Some(2)));
    assert!(
        !sel.select_only(2, all),
        "selecting the same row changes nothing"
    );
    assert!(sel.toggle(4, all));
    assert_eq!(sel.to_vec(), vec![2, 4]);
    assert_eq!(sel.anchor(), Some(4), "a toggle moves the range anchor");
    assert!(sel.toggle(4, all));
    assert!(!sel.contains(4) && sel.contains(2));
    assert!(
        !sel.toggle(5, |r| r != 5),
        "a disabled row cannot be toggled"
    );
    assert!(
        !sel.select_only(2, all),
        "re-selecting the lone row only re-anchors"
    );
    assert_eq!(sel.anchor(), Some(2));
    assert!(sel.extend(&order, 2, 5, all));
    assert_eq!(
        sel.to_vec(),
        vec![2, 3, 4, 5],
        "shift extends from the anchor"
    );
    assert!(sel.extend(&order, 2, 3, all));
    assert_eq!(sel.to_vec(), vec![2, 3], "extending back shrinks the range");
    assert!(sel.extend(&order, 2, 0, |r| r != 1));
    assert_eq!(
        sel.to_vec(),
        vec![0, 2],
        "a disabled row stays out of the range"
    );
    assert!(!sel.extend(&[], 0, 0, all));
    assert_eq!(sel.iter().collect::<Vec<_>>(), vec![0, 2]);
    assert_eq!(sel.len(), 2);

    assert_eq!(sel.tri_state(0..6, all), TriState::Mixed);
    assert!(sel.select_all(0..6, all));
    assert_eq!(sel.tri_state(0..6, all), TriState::All);
    assert!(
        sel.toggle_all(0..6, all),
        "toggling a full selection clears it"
    );
    assert_eq!(sel.tri_state(0..6, all), TriState::None);
    assert!(sel.is_empty());
    assert!(
        sel.toggle_all(0..6, all),
        "toggling an empty selection selects all"
    );
    assert_eq!(sel.len(), 6);
    assert!(sel.clear());
    assert!(!sel.clear());
    assert_eq!(sel.anchor(), None);
    assert!(sel.set(3, true));
    assert!(!sel.set(3, true));
    assert!(sel.set(3, false));
    assert!(!sel.set(3, false));
    sel.replace([1, 2, 3, 5], Some(2));
    sel.retain(|r| r != 2);
    assert_eq!(sel.to_vec(), vec![1, 3, 5]);
    assert_eq!(
        sel.anchor(),
        None,
        "dropping the anchor row drops the anchor"
    );
    sel.remap(|r| r.checked_sub(1));
    assert_eq!(sel.to_vec(), vec![0, 2, 4], "rows shift when the data does");
    sel.remap(|r| (r != 0).then_some(r));
    assert_eq!(sel.to_vec(), vec![2, 4]);

    // Gestures: from key and from modifiers, applied to the selection.
    let mods = SelectGesture::toggle_mods_for(ClientPlatform::MacOs);
    assert_eq!(mods, SelectGesture::TOGGLE_OPTION_CMD);
    assert_eq!(
        SelectGesture::toggle_mods_for(ClientPlatform::Other),
        SelectGesture::TOGGLE_CTRL_CMD
    );
    assert!(SelectGesture::TOGGLE_WITH_ALT.contains(SelectGesture::TOGGLE_CTRL_CMD));
    assert!(SelectGesture::TOGGLE_WITH_ALT.contains(KeyModifiers::ALT));
    assert_eq!(
        SelectGesture::toggle_mods(),
        SelectGesture::toggle_mods_for(ClientPlatform::current())
    );
    let toggle = SelectGesture::TOGGLE_CTRL_CMD;
    assert_eq!(
        SelectGesture::from_mods(KeyModifiers::SHIFT, toggle, 3),
        SelectGesture::Extend { from: 3 }
    );
    assert_eq!(
        SelectGesture::from_mods(KeyModifiers::CONTROL, toggle, 3),
        SelectGesture::Toggle
    );
    assert_eq!(
        SelectGesture::from_mods(KeyModifiers::NONE, toggle, 3),
        SelectGesture::Only
    );
    assert_eq!(
        SelectGesture::from_key(&letter(' '), 1),
        Some(SelectGesture::Toggle)
    );
    assert_eq!(
        SelectGesture::from_key(&chord(KeyCode::Down, KeyModifiers::SHIFT), 1),
        Some(SelectGesture::Extend { from: 1 })
    );
    assert_eq!(SelectGesture::from_key(&plain(KeyCode::Down), 1), None);
    assert_eq!(SelectGesture::from_key(&letter('x'), 1), None);
    assert_eq!(
        SelectGesture::from_key(
            &chord(KeyCode::Down, KeyModifiers::CONTROL | KeyModifiers::SHIFT),
            1
        ),
        None,
        "a chorded shift-arrow is not a selection gesture"
    );

    let mut rows = RowSelection::new();
    assert!(rows.apply(SelectGesture::Only, 1, &order, all));
    assert_eq!(rows.to_vec(), vec![1]);
    assert!(rows.apply(SelectGesture::Toggle, 3, &order, all));
    assert_eq!(rows.to_vec(), vec![1, 3]);
    assert!(rows.apply(SelectGesture::Extend { from: 3 }, 5, &order, all));
    assert_eq!(rows.to_vec(), vec![1, 3, 4, 5]);
    assert!(rows.apply(SelectGesture::All, 0, &order, all));
    assert_eq!(rows.len(), 6);
    assert!(
        !rows.apply(SelectGesture::Only, 9, &order, all),
        "a row past the order is ignored"
    );
    assert!(
        !rows.context_select(2, &order, all),
        "a right-click on a selected row keeps the selection"
    );
    assert!(rows.clear());
    assert!(
        rows.context_select(2, &order, all),
        "a right-click elsewhere selects that row"
    );
    assert_eq!(rows.to_vec(), vec![2]);
    assert!(!rows.context_select(9, &order, all));

    // Through a real table: the header box reports the tri-state and the
    // click policy decides what a row click does.
    let mut table = DataTable::try_new(
        WidgetId::of("scenario.selection.table"),
        two_columns(),
        vec![
            cells(&["lint", "ana"]),
            cells(&["build", "kai"]),
            cells(&["ship", "mira"]),
        ],
    )
    .expect("rectangular rows")
    .selection_mode(SelectionMode::Multi)
    .selection_column(true);
    assert_eq!(table.selection_state(), TriState::None);
    let (outcome, action) = table.on_click_check(0);
    assert_eq!(
        (outcome, action),
        (Outcome::Changed, Some(TableAction::SelectionChanged))
    );
    assert_eq!(table.selection_state(), TriState::Mixed);
    assert_eq!(table.selected_rows(), vec![0]);
    let (_, action) = table.on_click_check_all();
    assert_eq!(action, Some(TableAction::SelectionChanged));
    assert_eq!(table.selection_state(), TriState::All);
    let buf = paint(Rect::new(0, 0, 40, 6), |area, buf, ctx| {
        table.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert!(
        text_of(&buf).contains(TriState::All.glyph()),
        "a full selection paints the checked box:\n{}",
        text_of(&buf)
    );
    table.on_click_check_all();
    assert_eq!(table.selection_state(), TriState::None);
    let mut single = DataTable::try_new(
        WidgetId::of("scenario.selection.single"),
        two_columns(),
        vec![cells(&["lint", "ana"])],
    )
    .expect("rectangular rows")
    .selection_mode(SelectionMode::Single);
    assert_eq!(
        single.on_click_check(0),
        (Outcome::Consumed, None),
        "a single-select table has no check boxes"
    );
    let none = DataTable::try_new(
        WidgetId::of("scenario.selection.none"),
        two_columns(),
        vec![cells(&["lint", "ana"])],
    )
    .expect("rectangular rows")
    .selection_mode(SelectionMode::None);
    assert!(none.selected_rows().is_empty());
    // The same stored row counts as selected in single mode and not at all
    // when selection is off.
    single.selected = Some(0);
    assert_eq!(single.selected_rows(), vec![0]);
    let mut none = none;
    none.selected = Some(0);
    assert!(
        none.selected_rows().is_empty(),
        "a table with selection off reports no selected row"
    );

    assert_eq!(TriState::None.glyph(), Glyphs::UNICODE.check_off);
    assert_eq!(TriState::Mixed.glyph(), Glyphs::UNICODE.check_mixed);
    assert_eq!(TriState::All.glyph(), Glyphs::UNICODE.check_on);
    assert_eq!(
        TriState::All.glyph_in(&Glyphs::ASCII),
        Glyphs::ASCII.check_on
    );
    assert_eq!(
        TriState::Mixed.glyph_in(&Glyphs::ASCII),
        Glyphs::ASCII.check_mixed
    );
    assert_eq!(
        TriState::None.glyph_in(&Glyphs::ASCII),
        Glyphs::ASCII.check_off
    );
}

fn state_click_policy() {
    assert_eq!(Activate::default(), Activate::Double);
    assert_eq!(CheckClick::default(), CheckClick::BoxOnly);
    assert!(Activate::Single.fires(1, false));
    assert!(!Activate::Double.fires(1, true) && Activate::Double.fires(2, false));
    assert!(!Activate::ClickAgain.fires(2, false) && Activate::ClickAgain.fires(1, true));
    assert!(!Activate::Never.fires(9, true));

    let data = ClickPolicy::DATA;
    assert_eq!(
        (data.activate, data.edit_on, data.check),
        (Activate::Double, Activate::Double, CheckClick::BoxOnly)
    );
    assert_eq!(ClickPolicy::default(), data);
    let nav = ClickPolicy::NAVIGATION;
    assert_eq!(
        (nav.activate, nav.edit_on, nav.check),
        (Activate::Single, Activate::Never, CheckClick::BoxOnly)
    );
    let custom = ClickPolicy::DATA
        .activate(Activate::ClickAgain)
        .edit_on(Activate::Single)
        .check(CheckClick::Row);
    assert_eq!(
        (custom.activate, custom.edit_on, custom.check),
        (Activate::ClickAgain, Activate::Single, CheckClick::Row)
    );
    assert!(data.activates(2, false) && !data.activates(1, false));
    assert!(nav.activates(1, false));
    assert!(custom.activates(1, true) && !custom.activates(1, false));
    assert!(custom.edits(1, false) && !nav.edits(2, true));
    assert!(data.edits(2, false));
    assert!(
        !data.toggles(false) && data.toggles(true),
        "the box toggles, the row does not"
    );
    assert!(custom.toggles(false), "a row-click policy toggles anywhere");

    // The policy decides what a click does in a real list.
    let items = || ["ana", "kai", "mira"].map(ListItem::new).to_vec();
    let mut navigation = ListBox::new(
        WidgetId::of("scenario.click.a"),
        items(),
        SelectMode::Single,
    )
    .click(ClickPolicy::NAVIGATION);
    navigation.on_click_count(1, 1);
    assert_eq!(
        navigation.chosen,
        Some(1),
        "one click opens a navigation row"
    );
    let mut data_list = ListBox::new(
        WidgetId::of("scenario.click.b"),
        items(),
        SelectMode::Single,
    )
    .click(ClickPolicy::DATA);
    data_list.on_click_count(1, 1);
    assert_eq!(data_list.chosen, None, "a data row needs a double click");
    data_list.on_click_count(1, 2);
    assert_eq!(data_list.chosen, Some(1));
    let mut again = ListBox::new(
        WidgetId::of("scenario.click.c"),
        items(),
        SelectMode::Single,
    )
    .click(ClickPolicy::DATA.activate(Activate::ClickAgain));
    again.on_click_count(2, 1);
    assert_eq!(again.chosen, None, "the first click only moves the cursor");
    again.on_click_count(2, 1);
    assert_eq!(
        again.chosen,
        Some(2),
        "the second click on the same row activates"
    );
    let mut never = ListBox::new(
        WidgetId::of("scenario.click.d"),
        items(),
        SelectMode::Single,
    )
    .click(ClickPolicy::DATA.activate(Activate::Never));
    never.on_click_count(1, 3);
    assert_eq!(never.chosen, None);
    let mut multi = ListBox::new(WidgetId::of("scenario.click.e"), items(), SelectMode::Multi)
        .click(ClickPolicy::DATA.check(CheckClick::Row));
    multi.on_click_count(0, 1);
    assert!(
        multi.is_checked(0),
        "a row-click policy checks on a plain click"
    );
    let mut box_only = ListBox::new(WidgetId::of("scenario.click.f"), items(), SelectMode::Multi)
        .click(ClickPolicy::DATA.check(CheckClick::BoxOnly));
    box_only.on_click_count(0, 1);
    assert!(box_only.is_checked(0), "a plain click selects the row");
    assert_eq!(box_only.checked_count(), 1);
    box_only.on_click_count(1, 1);
    assert!(
        !box_only.is_checked(0),
        "a plain click on another row replaces the selection"
    );
    box_only.on_click_at(2, 1, KeyModifiers::NONE, true);
    assert!(
        box_only.is_checked(1) && box_only.is_checked(2),
        "the box toggles without replacing"
    );
}

// ---------------------------------------------------------------------------
// Text: TextBuffer, commands, history, entry policy
// ---------------------------------------------------------------------------

fn state_text_buffer() {
    let mut line = TextBuffer::single("hello world");
    assert_eq!(line.text(), "hello world");
    assert!(!line.is_empty() && line.width() == 11);
    assert_eq!(line.line_count(), 1);
    assert_eq!(
        line.cursor_offset(),
        11,
        "a new buffer parks the caret at the end"
    );
    line.move_home(false);
    assert_eq!(line.cursor_offset(), 0);
    line.move_end(false);
    assert_eq!(line.cursor_offset(), 11);
    line.move_left(false);
    assert_eq!(line.cursor_offset(), 10);
    line.move_right(false);
    assert_eq!(line.cursor_offset(), 11);
    line.move_word_left(false);
    assert_eq!(line.cursor_offset(), 6);
    line.move_word_left(false);
    assert_eq!(line.cursor_offset(), 0);
    line.move_word_right(false);
    assert_eq!(line.cursor_offset(), 5);
    line.move_word_right(true);
    assert_eq!(line.selection(), Some(5..11));
    assert_eq!(line.selected_text(), Some(" world"));
    assert!(line.has_selection());
    line.move_left(false);
    assert_eq!(
        (line.has_selection(), line.cursor_offset()),
        (false, 5),
        "an arrow collapses a selection to its edge"
    );
    line.select_all();
    assert_eq!(line.selected_text(), Some("hello world"));
    line.select_to(3);
    assert_eq!(
        line.selected_text(),
        Some("hel"),
        "select_to keeps the anchor"
    );
    line.clear_selection();
    assert!(!line.has_selection());
    line.select_range(6, 11);
    assert_eq!(line.selected_text(), Some("world"));
    line.select_word_at(1);
    assert_eq!(line.selected_text(), Some("hello"));
    assert_eq!(line.word_range_at(7), 6..11);
    assert_eq!(
        line.word_range_at(5),
        5..6,
        "a run of spaces is its own word"
    );
    line.select_line_at(2);
    assert_eq!(line.selected_text(), Some("hello world"));
    line.move_doc_end(false);
    assert_eq!(line.cursor_offset(), 11);
    line.move_doc_start(true);
    assert_eq!(line.selected_text(), Some("hello world"));
    line.move_doc_start(false);
    assert_eq!(line.cursor_offset(), 0);
    assert!(
        !line.move_up(false) && !line.move_down(false),
        "a single line has no rows to step to"
    );

    let mut edit = TextBuffer::single("abc");
    edit.insert_char('d');
    edit.insert_str("ef");
    assert_eq!(edit.text(), "abcdef");
    edit.backspace();
    assert_eq!(edit.text(), "abcde");
    edit.move_home(false);
    edit.delete();
    assert_eq!(edit.text(), "bcde");
    edit.insert_char('\n');
    assert_eq!(edit.text(), "bcde", "a single line refuses a newline");
    edit.insert_str("x\ny");
    assert_eq!(edit.text(), "xybcde", "pasted newlines are dropped");
    edit.select_range(0, 2);
    edit.insert_str("Z");
    assert_eq!(edit.text(), "Zbcde", "typing replaces the selection");
    edit.select_range(0, 1);
    edit.backspace();
    assert_eq!(edit.text(), "bcde", "backspace deletes the selection first");
    edit.set_text("one two three");
    assert_eq!((edit.text(), edit.cursor_offset()), ("one two three", 13));
    edit.delete_word_left();
    assert_eq!(edit.text(), "one two ");
    edit.set_text("one two three");
    edit.set_cursor_line_col(0, 3);
    edit.delete_to_line_end();
    assert_eq!(edit.text(), "one");
    edit.set_text("one two three");
    edit.set_cursor_line_col(0, 4);
    edit.delete_to_line_start();
    assert_eq!((edit.text(), edit.cursor_offset()), ("two three", 0));
    edit.insert_at(3, "-");
    assert_eq!(edit.text(), "two- three");
    edit.remove_range(3..4);
    assert_eq!(edit.text(), "two three");
    edit.remove_range(40..50);
    assert_eq!(
        edit.text(),
        "two three",
        "a range past the end removes nothing"
    );
    let mut spliced = TextBuffer::single("ad");
    spliced.insert_at(1, "bc");
    assert_eq!(
        (spliced.text(), spliced.cursor_offset()),
        ("abcd", 4),
        "an insert before the caret pushes it along"
    );
    spliced.remove_range(1..3);
    assert_eq!((spliced.text(), spliced.cursor_offset()), ("ad", 2));
    let before = edit.revision();
    edit.set_text("two three");
    assert_eq!(edit.revision(), before, "the same text is not a change");
    edit.set_text("a\nb");
    assert_eq!(edit.text(), "ab", "a single-line buffer folds pasted lines");
    let blank = TextBuffer::single("");
    assert!(blank.is_empty() && blank.width() == 0);
    assert_eq!(
        TextBuffer::single("日本").width(),
        4,
        "width counts terminal cells, not bytes or characters"
    );

    let mut doc = TextBuffer::multi("one\ntwo\nthree");
    assert_eq!(doc.line_count(), 3);
    assert_eq!(doc.cursor_pos(), CursorPos { line: 2, col: 5 });
    doc.set_cursor_line_col(1, 1);
    assert_eq!(doc.cursor_pos(), CursorPos { line: 1, col: 1 });
    assert_eq!(doc.cursor_offset(), 5);
    assert!(doc.move_up(false));
    assert_eq!(doc.cursor_pos(), CursorPos { line: 0, col: 1 });
    assert!(!doc.move_up(false), "the first line has nothing above");
    assert!(doc.move_down(false) && doc.move_down(false));
    assert_eq!(doc.cursor_pos().line, 2);
    assert!(!doc.move_down(false), "the last line has nothing below");
    assert_eq!(doc.offset_at(2, 0), 8);
    assert_eq!(
        TextBuffer::pos_of("one\ntwo", 5),
        CursorPos { line: 1, col: 1 }
    );
    doc.select_range(1, 6);
    assert_eq!(doc.selection_lines(), (0, 1));
    assert!(doc.has_selection_lines());
    doc.select_range(4, 6);
    assert_eq!(doc.selection_lines(), (1, 1));
    assert!(
        !doc.has_selection_lines(),
        "a selection inside one line spans no lines"
    );
    doc.clear_selection();
    doc.set_cursor_line_col(2, 0);
    assert_eq!(
        doc.selection_lines(),
        (2, 2),
        "no selection means the caret's line"
    );
    doc.insert_char('\n');
    assert_eq!(
        doc.line_count(),
        4,
        "a multi-line buffer splits on a newline"
    );
    doc.select_range(0, 3);
    doc.insert_str("1");
    assert_eq!(doc.text(), "1\ntwo\n\nthree");
    doc.select_line_at(3);
    assert_eq!(doc.selected_text(), Some("two"));
    doc.clear_selection();
    doc.move_down(true);
    assert!(
        doc.has_selection(),
        "shift-down grows a selection across lines"
    );

    // The same buffer behind a real field: typing and moving through keys
    // and the painted row agree with it.
    let mut input = TextInput::new(WidgetId::of("scenario.buffer.input"), "Name")
        .value("hello world")
        .edit_entry(EditEntry::Explicit);
    input.begin_edit();
    input.on_key(&chord(KeyCode::Left, KeyModifiers::ALT));
    input.on_key(&letter('!'));
    assert_eq!(input.field.buffer.text(), "hello !world");
    let buf = paint(Rect::new(0, 0, 40, 3), |area, buf, ctx| {
        input.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert!(text_of(&buf).contains("hello !world"));
}

fn state_text_commands() {
    // The clipboard: a host hook sees what the app copies, and only its owner
    // can take the hook away.
    let seen: Rc<RefCell<Vec<String>>> = Rc::default();
    let log = Rc::clone(&seen);
    clipboard::set_hook(7, move |text| log.borrow_mut().push(text.to_owned()));
    clipboard::copy("hello");
    assert_eq!(clipboard::text().as_deref(), Some("hello"));
    clipboard::clear_hook(8);
    clipboard::copy("again");
    clipboard::clear_hook(7);
    clipboard::copy("silent");
    assert_eq!(*seen.borrow(), vec!["hello".to_owned(), "again".to_owned()]);
    assert_eq!(clipboard::text().as_deref(), Some("silent"));

    // Every command through a real field.
    let mut field = EditField::single("hello world");
    field.run(TextCmd::SelectAll);
    assert_eq!(field.buffer.selected_text(), Some("hello world"));
    field.run(TextCmd::Copy);
    assert_eq!(clipboard::text().as_deref(), Some("hello world"));
    field.run(TextCmd::Cut);
    assert_eq!(field.text(), "");
    assert!(field.can_undo(), "a cut is undoable");
    field.run(TextCmd::Paste);
    assert_eq!(field.text(), "hello world");
    field.run(TextCmd::Undo);
    assert_eq!(field.text(), "");
    assert!(field.can_redo());
    field.run(TextCmd::Redo);
    assert_eq!(field.text(), "hello world");
    field.run(TextCmd::LineStart);
    assert_eq!(field.buffer.cursor_offset(), 0);
    field.run(TextCmd::LineEnd);
    assert_eq!(field.buffer.cursor_offset(), 11);
    field.run(TextCmd::SelectToLineStart);
    assert_eq!(field.buffer.selected_text(), Some("hello world"));
    field.run(TextCmd::LineEnd);
    field.run(TextCmd::DocStart);
    assert_eq!(
        field.buffer.cursor_offset(),
        11,
        "a single-line field has no document edges"
    );
    field.run(TextCmd::LineStart);
    field.run(TextCmd::SelectToLineEnd);
    assert_eq!(field.buffer.selected_text(), Some("hello world"));
    field.run(TextCmd::SelectToDocEnd);
    field.run(TextCmd::DocEnd);
    field.run(TextCmd::SelectToDocStart);
    assert_eq!(
        (field.buffer.cursor_offset(), field.buffer.selected_text()),
        (11, Some("hello world")),
        "the document commands leave a single line untouched"
    );

    let mut doc = EditField::multi("one\ntwo\nthree");
    doc.run(TextCmd::DocStart);
    assert_eq!(doc.buffer.cursor_offset(), 0);
    doc.run(TextCmd::DocEnd);
    assert_eq!(doc.buffer.cursor_offset(), 13);
    doc.run(TextCmd::SelectToDocStart);
    assert_eq!(doc.buffer.selected_text(), Some("one\ntwo\nthree"));
    doc.run(TextCmd::DocStart);
    doc.run(TextCmd::SelectToDocEnd);
    assert_eq!(doc.buffer.selected_text(), Some("one\ntwo\nthree"));
    doc.run(TextCmd::DocStart);
    doc.run(TextCmd::LineEnd);
    assert_eq!(doc.buffer.cursor_pos(), CursorPos { line: 0, col: 3 });
    doc.run(TextCmd::LineStart);
    assert_eq!(doc.buffer.cursor_pos().col, 0);

    // Key maps: the platform picks the family, the command keys sit on top.
    assert_eq!(
        FieldKeys::for_platform(ClientPlatform::MacOs),
        FieldKeys::Emacs
    );
    assert_eq!(
        FieldKeys::for_platform(ClientPlatform::Other),
        FieldKeys::Desktop
    );
    assert_eq!(CMD_TEXT_KEYS.len(), 15);
    assert!(CMD_TEXT_KEYS.contains(&(KeyBinding::cmd('c'), TextCmd::Copy)));
    let ctrl = |c: char| chord(KeyCode::Char(c), KeyModifiers::CONTROL);
    let cmd = |c: char| chord(KeyCode::Char(c), KeyModifiers::SUPER);
    let desktop = FieldKeys::Desktop.text_keymap();
    assert_eq!(lookup(&desktop, &ctrl('a')), Some(TextCmd::SelectAll));
    assert_eq!(lookup(&desktop, &ctrl('c')), Some(TextCmd::Copy));
    assert_eq!(lookup(&desktop, &ctrl('v')), Some(TextCmd::Paste));
    assert_eq!(lookup(&desktop, &ctrl('y')), Some(TextCmd::Redo));
    assert_eq!(lookup(&desktop, &ctrl('z')), Some(TextCmd::Undo));
    assert_eq!(
        lookup(&desktop, &cmd('c')),
        Some(TextCmd::Copy),
        "the command key works on every family"
    );
    assert_eq!(
        lookup(&desktop, &chord(KeyCode::Down, KeyModifiers::SUPER)),
        Some(TextCmd::DocEnd)
    );
    let emacs = FieldKeys::Emacs.text_keymap();
    assert_eq!(lookup(&emacs, &ctrl('l')), Some(TextCmd::SelectAll));
    assert_eq!(
        lookup(&emacs, &ctrl('y')),
        Some(TextCmd::Paste),
        "emacs yanks"
    );
    assert_eq!(
        lookup(&emacs, &ctrl('a')),
        None,
        "ctrl-a stays line start under emacs"
    );
    assert_eq!(
        lookup(
            &emacs,
            &chord(KeyCode::Left, KeyModifiers::SUPER | KeyModifiers::SHIFT)
        ),
        Some(TextCmd::SelectToLineStart)
    );
    assert_eq!(
        lookup(&emacs, &chord(KeyCode::Right, KeyModifiers::SUPER)),
        Some(TextCmd::LineEnd)
    );
    assert_eq!(
        lookup(&emacs, &chord(KeyCode::Up, KeyModifiers::SUPER)),
        Some(TextCmd::DocStart)
    );
    assert_eq!(
        lookup(
            &emacs,
            &chord(KeyCode::Up, KeyModifiers::SUPER | KeyModifiers::SHIFT)
        ),
        Some(TextCmd::SelectToDocStart)
    );
    assert_eq!(
        lookup(
            &emacs,
            &chord(KeyCode::Down, KeyModifiers::SUPER | KeyModifiers::SHIFT)
        ),
        Some(TextCmd::SelectToDocEnd)
    );
    assert_eq!(
        lookup(&emacs, &chord(KeyCode::Left, KeyModifiers::SUPER)),
        Some(TextCmd::LineStart)
    );
    assert_eq!(lookup(&emacs, &cmd('x')), Some(TextCmd::Cut));
    assert_eq!(lookup(&emacs, &cmd('v')), Some(TextCmd::Paste));
    assert_eq!(lookup(&emacs, &cmd('z')), Some(TextCmd::Undo));
    assert_eq!(lookup(&emacs, &cmd('a')), Some(TextCmd::SelectAll));

    // A real field obeys the family it is given.
    let mut named = TextInput::new(WidgetId::of("scenario.keys"), "Name")
        .value("abc")
        .edit_entry(EditEntry::Explicit)
        .field_keys(FieldKeys::Desktop);
    named.begin_edit();
    named.on_key(&ctrl('a'));
    assert_eq!(named.field.buffer.selected_text(), Some("abc"));
    named.on_key(&ctrl('x'));
    assert_eq!(named.text(), "");
    assert_eq!(clipboard::text().as_deref(), Some("abc"));
    named.on_key(&ctrl('v'));
    assert_eq!(named.text(), "abc");
    named.on_key(&ctrl('z'));
    assert_eq!(named.text(), "");
}

fn state_undo_history() {
    let mut buf = TextBuffer::single("a");
    let mut undo = UndoStack::new();
    assert!(!undo.can_undo() && !undo.can_redo());
    assert!(
        !undo.undo(&mut buf) && !undo.redo(&mut buf),
        "an empty history changes nothing"
    );

    undo.record(&buf, EditKind::Typing);
    buf.insert_char('b');
    undo.record(&buf, EditKind::Typing);
    buf.insert_char('c');
    assert_eq!(buf.text(), "abc");
    assert!(undo.can_undo() && !undo.can_redo());
    assert!(undo.undo(&mut buf));
    assert_eq!(buf.text(), "a", "a typing run is one undo step");
    assert_eq!(buf.cursor_offset(), 1);
    assert!(undo.can_redo() && !undo.can_undo());
    assert!(undo.redo(&mut buf));
    assert_eq!(buf.text(), "abc");

    // A sealed run starts a new step; deleting is its own kind of run.
    let mut buf = TextBuffer::single("");
    let mut undo = UndoStack::default();
    undo.record(&buf, EditKind::Typing);
    buf.insert_char('x');
    undo.seal();
    undo.record(&buf, EditKind::Typing);
    buf.insert_char('y');
    undo.record(&buf, EditKind::Deleting);
    buf.backspace();
    assert_eq!(buf.text(), "x");
    assert!(undo.undo(&mut buf));
    assert_eq!(buf.text(), "xy", "deleting did not fold into typing");
    assert!(undo.undo(&mut buf));
    assert_eq!(buf.text(), "x", "a seal splits the typing run");
    assert!(undo.undo(&mut buf));
    assert_eq!(buf.text(), "");
    assert!(!undo.undo(&mut buf));
    assert!(undo.redo(&mut buf));
    undo.record(&buf, EditKind::Other);
    assert!(!undo.can_redo(), "a new edit forks history");
    undo.record(&buf, EditKind::Other);
    undo.record(&buf, EditKind::Other);
    let mut steps = 0;
    while undo.undo(&mut buf) {
        steps += 1;
    }
    assert_eq!(steps, 4, "an Other edit never folds");
    undo.record(&buf, EditKind::Other);
    undo.clear();
    assert!(!undo.can_undo() && !undo.can_redo());
    assert_eq!(EditKind::default(), EditKind::Typing);

    // The limit forgets the oldest step.
    let mut buf = TextBuffer::single("");
    let mut undo = UndoStack::new();
    assert_eq!(UndoStack::LIMIT, 200);
    for _ in 0..UndoStack::LIMIT + 5 {
        undo.record(&buf, EditKind::Other);
        buf.insert_char('x');
    }
    let mut steps = 0;
    while undo.undo(&mut buf) {
        steps += 1;
    }
    assert_eq!(steps, UndoStack::LIMIT);
    assert_eq!(buf.text().len(), 5, "the five oldest steps fell off");

    // A real field records its own history.
    let mut input = TextInput::new(WidgetId::of("scenario.undo"), "Name")
        .value("")
        .edit_entry(EditEntry::Explicit)
        .field_keys(FieldKeys::Desktop);
    input.begin_edit();
    for c in ['h', 'i'] {
        input.on_key(&letter(c));
    }
    assert!(input.can_undo());
    input.on_key(&chord(KeyCode::Char('z'), KeyModifiers::CONTROL));
    assert_eq!(input.text(), "", "typing a word undoes as one step");
}

fn state_edit_entry() {
    assert_eq!(EditEntry::default(), EditEntry::Immediate);
    assert_eq!(FocusSelect::default(), FocusSelect::SelectAll);
    for (select, selected, cursor) in [
        (FocusSelect::SelectAll, Some("hello world"), 11),
        (FocusSelect::CaretEnd, None, 11),
        (FocusSelect::CaretStart, None, 0),
    ] {
        let mut buf = TextBuffer::single("hello world");
        buf.move_doc_start(false);
        select.apply(&mut buf);
        assert_eq!(buf.selected_text(), selected);
        assert_eq!(buf.cursor_offset(), cursor);
    }

    // Immediate: the first key edits at once, where the caret lands is the
    // field's focus-select policy.
    let typed = |entry: EditEntry, select: FocusSelect, key: Key| {
        let mut input = TextInput::new(WidgetId::of("scenario.entry"), "Name")
            .value("abc")
            .edit_entry(entry)
            .focus_select(select);
        let (outcome, _) = input.on_key(&key);
        (outcome, input.text().to_owned(), input.editing)
    };
    assert_eq!(
        typed(EditEntry::Immediate, FocusSelect::SelectAll, letter('x')),
        (Outcome::Changed, "x".to_owned(), true)
    );
    assert_eq!(
        typed(EditEntry::Immediate, FocusSelect::CaretEnd, letter('x')),
        (Outcome::Changed, "abcx".to_owned(), true)
    );
    assert_eq!(
        typed(EditEntry::Immediate, FocusSelect::CaretStart, letter('x')),
        (Outcome::Changed, "xabc".to_owned(), true)
    );
    // TypeToEdit: a letter starts the edit at the end of the text.
    assert_eq!(
        typed(EditEntry::TypeToEdit, FocusSelect::SelectAll, letter('x')),
        (Outcome::Changed, "abcx".to_owned(), true)
    );
    // Explicit: a letter does nothing until Enter opens the edit.
    assert_eq!(
        typed(EditEntry::Explicit, FocusSelect::SelectAll, letter('x')),
        (Outcome::Ignored, "abc".to_owned(), false)
    );
    assert_eq!(
        typed(
            EditEntry::Explicit,
            FocusSelect::SelectAll,
            plain(KeyCode::Enter)
        ),
        (Outcome::Changed, "abc".to_owned(), true)
    );
    let mut explicit = TextInput::new(WidgetId::of("scenario.entry.explicit"), "Name")
        .value("abc")
        .edit_entry(EditEntry::Explicit);
    assert_eq!(explicit.entry(), EditEntry::Explicit);
    assert!(
        !explicit.wants_text_input(),
        "an idle explicit field leaves letters to shortcuts"
    );
    explicit.on_key(&plain(KeyCode::Enter));
    assert!(explicit.wants_text_input());
    let immediate = TextInput::new(WidgetId::of("scenario.entry.immediate"), "Name")
        .edit_entry(EditEntry::Immediate);
    assert!(
        immediate.wants_text_input(),
        "an immediate field always takes letters"
    );
}

// ---------------------------------------------------------------------------
// Part: the parts real widgets register as hit regions
// ---------------------------------------------------------------------------

/// Every part `owner` registered while painting.
fn registered_parts(hits: &HitRegistry, owner: WidgetId) -> Vec<Part> {
    let mut parts: Vec<Part> = hits
        .regions()
        .iter()
        .filter(|r| r.owner == owner)
        .map(|r| r.part.part)
        .collect();
    parts.sort();
    parts.dedup();
    parts
}

/// Each wanted part of `owner` is a region a pointer can land on.
fn assert_registers(hits: &HitRegistry, owner: WidgetId, wanted: &[Part], what: &str) {
    let seen = registered_parts(hits, owner);
    for part in wanted {
        assert!(
            seen.contains(part),
            "{what} registers no {part:?} region, only {seen:?}"
        );
        let region = hits
            .regions()
            .iter()
            .rev()
            .find(|r| r.owner == owner && r.part.part == *part)
            .expect("a region of the part");
        assert!(!region.area.is_empty(), "{what}: {part:?} has an area");
        assert_eq!(
            hits.hit(region.area.as_position()),
            Some(region.id),
            "{what}: the top-left cell of {part:?} resolves to its own region"
        );
    }
}

fn state_part_regions() {
    use termrock::ui::modal::Frame;
    use termrock::ui::popup::Dismiss;
    use termrock::widgets::chips::{Chip, ChipBar};
    use termrock::widgets::grid::{CellKind, CellValue, ColumnSpec, DataGrid, GridRows, RowTotal};
    use termrock::widgets::hintbar::{HintBar, HintLayer};
    use termrock::widgets::keyhint::Hint;
    use termrock::widgets::option_editor::{Affordance, ColumnEditor, EditorConfig, Options};
    use termrock::widgets::panel::ScrollPanel;
    use termrock::widgets::picker::Picker;
    use termrock::widgets::rail::SidebarRail;
    use termrock::widgets::search_detail::{SearchConfig, SearchFocusMode};
    use termrock::widgets::tabs::{TabItem, Tabs};
    use termrock::widgets::tree::{TreeNode, TreeView};

    // Tabs: each tab, its close button and the new-tab button; a crowded
    // strip adds the overflow button.
    let id = WidgetId::of("scenario.parts.tabs");
    let items = |n: usize| -> Vec<TabItem> {
        (0..n)
            .map(|i| TabItem::new(&format!("tab {i}")).closable())
            .collect()
    };
    let mut tabs = Tabs::with_items(id, items(3)).allow_new(true);
    let wide = paint_hits(Rect::new(0, 0, 60, 3), |area, buf, ctx| {
        tabs.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert_registers(&wide.hits, id, &[Part::TAB, Part::CLOSE, Part::NEW], "tabs");
    let mut crowded = Tabs::with_items(id, items(9))
        .allow_new(true)
        .overflow_menu(true);
    let narrow = paint_hits(Rect::new(0, 0, 36, 3), |area, buf, ctx| {
        crowded.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert_registers(&narrow.hits, id, &[Part::TAB, Part::OVERFLOW], "tabs");

    // Chip bar: the lead title, a close button per chip and the overflow
    // button once the chips no longer fit; a roomy bar has the add button.
    let id = WidgetId::of("scenario.parts.chips");
    let mut bar = ChipBar::new(id).overflow_menu(true);
    bar.set_lead(Some("match all".to_owned()));
    bar.set_chips((0..8).map(|i| Chip::new(&format!("filter {i}"))).collect());
    let chips = paint_hits(Rect::new(0, 0, 50, 1), |area, buf, ctx| {
        bar.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert_registers(
        &chips.hits,
        id,
        &[Part::TITLE, Part::CLOSE, Part::OVERFLOW],
        "a chip bar",
    );
    let mut roomy = ChipBar::new(id);
    roomy.set_chips(vec![Chip::new("only")]);
    let roomy = paint_hits(Rect::new(0, 0, 50, 1), |area, buf, ctx| {
        roomy.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert_registers(&roomy.hits, id, &[Part::NEW], "a chip bar");

    // Grid: header, cells, row numbers, column seams and, on a column with a
    // pop-up editor, the open mark.
    let mut grid = super::seeded_grid("scenario.parts.grid", 5).resizable_columns(true);
    let grid_id = grid.id;
    let sheet = paint_hits(Rect::new(0, 0, 60, 10), |area, buf, ctx| {
        grid.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert_registers(
        &sheet.hits,
        grid_id,
        &[Part::HEADER, Part::CELL, Part::ROW_NUMBER, Part::SEAM],
        "a grid",
    );
    let mut picks = DataGrid::new(
        WidgetId::of("scenario.parts.picks"),
        vec![
            ColumnSpec::new("state", CellKind::Enum).editor(ColumnEditor::Select(Options::values(
                &["new", "open", "done"],
            ))),
        ],
    )
    .editor_config(EditorConfig {
        affordance: Affordance::Always,
        ..EditorConfig::default()
    });
    picks
        .try_set_rows(GridRows {
            rows: (0..3)
                .map(|_| vec![CellValue::Text("new".to_owned())])
                .collect(),
            total: RowTotal::Exact(3),
            more: false,
        })
        .expect("rectangular rows");
    let picks_id = picks.id;
    let open = paint_hits(Rect::new(0, 0, 40, 8), |area, buf, ctx| {
        picks.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert_registers(
        &open.hits,
        picks_id,
        &[Part::ACTIONS],
        "a grid with a pop-up editor",
    );

    // Table: check boxes, rows, cells, headers and seams.
    let table_id = WidgetId::of("scenario.parts.table");
    let mut table = DataTable::try_new(
        table_id,
        two_columns(),
        vec![cells(&["lint", "ana"]), cells(&["build", "kai"])],
    )
    .expect("rectangular rows")
    .selection_mode(SelectionMode::Multi)
    .selection_column(true)
    .resizable_columns(true);
    let sheet = paint_hits(Rect::new(0, 0, 40, 6), |area, buf, ctx| {
        table.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert_registers(
        &sheet.hits,
        table_id,
        &[
            Part::CONTAINER,
            Part::MARKER,
            Part::ROW,
            Part::CELL,
            Part::HEADER,
            Part::SEAM,
        ],
        "a table",
    );

    // Tree: the twisty of a branch.
    let tree_id = WidgetId::of("scenario.parts.tree");
    let mut branch = TreeNode::leaf("root");
    branch.children = vec![TreeNode::leaf("kid")];
    let mut tree = TreeView::new(tree_id, vec![branch]);
    let leaves = paint_hits(Rect::new(0, 0, 30, 4), |area, buf, ctx| {
        tree.render(area, buf, ctx, ctx.theme.canvas)
    });
    assert_registers(&leaves.hits, tree_id, &[Part::ICON], "a tree");

    // Rail: the collapse icon.
    let rail_id = WidgetId::of("scenario.parts.rail");
    let rail = SidebarRail::new(rail_id, "Files");
    let rails = paint_hits(Rect::new(0, 0, 30, 6), |area, buf, ctx| {
        rail.render_collapse(area, buf, ctx, ctx.theme.canvas)
    });
    assert_registers(&rails.hits, rail_id, &[Part::ICON], "a rail");

    // Hint bar: a clickable key.
    let layer = HintLayer::new(vec![Hint::new("q", "quit"), Hint::new("?", "help")]);
    let hints = paint_hits(Rect::new(0, 0, 40, 1), |area, buf, ctx| {
        HintBar::render_ctx(area, buf, ctx, &layer);
    });
    assert_registers(&hints.hits, HintBar::ID, &[Part::KEY], "the hint bar");

    // Picker: its query line, over the dismiss backdrop.
    let picker_id = WidgetId::of("scenario.parts.picker");
    let mut picker = Picker::new(picker_id, "Choose")
        .searchable(true)
        .expect("an empty picker is searchable");
    let picked = paint_hits(Rect::new(0, 0, 60, 16), |area, buf, ctx| {
        picker.draw(area, buf, ctx)
    });
    assert_registers(&picked.hits, picker_id, &[Part::QUERY], "a picker");

    // Panel: scrollbar track and thumb.
    let panel_id = WidgetId::of("scenario.parts.panel");
    let mut panel = ScrollPanel::new(panel_id, (1..=40).map(|i| format!("line {i}")).collect());
    let scrolled = paint_hits(Rect::new(0, 0, 30, 8), |area, buf, ctx| {
        panel.render(area, buf, ctx, ctx.theme.canvas, |theme, _| {
            theme.secondary()
        })
    });
    let bars: Vec<Part> = scrolled
        .hits
        .regions()
        .iter()
        .map(|r| r.part.part)
        .collect();
    assert!(
        bars.contains(&Part::TRACK) && bars.contains(&Part::THUMB),
        "a long panel paints a scrollbar track and thumb, saw {bars:?}"
    );

    // Modal frame: the dismiss backdrop that fills the screen.
    let frame = Frame::new(30, 8).dismiss(Dismiss::LIGHT);
    let framed = paint_hits(Rect::new(0, 0, 60, 20), |area, buf, ctx| {
        frame.render(area, buf, ctx);
    });
    let backdrop = framed
        .hits
        .regions()
        .iter()
        .find(|r| r.part.part == Part::BACKDROP)
        .expect("a dismissable frame registers a backdrop");
    assert_eq!(backdrop.area, Rect::new(0, 0, 60, 20));

    // Search detail in combined focus: the query field.
    let mut search =
        super::finder_search(SearchConfig::default().focus_mode(SearchFocusMode::Combined));
    let cat = super::finder_started(&mut search, super::FINDER_ROWS);
    let lines = super::preview_lines(5);
    let mut focus = Focus::default();
    focus.focus(search.id());
    let (_, found) =
        super::draw_source(&mut search, &cat, Rect::new(0, 0, 120, 24), &lines, &focus);
    assert_registers(&found, search.id(), &[Part::FIELD], "a combined search");
}

// ---------------------------------------------------------------------------
// Nonvisual exceptions: the sixteen Part constants no widget registers as a hit
// region, and the StateFlags bit set that nothing in the workspace consumes.
// ---------------------------------------------------------------------------

#[test]
fn catalog_state_part_constants_are_distinct_identity_tags() {
    let parts = [
        Part::ACTION,
        Part::ACTIONS,
        Part::BACKDROP,
        Part::BADGE,
        Part::BODY,
        Part::BORDER,
        Part::CELL,
        Part::CHANGE,
        Part::CLOSE,
        Part::CONTAINER,
        Part::DETAIL,
        Part::EMPTY,
        Part::FIELD,
        Part::GUTTER,
        Part::HEADER,
        Part::HELP,
        Part::ICON,
        Part::KEY,
        Part::LABEL,
        Part::MARKER,
        Part::META,
        Part::NEW,
        Part::OVERFLOW,
        Part::PLACEHOLDER,
        Part::PREFIX,
        Part::QUERY,
        Part::ROW,
        Part::ROW_NUMBER,
        Part::RULE,
        Part::SEAM,
        Part::SUMMARY,
        Part::TAB,
        Part::TEXT,
        Part::THUMB,
        Part::TITLE,
        Part::TRACK,
    ];
    assert_eq!(parts.len(), 36);
    assert_eq!(Part::ALL.len(), 36, "every built-in part is listed once");
    let raws: std::collections::BTreeSet<u16> = parts.iter().map(|p| p.raw()).collect();
    assert_eq!(raws.len(), 36, "no two parts share a number");
    let owner = WidgetId::of("scenario.parts");
    let ids: std::collections::BTreeSet<WidgetId> = parts
        .iter()
        .filter(|p| **p != Part::CONTAINER)
        .map(|p| owner.part(PartRef::of(*p)))
        .collect();
    assert_eq!(
        ids.len(),
        35,
        "each part addresses its own region of a widget"
    );
    assert_eq!(Part::CONTAINER.raw(), 0);
    assert_eq!(Part::ROW_NUMBER.name(), Some("ROW_NUMBER"));
}

#[test]
fn catalog_state_flags_combine_as_a_bit_set() {
    let all = [
        StateFlags::ACTIVE,
        StateFlags::BUSY,
        StateFlags::CHECKED,
        StateFlags::DIRTY,
        StateFlags::DISABLED,
        StateFlags::EDITING,
        StateFlags::ERROR,
        StateFlags::EXPANDED,
        StateFlags::FOCUSED,
        StateFlags::FOCUS_VISIBLE,
        StateFlags::HOVERED,
        StateFlags::LOADING,
        StateFlags::PRESSED,
        StateFlags::READ_ONLY,
        StateFlags::SELECTED,
        StateFlags::WARNING,
    ];
    let mut seen = 0u16;
    for flag in all {
        assert_eq!(flag.bits().count_ones(), 1, "{flag:?} is one bit");
        assert_eq!(seen & flag.bits(), 0, "{flag:?} is not shared");
        seen |= flag.bits();
    }
    assert_eq!(seen, u16::MAX, "sixteen flags fill the word");

    let none = StateFlags::empty();
    assert!(none.is_empty() && none.bits() == 0);
    assert_eq!(
        StateFlags::from_bits(StateFlags::FOCUSED.bits()),
        StateFlags::FOCUSED
    );
    let both = StateFlags::FOCUSED.union(StateFlags::HOVERED);
    assert!(both.contains(StateFlags::FOCUSED) && both.contains(both));
    assert!(
        !StateFlags::FOCUSED.contains(both),
        "contains needs every bit"
    );
    assert!(both.intersects(StateFlags::HOVERED) && !both.intersects(StateFlags::PRESSED));
    let mut flags = StateFlags::empty();
    flags.insert(StateFlags::SELECTED);
    flags.insert(StateFlags::DISABLED);
    assert!(flags.contains(StateFlags::SELECTED | StateFlags::DISABLED));
    flags.remove(StateFlags::DISABLED);
    assert!(!flags.intersects(StateFlags::DISABLED));
    flags.set(StateFlags::BUSY, true);
    flags.set(StateFlags::SELECTED, false);
    assert_eq!(flags, StateFlags::BUSY);
    assert_eq!(
        StateFlags::empty()
            .with(StateFlags::ERROR, true)
            .with(StateFlags::WARNING, true),
        StateFlags::ERROR | StateFlags::WARNING
    );
    assert_eq!(
        StateFlags::ERROR.with(StateFlags::ERROR, false),
        StateFlags::empty()
    );
    assert!(StateFlags::from_bits(0).is_empty());
    assert!(!StateFlags::ACTIVE.is_empty());
    assert!(!StateFlags::CHECKED.is_empty() && !StateFlags::EXPANDED.is_empty());
    assert!(!StateFlags::EDITING.is_empty() && !StateFlags::LOADING.is_empty());
    assert!(!StateFlags::READ_ONLY.is_empty() && !StateFlags::FOCUS_VISIBLE.is_empty());
    assert!(!StateFlags::DIRTY.is_empty() && !StateFlags::PRESSED.is_empty());
}
