//! The keyed store card of the Lists page (#301).
//!
//! A `ListBox` shows a `KeyedVec<ListItem>` through the keyed source methods
//! and answers every input with a `Response<ListAction>`. A second
//! `KeyedState`, the inspector, follows the same store and carries what a
//! list does not: marks, expansion, an edit target and a pointer press. The
//! card adds wiring and readouts only; identity, reconciliation and the
//! response algebra all live in the library.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

use crate::pages::{Cx, Hint};
use termrock::core::event::{Key, Outcome};
use termrock::core::id::{ItemKey, Part, WidgetId};
use termrock::core::intent::Phase;
use termrock::core::keyed::{KeyedSourceError, KeyedState, KeyedVec, Reconcile};
use termrock::core::list::{KeyedRowSource, RowSource, SourceStamp};
use termrock::core::pointer::PointerEvent;
use termrock::core::response::{Actions, Response};
use termrock::theme::Tone;
use termrock::ui::ctx::RenderCtx;
use termrock::ui::text;
use termrock::widgets::empty::{self, EmptyState};
use termrock::widgets::list::{ListAction, ListBox, ListItem, ListItemRef, ListSource, SelectMode};
use termrock::widgets::panel::Panel;

pub const KEYED: WidgetId = WidgetId::of("lists.keyed");

const ROWS: [(&str, &str, bool); 6] = [
    ("src/api/auth.rs", "modified", false),
    ("src/api/billing.rs", "modified", false),
    ("Cargo.lock", "locked", true),
    ("tests/checkout.rs", "new", false),
    ("docs/webhooks.md", "modified", false),
    ("src/config.rs", "modified", false),
];

pub const CARD_H: u16 = 14;
pub const CARD_W: u16 = 38;

/// A way a provider can break the keyed contract, so the list shows what a
/// rejected source looks like and keeps its previous state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    None,
    Positional,
    DuplicateAuthoritative,
    DuplicateVisible,
    UnknownVisible,
}

impl Fault {
    pub fn from_value(value: &str) -> Self {
        match value {
            "positional" => Self::Positional,
            "duplicate" => Self::DuplicateAuthoritative,
            "visible duplicate" => Self::DuplicateVisible,
            "unknown row" => Self::UnknownVisible,
            _ => Self::None,
        }
    }
}

/// The store seen through a fault. With `Fault::None` it is the store itself.
struct Source<'a> {
    store: &'a KeyedVec<ListItem>,
    fault: Fault,
}

impl Source<'_> {
    fn real_key(&self, i: usize) -> ItemKey {
        self.store.key_at(i).expect("row within the store")
    }
}

impl RowSource for Source<'_> {
    fn len(&self) -> usize {
        self.store.len()
    }

    fn enabled(&self, i: usize) -> bool {
        RowSource::enabled(self.store, i)
    }

    fn label(&self, i: usize) -> &str {
        RowSource::label(self.store, i)
    }

    fn key(&self, i: usize) -> ItemKey {
        match self.fault {
            Fault::DuplicateVisible if i == 1 => self.real_key(0),
            Fault::UnknownVisible if i + 1 == self.store.len() => ItemKey::text("ghost"),
            _ => self.real_key(i),
        }
    }
}

impl KeyedRowSource for Source<'_> {
    fn stamp(&self) -> SourceStamp {
        let stamp = self.store.stamp();
        match self.fault {
            Fault::None => stamp,
            _ => SourceStamp {
                data: stamp.data + 1_000,
                ..stamp
            },
        }
    }

    fn source_len(&self) -> usize {
        self.store.len()
    }

    fn source_key(&self, i: usize) -> ItemKey {
        match self.fault {
            Fault::Positional if i == 0 => ItemKey::index(0),
            Fault::DuplicateAuthoritative if i == 1 => self.real_key(0),
            _ => self.real_key(i),
        }
    }
}

impl ListSource for Source<'_> {
    fn item(&self, visible: usize) -> ListItemRef<'_> {
        self.store.item(visible)
    }
}

fn record(name: &str, meta: &str, locked: bool) -> ListItem {
    let tone = match meta {
        "new" => Tone::Success,
        "modified" => Tone::Warning,
        _ => Tone::Muted,
    };
    ListItem::new(name)
        .meta(meta)
        .meta_tone(tone)
        .disabled(locked)
}

fn seed() -> KeyedVec<ListItem> {
    KeyedVec::try_from_rows(
        ROWS.iter()
            .map(|(name, meta, locked)| (ItemKey::text(name), record(name, meta, *locked))),
    )
    .expect("the seed rows have distinct text keys")
}

