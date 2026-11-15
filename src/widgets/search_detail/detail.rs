use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use super::{
    QueryToken, SearchAction, SearchCmd, SearchConfig, SearchError, SearchFocusMode, SearchPhase,
    SearchSession, SearchSource,
};
use crate::core::event::Key;
use crate::core::focus::{Focus, FocusChange};
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::intent::Phase;
use crate::core::list::{KeyedRowSource, RowSource, SourceStamp};
use crate::core::pointer::PointerEvent;
use crate::core::response::Response;
use crate::ui::context::{ContextAnchor, ContextRequest, ContextSource};
use crate::ui::ctx::{RenderCtx, fill, legible_on_fill};
use crate::ui::layout::Split;
use crate::ui::text::truncate;
use crate::widgets::edit::EditField;
use crate::widgets::input::TextInput;
use crate::widgets::list_detail::{
    Event, ListDetail, ListDetailSource, MasterRow, MasterWidths, SourceFrame, paint_master_row,
};

type Reply = Response<SearchAction>;

/// A [`SearchSource`] that also supplies [`ListDetailSource`] master rows, so
/// [`SearchDetail::render_source`] can paint it with the standard list look.
/// Custom painting through [`SearchDetail::render_source_with`] needs only
/// [`SearchSource`].
pub trait SearchDetailSource: SearchSource + ListDetailSource {}

impl<T: SearchSource + ListDetailSource + ?Sized> SearchDetailSource for T {}

/// The query as a paint hook sees it. The hook cannot edit the query.
#[derive(Clone, Debug)]
pub struct QueryView<'a> {
    pub text: &'a str,
    pub placeholder: &'a str,
    /// The widget id that owns keyboard focus for the query.
    pub owner: WidgetId,
    pub focused: bool,
    /// Selected byte range of `text`.
    pub selection: Option<Range<usize>>,
    pub caret_byte: usize,
}

/// One visible result row handed to a paint callback.
pub struct RowPaint<'a, S: ?Sized> {
    pub source: &'a S,
    /// Index in the source's visible rows.
    pub row: usize,
    pub kind: super::ResultRowKind,
    /// The row's cells, gutter included. Its hit region is already registered.
    pub rect: Rect,
    /// The row's hit region id, for hover and pressed state.
    pub hit: WidgetId,
    /// Whether the selection is in this row.
    pub selected: bool,
    /// The selected cell of this row. An [`Item`](super::ResultRowKind::Item)
    /// row's single cell is cell 0.
    pub selected_cell: Option<usize>,
    /// Whether the results own keyboard focus.
    pub focused: bool,
    /// The displayed results are not the newest answer: loading or failed.
    /// Paint them dimmed; they cannot be acted on.
    pub stale: bool,
    owner: WidgetId,
    cells: &'a mut Vec<(usize, Rect)>,
}

impl<S: SearchSource + ?Sized> RowPaint<'_, S> {
    /// The hit region id the widget registers for cell `cell` of this row, for
    /// hover and pressed state. It exists only for a cell placed with
    /// [`place_cell`](Self::place_cell).
    pub fn cell_hit(&self, cell: usize) -> WidgetId {
        self.owner.part(PartRef::item(
            Part::CELL,
            self.source.cell_key(self.row, cell),
        ))
    }
}

impl<S: ?Sized> RowPaint<'_, S> {
    /// Reports where cell `cell` of a [`Cells`](super::ResultRowKind::Cells)
    /// row was drawn. The widget registers its hit region under the cell's
    /// stable key, so the painter only draws.
    pub fn place_cell(&mut self, cell: usize, rect: Rect) {
        self.cells.push((cell, rect));
    }
}

