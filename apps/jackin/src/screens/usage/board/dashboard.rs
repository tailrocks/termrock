use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
};

use ratatui::{buffer::Buffer, layout::Rect, style::Style};
use termrock::core::{id::WidgetId, pointer::MouseTarget};
use termrock::theme::Tone;
use termrock::ui::{ctx::RenderCtx, text};
use termrock::widgets::{
    button::{Button, ButtonRow, RowOverflow},
    list::{ListBox, ListItem, SelectMode},
    panel::Panel,
};

use super::{ID, RowKey};
use crate::usage_connections::{BoardMetric, UsageBoard, UsageEntry};

pub(super) const ROWS: WidgetId = ID.sub("row");

pub(super) struct Entry {
    key: RowKey,
    pub(super) label: String,
    pub(super) provider: String,
    pub(super) indices: Vec<usize>,
    pub(super) ranked: Vec<usize>,
    pub(super) account: Option<UsageEntry>,
}

#[derive(Clone, Copy)]
pub(super) enum Status<'a> {
    Loading,
    Updating,
    Error(&'a str),
    Ready,
}

impl<'a> Status<'a> {
    fn line(self) -> Option<(&'a str, Tone)> {
        match self {
            Self::Loading => Some(("Loading usage…", Tone::Secondary)),
            Self::Updating => Some(("Updating usage…", Tone::Secondary)),
            Self::Error(error) => Some((error, Tone::Error)),
            Self::Ready => None,
        }
    }
}

pub(super) struct Dashboard {
    pub(super) entries: Vec<Entry>,
    pub(super) list: ListBox,
    pub(super) buttons: Vec<(&'static str, Button)>,
}

impl Default for Dashboard {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            list: ListBox::new(ROWS, Vec::new(), SelectMode::Single)
                .type_ahead(false)
                .empty_text("No readings"),
            buttons: Vec::new(),
        }
    }
}

impl Dashboard {
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(super) fn cursor(&self) -> usize {
        self.list.cursor
    }

    pub(super) fn primary(&self) -> WidgetId {
        if self.entries.is_empty() {
            ID.sub("switch")
        } else {
            ROWS
        }
    }

