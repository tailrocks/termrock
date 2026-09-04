// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! TermRock-only catalog pages: remaining public visual components after
//! the frozen source prefix, painted with Junie card grammar.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use termrock::registry::PublicUiId;
use termrock::runtime::FrameTick;
use termrock::style::{JunieTheme, MotionPolicy};
use termrock::widgets::{
    Badge, Callout, EmptyKind, EmptyState, Kbd, Label, ProgressBar, ProgressKind, ProgressStatus,
    SemanticStatus, Separator, Skeleton, Spinner, SpinnerState, StatusIndicator,
};

use crate::catalog::PageId;
use crate::coverage::extras_on;
use crate::ctx::RenderCtx;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};

/// One extras page: a column of Junie cards, one per owned public UI id.
pub struct ExtrasPage {
    title: &'static str,
    blurb: &'static str,
    page: PageId,
}

impl ExtrasPage {
    #[must_use]
    pub fn feedback() -> Self {
        Self {
            title: "Feedback",
            blurb: "Alerts, toasts, badges, loading and status",
            page: PageId::FEEDBACK,
        }
    }
    #[must_use]
    pub fn overlays() -> Self {
        Self {
            title: "Overlays",
            blurb: "Drawers, popovers, menus and layered surfaces",
            page: PageId::OVERLAYS,
        }
    }
    #[must_use]
    pub fn charts() -> Self {
        Self {
            title: "Charts",
            blurb: "Series, meters, timelines",
            page: PageId::CHARTS,
        }
    }
    #[must_use]
    pub fn structure() -> Self {
        Self {
            title: "Structure",
            blurb: "Layout, inspectors, remaining public widgets",
            page: PageId::STRUCTURE,
        }
    }
}

impl Page for ExtrasPage {
    fn title(&self) -> &'static str {
        self.title
    }
    fn blurb(&self) -> &'static str {
        self.blurb
    }
    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let mut y = area.y;
        for id in extras_on(self.page) {
            if y.saturating_add(4) > area.bottom() {
                break;
            }
            let slot = Rect::new(
                area.x,
                y,
                area.width,
                4.min(area.bottom().saturating_sub(y)),
            );
            let (inner, bg) = layout::card(slot, buf, t, Some(id.as_str()), None, false);
            paint_demo(*id, inner, buf, ctx, t, bg);
            y = y.saturating_add(5);
        }
    }
    fn handle(&mut self, _ev: &PageEvent, _cx: &mut PageCtx<'_>) -> Route {
        Route::Ignored
    }
    fn hints(&self, _focus: Option<crate::id::WidgetId>) -> Vec<Hint> {
        vec![("↑ ↓", "Scroll"), ("Tab", "Focus")]
    }
}

fn paint_demo(
    id: PublicUiId,
    inner: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx<'_>,
    t: &JunieTheme,
    bg: ratatui::style::Color,
) {
    if inner.is_empty() {
        return;
    }
    let system = ctx.system;
    match id {
        PublicUiId::Badge => {
            Badge::new("live", system).paint(inner, buf, None);
        }
        PublicUiId::Callout => {
            Callout::new("Notice", system)
                .description("A callout is a titled status band.")
                .paint(inner, buf);
        }
        PublicUiId::EmptyState => {
            EmptyState::new("Nothing here", system)
                .kind(EmptyKind::NoData)
                .paint(inner, buf);
        }
        PublicUiId::Skeleton => {
            Skeleton::new(inner.height.min(3), system).paint(inner, buf);
        }
        PublicUiId::Separator => {
            Separator::new(system).paint(inner, buf);
        }
        PublicUiId::StatusIndicator => {
            StatusIndicator::new(SemanticStatus::Success, system).paint(inner, buf);
        }
        PublicUiId::Spinner => {
            let st = SpinnerState::new();
            let tick = FrameTick::manual(
                termrock::runtime::Instant::now(),
                std::time::Duration::from_millis(0),
                std::time::Duration::from_millis(80),
            );
            Spinner::labeled("Working", system).paint(inner, buf, &st, tick, MotionPolicy::Full);
        }
        PublicUiId::ProgressBar => {
            ProgressBar::new(ProgressKind::Determinate { fraction: 0.4 }, system)
                .status(ProgressStatus::Running)
                .paint(inner, buf);
        }
        PublicUiId::Kbd => {
            Kbd::new("⌘K", system).paint(inner, buf);
        }
        PublicUiId::Label => {
            Label::<()>::new("Label", system).paint(inner, buf);
        }
        _ => {
            buf.set_string(
                inner.x,
                inner.y,
                id.as_str(),
                Style::new().fg(t.text_secondary).bg(bg),
            );
        }
    }
}