#[derive(Clone, Copy, Debug)]
struct Anchor {
    key: ItemKey,
    id: WidgetId,
    rect: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LayoutKey {
    area: Rect,
    offset: usize,
    drawer: bool,
}

struct NoRows;

impl RowSource for NoRows {
    fn len(&self) -> usize {
        0
    }
    fn enabled(&self, _: usize) -> bool {
        false
    }
    fn label(&self, _: usize) -> &str {
        ""
    }
    fn key(&self, i: usize) -> ItemKey {
        ItemKey::Index(i)
    }
}

impl KeyedRowSource for NoRows {
    fn stamp(&self) -> SourceStamp {
        SourceStamp {
            instance: ItemKey::num(0),
            data: 0,
            view: 0,
        }
    }
    fn source_len(&self) -> usize {
        0
    }
    fn source_key(&self, i: usize) -> ItemKey {
        ItemKey::Index(i)
    }
}

/// A query box, keyed results with a preview, and the stale-safe request
/// lifecycle between them. See the [module](super) documentation.
///
/// The widget id is the results focus owner; in [`SearchFocusMode::Combined`]
/// it owns the query too. Selection lives only in the [`SearchSession`]: the
/// list underneath paints a row supplied per frame and keeps no cursor of its
/// own.
pub struct SearchDetail {
    id: WidgetId,
    session: SearchSession,
    list: ListDetail,
    layout_generation: u64,
    layout: Option<LayoutKey>,
    anchors: Vec<Anchor>,
    scratch: Vec<(usize, Rect)>,
    carried: Reply,
}

impl SearchDetail {
    pub fn new(id: WidgetId, query: TextInput, split: Split) -> Self {
        Self {
            id,
            session: SearchSession::new(query, SearchConfig::default()),
            list: ListDetail::new(id, id.sub("detail"), split),
            layout_generation: 0,
            layout: None,
            anchors: Vec::new(),
            scratch: Vec::new(),
            carried: Response::ignored(),
        }
    }

    pub fn config(mut self, config: SearchConfig) -> Self {
        *self.session.config_mut() = config;
        self
    }

    pub fn config_mut(&mut self) -> &mut SearchConfig {
        self.session.config_mut()
    }

    /// The narrowest area that shows the preview beside the results. Below it
    /// the results keep the area and show a summary, and the preview opens as
    /// a drawer. The default is the shared list-detail width.
    pub fn collapse_below(mut self, width: u16) -> Self {
        self.list.set_collapse_below(width);
        self
    }

    /// Sets the short status text at the right of a [`Separate`](SearchFocusMode::Separate)
    /// query, for example the active scope. It never changes the query text.
    pub fn set_query_readout(&mut self, text: &str) {
        self.session.query_input_mut().set_readout(text);
    }

    /// Whether the query is being edited, for a host that suspends its own
    /// single-key shortcuts while text is typed.
    pub fn editing(&self) -> bool {
        self.session.query_input().editing
    }

    /// Ends query editing when focus leaves the query. Other focus changes are
    /// ignored.
    pub fn on_blur(&mut self, change: FocusChange) -> Reply {
        if change.from != self.session.query_input().id {
            return Response::ignored();
        }
        let _ = self.session.query_input_mut().on_blur();
        self.session.edited_query().repaint()
    }

    /// The results focus owner, and the query's too in Combined mode.
    pub fn id(&self) -> WidgetId {
        self.id
    }

    /// The preview pane's focus owner.
    pub fn detail_id(&self) -> WidgetId {
        self.list.detail
    }

    /// The request lifecycle and selection. Read it for phase, error and the
    /// accepted stamp; change it only through this widget.
    pub fn session(&self) -> &SearchSession {
        &self.session
    }

    pub fn query(&self) -> &str {
        self.session.query()
    }

    pub fn selected_key(&self) -> Option<ItemKey> {
        self.session.selected_key()
    }

    pub fn phase(&self) -> SearchPhase {
        self.session.phase()
    }

    pub fn open_detail(&mut self, focus: &mut Focus) {
        self.list.open_detail(focus);
    }

    pub fn close_detail(&mut self, focus: &mut Focus) {
        self.list.close_detail(focus);
    }

    /// Whether the preview has focus.
    pub fn on_detail(&self, focus: Option<WidgetId>) -> bool {
        self.list.on_detail(focus)
    }

    fn query_focus_id(&self) -> WidgetId {
        match self.session.config().focus_mode {
            SearchFocusMode::Combined => self.id,
            SearchFocusMode::Separate => self.session.query_input().id,
        }
    }

