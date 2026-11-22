mod board;
use crate::screens::Jackin;
use termrock::core::event::{Key, Outcome};
use termrock::core::hit::Axis;
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::pointer::PointerEvent;
use termrock::core::scroll::{ScrollState, TrackClick};
use termrock::shell::Page;
use termrock::theme::{StatusKind, Tone};
use termrock::ui::columns::{self, Col};
use termrock::ui::ctx::{RenderCtx, fill, legible_on_fill};
use termrock::ui::text::{fit, truncate, width};
use termrock::widgets::empty::{self, EmptyState};
use termrock::widgets::keyhint::{Hint, hint};
use termrock::widgets::panel::Panel;
use termrock::widgets::progress::{Meter, MeterVisual, spinner_frame};

use super::accounts::QuotaTone;
use super::row_meta::ClockStamp;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use termrock::widgets::scrollbar;
use termrock::widgets::viewport::{Span, render_spans};

use super::{Cx, Go, Screen};
use crate::domain::account::{Account, AccountId, IssueCode, Lifecycle};
use crate::domain::agent::UsageSurface;
use crate::domain::usage::{Freshness, OverallSummary, QuotaStatus};
use crate::world::{Msg, World};
use termrock::ui::text::plural;

enum OvLine {
    Text(Vec<Span>),

    Provider {
        name: String,
        accounts: usize,
        window: Option<(String, u8, QuotaTone)>,
        status: (Option<StatusKind>, String),
    },
}

#[allow(clippy::too_many_arguments)]
fn draw_provider_row(
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    body: Rect,
    y: u16,
    name: &str,
    accounts: usize,
    window: &Option<(String, u8, QuotaTone)>,
    status: &(Option<StatusKind>, String),
    bg: ratatui::style::Color,
) {
    let t = ctx.theme;
    let right = body.right();
    let put = |buf: &mut Buffer, x: u16, w: u16, text: &str, st: Style| {
        if x < right {
            let w = w.min(right - x);
            buf.set_string(
                x,
                y,
                fit(&truncate(text, w as usize), w as usize),
                st.bg(bg),
            );
        }
    };
    put(buf, body.x, 12, name, t.primary());
    put(buf, body.x + 13, 9, &accounts.to_string(), t.secondary());
    let wx = body.x + 23;
    match window {
        Some((label, pct, tone)) => {
            put(buf, wx, 10, label, t.secondary());
            if wx + 11 + 15 <= right {
                tone.on(Meter::new(Some(*pct)))
                    .value(format!("{pct:>3}%"))
                    .visual(MeterVisual::Block)
                    .render(Rect::new(wx + 11, y, 15, 1), buf, ctx, bg);
            }
        }
        None => put(buf, wx, 26, "—", t.muted()),
    }
    let sx = body.x + 50;
    if sx >= right {
        return;
    }
    let (kind, word) = status;
    match kind {
        Some(kind) => {
            let (glyph, gs) = t.status(*kind);
            put(buf, sx, 2, glyph, gs);
            put(
                buf,
                sx + 2,
                right.saturating_sub(sx + 2),
                word,
                t.status_label(*kind),
            );
        }
        None => put(buf, sx, right - sx, word, t.muted()),
    }
}

pub const LIST: WidgetId = WidgetId::of("usage.list");
pub const DETAIL: WidgetId = WidgetId::of("usage.detail");

#[derive(Debug, Clone, PartialEq, Eq)]
enum Row {
    Overview,
    Heading(UsageSurface),
    Account(AccountId),
}

type DetailLine = (String, Tone, Option<(u8, QuotaTone)>);

#[derive(Default)]
pub struct UsageScreen {
    board: board::Board,
    pub selected: Option<AccountId>,
    all_usage: bool,
    rows: Vec<Row>,
    scroll: ScrollState,
    detail_scroll: ScrollState,

    pub show_detail: bool,
    pub refreshing: bool,
    clock: ClockStamp,
}

impl UsageScreen {
    pub fn select(&mut self, id: Option<AccountId>) {
        self.selected = id;
        self.all_usage = false;
        self.board.deactivate();
    }