fn variant_name(error: &KeyedSourceError) -> &'static str {
    match error {
        KeyedSourceError::PositionalKey { .. } => "PositionalKey",
        KeyedSourceError::DuplicateAuthoritative { .. } => "DuplicateAuthoritative",
        KeyedSourceError::DuplicateVisible { .. } => "DuplicateVisible",
        KeyedSourceError::UnknownVisible { .. } => "UnknownVisible",
        KeyedSourceError::MissingNode { .. } => "MissingNode",
        KeyedSourceError::Cycle { .. } => "Cycle",
    }
}

/// One line per way a source can be rejected.
fn describe_error(error: &KeyedSourceError) -> String {
    match error {
        KeyedSourceError::PositionalKey {
            index,
            authoritative,
        } => format!(
            "PositionalKey at {index} ({} set): a row is named by position",
            if *authoritative {
                "authoritative"
            } else {
                "visible"
            }
        ),
        KeyedSourceError::DuplicateAuthoritative { .. } => {
            "DuplicateAuthoritative: two records share a key".into()
        }
        KeyedSourceError::DuplicateVisible { .. } => {
            "DuplicateVisible: a key is shown twice".into()
        }
        KeyedSourceError::UnknownVisible { .. } => {
            "UnknownVisible: a shown key is not in the store".into()
        }
        KeyedSourceError::MissingNode { .. } => "MissingNode: a child is not in the tree".into(),
        KeyedSourceError::Cycle { .. } => "Cycle: a node is its own ancestor".into(),
    }
}

pub struct KeyedCard {
    store: KeyedVec<ListItem>,
    pub list: ListBox,
    inspect: KeyedState,
    fault: Fault,
    fresh: u64,
    renamed: u64,
    generation: u64,
    area: Rect,
    reconciled: Reconcile,
    error: Option<KeyedSourceError>,
    batch: Actions<String>,
    answer: Response<()>,
    press: String,
    note: String,
}

impl KeyedCard {
    pub fn new() -> Self {
        Self {
            store: seed(),
            list: ListBox::new(KEYED, Vec::new(), SelectMode::Multi).type_ahead(false),
            inspect: KeyedState::new(),
            fault: Fault::None,
            fresh: 0,
            renamed: 0,
            generation: 0,
            area: Rect::ZERO,
            reconciled: Reconcile::default(),
            error: None,
            batch: Actions::None,
            answer: Response::ignored(),
            press: "no press yet".into(),
            note: "i insert · x remove · u n move · r rename".into(),
        }
    }

    pub fn set_fault(&mut self, fault: Fault) {
        self.fault = fault;
    }

    fn name(&self, key: ItemKey) -> String {
        self.store
            .get(key)
            .map_or_else(|| "a removed row".to_owned(), |item| item.label.clone())
    }

    fn describe(&self, action: ListAction) -> String {
        match action {
            ListAction::Activated(key) => format!("Activated {}", self.name(key)),
            ListAction::SelectionChanged => "SelectionChanged".to_owned(),
        }
    }

    fn cursor_key(&self) -> Option<ItemKey> {
        self.list.keyed().cursor()
    }

    /// The list's answer, with each action described for the readout.
    fn listed(&self, response: Response<ListAction>) -> Response<String> {
        response.map_actions(|action| self.describe(action))
    }

    /// Takes the answer of one input: the readout keeps its flow, its
    /// invalidation and its ordered actions, and the status line announces
    /// each action.
    fn absorb(&mut self, mut response: Response<String>, cx: &mut Cx) -> Outcome {
        let announced = response.clone().on_actions(|text| cx.status(text));
        let outcome = announced.outcome();
        self.answer = announced;
        self.batch = response.take_actions();
        outcome
    }

    fn noted(&mut self, text: impl Into<String>, cx: &mut Cx) -> Outcome {
        let text = text.into();
        self.note = text.clone();
        let mut response = Response::<String>::changed();
        response.push_action(text);
        self.absorb(response, cx)
    }

    /// Brings the inspector in line with the store after a mutation. The
    /// readout keeps the last reconciliation that changed anything.
    fn sync(&mut self) {
        let held = self.inspect.stamp();
        // A `KeyedVec` validates its own keys, so it never fails to reconcile.
        if let Ok(done) = self.inspect.reconcile(&self.store)
            && self.inspect.stamp() != held
        {
            self.reconciled = done;
        }
    }