    /// Folds in what an earlier context selection held back, and returns the
    /// preview to its top when the selected key changed.
    fn settle(&mut self, reply: Reply) -> Reply {
        let carried = std::mem::take(&mut self.carried);
        let reply = carried.merge(reply);
        if reply
            .actions()
            .any(|a| matches!(a, SearchAction::SelectionChanged(_)))
        {
            self.list.reset_detail();
        }
        reply
    }

    fn drop_anchors(&mut self) {
        self.anchors.clear();
    }

    fn verify<S: SearchSource + ?Sized>(&self, source: &S) -> Result<(), SearchError> {
        if self.session.accepted_stamp().is_none() {
            return Ok(());
        }
        self.session.verify_display(source.stamp())
    }

    // ---- query lifecycle --------------------------------------------------

    pub fn set_query(&mut self, text: &str) -> Reply {
        let reply = self.session.set_query(text);
        self.settle(reply)
    }

    pub fn configure_query(&mut self, configure: impl FnOnce(&mut EditField)) -> Reply {
        let reply = self.session.configure_query(configure);
        self.settle(reply)
    }

    pub fn refresh_source(&mut self, stamp: SourceStamp) -> Reply {
        let reply = self.session.refresh_source(stamp);
        self.settle(reply)
    }

    pub fn retry(&mut self) -> Reply {
        let reply = self.session.retry();
        self.settle(reply)
    }

    /// Accepts the results for `token`. The preview returns to its top when
    /// the selected key changed, when the query changed and
    /// [`SearchConfig::reset_preview_on_query`] is set, or on every accepted
    /// result when [`SearchConfig::preserve_preview_on_same_key`] is off.
    pub fn submit<S: SearchSource + ?Sized>(
        &mut self,
        token: QueryToken,
        source: &S,
    ) -> Result<Reply, SearchError> {
        let before = self.session.accepted_query().map(str::to_owned);
        let reply = self.session.submit(token, source)?;
        let applied = reply.is_consumed();
        if applied {
            self.drop_anchors();
            let config = self.session.config();
            let query_changed = before.as_deref() != self.session.accepted_query();
            if (query_changed && config.reset_preview_on_query)
                || !config.preserve_preview_on_same_key
            {
                self.list.reset_detail();
            }
        }
        Ok(self.settle(reply))
    }

    pub fn fail(&mut self, token: QueryToken, message: String) -> Result<Reply, SearchError> {
        let reply = self.session.fail(token, message)?;
        self.drop_anchors();
        Ok(self.settle(reply))
    }

    // ---- keyboard ---------------------------------------------------------

    /// Routes one key by focus: the preview and its actions, then context
    /// keys, then query and result keys. Focus and detail commands the
    /// session leaves to its host run here.
    pub fn on_key<S: SearchSource + ?Sized>(
        &mut self,
        key: &Key,
        source: &S,
        focus: &mut Focus,
    ) -> Result<Reply, SearchError> {
        self.verify(source)?;
        let current = focus.current();
        let combined = self.session.config().focus_mode == SearchFocusMode::Combined;
        let on_results = current == Some(self.id);
        let on_query = if combined {
            on_results
        } else {
            current == Some(self.session.query_input().id)
        };
        let cmd = self.session.command_for(key);
        let reply = if self.list.on_detail(current) {
            self.detail_key(key, focus)
        } else if on_results && self.keyed_context(key, cmd) {
            self.keyboard_context(key)
        } else if on_results || on_query {
            let page = self.list.master_page();
            let reply = self.session.on_key(key, on_query, page);
            if reply.is_consumed() {
                reply
            } else {
                self.host_key(key, cmd, focus, on_results)
            }
        } else {
            Response::ignored()
        };
        Ok(self.settle(reply))
    }

    fn keyed_context(&self, key: &Key, cmd: Option<SearchCmd>) -> bool {
        cmd == Some(SearchCmd::Context) || self.session.config().context.keys.matches(key)
    }