    pub fn select_all(&mut self) {
        self.selected = None;
        self.all_usage = true;
        self.board.deactivate();
    }

    fn build_rows(&mut self, w: &World) {
        let mut rows = vec![Row::Overview];
        for s in UsageSurface::ALL {
            let accounts: Vec<&Account> = w.surface_accounts(s);
            if accounts.is_empty() && s != UsageSurface::Unsupported {
                continue;
            }
            rows.push(Row::Heading(s));
            for a in accounts {
                rows.push(Row::Account(a.id.clone()));
            }
        }
        self.rows = rows;
        if let Some(id) = &self.selected
            && !self
                .rows
                .iter()
                .any(|r| matches!(r, Row::Account(x) if x == id))
        {
            self.selected = None;
        }
        self.scroll.set_content(self.rows.len());
    }

    fn cursor(&self) -> usize {
        match &self.selected {
            Some(id) => self
                .rows
                .iter()
                .position(|r| matches!(r, Row::Account(x) if x == id))
                .unwrap_or(0),
            None => 0,
        }
    }

    fn move_cursor(&mut self, delta: isize) {
        let n = self.rows.len();
        if n == 0 {
            return;
        }
        let mut i = self.cursor() as isize;
        loop {
            i = (i + delta).clamp(0, n as isize - 1);
            match &self.rows[i as usize] {
                Row::Heading(_) => {
                    if i == 0 || i == n as isize - 1 {
                        break;
                    }
                    continue;
                }
                Row::Overview => {
                    self.selected = None;
                    break;
                }
                Row::Account(id) => {
                    self.selected = Some(id.clone());
                    break;
                }
            }
        }
        self.scroll.ensure_visible(self.cursor());
        self.detail_scroll.jump_start();
    }

    fn refresh(&mut self, w: &mut World, cx: &mut Cx) {
        let mut n = 0;
        let ids: Vec<AccountId> = w
            .accounts
            .accounts
            .iter()
            .filter(|a| a.enabled)
            .map(|a| a.id.clone())
            .collect();
        for (i, id) in ids.iter().enumerate() {
            if w.refresh_account(id, i as i64 * 120).is_some() {
                n += 1;
            }
        }
        if n == 0 {
            cx.status("Nothing to refresh");
        } else {
            self.refreshing = true;
            cx.status(format!(
                "Reloading the broker projection · {}",
                plural(n, "account", "accounts")
            ));
        }
    }