    pub fn on_key(&mut self, key: &Key, cx: &mut Cx) -> Outcome {
        self.sync();
        let at = self.cursor_key();
        let position = at.and_then(|key| self.store.position(key)).unwrap_or(0);
        match key.code {
            KeyCode::Char('i') => {
                self.fresh += 1;
                let name = format!("src/new_{}.rs", self.fresh);
                let key = ItemKey::text(&name);
                match self
                    .store
                    .insert(position, key, record(&name, "new", false))
                {
                    Ok(()) => self.done(format!("inserted {name} at {position}"), cx),
                    Err(error) => self.noted(describe_error(&error), cx),
                }
            }
            KeyCode::Char('x') => match at.and_then(|key| self.store.remove(key)) {
                Some(item) => self.done(format!("removed {}", item.label), cx),
                None => self.noted("nothing to remove", cx),
            },
            KeyCode::Char(c @ ('u' | 'n')) => {
                let to = if c == 'u' {
                    position.saturating_sub(1)
                } else {
                    position + 1
                };
                match at {
                    Some(key) if self.store.move_to(key, to) => {
                        self.done(format!("moved {} to {to}", self.name(key)), cx)
                    }
                    _ => self.noted("already at the end", cx),
                }
            }
            KeyCode::Char('r') => self.rename(at, cx),
            KeyCode::Char('f') => {
                let same: Vec<(ItemKey, ListItem)> = self
                    .store
                    .iter()
                    .map(|(key, item)| (key, item.clone()))
                    .collect();
                match self.store.replace_if_changed(same) {
                    Ok(false) => self.noted("refresh found nothing new: stamp kept", cx),
                    Ok(true) => self.done("refresh replaced the rows".into(), cx),
                    Err(error) => self.noted(describe_error(&error), cx),
                }
            }
            KeyCode::Char('v') => {
                let mut rows: Vec<(ItemKey, ListItem)> = self
                    .store
                    .iter()
                    .map(|(key, item)| (key, item.clone()))
                    .collect();
                rows.reverse();
                match self.store.replace(rows) {
                    Ok(()) => self.done("reversed all rows, same instance".into(), cx),
                    Err(error) => self.noted(describe_error(&error), cx),
                }
            }
            KeyCode::Char('d') => {
                let key = at.unwrap_or(ItemKey::text("src/api/auth.rs"));
                let name = self.name(key);
                match self.store.push(key, record(&name, "copy", false)) {
                    Ok(()) => self.done("pushed a copy".into(), cx),
                    Err(error) => self.noted(format!("push refused: {}", variant_name(&error)), cx),
                }
            }
            KeyCode::Char('e') => self.toggle_edit(at, cx),
            KeyCode::Char('o') => self.toggle_expanded(at, cx),
            KeyCode::Char('m') => self.toggle_mark(at, cx),
            KeyCode::Char('M') => {
                let rows = self.list.selection.iter().collect::<Vec<_>>();
                let anchor = self.list.selection.anchor();
                let changed = self.inspect.adopt_selection_view(rows, anchor);
                self.noted(
                    if changed {
                        "marks now follow the checked rows"
                    } else {
                        "marks already match the checked rows"
                    },
                    cx,
                )
            }
            KeyCode::Char('c') => {
                let changed = self.inspect.replace_selection(std::iter::empty());
                self.noted(
                    if changed {
                        "marks cleared"
                    } else {
                        "no marks to clear"
                    },
                    cx,
                )
            }
            _ => {
                let source = Source {
                    store: &self.store,
                    fault: self.fault,
                };
                let result = self.list.on_key_source(key, &source);
                self.finish_list(result, cx)
            }
        }
    }

    /// A mutation moved the store: say what happened and let the inspector
    /// reconcile, which is where an edit target or a press is cancelled.
    fn done(&mut self, text: String, cx: &mut Cx) -> Outcome {
        self.sync();
        let mut text = text;
        if self.reconciled.edit_cancelled {
            text.push_str(" · the edit was cancelled");
        }
        if self.reconciled.pointer_cancelled {
            text.push_str(" · the press was dropped");
        }
        self.noted(text, cx)
    }

    fn rename(&mut self, at: Option<ItemKey>, cx: &mut Cx) -> Outcome {
        let Some(key) = at else {
            return self.noted("nothing to rename", cx);
        };
        self.renamed += 1;
        let tag = format!("renamed {}", self.renamed);
        self.store.update(key, |item| item.meta = Some(tag.clone()));
        let committed = self.inspect.edit_key() == Some(key);
        if committed {
            self.inspect.finish_edit();
        }
        let text = if committed {
            format!("committed the edit of {}", self.name(key))
        } else {
            format!("{} {tag}", self.name(key))
        };
        self.done(text, cx)
    }

