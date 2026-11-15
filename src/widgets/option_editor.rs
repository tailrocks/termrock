use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::core::event::{Key, Outcome};
use crate::core::focus::BlurAction;
use crate::core::hit::Axis;
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keymap::{Nav, NavKeys, unassigned_chord};
use crate::core::list::{Cursor, KeyStep, Moved, RowSource};
use crate::core::selection::{RowSelection, SelectGesture};
use crate::theme::{CursorStyle, Tone};
use crate::ui::ctx::{RenderCtx, fill};
use crate::ui::popup::{Dismiss, Place, Placement, backdrop_id, place_with, surface_for};
use crate::ui::text::{truncate, width};
use crate::widgets::edit::EditField;
use crate::widgets::option_list::{OptionList, WheelMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionItem {
    pub value: String,

    pub label: String,

    pub tone: Option<Tone>,

    pub description: Option<String>,

    pub disabled: Option<String>,
}

impl OptionItem {
    pub fn new(value: &str) -> Self {
        Self {
            value: value.to_owned(),
            label: value.to_owned(),
            tone: None,
            description: None,
            disabled: None,
        }
    }
    pub fn label(mut self, label: &str) -> Self {
        self.label = label.to_owned();
        self
    }
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }
    pub fn description(mut self, text: &str) -> Self {
        self.description = Some(text.to_owned());
        self
    }
    pub fn disabled(mut self, reason: &str) -> Self {
        self.disabled = Some(reason.to_owned());
        self
    }
    pub fn is_disabled(&self) -> bool {
        self.disabled.is_some()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options(pub Vec<OptionItem>);

impl Options {
    pub fn new(items: impl IntoIterator<Item = OptionItem>) -> Self {
        Self(items.into_iter().collect())
    }

    pub fn values(values: &[&str]) -> Self {
        Self(values.iter().map(|v| OptionItem::new(v)).collect())
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn get(&self, i: usize) -> Option<&OptionItem> {
        self.0.get(i)
    }

    pub fn find(&self, value: &str) -> Option<usize> {
        self.0.iter().position(|o| o.value == value)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ColumnEditor {
    #[default]
    Text,

    Select(Options),

    Combo(Options),

    Radio(Options),

    Checkbox(Options),

    Segmented(Options),

    Tags(Options),
}

impl ColumnEditor {
    pub fn options(&self) -> Option<&Options> {
        match self {
            Self::Text => None,
            Self::Select(o)
            | Self::Combo(o)
            | Self::Radio(o)
            | Self::Checkbox(o)
            | Self::Segmented(o)
            | Self::Tags(o) => Some(o),
        }
    }

    pub fn is_constrained(&self) -> bool {
        self.options().is_some()
    }

    pub fn has_popup(&self) -> bool {
        self.is_constrained() && !matches!(self, Self::Checkbox(_))
    }

    pub fn for_options(options: Options) -> Self {
        match options.len() {
            0..=2 => Self::Checkbox(options),
            3..=5 => Self::Segmented(options),
            6..=15 => Self::Select(options),
            _ => Self::Combo(options),
        }
    }

    pub fn checked(&self, raw: &str) -> Option<bool> {
        let Self::Checkbox(opts) = self else {
            return None;
        };
        match opts.find(raw)? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }

    fn parts<'a>(&self, raw: &'a str, cfg: &EditorConfig) -> Vec<&'a str> {
        match self {
            Self::Tags(_) => raw
                .split(cfg.tag_separator.trim())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect(),
            _ => vec![raw],
        }
    }

    pub fn display(&self, raw: &str, cfg: &EditorConfig) -> String {
        let Some(opts) = self.options() else {
            return raw.to_owned();
        };
        let labels: Vec<&str> = self
            .parts(raw, cfg)
            .into_iter()
            .map(|p| opts.find(p).map_or(p, |i| opts.0[i].label.as_str()))
            .collect();
        labels.join(cfg.tag_separator)
    }

    pub fn out_of_set(&self, raw: &str, cfg: &EditorConfig) -> bool {
        let Some(opts) = self.options() else {
            return false;
        };
        if raw.trim().is_empty() {
            return false;
        }
        self.parts(raw, cfg).iter().any(|p| opts.find(p).is_none())
    }

    pub fn tone(&self, raw: &str) -> Option<Tone> {
        let opts = self.options()?;
        if matches!(self, Self::Tags(_)) {
            return None;
        }
        opts.find(raw).and_then(|i| opts.0[i].tone)
    }

    pub fn toggled(&self, raw: &str) -> Option<String> {
        let Self::Checkbox(opts) = self else {
            return None;
        };
        let enabled: Vec<&OptionItem> = opts.0.iter().filter(|o| !o.is_disabled()).collect();
        match opts.find(raw) {
            Some(i) => enabled
                .iter()
                .find(|o| o.value != opts.0[i].value)
                .map(|o| o.value.clone()),
            None => enabled.first().map(|o| o.value.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorConfig {
    pub dismiss: Dismiss,

    pub blur: BlurAction,

    pub max_rows: u16,

    pub popup_width: (u16, u16),

    pub cursor_style: CursorStyle,

    pub type_ahead: bool,

    pub type_ahead_timeout: std::time::Duration,

    pub nav: NavKeys,

    pub affordance: Affordance,

    pub flag_out_of_set: bool,

    pub tag_separator: &'static str,

    pub placement: Placement,

    pub wheel: WheelMode,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            dismiss: Dismiss::LIGHT,
            blur: BlurAction::Cancel,
            max_rows: 8,
            popup_width: (14, 48),
            cursor_style: CursorStyle::Fill,
            type_ahead: true,
            type_ahead_timeout: crate::core::typeahead::TypeAhead::DEFAULT_TIMEOUT,
            nav: NavKeys::default(),
            affordance: Affordance::Focused,
            flag_out_of_set: true,
            tag_separator: ", ",
            placement: Placement::Below,
            wheel: WheelMode::Viewport,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Affordance {
    #[default]
    Focused,

    Always,

    Never,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChoiceResult {
    Commit(String),

    Cancel,

    CommitAndTab { value: String, backward: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Select,
    Combo,
    Radio,
    Segmented,
    Tags,
}

#[derive(Debug, Clone)]
pub struct ChoiceEditor {
    pub id: WidgetId,
    kind: Kind,
    options: Options,

    pub cursor: usize,

    pub current: Option<usize>,

    pub selection: RowSelection,

    pub query: EditField,

    pub error: Option<String>,

    pub list: OptionList,
    typeahead: crate::core::typeahead::TypeAhead,
    shown_cache: Option<ShownCache>,
    content_w: Option<u16>,
    strip_w: Option<u16>,
}

impl ChoiceEditor {
    pub fn open(
        id: WidgetId,
        editor: &ColumnEditor,
        raw: &str,
        cfg: &EditorConfig,
    ) -> Option<Self> {
        let kind = match editor {
            ColumnEditor::Select(_) => Kind::Select,
            ColumnEditor::Combo(_) => Kind::Combo,
            ColumnEditor::Radio(_) => Kind::Radio,
            ColumnEditor::Segmented(_) => Kind::Segmented,
            ColumnEditor::Tags(_) => Kind::Tags,
            ColumnEditor::Text | ColumnEditor::Checkbox(_) => return None,
        };
        let options = editor.options()?.clone();
        let current = options.find(raw);
        let checked = if kind == Kind::Tags {
            let mut s = RowSelection::new();
            for i in editor
                .parts(raw, cfg)
                .iter()
                .filter_map(|p| options.find(p))
            {
                s.set(i, true);
            }
            s
        } else {
            RowSelection::new()
        };
        let first_enabled = options.0.iter().position(|o| !o.is_disabled()).unwrap_or(0);
        let cursor = current
            .filter(|&i| !options.0[i].is_disabled())
            .or_else(|| checked.iter().next())
            .unwrap_or(first_enabled);
        Some(Self {
            id,
            kind,
            options,
            cursor,
            current,
            selection: checked,
            query: EditField::single(""),
            error: None,
            list: Self::list(id, cfg),
            typeahead: crate::core::typeahead::TypeAhead::with_timeout(cfg.type_ahead_timeout),
            shown_cache: None,
            content_w: None,
            strip_w: None,
        })
    }

    /// The options this editor was opened with. They never change while it is
    /// open, which keeps the cached filter and widths valid.
    pub fn options(&self) -> &Options {
        &self.options
    }

    fn list(id: WidgetId, cfg: &EditorConfig) -> OptionList {
        let mut list = OptionList::new(id)
            .dismiss(cfg.dismiss)
            .placement(cfg.placement)
            .max_rows(cfg.max_rows)
            .width(cfg.popup_width.0, cfg.popup_width.1)
            .look(cfg.cursor_style)
            .nav(cfg.nav)
            .type_ahead(cfg.type_ahead)
            .wheel_mode(cfg.wheel);

        list.scroll.v.context = 0;
        list.scroll.v.viewport_len = cfg.max_rows as usize;
        list
    }

    pub fn option_id(&self, i: usize) -> WidgetId {
        self.id.part(Self::option_part(i))
    }

    fn option_part(i: usize) -> PartRef {
        PartRef::item(Part::ROW, ItemKey::index(i))
    }

    fn option_of(&self, id: WidgetId) -> Option<usize> {
        self.id.item_of(id, Part::ROW, 0..self.options.len())
    }

    pub(crate) fn contains_id(&self, id: WidgetId) -> bool {
        id == self.id || id == backdrop_id(self.id) || self.option_of(id).is_some()
    }

    pub fn shown(&self) -> Vec<usize> {
        self.filter(&self.query.text().trim().to_lowercase())
    }

    fn enabled(&self, i: usize) -> bool {
        self.options.get(i).is_some_and(|o| !o.is_disabled())
    }

    fn shown_rows(&self) -> Shown<'_> {
        Shown {
            ed: self,
            rows: self.shown(),
        }
    }

    fn value(&self, cfg: &EditorConfig) -> Option<String> {
        if self.kind == Kind::Tags {
            return Some(
                self.selection
                    .iter()
                    .map(|i| self.options.0[i].value.as_str())
                    .collect::<Vec<_>>()
                    .join(cfg.tag_separator),
            );
        }
        if !self.enabled(self.cursor) || !self.shown().contains(&self.cursor) {
            return None;
        }
        Some(self.options.0[self.cursor].value.clone())
    }

    pub fn blur_value(&self, cfg: &EditorConfig) -> Option<String> {
        self.value(cfg)
    }

    fn toggle_checked(&mut self, i: usize) {
        let order: Vec<usize> = (0..self.options.0.len()).collect();
        let options = &self.options;
        self.selection.apply(SelectGesture::Toggle, i, &order, |k| {
            !options.0[k].is_disabled()
        });
    }

    pub fn on_key(&mut self, key: &Key, cfg: &EditorConfig) -> (Outcome, Option<ChoiceResult>) {
        if self.kind == Kind::Combo {
            use crate::widgets::field_common::EditAction;
            let action = self.query.action(key);
            if !matches!(
                action,
                EditAction::Commit | EditAction::Cancel | EditAction::Tab { .. } | EditAction::None
            ) {
                if self.query.apply(action, key).edited {
                    self.after_query();
                }
                return (Outcome::Changed, None);
            }
        }
        if unassigned_chord::<()>(key, &[]) {
            return (Outcome::Consumed, None);
        }
        let combo = self.kind == Kind::Combo;
        match key.code {
            KeyCode::Esc if cfg.dismiss.escape => {
                return (Outcome::Changed, Some(ChoiceResult::Cancel));
            }
            KeyCode::Esc => return (Outcome::Consumed, None),
            KeyCode::Enter => {
                return match self.value(cfg) {
                    Some(v) => (Outcome::Changed, Some(ChoiceResult::Commit(v))),
                    None => (Outcome::Consumed, None),
                };
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let backward = key.code == KeyCode::BackTab;
                return match self.value(cfg) {
                    Some(value) => (
                        Outcome::Changed,
                        Some(ChoiceResult::CommitAndTab { value, backward }),
                    ),
                    None => (Outcome::Consumed, None),
                };
            }
            KeyCode::Char(' ') if self.kind == Kind::Tags => {
                self.toggle_checked(self.cursor);
                return (Outcome::Changed, None);
            }
            KeyCode::Char(' ') if self.kind != Kind::Combo => {
                return match self.value(cfg) {
                    Some(v) => (Outcome::Changed, Some(ChoiceResult::Commit(v))),
                    None => (Outcome::Consumed, None),
                };
            }
            _ => {}
        }
        let step = KeyStep::of(key, &cfg.nav, cfg.type_ahead && !combo);
        let mut ta = std::mem::take(&mut self.typeahead);
        let shown = self.shown_rows();
        let at = shown.rows.iter().position(|&i| i == self.cursor);

        let axis = if self.kind == Kind::Segmented {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        let mut cursor = Cursor::new(at.unwrap_or(0)).axis(axis);
        let moved = match step {
            Some(KeyStep::Type(c)) => cursor.type_ahead(c, &mut ta, &shown),
            Some(KeyStep::Nav(nav)) => {
                let nav = match nav {
                    Nav::Up if axis == Axis::Horizontal => Nav::Left,
                    Nav::Down if axis == Axis::Horizontal => Nav::Right,
                    n => n,
                };
                match (at, nav) {
                    (None, Nav::Up | Nav::Down | Nav::Left | Nav::Right) => cursor.first(&shown),
                    _ => cursor.apply(nav, &shown, self.list.scroll.viewport_len),
                }
            }
            None => Moved::Ignored,
        };
        let to = moved.index().map(|k| shown.rows[k]);
        self.typeahead = ta;
        match (step, moved, to) {
            (Some(KeyStep::Type(_)), Moved::Stayed(_), _) => (Outcome::Consumed, None),
            (_, _, Some(i)) => {
                self.cursor = i;
                (Outcome::Changed, None)
            }
            _ => (Outcome::Consumed, None),
        }
    }

    fn after_query(&mut self) {
        self.error = None;
        if let Some(&i) = self.shown().iter().find(|&&i| self.enabled(i)) {
            self.cursor = i;
        }
        self.list.scroll.offset = 0;
    }

    pub fn on_paste(&mut self, text: &str) -> Outcome {
        if self.kind != Kind::Combo {
            return Outcome::Consumed;
        }
        if self.query.paste(text.trim()) {
            self.after_query();
        }
        Outcome::Changed
    }

    pub fn on_click(&mut self, id: WidgetId) -> (Outcome, Option<ChoiceResult>) {
        if id == backdrop_id(self.id) {
            return if self.list.dismiss.outside_click {
                (Outcome::Changed, Some(ChoiceResult::Cancel))
            } else {
                (Outcome::Consumed, None)
            };
        }
        let Some(i) = self.option_of(id) else {
            return (Outcome::Consumed, None);
        };
        if !self.enabled(i) {
            return (Outcome::Consumed, None);
        }
        self.cursor = i;
        if self.kind == Kind::Tags {
            self.toggle_checked(i);
            return (Outcome::Changed, None);
        }
        (
            Outcome::Changed,
            Some(ChoiceResult::Commit(self.options.0[i].value.clone())),
        )
    }

    fn step(&mut self, delta: isize) {
        let shown = self.shown_rows();
        let at = shown.rows.iter().position(|&i| i == self.cursor);
        let mut cursor = Cursor::new(at.unwrap_or(0));
        let mut moved = match at {
            None => cursor.first(&shown),
            Some(k) => Moved::Stayed(k),
        };
        if at.is_some() {
            for _ in 0..delta.unsigned_abs() {
                moved = cursor.step(delta.signum(), &shown);
            }
        }
        if let Some(k) = moved.index() {
            self.cursor = shown.rows[k];
        }
    }

    pub fn wheel(&mut self, axis: Axis, delta: i32) -> crate::core::response::Response<()> {
        use crate::core::response::Response;
        use crate::core::scroll::Scrollable;
        if self.kind == Kind::Segmented || axis == Axis::Horizontal {
            return Response::consumed();
        }
        if self.list.wheel == WheelMode::Viewport {
            return self.list.wheel(axis, delta);
        }
        let before = self.cursor;
        self.step(delta as isize);
        if before == self.cursor {
            Response::consumed()
        } else {
            Response::changed()
        }
    }

    pub fn render(&mut self, cell: Rect, buf: &Buffer, ctx: &mut RenderCtx, cfg: &EditorConfig) {
        if self.kind == Kind::Segmented {
            self.render_strip(cell, buf, ctx, cfg);
        } else {
            self.render_list(cell, buf, ctx, cfg);
        }
    }

    fn prefix(&self) -> u16 {
        match self.kind {
            Kind::Radio | Kind::Tags => 6,
            _ => 3,
        }
    }

    fn render_list(&mut self, cell: Rect, buf: &Buffer, ctx: &mut RenderCtx, cfg: &EditorConfig) {
        let screen = *buf.area();
        let search = u16::from(self.kind == Kind::Combo);
        let shown_len = self.shown_cached().len();
        let rows = (shown_len.max(1) as u16).min(cfg.max_rows.max(1));
        let content_w = self.content_width().saturating_add(self.prefix() + 7);
        let (min_w, max_w) = cfg.popup_width;
        let w = content_w.max(cell.width + 2).clamp(min_w, max_w);
        let pa = place_with(
            screen,
            cell,
            w,
            rows + 2 + search,
            cfg.placement,
            Place::DEFAULT,
        );
        let visible = pa.height.saturating_sub(2 + search).max(1) as usize;
        let cursor = self.cursor;
        let at = self.shown_cached().binary_search(&cursor).unwrap_or(0);

        self.list.scroll.set_content(shown_len);
        self.list.scroll.set_viewport(visible);
        self.list.scroll.follow(at);
        let inner = pa
            .intersection(screen)
            .inner(ratatui::layout::Margin::new(1, 1));
        if search > 0 {
            let line = Rect::new(inner.x + 3, inner.y, inner.width.saturating_sub(3), 1);
            let mut scratch = crate::ui::ctx::scratch_buffer(line);
            self.query
                .draw_line(&mut scratch, line, &Self::query_look(ctx.theme));
        }
        let snapshot = self.list_snapshot(inner.height.saturating_sub(search) as usize, search);
        let cfg = cfg.clone();
        ctx.overlay(move |buf, ctx| snapshot.draw(pa, &cfg, buf, ctx));
    }

    fn list_snapshot(&mut self, list_height: usize, search: u16) -> ChoiceListSnapshot {
        let offset = self.list.scroll.offset;
        self.shown_cached();
        let shown = &self.shown_cache.as_ref().expect("filled above").rows;
        let rows: Vec<ChoiceRow> = shown
            .iter()
            .skip(offset)
            .take(list_height)
            .map(|&i| self.choice_row(i))
            .collect();
        #[cfg(test)]
        crate::core::work::add(crate::core::work::Counter::SnapshotRows, rows.len());
        ChoiceListSnapshot {
            id: self.id,
            kind: self.kind,
            search,
            query: (search > 0).then(|| self.query.clone()),
            empty: shown.is_empty(),
            more_above: offset > 0,
            more_below: offset + list_height < shown.len(),
            rows,
        }
    }

    fn choice_row(&self, i: usize) -> ChoiceRow {
        let o = &self.options.0[i];
        ChoiceRow {
            index: i,
            label: o.label.clone(),
            note: o
                .disabled
                .as_deref()
                .or(o.description.as_deref())
                .map(str::to_owned),
            tone: o.tone,
            disabled: o.is_disabled(),
            focused: i == self.cursor,
            current: self.current == Some(i),
            checked: self.selection.contains(i),
        }
    }

    fn shown_cached(&mut self) -> &[usize] {
        let query = self.query.text().trim().to_lowercase();
        if self.shown_cache.as_ref().is_none_or(|c| c.query != query) {
            #[cfg(test)]
            crate::core::work::add(crate::core::work::Counter::SourceRows, self.options.len());
            let rows = self.filter(&query);
            self.shown_cache = Some(ShownCache { query, rows });
        }
        &self.shown_cache.as_ref().expect("filled above").rows
    }

    fn content_width(&mut self) -> u16 {
        *self.content_w.get_or_insert_with(|| {
            #[cfg(test)]
            crate::core::work::add(crate::core::work::Counter::WidthRows, self.options.len());
            let widest = self
                .options
                .0
                .iter()
                .map(|o| {
                    let extra = o
                        .disabled
                        .as_deref()
                        .or(o.description.as_deref())
                        .map_or(0, |d| width(d) + 2);
                    width(&o.label) + extra
                })
                .max()
                .unwrap_or(0);
            u16::try_from(widest).unwrap_or(u16::MAX)
        })
    }

    fn strip_width(&mut self) -> u16 {
        *self.strip_w.get_or_insert_with(|| {
            #[cfg(test)]
            crate::core::work::add(crate::core::work::Counter::WidthRows, self.options.len());
            let labels: usize = self.options.0.iter().map(|o| width(&o.label) + 2).sum();
            let separators = self.options.len().saturating_sub(1);
            u16::try_from(labels + separators + 2).unwrap_or(u16::MAX)
        })
    }

    fn query_look(t: &crate::theme::Theme) -> crate::widgets::edit::LineLook {
        let fs = t.field_style(crate::ui::ctx::VisualState {
            editing: true,
            ..Default::default()
        });
        crate::widgets::edit::LineLook {
            text: fs,
            selection: t.selection(),
            ellipsis: fs.fg(t.text_muted),
            ellipsis_glyph: t.glyphs.ellipsis,
            underline: None,
            follow_caret: true,
        }
    }

    fn render_strip(&mut self, cell: Rect, buf: &Buffer, ctx: &mut RenderCtx, cfg: &EditorConfig) {
        let screen = *buf.area();
        let w = self.strip_width();
        let pa = place_with(
            screen,
            cell,
            w.max(cell.width),
            3,
            cfg.placement,
            Place::DEFAULT,
        );
        let inner = pa
            .intersection(screen)
            .inner(ratatui::layout::Margin::new(1, 1));
        let mut segments = Vec::new();
        let mut x = inner.x;
        for (i, o) in self.options.0.iter().enumerate() {
            let sw = width(&o.label) as u16 + 2;
            if x + sw > inner.right() {
                break;
            }
            segments.push(self.choice_row(i));
            x += sw + 1;
        }
        #[cfg(test)]
        crate::core::work::add(crate::core::work::Counter::SnapshotRows, segments.len());
        let snapshot = ChoiceStripSnapshot {
            id: self.id,
            total: self.options.len(),
            segments,
        };
        let cfg = cfg.clone();
        ctx.overlay(move |buf, ctx| snapshot.draw(pa, &cfg, buf, ctx));
    }

    fn filter(&self, query: &str) -> Vec<usize> {
        (0..self.options.len())
            .filter(|&i| {
                query.is_empty()
                    || self.options.0[i].label.to_lowercase().contains(query)
                    || self.options.0[i].value.to_lowercase().contains(query)
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
struct ShownCache {
    query: String,
    rows: Vec<usize>,
}

#[derive(Debug, Clone)]
struct ChoiceRow {
    index: usize,
    label: String,
    note: Option<String>,
    tone: Option<Tone>,
    disabled: bool,
    focused: bool,
    current: bool,
    checked: bool,
}

/// What an open list popup paints: the visible rows and the query field.
struct ChoiceListSnapshot {
    id: WidgetId,
    kind: Kind,
    search: u16,
    query: Option<EditField>,
    empty: bool,
    more_above: bool,
    more_below: bool,
    rows: Vec<ChoiceRow>,
}

/// What an open segmented popup paints: the segments that fit.
struct ChoiceStripSnapshot {
    id: WidgetId,
    total: usize,
    segments: Vec<ChoiceRow>,
}

fn option_part(i: usize) -> PartRef {
    PartRef::item(Part::ROW, ItemKey::index(i))
}

impl ChoiceListSnapshot {
    fn option_id(&self, i: usize) -> WidgetId {
        self.id.part(option_part(i))
    }

    fn draw(self, pa: Rect, cfg: &EditorConfig, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let bg = t.surface_elevated;
        let inner = surface_for(pa, buf, ctx, t, Some((self.id, cfg.dismiss)));
        ctx.scrollable(self.id, inner);
        if self.search > 0 && inner.height > 0 {
            let q = Rect::new(inner.x, inner.y, inner.width, 1);
            let fs = t.field_style(crate::ui::ctx::VisualState {
                editing: true,
                ..Default::default()
            });
            fill(buf, q, fs);
            buf.set_string(q.x + 1, q.y, "⌕", fs.fg(t.text_muted));

            let line = Rect::new(q.x + 3, q.y, q.width.saturating_sub(3), 1);
            let mut query = self.query.clone().expect("search row has a query");
            let caret = if query.text().is_empty() {
                buf.set_string(
                    line.x,
                    line.y,
                    "Search",
                    fs.patch(t.placeholder(Default::default())),
                );
                line.as_position()
            } else {
                query.draw_line(buf, line, &ChoiceEditor::query_look(t))
            };
            ctx.set_cursor(caret);

            ctx.register_part(self.id, PartRef::of(Part::QUERY), line);
        }
        let list = Rect::new(
            inner.x,
            inner.y + self.search,
            inner.width,
            inner.height.saturating_sub(self.search),
        );
        if self.empty {
            buf.set_string(
                list.x + 3,
                list.y,
                "No matches",
                Style::new().fg(t.text_muted).bg(bg),
            );
            return;
        }
        for (row_i, o) in self.rows.iter().take(list.height as usize).enumerate() {
            let i = o.index;
            let y = list.y + row_i as u16;
            let rid = self.option_id(i);
            let mut rs = ctx.state(rid);
            rs.focused = o.focused;
            rs.disabled = o.disabled;
            if rs.disabled {
                rs.hovered = false;
                rs.pressed = false;
            }
            rs.selected = o.current;
            let cr = t.cursor_row(rs, bg, cfg.cursor_style);
            let st = cr.style;
            let row = Rect::new(list.x, y, list.width, 1);
            fill(buf, row, st);
            buf.set_string(row.x, y, cr.gutter, cr.gutter_style);
            let ink = if rs.disabled {
                t.disabled
            } else {
                t.row_ink(&cr, t.text_secondary)
            };
            let mut x = row.x + 1;
            match self.kind {
                Kind::Radio => {
                    let g = if o.current {
                        t.glyphs.radio_on
                    } else {
                        t.glyphs.radio_off
                    };
                    buf.set_string(x, y, g, st.fg(ink));
                    x += width(g) as u16 + 1;
                }
                Kind::Tags => {
                    let g = if o.checked {
                        t.glyphs.check_on
                    } else {
                        t.glyphs.check_off
                    };
                    buf.set_string(x, y, g, st.fg(ink));
                    x += width(g) as u16 + 1;
                }
                _ => {
                    if o.current {
                        buf.set_string(x, y, t.glyphs.current, st.fg(ink));
                    }
                    x += 2;
                }
            }
            let note = o.note.as_deref();
            let note_w = note.map_or(0, |n| width(n) as u16 + 2);
            let avail = row.right().saturating_sub(x + 1);
            let label_w = if note_w > 0 && avail > note_w + 4 {
                avail - note_w
            } else {
                avail
            };
            let mut ls = if rs.disabled { st.fg(t.disabled) } else { st };
            if let Some(tone) = o.tone
                && !rs.disabled
                && !cr.on_fill
            {
                ls = ls.fg(t.tone(tone));
            }
            buf.set_string(x, y, truncate(&o.label, label_w as usize), ls);
            if let Some(n) = note
                && label_w < avail
            {
                let ns = st.fg(if rs.disabled {
                    t.disabled
                } else {
                    t.row_ink(&cr, t.text_muted)
                });
                let nx = row.right().saturating_sub(note_w - 1);
                buf.set_string(nx, y, truncate(n, (note_w - 2) as usize), ns);
            }
            if !rs.disabled {
                ctx.register_part(self.id, option_part(i), row);
            }
        }
        let edge = Style::new().fg(t.text_secondary).bg(bg);
        let x = pa.right().saturating_sub(3);
        if self.more_above {
            buf.set_string(x, pa.y, t.glyphs.more_above, edge);
        }
        if self.more_below {
            buf.set_string(x, pa.bottom().saturating_sub(1), t.glyphs.more_below, edge);
        }
    }
}

impl ChoiceStripSnapshot {
    fn draw(self, pa: Rect, cfg: &EditorConfig, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let sep = 1u16;
        let t = ctx.theme;
        let bg = t.surface_elevated;
        let inner = surface_for(pa, buf, ctx, t, Some((self.id, cfg.dismiss)));
        let mut x = inner.x;
        for o in &self.segments {
            let i = o.index;
            let sw = width(&o.label) as u16 + 2;
            if x + sw > inner.right() {
                break;
            }
            let rid = self.id.part(option_part(i));
            let mut rs = ctx.state(rid);
            rs.focused = o.focused;
            rs.disabled = o.disabled;
            if rs.disabled {
                rs.hovered = false;
                rs.pressed = false;
            }
            let cr = t.cursor_row(rs, bg, cfg.cursor_style);
            let seg = Rect::new(x, inner.y, sw, 1);
            fill(buf, seg, cr.style);
            let mut ls = cr.style;
            if rs.disabled {
                ls = ls.fg(t.disabled);
            } else if let Some(tone) = o.tone
                && !cr.on_fill
            {
                ls = ls.fg(t.tone(tone));
            }
            if o.current {
                ls = ls.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
            }
            buf.set_string(x + 1, inner.y, &o.label, ls);
            if !rs.disabled {
                ctx.register_part(self.id, option_part(i), seg);
            }
            x += sw;
            if i + 1 < self.total && x < inner.right() {
                buf.set_string(
                    x,
                    inner.y,
                    t.glyphs.divider,
                    Style::new().fg(t.border_strong).bg(bg),
                );
                x += sep;
            }
        }
    }
}

impl crate::core::pointer::MouseTarget for ChoiceEditor {
    type Action = ChoiceResult;

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<ChoiceResult> {
        use crate::core::intent::Phase;
        use crate::core::response::Response;
        if let Some((axis, delta)) = ev.wheel() {
            if ev.part().part == Part::BACKDROP {
                return Response::ignored();
            }
            return self.wheel(axis, delta).lift();
        }
        if self.kind == Kind::Combo
            && let Some(r) = crate::widgets::option_list::query_pointer(&mut self.query, ev)
        {
            return r.lift();
        }
        match ev.phase() {
            Some(Phase::Click) => match self.on_click(ev.id) {
                (_, Some(r)) => Response::emit(r),
                (o, None) => o.into(),
            },
            Some(Phase::Press | Phase::Drag | Phase::Secondary) => Response::consumed(),
            _ => Response::ignored(),
        }
    }
}

struct Shown<'a> {
    ed: &'a ChoiceEditor,
    rows: Vec<usize>,
}

impl RowSource for Shown<'_> {
    fn len(&self) -> usize {
        self.rows.len()
    }
    fn enabled(&self, k: usize) -> bool {
        self.ed.enabled(self.rows[k])
    }
    fn label(&self, k: usize) -> &str {
        &self.ed.options.0[self.rows[k]].label
    }
}