    fn account_lines(a: &Account, w: &World) -> Vec<DetailLine> {
        let mut v: Vec<DetailLine> = vec![];
        v.push((
            format!(
                "Provider     {} · surface {}",
                a.provider.label(),
                a.surface.surface_name()
            ),
            Tone::Normal,
            None,
        ));
        v.push((
            format!(
                "Account      {} · {}",
                a.identity.label(),
                a.identity.plan.clone().unwrap_or("plan unknown".into())
            ),
            if a.identity.subject.is_some() {
                Tone::Normal
            } else {
                Tone::Muted
            },
            None,
        ));
        v.push((
            format!(
                "Credential   {} · {}",
                a.source.origin_label(),
                a.source.safe_detail()
            ),
            Tone::Normal,
            None,
        ));
        let status = match a.usage.freshness.phase {
            Freshness::Current => format!(
                "{} · current · refreshed {}",
                a.lifecycle.label(),
                a.last_refresh_secs
                    .map(|s| w.clock.ago(s))
                    .unwrap_or("never".into())
            ),
            Freshness::Stale => format!(
                "{} · stale · last good {}",
                a.lifecycle.label(),
                a.usage
                    .freshness
                    .last_good_secs
                    .map(|s| w.clock.ago(s))
                    .unwrap_or("?".into())
            ),
            Freshness::Refreshing => format!("{} · refreshing…", a.lifecycle.label()),
            Freshness::Failed => format!(
                "{} · error: {}",
                a.lifecycle.label(),
                a.issue
                    .as_ref()
                    .map(|i| w.surfaces.vocabulary.reason(&i.reason))
                    .unwrap_or("refresh failed".into())
            ),
        };
        v.push((
            format!("Status       {status}"),
            match a.usage.freshness.phase {
                Freshness::Failed => Tone::Error,
                Freshness::Stale => Tone::Warning,
                _ => Tone::Normal,
            },
            None,
        ));
        v.push((String::new(), Tone::Normal, None));
        v.push(("Limits".into(), Tone::Secondary, None));
        if a.usage.windows.is_empty() {
            v.push((
                match a.lifecycle {
                    Lifecycle::NeedsLogin => {
                        "  needs login · no quota until the agent signs in".into()
                    }
                    Lifecycle::NeedsSecret => {
                        "  needs secret · no quota until a key is present".into()
                    }
                    Lifecycle::Unsupported => {
                        "  unsupported · this provider exposes no usage".into()
                    }
                    _ => "  not started".into(),
                },
                Tone::Muted,
                None,
            ));
        }
        for win in &a.usage.windows {
            if win.has_meter() {
                let tone = super::accounts::meter_tone(win, a);
                let mut text = format!("  {}", win.label);
                let rest = super::accounts::meter_detail(win, w);
                text.push('\u{1}');
                text.push_str(&rest);
                v.push((text, Tone::Normal, Some((win.used_pct.unwrap_or(0), tone))));
            } else {
                let value = win.value_label();
                let text = if value.to_lowercase().starts_with(&win.label.to_lowercase()) {
                    format!("  {value}")
                } else {
                    format!("  {}   {value}", win.label)
                };
                v.push((
                    text,
                    if win.status == QuotaStatus::Error {
                        Tone::Error
                    } else {
                        Tone::Muted
                    },
                    None,
                ));
            }
        }
        if a.usage.freshness.phase != Freshness::Current && !a.usage.windows.is_empty() {
            v.push((String::new(), Tone::Normal, None));
            v.push((
                format!(
                    "Last good    kept from {}{}",
                    a.usage
                        .freshness
                        .last_good_secs
                        .map(|s| w.clock.ago(s))
                        .unwrap_or("?".into()),
                    a.issue
                        .as_ref()
                        .filter(|i| i.code == IssueCode::QuotaUnsupported)
                        .map(|_| " · quota not visible")
                        .unwrap_or("")
                ),
                Tone::Muted,
                None,
            ));
        }
        v
    }