    fn detail_key(&mut self, key: &Key, focus: &mut Focus) -> Reply {
        let reply = self.list.on_detail_key(key, focus);
        if reply.is_consumed() {
            return self.map_detail(reply);
        }
        let types = key.mods.difference(KeyModifiers::SHIFT).is_empty()
            && matches!(key.code, KeyCode::Char(_));
        if self.session.config().type_to_query_from_detail && types {
            self.list.close_detail(focus);
            focus.focus(self.query_focus_id());
            return self.session.on_key(key, true, self.list.master_page());
        }
        Response::ignored()
    }

    fn map_detail(&mut self, reply: Response<Event>) -> Reply {
        let (outcome, actions) = reply.into_parts();
        let mut out: Reply = outcome.into();
        for event in actions {
            out |= match event {
                Event::Action(name) => self.session.detail_action(name),
                Event::Item(index) => self.session.detail_item(index),
                Event::Row(_) => Response::ignored(),
            };
        }
        out
    }

    fn host_key(
        &mut self,
        key: &Key,
        cmd: Option<SearchCmd>,
        focus: &mut Focus,
        on_results: bool,
    ) -> Reply {
        match cmd {
            Some(SearchCmd::FocusQuery) => {
                focus.focus(self.query_focus_id());
                Response::changed()
            }
            Some(SearchCmd::FocusResults) if !on_results => {
                focus.focus(self.id);
                Response::changed()
            }
            Some(SearchCmd::OpenDetail) if self.session.selected_key().is_some() => {
                self.list.open_detail(focus);
                Response::changed()
            }
            Some(SearchCmd::CloseDetail) => {
                self.list.close_detail(focus);
                Response::changed()
            }
            _ if on_results => self.list.on_detail_key(key, focus).erase().lift(),
            _ => Response::ignored(),
        }
    }

    /// Pastes into the query when it has focus. From the preview it is ignored
    /// unless [`SearchConfig::type_to_query_from_detail`] sends it back.
    pub fn on_paste(&mut self, text: &str, focus: &mut Focus) -> Reply {
        let current = focus.current();
        let query_focused = current == Some(self.query_focus_id());
        let from_detail = self.list.on_detail(current);
        let reply = if from_detail && self.session.config().type_to_query_from_detail {
            self.list.close_detail(focus);
            focus.focus(self.query_focus_id());
            self.session.on_paste(text, true)
        } else {
            self.session.on_paste(text, query_focused)
        };
        self.settle(reply)
    }

    // ---- context ----------------------------------------------------------

    fn keyboard_context(&mut self, key: &Key) -> Reply {
        let config = self.session.config().context;
        let request = config.on_key(self, key).or_else(|| self.context_cursor());
        match request {
            Some(request) => Response::emit(SearchAction::ContextRequested(request)),
            None => self.session.begin_context(),
        }
    }

    /// Emits a keyboard context request that had no anchor when it was asked,
    /// once a paint has placed its key. Call it after rendering. It returns
    /// ignored while nothing is waiting or the key is not on screen yet.
    pub fn emit_pending_context(&mut self) -> Reply {
        let Some(key) = self.session.context_intent() else {
            return Response::ignored();
        };
        let Some(anchor) = self.anchors.iter().find(|a| a.key == key).copied() else {
            return Response::ignored();
        };
        let reply = self.session.emit_context(ContextAnchor::Control {
            id: anchor.id,
            fallback: anchor.rect,
        });
        self.settle(reply)
    }

    fn secondary(&mut self, ev: &PointerEvent) -> Reply {
        let config = self.session.config().context;
        match config.on_secondary(self, ev) {
            Some(request) => Response::emit(SearchAction::ContextRequested(request)),
            None => Response::consumed(),
        }
    }

    // ---- pointer ----------------------------------------------------------

    /// Routes a pointer event: the seam and scrollbars, the query, then result
    /// rows and cells by their stable keys.
    pub fn pointer<S: SearchSource + ?Sized>(
        &mut self,
        ev: &PointerEvent,
        source: &S,
        focus: &mut Focus,
    ) -> Result<Reply, SearchError> {
        self.verify(source)?;
        if let Some(outcome) = self.list.on_pointer(ev) {
            return Ok(outcome.into());
        }
        let reply = self.route_pointer(ev, focus);
        Ok(self.settle(reply))
    }