    pub(super) fn replace(&mut self, data: &UsageBoard) {
        let previous = self.entries.get(self.cursor()).map(|entry| &entry.key);
        let mut groups: HashMap<_, usize> = HashMap::new();
        let mut entries: Vec<Entry> = Vec::new();
        let accounts: HashMap<_, _> = data
            .providers
            .iter()
            .flat_map(|provider| {
                provider.accounts.iter().map(move |a| {
                    (
                        (provider.id.as_str(), a.id.as_str()),
                        (provider.name.as_str(), a),
                    )
                })
            })
            .collect();
        let provider_names: HashMap<_, _> = data
            .providers
            .iter()
            .map(|provider| (provider.id.as_str(), provider.name.as_str()))
            .collect();
        for (i, row) in data.rows.iter().enumerate() {
            if !data.priority
                && let Some(id) = &row.account
            {
                let key = (row.provider.as_str(), id.as_str());
                if let Some(&index) = groups.get(&key) {
                    entries[index].indices.push(i);
                    continue;
                }
                groups.insert(key, entries.len());
                let (provider, account) = accounts
                    .get(&key)
                    .copied()
                    .map_or((row.group.as_str(), None), |(provider, a)| {
                        (provider, Some(a))
                    });
                entries.push(Entry {
                    key: RowKey::Account(row.provider.clone(), id.clone()),
                    label: account.map_or_else(|| row.title.clone(), |a| a.label.clone()),
                    provider: short_provider(&row.provider, provider).into(),
                    indices: vec![i],
                    ranked: Vec::new(),
                    account: account.cloned(),
                });
            } else {
                entries.push(Entry {
                    key: super::Board::metric_key(row),
                    label: row.title.clone(),
                    provider: short_provider(
                        &row.provider,
                        provider_names
                            .get(row.provider.as_str())
                            .copied()
                            .unwrap_or(&row.group),
                    )
                    .into(),
                    indices: vec![i],
                    ranked: Vec::new(),
                    account: None,
                });
            }
        }
        if !data.priority {
            let unified_members: HashSet<_> = data
                .rows
                .iter()
                .flat_map(|row| &row.members)
                .map(|account| (account.provider.as_str(), account.id.as_str()))
                .collect();
            for provider in &data.providers {
                for account in &provider.accounts {
                    let key = (provider.id.as_str(), account.id.as_str());
                    if !groups.contains_key(&key) && !unified_members.contains(&key) {
                        entries.push(Entry {
                            key: RowKey::Account(provider.id.clone(), account.id.clone()),
                            label: account.label.clone(),
                            provider: short_provider(&provider.id, &provider.name).into(),
                            indices: Vec::new(),
                            ranked: Vec::new(),
                            account: Some(account.clone()),
                        });
                    }
                }
            }
        }
        let cursor = previous
            .and_then(|key| entries.iter().position(|entry| &entry.key == key))
            .unwrap_or_else(|| self.cursor().min(entries.len().saturating_sub(1)));

        for entry in &mut entries {
            entry.ranked.clone_from(&entry.indices);
            entry.ranked.sort_by_key(|&index| {
                let row = &data.rows[index];
                Reverse((urgency(row.severity), row.bounded))
            });
        }
        let mut labels = HashMap::new();
        let mut provider_labels = HashMap::new();
        for entry in &entries {
            *labels.entry(entry.label.as_str()).or_insert(0) += 1;
            *provider_labels
                .entry((entry.provider.as_str(), entry.label.as_str()))
                .or_insert(0) += 1;
        }
        self.list.items = entries
            .iter()
            .map(|entry| {
                let reading = entry.ranked.first().map(|&index| &data.rows[index]);
                let duplicate = labels
                    .get(entry.label.as_str())
                    .is_some_and(|&count| count > 1);
                let mut label = if duplicate {
                    format!("{} · {}", entry.provider, entry.label)
                } else {
                    entry.label.clone()
                };
                if let Some(account) = &entry.account
                    && provider_labels
                        .get(&(entry.provider.as_str(), entry.label.as_str()))
                        .is_some_and(|&count| count > 1)
                {
                    let tail: Vec<_> = account.id.chars().rev().take(8).collect();
                    let discriminator: String = tail.into_iter().rev().collect();
                    label = format!("{label} ({discriminator})");
                }
                if !data.priority && entry.account.is_some() {
                    if let Some(reading) = reading {
                        label = format!("{label} · {}", reading.label);
                    }
                } else if !duplicate {
                    label = format!("{label} · {}", entry.provider);
                }
                let (value, tone) = reading.map_or(("No visible readings", Tone::Muted), |row| {
                    (row.value.as_str(), row.severity)
                });
                ListItem::new(&label)
                    .meta(&text::truncate(value, 32))
                    .meta_tone(tone)
            })
            .collect();
        self.entries = entries;
        self.list.cursor = cursor;
        self.list.core.scroll.set_content(self.entries.len());
        self.list.core.scroll.ensure_visible(cursor);
    }