    fn overview_lines(w: &World) -> Vec<OvLine> {
        let s = OverallSummary::compute(&w.accounts.accounts);
        let text = |s: String, tone: Tone| OvLine::Text(vec![Span::new(s, tone)]);
        let mut health = vec![Span::new("Health       ", Tone::Normal)];
        health.extend(super::health_spans(&s));
        let mut v = vec![
            OvLine::Text(health),
            text(
                format!(
                    "Freshness    {} of {} current · broker projection {}",
                    s.counts.enabled - s.counts.stale - s.counts.failed - s.counts.refreshing,
                    s.counts.enabled,
                    w.clock.ago(w.last_refresh_secs)
                ),
                Tone::Normal,
            ),
            text(format!("Registry     {}", s.counts_line()), Tone::Normal),
            text(String::new(), Tone::Normal),
            text(
                format!(
                    "{:<12} {:<9} {:<26} {}",
                    "Provider", "Accounts", "Worst window", "Status"
                ),
                Tone::Muted,
            ),
        ];
        for surface in UsageSurface::ALL {
            let accounts: Vec<&Account> = w
                .accounts
                .accounts
                .iter()
                .filter(|a| a.surface == surface && a.enabled)
                .collect();
            if surface == UsageSurface::Unsupported {
                v.push(text(
                    format!(
                        "{:<12} {:<9} {:<26} {}",
                        "—", "—", "—", "unsupported sentinel"
                    ),
                    Tone::Muted,
                ));
                continue;
            }
            if accounts.is_empty() {
                v.push(text(
                    format!(
                        "{:<12} {:<9} {:<26} {}",
                        surface.label(),
                        "—",
                        "—",
                        "not discovered"
                    ),
                    Tone::Muted,
                ));
                continue;
            }
            let worst = accounts
                .iter()
                .flat_map(|a| a.usage.windows.iter().map(move |w| (a, w)))
                .filter(|(_, w)| w.used_pct.is_some())
                .max_by_key(|(_, w)| w.used_pct.unwrap_or(0));

            let (window, status) = match worst {
                Some((a, win)) => {
                    let st = match (win.status, a.usage.freshness.phase) {
                        (QuotaStatus::Exhausted, _) => {
                            (Some(StatusKind::Fail), "exhausted".to_owned())
                        }
                        (_, Freshness::Failed) => (
                            Some(StatusKind::Fail),
                            a.issue
                                .as_ref()
                                .map(|i| {
                                    w.surfaces
                                        .vocabulary
                                        .reason(&i.reason)
                                        .split(':')
                                        .next()
                                        .unwrap_or("error")
                                        .to_lowercase()
                                })
                                .unwrap_or("error".into()),
                        ),
                        (QuotaStatus::Warning, _) => (Some(StatusKind::Warn), "warning".to_owned()),
                        (_, Freshness::Stale) => (Some(StatusKind::Warn), "stale".to_owned()),
                        _ => (Some(StatusKind::Ok), "current".to_owned()),
                    };
                    (
                        Some((
                            win.label.clone(),
                            win.used_pct.unwrap_or(0),
                            super::accounts::meter_tone(win, a),
                        )),
                        st,
                    )
                }
                None => (None, (None, accounts[0].status_word().to_owned())),
            };
            v.push(OvLine::Provider {
                name: surface.label().to_owned(),
                accounts: accounts.len(),
                window,
                status,
            });
        }
        let unresolved: Vec<&Account> = w
            .accounts
            .accounts
            .iter()
            .filter(|a| a.enabled && a.identity.subject.is_none())
            .collect();
        if !unresolved.is_empty() {
            v.push(text(String::new(), Tone::Normal));
            for a in unresolved {
                v.push(text(
                    format!(
                        "Unresolved   {} · {} ({}) — not an authenticated account",
                        a.surface.label(),
                        a.source.safe_detail(),
                        a.confidence.label()
                    ),
                    Tone::Muted,
                ));
            }
        }
        v.push(text(String::new(), Tone::Normal));
        v.push(text("Rollups sum identical windows and units only; a provider with mixed windows shows its worst window.".into(), Tone::Faint));
        if !s.comparable.is_empty() {
            v.push(text(String::new(), Tone::Normal));
            for c in &s.comparable {
                v.push(text(
                    format!(
                        "Comparable   {} · {} · {} · {}–{}% remaining",
                        c.surface.label(),
                        c.label,
                        plural(c.accounts, "account", "accounts"),
                        c.min_remaining_pct,
                        c.max_remaining_pct
                    ),
                    Tone::Secondary,
                ));
            }
        }
        v
    }

