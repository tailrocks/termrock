use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::core::event::{Key, Outcome};
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keymap::NavKeys;
use crate::core::list::{Cursor, Navigable, RowSource};
use crate::core::scroll::{ScrollPane, Scrollable};
use crate::ui::ctx::{RenderCtx, VisualState};
use crate::ui::list_core::ListCore;
use crate::ui::row::{RowLook, RowUi};
use crate::ui::specimen::{Specimen, SpecimenState};
use crate::ui::text::{paint_clipped, paint_truncated};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum StepState {
    #[default]
    Queued,
    Running,
    Done,
    Skipped,
    Failed,
    Blocked,
}

impl StepState {
    pub fn label(self) -> &'static str {
        match self {
            StepState::Queued => "queued",
            StepState::Running => "running",
            StepState::Done => "done",
            StepState::Skipped => "skipped",
            StepState::Failed => "failed",
            StepState::Blocked => "blocked",
        }
    }

    pub fn terminal(self) -> bool {
        matches!(
            self,
            StepState::Done | StepState::Skipped | StepState::Failed
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub label: String,
    pub state: StepState,

    pub meta: Option<String>,
}

impl Step {
    fn shown_meta(&self) -> Option<String> {
        match (&self.meta, self.state) {
            (Some(m), _) => Some(m.clone()),
            (None, StepState::Queued) => Some("queued".into()),
            (None, StepState::Skipped) => Some("skipped".into()),
            (None, StepState::Blocked) => Some("blocked".into()),
            _ => None,
        }
    }

    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            state: StepState::Queued,
            meta: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StepRail {
    pub id: WidgetId,
    pub steps: Vec<Step>,
    pub selectable: bool,

    pub core: ListCore,
    pub area: Rect,

    pub numbered: bool,
}

impl std::ops::Deref for StepRail {
    type Target = ListCore;
    fn deref(&self) -> &ListCore {
        &self.core
    }
}

impl std::ops::DerefMut for StepRail {
    fn deref_mut(&mut self) -> &mut ListCore {
        &mut self.core
    }
}

impl StepRail {
    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.core.nav = nav;
        self
    }

    pub fn new(id: WidgetId, steps: Vec<Step>) -> Self {
        let n = steps.len();
        Self {
            id,
            steps,
            selectable: false,
            core: ListCore::new(n),
            area: Rect::ZERO,
            numbered: true,
        }
    }

    pub fn content(mut self, content: crate::ui::content::ContentState) -> Self {
        self.core.content = content;
        self
    }

    pub fn track_click(mut self, click: crate::core::scroll::TrackClick) -> Self {
        self.core.scroll.track_click = Some(click);
        self
    }

    pub fn selectable(mut self, on: bool) -> Self {
        self.selectable = on;
        self
    }

    pub fn set_state(&mut self, i: usize, state: StepState) {
        if let Some(s) = self.steps.get_mut(i) {
            s.state = state;
        }
    }

    pub fn set_meta(&mut self, i: usize, meta: Option<String>) {
        if let Some(s) = self.steps.get_mut(i) {
            s.meta = meta;
        }
    }

    pub fn frontier(&self) -> Option<usize> {
        self.steps.iter().position(|s| !s.state.terminal())
    }

    pub fn counts(&self) -> (usize, usize, usize) {
        let c = |st| self.steps.iter().filter(|s| s.state == st).count();
        (
            c(StepState::Done),
            c(StepState::Skipped),
            c(StepState::Failed),
        )
    }

    pub fn failed(&self) -> Option<usize> {
        self.steps.iter().position(|s| s.state == StepState::Failed)
    }

    pub fn row_id(&self, i: usize) -> WidgetId {
        self.id.part(Self::row_part(i))
    }

    fn row_part(i: usize) -> PartRef {
        PartRef::item(Part::ROW, ItemKey::index(i))
    }

    fn set_cursor(&mut self, i: usize) {
        self.core.cursor = i.min(self.steps.len().saturating_sub(1));
        self.core.scroll.ensure_visible(self.core.cursor);
    }

    pub fn on_key(&mut self, key: &Key) -> Outcome {
        if !self.selectable || self.steps.is_empty() {
            return Outcome::Ignored;
        }
        if !key.plain() {
            return Outcome::Ignored;
        }
        let Some(nav) = self.core.nav.action(key) else {
            return Outcome::Ignored;
        };
        match self
            .cursor()
            .apply(nav, self, self.core.scroll.viewport_len)
            .index()
        {
            Some(i) => {
                self.set_cursor(i);
                Outcome::Changed
            }
            None => Outcome::Ignored,
        }
    }

    pub fn on_click(&mut self, row: usize) -> Outcome {
        if !self.selectable || row >= self.steps.len() {
            return Outcome::Consumed;
        }
        self.core.scroll.pointer_move();
        self.set_cursor(row);
        Outcome::Changed
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        self.core.scroll.begin(ctx.track_click);
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }
        self.area = area;
        let t = ctx.theme;
        let focused = self.selectable && ctx.interaction.focused(self.id);
        self.core.scroll.set_content(self.steps.len());
        self.core.scroll.set_viewport(area.height as usize);
        let n = self.steps.len();

        if self.selectable && self.core.is_focus_stop(n, focused) {
            ctx.control(self.id, area, false);
        }
        ctx.scrollable(self.id, area);
        if (!self.core.content.is_ready() || self.core.empty.is_some())
            && self.core.draw_content(n, area, buf, ctx, bg, None)
        {
            return;
        }
        let has_sb = self.core.scroll.overflows();
        let row_w = area.width.saturating_sub(u16::from(has_sb));
        let frontier = self.frontier();
        for (k, i) in self.core.scroll.visible_range().enumerate() {
            let y = area.y + k as u16;
            let step = &self.steps[i];
            let rid = self.row_id(i);

            let mut s = if self.selectable {
                ctx.child_state(self.id, rid, i == self.core.cursor)
            } else {
                VisualState::default()
            };
            s.focused = focused && i == self.core.cursor;
            let row = Rect::new(area.x, y, row_w, 1);
            let st = RowUi::paint(ctx, buf, row, s, RowLook::Bar, bg).style();

            let plane = st.bg.unwrap_or(bg);
            let mark = |ink| st.fg(t.legible_mark(ink, plane));
            let (glyph, gstyle) = match step.state {
                StepState::Queued | StepState::Skipped | StepState::Blocked => (" ", st),
                StepState::Running => {
                    (t.glyphs.spinner_frame(ctx.interaction.tick), mark(t.accent))
                }
                StepState::Done => (t.glyphs.done, mark(t.success)),
                StepState::Failed => ("!", mark(t.error).add_modifier(Modifier::BOLD)),
            };
            paint_clipped(buf, row, 1, glyph, gstyle);
            let mut off = 3usize;
            if self.numbered {
                let num = format!("{:02}", i + 1);
                paint_clipped(
                    buf,
                    row,
                    off,
                    &num,
                    st.fg(if step.state == StepState::Running {
                        t.text_secondary
                    } else {
                        t.text_faint
                    })
                    .remove_modifier(Modifier::BOLD),
                );
                off += crate::ui::text::width(&num) + 1;
            }
            let label_style = match step.state {
                StepState::Queued => st.fg(t.text_muted),
                StepState::Running => st.fg(t.text_primary).add_modifier(Modifier::BOLD),
                StepState::Done => st.fg(t.text_secondary),
                StepState::Skipped => st.fg(t.text_faint),
                StepState::Failed => st.fg(t.error).add_modifier(Modifier::BOLD),
                StepState::Blocked => st.fg(t.text_secondary),
            };
            let label_style = if s.focused && !s.pressed {
                label_style.add_modifier(Modifier::BOLD)
            } else {
                label_style
            };
            let meta = step.shown_meta();
            let meta_style = match step.state {
                StepState::Running => st.fg(t.text_secondary),
                StepState::Failed => st.fg(t.error),
                StepState::Blocked => st.fg(t.warning),
                _ => st.fg(t.text_faint),
            }
            .remove_modifier(Modifier::BOLD);
            let avail = usize::from(row.width).saturating_sub(off + 1);
            let mw = meta
                .as_ref()
                .map(|m| crate::ui::text::width(m))
                .unwrap_or(0);
            let show_meta = mw > 0 && avail >= mw + 12;
            let lw = if show_meta { avail - mw - 2 } else { avail };
            let label_room = Rect::new(
                row.x.saturating_add(u16::try_from(off).unwrap_or(u16::MAX)),
                y,
                u16::try_from(lw).unwrap_or(u16::MAX),
                1,
            )
            .intersection(row);
            paint_truncated(buf, label_room, 0, &step.label, label_style);
            if show_meta && let Some(m) = &meta {
                let meta_room =
                    Rect::new(row.right().saturating_sub(mw as u16 + 1), y, mw as u16, 1)
                        .intersection(row);
                paint_clipped(buf, meta_room, 0, m, meta_style);
            }
            if self.selectable {
                ctx.register_part(self.id, Self::row_part(i), row);
            }
            let _ = frontier;
        }
        if has_sb {
            crate::ui::fade::scroll_edges_except(
                buf,
                ctx,
                Rect::new(
                    area.x,
                    area.y,
                    (area.right() - 1).saturating_sub(area.x),
                    area.height,
                ),
                &self.core.scroll,
                &self
                    .scroll
                    .visible_range()
                    .position(|i| i == self.core.cursor)
                    .map(|k| area.y + k as u16)
                    .into_iter()
                    .collect::<Vec<_>>(),
            );
            let sb = Rect::new(area.right() - 1, area.y, 1, area.height);
            self.core.scroll.draw_v(sb, buf, ctx, self.id, focused);
        }
        let _ = Style::new();
    }
}