    pub(super) fn button_pointer(
        &mut self,
        ev: &termrock::core::pointer::PointerEvent,
    ) -> Option<&'static str> {
        self.buttons
            .iter_mut()
            .find(|(_, button)| button.id == ev.id)
            .and_then(|(action, button)| button.pointer(ev).has_actions().then_some(*action))
    }

    pub(super) fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        priority: bool,
        data: Option<&UsageBoard>,
        status: Status<'_>,
    ) {
        let theme = ctx.theme;
        let title = if priority { "Priority" } else { "All Usage" };
        let meta = data.map_or_else(String::new, |data| {
            if !priority && data.rows.iter().all(|row| row.account.is_some()) {
                text::plural(self.entries.len(), "account", "accounts")
            } else {
                text::plural(data.rows.len(), "reading", "readings")
            }
        });
        let inner = Panel::framed(Some(title))
            .meta(&meta)
            .render(area, buf, theme);
        if inner.is_empty() {
            return;
        }

        let scope = if priority { "All Usage" } else { "Priority" };
        let mut buttons = vec![("switch", Button::secondary(ID.sub("switch"), scope))];
        if let Some(data) = data {
            if !data.providers.is_empty() {
                buttons.push((
                    "providers",
                    Button::secondary(ID.sub("providers"), "Providers"),
                ));
            }
            buttons.push(("reload", Button::secondary(ID.sub("reload"), "Refresh")));
            buttons.push((
                "more",
                Button::secondary(
                    ID.sub("more"),
                    if data.attention.is_empty() {
                        "Actions…"
                    } else {
                        "Attention…"
                    },
                ),
            ));
            if data.empty {
                buttons.push(("add", Button::secondary(ID.sub("add"), "Add account")));
            }
            if let Some(entry) = self.entries.get(self.cursor()) {
                let label = if entry.indices.len() > 1 {
                    "Metrics"
                } else {
                    "Details"
                };
                buttons.push(("save", Button::primary(ID.sub("save"), label)));
            }
        } else if matches!(status, Status::Error(_)) {
            buttons.push(("save", Button::primary(ID.sub("save"), "Retry")));
        }
        let (keys, mut buttons): (Vec<_>, Vec<_>) = buttons.into_iter().unzip();
        if matches!(status, Status::Loading | Status::Updating) {
            for button in buttons.iter_mut().skip(1) {
                button.disabled = true;
            }
        }
        let toolbar = ButtonRow::new().overflow(RowOverflow::Wrap);
        let toolbar_h = toolbar.rows(inner.width, &buttons, theme).min(inner.height);
        let toolbar_area = Rect::new(inner.x, inner.y, inner.width, toolbar_h);
        toolbar.render(ID, toolbar_area, &mut buttons, buf, ctx, theme.canvas);
        self.buttons = keys.into_iter().zip(buttons).collect();

        let mut body = Rect::new(
            inner.x,
            toolbar_area.bottom(),
            inner.width,
            inner.height - toolbar_h,
        );
        if let Some((message, tone)) = status.line() {
            line(
                body,
                0,
                buf,
                message,
                Style::new().fg(theme.tone(tone)).bg(theme.canvas),
            );
            body = Rect::new(
                body.x,
                body.y.saturating_add(1),
                body.width,
                body.height.saturating_sub(1),
            );
        }
        let Some(data) = data else { return };
        if self.entries.is_empty() {
            let message = if matches!(data.summary.as_str(), "Priority" | "All Usage") {
                if priority {
                    "No priority readings. Open All Usage to browse accounts."
                } else {
                    "No usage readings available."
                }
            } else {
                &data.summary
            };
            for (i, part) in text::wrap(message, body.width as usize)
                .iter()
                .take(body.height as usize)
                .enumerate()
            {
                line(
                    body,
                    i as u16,
                    buf,
                    part,
                    theme.secondary().bg(theme.canvas),
                );
            }
            return;
        }
        self.list.chosen = Some(self.cursor());
        if body.height < 5 || body.width < 40 {
            self.list.render(body, buf, ctx, theme.canvas);
            return;
        }
        let (list, preview) = if inner.width >= 96 {
            let list_w = body.width * 54 / 100;
            (
                Rect::new(body.x, body.y, list_w, body.height),
                Rect::new(
                    body.x + list_w + 2,
                    body.y,
                    body.width - list_w - 2,
                    body.height,
                ),
            )
        } else {
            let preview_h = 5.min(body.height / 2);
            (
                Rect::new(body.x, body.y, body.width, body.height - preview_h),
                Rect::new(body.x, body.bottom() - preview_h, body.width, preview_h),
            )
        };
        let label = if priority {
            "Reading"
        } else {
            "Account / reading"
        };
        let bare_percent = data
            .rows
            .iter()
            .any(|row| row.bounded && row.value.trim().ends_with('%'));
        let remaining = data
            .settings
            .flags
            .iter()
            .find(|(key, _, _)| *key == "remaining")
            .is_none_or(|(_, _, on)| *on);
        let heading = match (bare_percent, remaining) {
            (true, true) => "Value (% left)",
            (true, false) => "Value (% used)",
            (false, _) => "Value",
        };
        line(list, 0, buf, label, theme.secondary().bg(theme.canvas));
        let width = text::width(heading) as u16;
        let x = list.right().saturating_sub(width + 3);
        buf.set_string(x, list.y, heading, theme.secondary().bg(theme.canvas));
        self.list.render(
            Rect::new(list.x, list.y + 1, list.width, list.height - 1),
            buf,
            ctx,
            theme.canvas,
        );
        if let Some(entry) = self.entries.get(self.cursor()) {
            render_preview(entry, data, preview, buf, ctx);
        }
    }
}

fn line(area: Rect, i: u16, buf: &mut Buffer, value: &str, style: Style) {
    if i < area.height && area.width > 0 {
        buf.set_string(
            area.x,
            area.y + i,
            text::truncate(value, area.width as usize),
            style,
        );
    }
}