    fn draw_list(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let bg = t.canvas;
        let focused = ctx.interaction.focused(LIST);
        self.scroll.set_viewport(area.height as usize);
        self.scroll.follow(self.cursor());
        ctx.control(LIST, area, false);
        ctx.scrollable(LIST, area);
        let has_sb = self.scroll.overflows();
        let row_w = area.width.saturating_sub(u16::from(has_sb));
        let cur = self.cursor();
        for (k, i) in self.scroll.visible_range().enumerate() {
            let y = area.y + k as u16;
            let rid = LIST.child(i);
            match &self.rows[i] {
                Row::Heading(s) => {
                    buf.set_string(area.x + 2, y, s.label(), t.muted().bg(bg));
                }
                Row::Overview | Row::Account(_) => {
                    let mut st8 = ctx.state(rid);
                    st8.focused = focused && i == cur;
                    st8.selected = i == cur;
                    let st = t.row(st8, bg);
                    let r = Rect::new(area.x, y, row_w, 1);
                    fill(buf, r, st);
                    buf.set_string(
                        r.x,
                        y,
                        t.gutter_symbol(st8),
                        t.gutter(st8, st.bg.unwrap_or(bg), false),
                    );
                    if st8.selected {
                        buf.set_string(
                            r.x + 1,
                            y,
                            "›",
                            st.fg(if focused { t.accent } else { t.text_secondary }),
                        );
                    }
                    match &self.rows[i] {
                        Row::Overview => buf.set_string(r.x + 3, y, "Overview", st),
                        Row::Account(id) => {
                            let Some(a) = w.accounts.get(id) else {
                                continue;
                            };
                            let mut label = a.display_name.clone();
                            if a.default_for_provider {
                                label.push_str("  ★");
                            }
                            let health = if !a.enabled {
                                ""
                            } else if a.is_error_state()
                                || matches!(a.usage.worst_status(), Some(QuotaStatus::Exhausted))
                            {
                                "!"
                            } else if matches!(a.usage.worst_status(), Some(QuotaStatus::Warning))
                                || a.usage.freshness.phase == Freshness::Stale
                            {
                                "▲"
                            } else {
                                ""
                            };
                            let meta = if a.usage.freshness.phase == Freshness::Refreshing {
                                spinner_frame(w.now_ms() as u64 / 80).to_owned()
                            } else if !a.enabled {
                                "disabled".into()
                            } else if a.lifecycle != Lifecycle::Available {
                                a.lifecycle.label().to_owned()
                            } else {
                                String::new()
                            };
                            let mw = width(&meta) as u16 + if health.is_empty() { 0 } else { 2 };
                            let lw = r.width.saturating_sub(5 + mw + 2) as usize;
                            buf.set_string(
                                r.x + 5,
                                y,
                                fit(&truncate(&label, lw), lw),
                                if a.enabled { st } else { st.fg(t.text_muted) },
                            );
                            let mut rx = r.right().saturating_sub(1);
                            if !meta.is_empty() {
                                rx = rx.saturating_sub(width(&meta) as u16);
                                buf.set_string(
                                    rx,
                                    y,
                                    &meta,
                                    st.fg(t.text_muted).remove_modifier(Modifier::BOLD),
                                );
                                rx = rx.saturating_sub(1);
                            }
                            if !health.is_empty() {
                                rx = rx.saturating_sub(1);
                                buf.set_string(
                                    rx,
                                    y,
                                    health,
                                    st.fg(if health == "!" { t.error } else { t.warning }),
                                );
                            }
                        }
                        _ => {}
                    }

                    legible_on_fill(buf, r, t, bg);
                    ctx.clickable(rid, r);
                }
            }
        }
        if has_sb {
            scrollbar::render_with_fade(
                Rect::new(area.x, area.y, area.width, area.height),
                buf,
                ctx,
                LIST,
                &self.scroll,
                focused,
            );
        }
    }