impl crate::ui::measure::Measure for StepRail {
    fn measure(&self, _avail: ratatui::layout::Size, _ctx: &RenderCtx) -> ratatui::layout::Size {
        let lead = 3 + if self.numbered { 3 } else { 0 };
        let width = self
            .steps
            .iter()
            .map(|step| {
                let label = crate::ui::text::width(&step.label);
                let need = match step.shown_meta() {
                    Some(m) => {
                        let mw = crate::ui::text::width(&m);
                        label
                            .saturating_add(mw)
                            .saturating_add(2)
                            .max(mw.saturating_add(12))
                    }
                    None => label,
                };
                need.saturating_add(lead + 1)
            })
            .max()
            .unwrap_or(0);
        ratatui::layout::Size::new(
            crate::ui::measure::terminal_extent(width),
            crate::ui::measure::terminal_extent(self.steps.len()),
        )
    }
}

impl Scrollable for StepRail {
    fn pane(&mut self) -> &mut ScrollPane {
        &mut self.core.scroll
    }
}

impl RowSource for StepRail {
    fn len(&self) -> usize {
        self.steps.len()
    }
    fn label(&self, i: usize) -> &str {
        &self.steps[i].label
    }
}

impl Navigable for StepRail {
    fn cursor(&self) -> Cursor {
        self.core.cursor_model()
    }
}

impl crate::core::pointer::MouseTarget for StepRail {
    type Action = ();

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<()> {
        use crate::core::intent::Phase;
        if let Some(r) = crate::core::pointer::scroll_input(self, ev) {
            return r;
        }
        match ev.index_of(Part::ROW) {
            Some(row) if ev.is(Phase::Click) => self.on_click(row).into(),
            _ => crate::core::response::Response::ignored(),
        }
    }
}

impl Specimen for StepRail {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        match state {
            Rest | Focused | Narrow => true,

            Selected => self.selectable,
            _ => false,
        }
    }

    fn specimen_height(&self, _state: SpecimenState) -> u16 {
        self.steps.len() as u16
    }

    fn specimen(
        &self,
        state: SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        let mut r = self.clone();
        if state == SpecimenState::Selected {
            r.cursor = 1.min(r.steps.len().saturating_sub(1));
        }
        r.render(area, buf, ctx, bg);
    }
}