    fn toggle_edit(&mut self, at: Option<ItemKey>, cx: &mut Cx) -> Outcome {
        if self.inspect.cancel_edit() {
            return self.noted("edit cancelled", cx);
        }
        match at {
            Some(key) if self.inspect.begin_edit(key) => {
                self.noted(format!("editing {} · r commits", self.name(key)), cx)
            }
            Some(key) => self.noted(format!("{} cannot be edited", self.name(key)), cx),
            None => self.noted("no row to edit", cx),
        }
    }

    fn toggle_expanded(&mut self, at: Option<ItemKey>, cx: &mut Cx) -> Outcome {
        let Some(key) = at else {
            return self.noted("no row to expand", cx);
        };
        let open = !self.inspect.is_expanded(key);
        self.inspect.set_expanded(key, open);
        self.noted(
            format!(
                "{} {}",
                self.name(key),
                if open { "expanded" } else { "collapsed" }
            ),
            cx,
        )
    }

    fn toggle_mark(&mut self, at: Option<ItemKey>, cx: &mut Cx) -> Outcome {
        let Some(key) = at else {
            return self.noted("no row to mark", cx);
        };
        let marked = !self.inspect.is_selected(key);
        if self.inspect.set_selected(key, marked) {
            let word = if marked { "marked" } else { "unmarked" };
            self.noted(format!("{} {word}", self.name(key)), cx)
        } else {
            self.noted(format!("{} cannot be marked", self.name(key)), cx)
        }
    }

    fn finish_list(
        &mut self,
        result: Result<Response<ListAction>, KeyedSourceError>,
        cx: &mut Cx,
    ) -> Outcome {
        match result {
            Ok(response) => {
                self.error = None;
                let response = self.listed(response);
                self.sync();
                self.absorb(response, cx)
            }
            Err(error) => {
                self.error = Some(error);
                Outcome::Changed
            }
        }
    }