    fn draw_detail(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let bg = t.canvas;
        let focused = ctx.interaction.focused(DETAIL);
        ctx.control(DETAIL, area, false);
        ctx.scrollable(DETAIL, area);
        let title_row = area.y;
        let body = Rect::new(
            area.x,
            area.y + 2,
            area.width,
            area.height.saturating_sub(2),
        );
        let meter_w = if body.width >= 70 { 24 } else { 16 };
        match self
            .selected
            .clone()
            .and_then(|id| w.accounts.get(&id).cloned())
        {
            None => {
                let s = OverallSummary::compute(&w.accounts.accounts);
                buf.set_string(area.x, title_row, "Overview", t.title().bg(bg));
                let meta = format!(
                    "{} · {}",
                    plural(s.counts.accounts, "account", "accounts"),
                    plural(s.counts.providers, "provider", "providers")
                );
                buf.set_string(
                    area.right().saturating_sub(width(&meta) as u16),
                    title_row,
                    &meta,
                    t.faint().bg(bg),
                );
                let lines = Self::overview_lines(w);
                self.detail_scroll.set_content(lines.len());
                self.detail_scroll.set_viewport(body.height as usize);
                for (k, i) in self.detail_scroll.visible_range().enumerate() {
                    let y = body.y + k as u16;
                    match &lines[i] {
                        OvLine::Text(spans) => {
                            render_spans(buf, body.x, y, body.width, t, spans, bg);
                        }
                        OvLine::Provider {
                            name,
                            accounts,
                            window,
                            status,
                        } => draw_provider_row(
                            buf, ctx, body, y, name, *accounts, window, status, bg,
                        ),
                    }
                }
            }
            Some(a) => {
                let title = a.title();
                buf.set_string(
                    area.x,
                    title_row,
                    termrock::ui::reveal::cut(
                        Rect::new(area.x, title_row, area.width.saturating_sub(20), 1),
                        &title,
                    ),
                    t.title().bg(bg),
                );
                let meta = a.status_word();
                buf.set_string(
                    area.right().saturating_sub(width(meta) as u16),
                    title_row,
                    meta,
                    t.faint().bg(bg),
                );
                let lines = Self::account_lines(&a, w);
                self.detail_scroll.set_content(lines.len());
                self.detail_scroll.set_viewport(body.height as usize);

                let (label_need, rest_need) = lines
                    .iter()
                    .filter(|(_, _, m)| m.is_some())
                    .map(|(text, ..)| {
                        let (l, r) = text.split_once('\u{1}').unwrap_or((text, ""));
                        (width(l) as u16, width(r) as u16)
                    })
                    .fold((0, 0), |(a, b), (l, r)| (a.max(l), b.max(r)));
                let cols = columns::solve(
                    &[
                        Col::content(label_need.min(10), label_need),
                        Col::fixed(meter_w + 6),
                        Col::content(rest_need.min(8), rest_need).flex(1, u16::MAX),
                    ],
                    body.width,
                    2,
                );
                let label_w = cols[0];
                for (k, i) in self.detail_scroll.visible_range().enumerate() {
                    let (text, tone, meter) = &lines[i];
                    let y = body.y + k as u16;
                    match meter {
                        Some((pct, mt)) => {
                            let (label, rest) = text.split_once('\u{1}').unwrap_or((text, ""));
                            super::accounts::quota_row(label, *pct, rest, *mt)
                                .label_width(label_w)
                                .label_gap(2)
                                .meter_width(meter_w + 6)
                                .render(Rect::new(body.x, y, body.width, 1), buf, ctx, bg);
                        }
                        None => {
                            let style = if text == "Limits" {
                                t.secondary().add_modifier(Modifier::BOLD)
                            } else {
                                Style::new().fg(t.tone(*tone))
                            };
                            buf.set_string(
                                body.x,
                                y,
                                termrock::ui::reveal::cut(
                                    Rect::new(body.x, y, body.width, 1),
                                    text,
                                ),
                                style.bg(bg),
                            );
                        }
                    }
                }
            }
        }
        if self.detail_scroll.overflows() {
            scrollbar::render_with_fade(
                Rect::new(area.x, body.y, area.width, body.height),
                buf,
                ctx,
                DETAIL,
                &self.detail_scroll,
                focused,
            );
        }
    }
}

impl Page<Jackin> for UsageScreen {
    fn enter(&mut self, w: &mut World, cx: &mut Cx) {
        self.build_rows(w);
        self.show_detail = false;
        cx.focus.focus(LIST);
        if self.selected.is_none()
            && let Some(service) = w.backend.usage_connections()
            && (self.all_usage
                || w.accounts.accounts.is_empty()
                || service.has_connected_accounts())
        {
            self.board.open(w, cx, self.all_usage.then_some(false));
        }
        if !self.board.active() {
            cx.focus.focus(LIST);
        }
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(if self.board.active() {
            self.board.primary_focus()
        } else {
            LIST
        })
    }

    fn animating(&self, w: &World) -> bool {
        w.accounts
            .accounts
            .iter()
            .any(|a| a.usage.freshness.phase == Freshness::Refreshing)
    }