    fn route_pointer(&mut self, ev: &PointerEvent, focus: &mut Focus) -> Reply {
        let query_id = self.session.query_input().id;
        if ev.on(query_id) {
            return self.query_pointer(ev);
        }
        if ev.is(Phase::Click) && !ev.on(self.id) {
            let (outcome, event) = self.list.click(ev.id, focus, 0);
            return self.map_detail((outcome, event).into());
        }
        if !ev.on(self.id) {
            return Response::ignored();
        }
        if ev.part().part == Part::FIELD {
            focus.focus(self.id);
            return Response::consumed();
        }
        let Some(key) = ev.key_of(Part::CELL).or_else(|| ev.key_of(Part::ROW)) else {
            return Response::ignored();
        };
        let generation = self.layout_generation;
        match ev.phase() {
            Some(Phase::Press) => {
                self.list.close_detail(focus);
                self.session.press(key, generation)
            }
            Some(Phase::Click) => self.session.release(key, generation, ev.count().max(1)),
            Some(Phase::Release) => {
                self.session.cancel_press();
                Response::consumed()
            }
            Some(Phase::Secondary) => {
                self.list.close_detail(focus);
                self.secondary(ev)
            }
            _ => Response::ignored(),
        }
    }

    fn query_pointer(&mut self, ev: &PointerEvent) -> Reply {
        use crate::core::pointer::MouseTarget;
        let routed = self.session.query_input_mut().route(ev);
        let routed = routed.unwrap_or_else(|| self.session.query_input_mut().pointer(ev));
        routed.lift().merge(self.session.edited_query())
    }

    // ---- paint ------------------------------------------------------------

    /// Paints the query and the results with the standard list look. Each row
    /// is its label, detail and meta; a
    /// [`Cells`](super::ResultRowKind::Cells) row shows its label only, so a
    /// source with selectable cells paints them through
    /// [`render_source_with`](Self::render_source_with).
    pub fn render_source<S: SearchDetailSource + ?Sized>(
        &mut self,
        area: Rect,
        source: &S,
        detail: &SourceFrame<'_>,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
    ) -> Result<(), SearchError> {
        let widths = self
            .session
            .accepted_stamp()
            .map(|_| self.list.master_widths(source));
        self.render_source_with(
            area,
            source,
            detail,
            buf,
            ctx,
            |mut row, buf, ctx| {
                if let Some(widths) = widths {
                    paint_standard_row(&mut row, widths, buf, ctx);
                }
            },
            paint_query_default,
        )
    }

    /// Paints with the host's own row and query looks.
    ///
    /// `paint_row` draws one visible row inside the rect it is given. Its hit
    /// region is registered first; a [`Cells`](super::ResultRowKind::Cells)
    /// row reports each cell rect with [`RowPaint::place_cell`], and the widget
    /// registers the hit under the cell's key. `paint_query` draws the query
    /// in [`SearchFocusMode::Combined`]; in
    /// [`Separate`](SearchFocusMode::Separate) mode the query is a
    /// [`TextInput`] and `paint_query` is not called. Both callbacks live for
    /// this call.
    ///
    /// With no accepted results yet the list is empty and `source` is not
    /// read. Otherwise `source` must be the last accepted projection, whether
    /// or not a newer request is pending.
    #[allow(clippy::too_many_arguments)]
    pub fn render_source_with<S: SearchSource + ?Sized>(
        &mut self,
        area: Rect,
        source: &S,
        detail: &SourceFrame<'_>,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        mut paint_row: impl FnMut(RowPaint<'_, S>, &mut Buffer, &mut RenderCtx),
        mut paint_query: impl FnMut(Rect, QueryView<'_>, &mut Buffer, &mut RenderCtx),
    ) -> Result<(), SearchError> {
        let has_rows = self.session.accepted_stamp().is_some();
        if has_rows {
            self.session.verify_display(source.stamp())?;
        }
        let (query_area, body) = self.split(area);
        self.paint_query_area(query_area, buf, ctx, &mut paint_query);

        let selected = self.session.selected_cell();
        let stale = !self.session.is_interactive();
        let Self {
            id,
            list,
            anchors,
            scratch,
            ..
        } = self;
        anchors.clear();
        if has_rows {
            list.render_source_selected_with(
                body,
                source,
                detail,
                selected.map(|(row, _)| row),
                buf,
                ctx,
                |source, master, buf, ctx| {
                    paint_one(
                        *id,
                        source,
                        &master,
                        selected,
                        stale,
                        (anchors, scratch),
                        &mut paint_row,
                        buf,
                        ctx,
                    );
                },
            );
        } else {
            list.render_source_selected_with(
                body,
                &NoRows,
                detail,
                None,
                buf,
                ctx,
                |_, _, _, _| {},
            );
        }
        self.track_layout(area);
        Ok(())
    }

