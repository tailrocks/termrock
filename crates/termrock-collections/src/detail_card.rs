use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::props::{self, Prop};
use termrock_controls::action_row::ActionRow;
use termrock_controls::button::Button;
use termrock_feedback::empty::{self, EmptyState};
use termrock_runtime::ctx::RenderCtx;
use termrock_runtime::measure::terminal_extent;
use termrock_viewport::panel::Panel;

#[derive(Debug, Clone)]
pub struct DetailCard {
    pub title: String,

    pub meta: Option<String>,

    pub props: Vec<Prop>,

    pub actions: ActionRow,

    pub max_width: Option<u16>,

    pub max_height: Option<u16>,

    pub reserve: u16,

    pub empty: Option<EmptyState>,

    pub focused: bool,

    pub fit: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetailLayout {
    pub card: Rect,

    pub bg: Color,

    pub props_h: u16,

    pub slot: Rect,

    pub actions: Vec<Rect>,
}

impl DetailCard {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            meta: None,
            props: Vec::new(),
            actions: ActionRow::new(),
            max_width: None,
            max_height: None,
            reserve: 3,
            empty: None,
            focused: false,
            fit: false,
        }
    }

    pub fn fit(mut self, on: bool) -> Self {
        self.fit = on;
        self
    }

    pub fn empty(state: EmptyState) -> Self {
        Self {
            empty: Some(state),
            ..Self::new("")
        }
    }

    pub fn meta(mut self, meta: &str) -> Self {
        self.meta = Some(meta.to_owned());
        self
    }

    pub fn props(mut self, props: Vec<Prop>) -> Self {
        self.props = props;
        self
    }

    pub fn actions(mut self, row: ActionRow) -> Self {
        self.actions = row;
        self
    }

    pub fn max_size(mut self, width: Option<u16>, height: Option<u16>) -> Self {
        self.max_width = width;
        self.max_height = height;
        self
    }

    pub fn reserve(mut self, rows: u16) -> Self {
        self.reserve = rows;
        self
    }

    pub fn focused(mut self, on: bool) -> Self {
        self.focused = on;
        self
    }

    pub fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
        buttons: &mut [&mut Button],
    ) -> DetailLayout {
        let t = ctx.theme;
        if let Some(e) = &self.empty {
            empty::render(area, buf, t, e, bg);
            return DetailLayout {
                bg,
                ..DetailLayout::default()
            };
        }
        let mut card = Rect::new(
            area.x,
            area.y,
            self.max_width.map_or(area.width, |w| area.width.min(w)),
            self.max_height.map_or(area.height, |h| area.height.min(h)),
        );
        let mut panel = Panel::card(Some(&self.title)).focused(self.focused);
        if let Some(m) = &self.meta {
            panel = panel.meta(m);
        }
        if self.fit {
            let mut scratch = termrock_runtime::ctx::scratch_buffer(card);
            let probe = panel.render(card, &mut scratch, t);
            let chrome = card.height.saturating_sub(probe.height);
            let need = usize::from(chrome)
                .saturating_add(usize::from(props::measure(&self.props, probe.width)))
                .saturating_add(usize::from(self.reserve));
            card.height = card.height.min(terminal_extent(need));
        }
        let cbg = panel.bg(t);
        let inner = panel.render(card, buf, t);
        let props_h = props::render(
            Rect::new(
                inner.x,
                inner.y,
                inner.width,
                inner.height.saturating_sub(self.reserve),
            ),
            buf,
            t,
            &self.props,
            cbg,
        );
        let actions_y = inner.bottom().saturating_sub(1);
        let slot_y = (inner.y + props_h + 1).min(actions_y);
        let slot = Rect::new(inner.x, slot_y, inner.width, actions_y - slot_y);
        let actions = if buttons.is_empty() || inner.height == 0 {
            Vec::new()
        } else {
            self.actions.render(
                Rect::new(inner.x, actions_y, inner.width, 1),
                buf,
                ctx,
                cbg,
                buttons,
            )
        };
        DetailLayout {
            card,
            bg: cbg,
            props_h,
            slot,
            actions,
        }
    }
}