fn render_preview(entry: &Entry, data: &UsageBoard, area: Rect, buf: &mut Buffer, ctx: &RenderCtx) {
    let theme = ctx.theme;
    let pace = entry.ranked.iter().any(|&index| {
        data.rows[index]
            .bounded_bar()
            .is_some_and(|(_, pace)| pace.is_some())
    });
    let meta = if pace {
        format!("{} · │ pace", entry.provider)
    } else {
        entry.provider.clone()
    };
    let inner = Panel::card(Some(&entry.label))
        .meta(&meta)
        .render(area, buf, theme);
    if entry.indices.is_empty() {
        line(
            inner,
            0,
            buf,
            "No visible readings",
            theme.muted().bg(theme.surface),
        );
        if let Some(account) = &entry.account {
            for (i, item) in account
                .summary
                .iter()
                .take(inner.height.saturating_sub(1) as usize)
                .enumerate()
            {
                line(
                    inner,
                    i as u16 + 1,
                    buf,
                    item,
                    theme.secondary().bg(theme.surface),
                );
            }
        }
        return;
    }
    let indices = &entry.ranked;
    let value_width = indices
        .iter()
        .take(inner.height as usize)
        .map(|&index| text::width(&data.rows[index].value) as u16)
        .max()
        .unwrap_or(0)
        .max(inner.width / 3)
        .min(inner.width.saturating_sub(14).max(inner.width / 2));
    for (i, &index) in indices.iter().take(inner.height as usize).enumerate() {
        let BoardMetric {
            label,
            value,
            reset,
            severity,
            inspector,
            ..
        } = &data.rows[index];
        let bar_width = if inner.width.saturating_sub(value_width) >= 32 {
            12
        } else {
            0
        };
        let label_width = inner
            .width
            .saturating_sub(value_width + if bar_width > 0 { bar_width + 4 } else { 2 });
        let row_area = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
        let (label_area, value_area) = if label_width == 0 {
            (Rect::ZERO, row_area)
        } else {
            (
                Rect::new(row_area.x, row_area.y, label_width, 1),
                Rect::new(row_area.right() - value_width, row_area.y, value_width, 1),
            )
        };
        let label_style = theme.primary().bg(theme.surface);
        let value_style = Style::new().fg(theme.tone(*severity)).bg(theme.surface);
        line(label_area, 0, buf, label, label_style);
        line(value_area, 0, buf, value, value_style);
        if bar_width > 0
            && let Some((amount, pace)) = data.rows[index].bounded_bar()
        {
            let bar_x = value_area.x.saturating_sub(bar_width + 2);
            let glyphs = theme.glyphs;
            for (cell, segment) in quota_bar(amount, pace, bar_width).enumerate() {
                let (glyph, style) = match segment {
                    BarSegment::Pace => ("│", theme.primary().bg(theme.surface)),
                    BarSegment::Fill => (glyphs.bar_fill, value_style),
                    BarSegment::Track => (glyphs.bar_empty, theme.faint().bg(theme.surface)),
                };
                buf.set_string(bar_x + cell as u16, row_area.y, glyph, style);
            }
        }
        if indices.len() == 1 {
            if let Some(reset) = reset {
                line(inner, 1, buf, reset, theme.secondary().bg(theme.surface));
            }
            for (j, detail) in inspector
                .iter()
                .take(inner.height.saturating_sub(2) as usize)
                .enumerate()
            {
                line(
                    inner,
                    j as u16 + 2,
                    buf,
                    detail,
                    theme.muted().bg(theme.surface),
                );
            }
        }
    }
}

fn urgency(tone: Tone) -> u8 {
    match tone {
        Tone::Danger | Tone::Error => 2,
        Tone::Warning => 1,
        _ => 0,
    }
}

fn short_provider<'a>(id: &str, name: &'a str) -> &'a str {
    if id == "antigravity" {
        "Antigravity"
    } else {
        name
    }
}

pub(super) enum BarSegment {
    Pace,
    Fill,
    Track,
}

pub(super) fn quota_bar(
    amount: u8,
    pace: Option<u8>,
    width: u16,
) -> impl Iterator<Item = BarSegment> {
    let filled = (u16::from(amount.min(100)) * width).div_ceil(100);
    let tick = pace.map(|value| (u16::from(value) * width / 100).min(width.saturating_sub(1)));
    (0..width).map(move |i| {
        if tick == Some(i) {
            BarSegment::Pace
        } else if i < filled {
            BarSegment::Fill
        } else {
            BarSegment::Track
        }
    })
}