    pub fn on_pointer(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Option<Outcome> {
        let key = ev.on(KEYED).then(|| ev.key_of(Part::ROW)).flatten();
        let mut extra = Response::<String>::ignored();
        match (ev.phase(), key) {
            (Some(Phase::Press), Some(key)) => {
                self.sync();
                let captured = self.inspect.capture_press(key, self.generation);
                self.press = format!(
                    "press on {}: {}",
                    self.name(key),
                    if captured { "captured" } else { "refused" }
                );
                extra.push_action(self.press.clone());
            }
            (Some(Phase::Click | Phase::Release), Some(key)) => {
                let held = self.inspect.pressed();
                let accepted = self.inspect.release_press(key, self.generation);
                self.press = format!(
                    "release on {}: {}",
                    self.name(key),
                    match (held, accepted) {
                        (None, _) => "no press held",
                        (Some(_), true) => "same row, accepted",
                        (Some(_), false) => "other row or moved layout, refused",
                    }
                );
                extra.push_action(self.press.clone());
            }
            (Some(Phase::DragStart), _) => {
                self.inspect.cancel_press();
                self.press = "pointer left the row: press dropped".into();
            }
            _ => {}
        }
        let source = Source {
            store: &self.store,
            fault: self.fault,
        };
        let result = self.list.on_pointer_source(ev, &source);
        match result {
            Ok(response) => {
                self.error = None;
                let response = self.listed(response).merge(extra);
                self.sync();
                let handled = response.is_consumed() || response.has_actions();
                let out = self.absorb(response, cx);
                if ev.is(Phase::Click) && key.is_some() {
                    cx.focus.focus(KEYED);
                }
                handled.then_some(out)
            }
            Err(error) => {
                self.error = Some(error);
                Some(Outcome::Changed)
            }
        }
    }

    pub fn hints(&self) -> Vec<Hint> {
        vec![
            ("i x", "Insert / remove"),
            ("u n", "Move"),
            ("r", "Rename"),
            ("e", "Edit"),
            ("m", "Mark"),
            ("o", "Expand"),
        ]
    }

    pub fn meta(&self) -> String {
        format!("{} rows", self.store.len())
    }

    pub fn render(&mut self, outer: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let meta = self.meta();
        let panel = Panel::card(Some("Keyed store"))
            .meta(&meta)
            .focused(ctx.interaction.focused(KEYED));
        let bg = panel.bg(t);
        let inner = panel.render(outer, buf, t);
        if inner.is_empty() {
            return;
        }
        buf.set_string(
            inner.x,
            inner.y,
            text::truncate(&self.note, inner.width as usize),
            t.muted().bg(bg),
        );
        let body = Rect::new(
            inner.x,
            inner.y + 2,
            inner.width,
            inner.height.saturating_sub(2),
        );
        if body != self.area {
            self.area = body;
            self.generation += 1;
            self.inspect.invalidate_layout(self.generation);
        }
        if self.store.is_empty() {
            ctx.control(KEYED, body, false);
            empty::render_ctx(
                body,
                buf,
                ctx,
                &EmptyState::new("The store is empty").hint("Press i to insert a record"),
                bg,
            );
            return;
        }
        let source = Source {
            store: &self.store,
            fault: self.fault,
        };
        let shown = self.list.render_source(body, &source, buf, ctx, bg);
        if let Err(error) = shown {
            ctx.control(KEYED, body, false);
            empty::render_ctx(
                body,
                buf,
                ctx,
                &EmptyState::new("Source rejected").hint(&format!(
                    "{} · the list keeps its state",
                    describe_error(&error)
                )),
                bg,
            );
            self.error = Some(error);
        } else if self.fault == Fault::None {
            self.error = None;
        }
        self.sync();
        self.inspect.adopt_cursor_view(self.list.cursor);
    }

    pub fn readout(&self) -> Vec<String> {
        let state = self.list.keyed();
        let list = |keys: Vec<ItemKey>| {
            if keys.is_empty() {
                "none".to_owned()
            } else {
                keys.into_iter()
                    .map(|key| self.name(key))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        };
        let label = |key: Option<ItemKey>| key.map_or("none".to_owned(), |key| self.name(key));
        let stamp = self.store.stamp();
        let sync = |held: Option<SourceStamp>| {
            if held == Some(stamp) {
                "in sync"
            } else {
                "behind"
            }
        };
        let r = &self.reconciled;
        let flag = |on: bool, word: &str| {
            if on {
                word.to_owned()
            } else {
                format!("no {word}")
            }
        };
        let cursor = state.cursor();
        let identity = cursor.map_or("none".to_owned(), |key| {
            format!(
                "{} · store {:?} · shown {:?} · authoritative {} · enabled {}",
                self.name(key),
                self.store.position(key),
                state.visible_index(key),
                state.is_authoritative(key),
                state.is_enabled(key),
            )
        });
        let mut lines = vec![
            format!(
                "store: {} rows · data {} view {} · list {} · inspector {}",
                self.store.len(),
                stamp.data,
                stamp.view,
                sync(state.stamp()),
                sync(self.inspect.stamp()),
            ),
            format!(
                "cursor: {identity} · first shown {}",
                label(state.visible_key(0))
            ),
            format!(
                "list: {} of {} shown checked: {} · anchor {}",
                state.selected_len(),
                state.visible_len(),
                list(state.selected().collect()),
                label(state.anchor()),
            ),
            format!(
                "inspector: cursor row {:?} · anchor row {:?} · marked rows {:?}",
                self.inspect.cursor_view(),
                self.inspect.anchor_view(),
                self.inspect.selected_view(),
            ),
            format!(
                "marked: {} · expanded: {} (revision {}) · editing: {}",
                list(self.inspect.selected().collect()),
                list(self.inspect.expanded().collect()),
                self.inspect.expanded_revision(),
                label(self.inspect.edit_key()),
            ),
            format!(
                "reconcile: {} · {} · {} · {} · {}",
                flag(r.data_changed, "data change"),
                flag(r.view_changed, "view change"),
                flag(r.cursor_changed, "cursor move"),
                flag(r.edit_cancelled, "edit cancel"),
                flag(r.pointer_cancelled, "press cancel"),
            ),
            format!(
                "response: {:?} · {:?} · {}",
                self.answer.flow(),
                self.answer.invalidate(),
                match &self.batch {
                    Actions::None => "no actions".to_owned(),
                    Actions::One(_) => "one action".to_owned(),
                    Actions::Many(all) => format!("{} actions", all.len()),
                },
            ),
            format!(
                "actions: {}",
                if self.batch.is_empty() {
                    "none".to_owned()
                } else {
                    self.batch.iter().cloned().collect::<Vec<_>>().join(" | ")
                },
            ),
            format!("pointer: {}", self.press),
        ];
        if let Some(error) = &self.error {
            lines.push(format!("rejected: {}", describe_error(error)));
        }
        lines
    }
}