    fn split(&self, area: Rect) -> (Rect, Rect) {
        let rows = match self.session.config().focus_mode {
            SearchFocusMode::Combined => 1,
            SearchFocusMode::Separate => self.session.query_input().height_for(area.width),
        }
        .min(area.height);
        let gap = u16::from(self.session.config().focus_mode == SearchFocusMode::Combined);
        let query = Rect::new(area.x, area.y, area.width, rows);
        let top = rows.saturating_add(gap).min(area.height);
        let body = Rect::new(area.x, area.y + top, area.width, area.height - top);
        (query, body)
    }

    fn paint_query_area(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        paint_query: &mut impl FnMut(Rect, QueryView<'_>, &mut Buffer, &mut RenderCtx),
    ) {
        match self.session.config().focus_mode {
            SearchFocusMode::Combined => {
                ctx.register_part(self.id, PartRef::of(Part::FIELD), area);
                let input = self.session.query_input();
                let buffer = &input.field.buffer;
                paint_query(
                    area,
                    QueryView {
                        text: buffer.text(),
                        placeholder: &input.placeholder,
                        owner: self.id,
                        focused: ctx.interaction.focused(self.id),
                        selection: buffer.selection(),
                        caret_byte: buffer.cursor_offset(),
                    },
                    buf,
                    ctx,
                );
            }
            SearchFocusMode::Separate => {
                let bg = ctx.theme.canvas;
                self.session.query_input_mut().render(area, buf, ctx, bg);
            }
        }
    }

    /// Hit regions move without the source changing when the area, the scroll
    /// offset or the drawer changes. A press captured before such a paint must
    /// not activate after it.
    fn track_layout(&mut self, area: Rect) {
        let key = LayoutKey {
            area,
            offset: self.list.master_offset(),
            drawer: self.list.drawer_open(),
        };
        if self.layout != Some(key) {
            self.layout = Some(key);
            self.layout_generation = self.layout_generation.wrapping_add(1);
            self.session.invalidate_layout(self.layout_generation);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_one<S: SearchSource + ?Sized>(
    owner: WidgetId,
    source: &S,
    master: &MasterRow,
    selected: Option<(usize, usize)>,
    stale: bool,
    (anchors, scratch): (&mut Vec<Anchor>, &mut Vec<(usize, Rect)>),
    paint_row: &mut impl FnMut(RowPaint<'_, S>, &mut Buffer, &mut RenderCtx),
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
) {
    let row = master.index;
    let kind = source.row_kind(row);
    let selected_cell = selected.and_then(|(r, c)| (r == row).then_some(c));
    scratch.clear();
    paint_row(
        RowPaint {
            source,
            row,
            kind,
            rect: master.rect,
            hit: owner.part(PartRef::item(Part::ROW, master.key)),
            selected: master.selected,
            selected_cell,
            focused: master.focused,
            stale,
            owner,
            cells: scratch,
        },
        buf,
        ctx,
    );
    match kind {
        super::ResultRowKind::Item => anchors.push(Anchor {
            key: master.key,
            id: owner.part(PartRef::item(Part::ROW, master.key)),
            rect: master.rect,
        }),
        super::ResultRowKind::Cells => {
            for &(cell, rect) in scratch.iter().filter(|(c, _)| *c < source.cell_count(row)) {
                let key = source.cell_key(row, cell);
                let id = ctx.register_part(owner, PartRef::item(Part::CELL, key), rect);
                anchors.push(Anchor { key, id, rect });
            }
        }
        _ => {}
    }
}

/// The query line [`SearchDetail::render_source`] uses in Combined mode.
pub fn paint_query_default(
    area: Rect,
    query: QueryView<'_>,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
) {
    let t = ctx.theme;
    let state = crate::ui::ctx::VisualState {
        focused: query.focused,
        editing: query.focused,
        ..Default::default()
    };
    let style = t.field_style(state);
    fill(buf, area, style);
    let bg = style.bg.unwrap_or(t.field);
    buf.set_string(
        area.x,
        area.y,
        t.gutter_symbol(state),
        t.gutter(state, bg, false),
    );
    let room = area.width.saturating_sub(3) as usize;
    let (text, text_style) = if query.text.is_empty() {
        (query.placeholder, style.fg(t.text_muted))
    } else {
        (query.text, style)
    };
    buf.set_string(area.x + 2, area.y, truncate(text, room), text_style);
}

fn paint_standard_row<S: SearchDetailSource + ?Sized>(
    row: &mut RowPaint<'_, S>,
    widths: MasterWidths,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
) {
    let mut data = row.source.row(row.row);
    data.faint |= row.stale || matches!(row.kind, super::ResultRowKind::Heading);
    paint_master_row(
        buf,
        ctx,
        row.hit,
        row.rect,
        data,
        row.selected,
        row.focused,
        widths,
    );
    if row.kind == super::ResultRowKind::Cells {
        paint_cells(row, widths.label as u16, buf, ctx);
    }
}

/// The cells of a [`Cells`](super::ResultRowKind::Cells) row as chips after the
/// row label. A chip that does not fit is left out and not placed.
fn paint_cells<S: SearchSource + ?Sized>(
    row: &mut RowPaint<'_, S>,
    label_w: u16,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
) {
    let t = ctx.theme;
    let bg = t.canvas;
    let right = row.rect.right().saturating_sub(1);
    let mut x = row.rect.x + 4 + label_w + 2;
    for cell in 0..row.source.cell_count(row.row) {
        let text = format!(" {} ", row.source.cell_label(row.row, cell));
        let w = crate::ui::text::width(&text) as u16;
        if x + w > right {
            break;
        }
        let mut state = ctx.state(row.cell_hit(cell));
        state.selected = row.selected_cell == Some(cell);
        state.focused = row.focused && state.selected;
        state.disabled = !row.source.cell_enabled(row.row, cell);
        let rect = Rect::new(x, row.rect.y, w, 1);
        let style = t.row(state, bg);
        fill(buf, rect, style);
        let text_style = if row.stale {
            style.fg(t.text_muted)
        } else if state.selected {
            style.fg(t.accent)
        } else {
            style
        };
        buf.set_string(x, rect.y, &text, text_style);
        legible_on_fill(buf, rect, t, bg);
        row.place_cell(cell, rect);
        x += w + 1;
    }
}

impl ContextSource for SearchDetail {
    type Target = ItemKey;

    /// The enabled item or cell under the pointer, only while the displayed
    /// results answer the newest request.
    fn context_target(&self, ev: &PointerEvent) -> Option<ItemKey> {
        if !ev.on(self.id) || !self.session.is_interactive() {
            return None;
        }
        let key = ev.key_of(Part::CELL).or_else(|| ev.key_of(Part::ROW))?;
        self.session.is_selectable(key).then_some(key)
    }

    /// Moves the selection to `target`. The [`SearchAction::SelectionChanged`]
    /// it causes is held and comes back with the next response of this widget.
    fn context_select(&mut self, target: ItemKey) {
        let reply = self.session.select(target);
        let reply = self.settle(reply);
        self.carried = reply;
    }

    /// The selection anchored to its painted control. `None` until a paint has
    /// placed the key: a stale position is never turned into an anchor.
    fn context_cursor(&self) -> Option<ContextRequest<ItemKey>> {
        if !self.session.is_interactive() {
            return None;
        }
        let key = self.session.selected_key()?;
        let anchor = self.anchors.iter().find(|a| a.key == key)?;
        Some(ContextRequest {
            target: key,
            anchor: ContextAnchor::Control {
                id: anchor.id,
                fallback: anchor.rect,
            },
        })
    }
}