    fn on_tick(&mut self, w: &mut World, _cx: &mut Cx) -> Outcome {
        self.board.tick(w);
        self.build_rows(w);
        if self.refreshing && !self.animating(w) {
            self.refreshing = false;
            w.last_refresh_secs = w.now_secs();
            return Outcome::Changed;
        }
        // The panel meta carries the broker age whenever nothing refreshes, an
        // empty registry included, so a second boundary always has text to move.
        if self.animating(w) || self.clock.due(w, true, false) {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        if key.plain()
            && w.backend.usage_connections().is_some()
            && matches!(key.code, KeyCode::Char('p' | 'a'))
        {
            self.board.open(w, cx, Some(key.code == KeyCode::Char('p')));
            return Outcome::Changed;
        }
        if self.board.active() {
            return match key.code {
                KeyCode::Char('m') if key.plain() => {
                    cx.go(Go::Accounts { select: None });
                    Outcome::Changed
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    cx.go(Go::Manager);
                    Outcome::Changed
                }
                _ => self.board.key(key, w, cx),
            };
        }
        self.build_rows(w);
        let in_detail = cx.focus.is(DETAIL);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') if !in_detail => {
                self.move_cursor(-1);
                Outcome::Changed
            }
            KeyCode::Down | KeyCode::Char('j') if !in_detail => {
                self.move_cursor(1);
                Outcome::Changed
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.detail_scroll.scroll_by(-1);
                Outcome::Changed
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.detail_scroll.scroll_by(1);
                Outcome::Changed
            }
            KeyCode::PageUp => {
                self.detail_scroll.page_up();
                Outcome::Changed
            }
            KeyCode::PageDown => {
                self.detail_scroll.page_down();
                Outcome::Changed
            }
            KeyCode::Enter => {
                self.show_detail = true;
                cx.focus.focus(DETAIL);
                Outcome::Changed
            }
            KeyCode::Char('r') if key.plain() => {
                self.refresh(w, cx);
                Outcome::Changed
            }
            KeyCode::Char('m') if key.plain() => {
                cx.go(Go::Accounts {
                    select: self.selected.clone(),
                });
                Outcome::Changed
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                if self.show_detail || in_detail {
                    self.show_detail = false;
                    cx.focus.focus(LIST);
                    return Outcome::Changed;
                }
                cx.go(Go::Manager);
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        if self.board.active() {
            return self.board.pointer(ev, w, cx);
        }
        if let Some(o) = scrollbar::pointer(ev, LIST, &mut self.scroll, TrackClick::Jump)
            .or_else(|| scrollbar::pointer(ev, DETAIL, &mut self.detail_scroll, TrackClick::Jump))
        {
            return o;
        }
        match (ev.wheel(), ev.phase()) {
            (Some((Axis::Vertical, delta)), _) => self.wheel(ev.id, delta),
            (None, Some(Phase::Click)) => self.click(ev.id, w, cx),
            _ => Outcome::Ignored,
        }
    }

    fn on_layer_result(
        &mut self,
        tag: &super::LayerTag,
        result: super::ModalResult,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        if tag.kind == "usage_connection" {
            if matches!(
                result,
                super::ModalResult::Form(None) | super::ModalResult::Cancelled
            ) {
                self.board.account_closed(w);
            }
            return self.board.account.result(result, w, cx);
        }
        if tag.kind == "usage_board" {
            self.board.result(result, w, cx)
        } else {
            Outcome::Ignored
        }
    }

    fn on_msg(&mut self, msg: &Msg, w: &mut World, cx: &mut Cx) -> Outcome {
        if let Msg::UsageUpdated { .. } = msg {
            self.board.refreshed(w);
            return Outcome::Changed;
        }
        if let Msg::UsageConnection { ticket, update } = msg {
            let result = self.board.receive(*ticket, update, cx);
            return if result == Outcome::Ignored {
                self.board.account.update(*ticket, update, cx)
            } else {
                result
            };
        }

        if let Msg::AccountRefreshed { .. } = msg {
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        if self.board.active() {
            self.board.render(area, buf, ctx);
            return;
        }
        self.build_rows(w);
        self.clock.painted(w);
        let t = ctx.theme;
        let n = w
            .accounts
            .accounts
            .iter()
            .filter(|a| a.usage.freshness.phase == Freshness::Refreshing)
            .count();
        let meta = if n > 0 {
            format!("{} refreshing {n}", spinner_frame(w.now_ms() as u64 / 80))
        } else {
            format!("broker · {}", w.clock.ago(w.last_refresh_secs))
        };
        let focused_any = ctx.interaction.focused(LIST) || ctx.interaction.focused(DETAIL);
        let connected = w.backend.usage_connections().is_some();
        let title = if connected {
            "Usage"
        } else {
            "Usage · read-only"
        };
        let inner = Panel::framed(Some(title))
            .focused(focused_any)
            .meta(&meta)
            .render(area, buf, t);
        if w.accounts.accounts.is_empty() {
            let e = if connected {
                EmptyState::new("Connected account usage")
                    .hint("p opens Priority · a opens All Usage · m opens Accounts")
            } else {
                EmptyState::new("No providers configured.")
                    .hint("r refreshes · m opens Accounts to add one")
            };
            empty::render(inner, buf, t, &e, t.canvas);
            ctx.control(LIST, Rect::new(inner.x, inner.y, 1, 1), false);
            return;
        }
        if area.width < 100 {
            if self.show_detail {
                self.draw_detail(inner, buf, ctx, w);
                ctx.control(LIST, Rect::ZERO, false);
            } else {
                self.draw_list(inner, buf, ctx, w);
                ctx.control(DETAIL, Rect::ZERO, false);
            }
            return;
        }
        let list_w = (inner.width * 34 / 100).clamp(28, 40);
        let list = Rect::new(inner.x, inner.y, list_w, inner.height);
        let detail = Rect::new(
            inner.x + list_w + 2,
            inner.y,
            inner.width.saturating_sub(list_w + 2),
            inner.height,
        );
        self.draw_list(list, buf, ctx, w);
        self.draw_detail(detail, buf, ctx, w);
    }

    fn hints(&self, focus: Option<WidgetId>, w: &World) -> Vec<Hint> {
        if self.board.active() {
            return vec![
                hint("p", "Priority"),
                hint("a", "All Usage"),
                hint("↑↓", "Move"),
                hint("Enter", "Open"),
                hint("r", "Refresh"),
                hint("m", "Accounts"),
                hint("Esc", "Back"),
            ];
        }
        if focus == Some(DETAIL) {
            return vec![
                hint("↑↓", "Scroll"),
                hint("r", "Refresh"),
                hint("m", "Manage in Accounts"),
                hint("Esc", "Back to list"),
            ];
        }
        let mut hints = vec![
            hint("↑↓", "Move"),
            hint("Enter", "Details"),
            hint("r", "Refresh"),
            hint("m", "Manage in Accounts"),
            hint("Esc", "Close"),
        ];
        if w.backend.usage_connections().is_some() {
            hints.insert(0, hint("a", "All Usage"));
        }
        hints
    }

    fn crumb(&self, w: &World) -> String {
        match self.selected.as_ref().and_then(|id| w.accounts.get(id)) {
            Some(a) => format!("Usage › {} › {}", a.surface.surface_name(), a.display_name),
            None => "Usage › Overview".into(),
        }
    }
}

impl Screen for UsageScreen {
    fn form_changed(
        &mut self,
        tag: &super::LayerTag,
        form: &mut super::modals::FormDialog,
        _w: &World,
    ) {
        if tag.kind == "usage_connection" {
            self.board.account.form_changed(form);
        }
    }
}

impl UsageScreen {
    fn click(&mut self, id: WidgetId, w: &mut World, cx: &mut Cx) -> Outcome {
        self.build_rows(w);
        for i in self.scroll.visible_range() {
            if LIST.child(i) == id {
                match &self.rows[i] {
                    Row::Account(a) => self.selected = Some(a.clone()),
                    Row::Overview => self.selected = None,
                    Row::Heading(_) => {}
                }
                self.detail_scroll.jump_start();
                cx.focus.focus(LIST);
                return Outcome::Changed;
            }
        }
        if id == DETAIL {
            cx.focus.focus(DETAIL);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    fn wheel(&mut self, id: WidgetId, delta: i32) -> Outcome {
        if id == LIST || id == scrollbar::id_for(LIST) {
            self.scroll.scroll_by(delta as isize);
            return Outcome::Changed;
        }
        if id == DETAIL || id == scrollbar::id_for(DETAIL) {
            self.detail_scroll.scroll_by(delta as isize);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }
}
