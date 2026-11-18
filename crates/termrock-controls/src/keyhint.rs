use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::layout::Rect;
use ratatui::style::Style;

use termrock_runtime::measure::terminal_extent;
use termrock_theme::{BadgeKind, HintAlign, HintBarStyle, StatusKind, Theme, Tone};

pub use termrock_core::hints::{
    ARROWS, CONTEXT_MENU, DELETE, EDIT_CELL, EDIT_CODE, EDIT_FIELD, EDIT_TEXT, Hint, HintPriority,
    LEFT_RIGHT, UP_DOWN, deliverable_line, hint, modifier_click, normalize_key, range, toggle_hint,
};

pub fn render(
    area: Rect,
    buf: &mut Buffer,
    t: &Theme,
    hints: &[Hint],
    badge: Option<(&str, BadgeKind)>,
    right: Option<&str>,
) -> usize {
    render_toned(
        area,
        buf,
        t,
        hints,
        badge,
        right.map(|r| (r, Tone::Secondary)),
    )
}

pub fn render_toned(
    area: Rect,
    buf: &mut Buffer,
    t: &Theme,
    hints: &[Hint],
    badge: Option<(&str, BadgeKind)>,
    right: Option<(&str, Tone)>,
) -> usize {
    render_styled(area, buf, t, hints, badge, right, &t.hintbar)
}

pub fn render_aligned(
    area: Rect,
    buf: &mut Buffer,
    t: &Theme,
    hints: &[Hint],
    badge: Option<(&str, BadgeKind)>,
    right: Option<(&str, Tone)>,
    align: HintAlign,
) -> usize {
    let style = HintBarStyle { align, ..t.hintbar };
    render_styled(area, buf, t, hints, badge, right, &style)
}

const GAP: usize = 2;

/// Display widths of one hint's key and action, each measured once.
#[derive(Clone, Copy)]
struct HintCells {
    key: usize,
    action: usize,
}

impl HintCells {
    fn of(h: &Hint) -> Self {
        Self {
            key: termrock_text::cells::width(&normalize_key(h.key.as_ref())),
            action: termrock_text::cells::width(&h.action),
        }
    }

    /// Cells the hint takes: the key alone when compact, else key, space, action.
    fn cells(self, compact: bool) -> usize {
        if compact {
            self.key
        } else {
            self.key.saturating_add(1).saturating_add(self.action)
        }
    }
}

fn hint_width(h: &Hint, compact: bool) -> usize {
    HintCells::of(h).cells(compact)
}

/// Full width of the kept hints plus their gaps and the cut marker slot, with
/// no narrowing, so a fit against `room` never sees a wrapped total.
#[doc(hidden)]
pub fn cost(hints: &[Hint], keep: &[usize], compact: &[bool]) -> usize {
    let mut c = keep
        .iter()
        .map(|&i| hint_width(&hints[i], compact[i]).saturating_add(GAP))
        .fold(0usize, usize::saturating_add);
    if keep.len() < hints.len() {
        c = c.saturating_add(GAP);
    }
    c
}

/// Which hints show in `room` cells, and which of those drop to the key alone.
///
/// Every hint is measured once. Full widths are tried first, then compact
/// non-pinned hints when the style allows it. When compact-all still does not
/// fit, pruning starts again from full widths: non-pinned hints go last to
/// first, then pinned ones last to first. A running total and a keep mask
/// replace a cost and a retain per removal.
#[doc(hidden)]
pub fn fit(hints: &[Hint], room: u16, style: &HintBarStyle) -> (Vec<usize>, Vec<bool>) {
    let room = usize::from(room) as u128;
    let cells: Vec<HintCells> = hints.iter().map(HintCells::of).collect();
    let slot = |w: usize| w as u128 + GAP as u128;
    let total = |compact: &dyn Fn(usize) -> bool| -> u128 {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                termrock_metrics::record_work!(FitVisits, 1usize);
                slot(c.cells(compact(i)))
            })
            .sum()
    };
    let all: Vec<usize> = (0..hints.len()).collect();
    let full = vec![false; hints.len()];
    let mut kept = total(&|_| false);
    if kept <= room {
        return (all, full);
    }
    let pinned: Vec<bool> = hints.iter().map(|h| h.is_pinned(style)).collect();
    if style.compact {
        let compact: Vec<bool> = pinned.iter().map(|p| !p).collect();
        if total(&|i| compact[i]) <= room {
            return (all, compact);
        }
    }
    let mut keep = vec![true; hints.len()];
    let order = (0..hints.len())
        .rev()
        .filter(|&i| !pinned[i])
        .chain((0..hints.len()).rev().filter(|&i| pinned[i]));
    for gone in order {
        termrock_metrics::record_work!(FitVisits, 1usize);
        keep[gone] = false;
        kept -= slot(cells[gone].cells(false));
        if kept + GAP as u128 <= room {
            break;
        }
    }
    let keep: Vec<usize> = (0..hints.len()).filter(|&i| keep[i]).collect();
    (keep, full)
}

/// Natural width of the hint bar, saturating at `u16::MAX`.
pub fn width(hints: &[Hint], badge: Option<&str>, status: Option<&str>) -> u16 {
    let all: Vec<usize> = (0..hints.len()).collect();
    let full = vec![false; hints.len()];
    let badge = badge.map_or(0, |b| termrock_text::cells::width(b).saturating_add(4));
    let status = status
        .map(termrock_text::cells::width)
        .filter(|w| *w > 0)
        .map_or(0, |w| w.saturating_add(3));
    terminal_extent(
        1usize
            .saturating_add(badge)
            .saturating_add(cost(hints, &all, &full))
            .saturating_add(status),
    )
}

pub fn render_status(
    area: Rect,
    buf: &mut Buffer,
    t: &Theme,
    hints: &[Hint],
    badge: Option<(&str, BadgeKind)>,
    right: Option<(&str, Option<StatusKind>)>,
) -> usize {
    render_status_at(area, buf, t, hints, badge, right).0
}

pub fn render_status_at(
    area: Rect,
    buf: &mut Buffer,
    t: &Theme,
    hints: &[Hint],
    badge: Option<(&str, BadgeKind)>,
    right: Option<(&str, Option<StatusKind>)>,
) -> (usize, Vec<(usize, Rect)>) {
    let right = right.map(|(text, kind)| match kind {
        Some(kind) => {
            let (mark, mark_style) = t.status(kind);
            Right {
                text,
                mark,
                mark_style,
                style: t.status_label(kind),
            }
        }
        None => Right::toned(t, text, Tone::Secondary),
    });
    let mut spots = Vec::new();
    let n = render_right(
        area,
        buf,
        t,
        hints,
        badge,
        right,
        &[],
        &t.hintbar,
        &mut spots,
    );
    (n, spots)
}

pub fn render_status_styled_at(
    area: Rect,
    buf: &mut Buffer,
    t: &Theme,
    hints: &[Hint],
    badge: Option<(&str, BadgeKind)>,
    right: Option<(&str, Option<StatusKind>)>,
    style: &HintBarStyle,
) -> (usize, Vec<(usize, Rect)>) {
    let right = right.map(|(text, kind)| match kind {
        Some(kind) => {
            let (mark, mark_style) = t.status(kind);
            Right {
                text,
                mark,
                mark_style,
                style: t.status_label(kind),
            }
        }
        None => Right::toned(t, text, Tone::Secondary),
    });
    let mut spots = Vec::new();
    let n = render_right(area, buf, t, hints, badge, right, &[], style, &mut spots);
    (n, spots)
}

struct Right<'a> {
    text: &'a str,
    mark: &'a str,
    mark_style: Style,
    style: Style,
}

impl<'a> Right<'a> {
    fn toned(t: &'a Theme, text: &'a str, tone: Tone) -> Self {
        let mark = match tone {
            Tone::Error => "!",
            Tone::Warning => t.glyphs.warning,
            _ => "",
        };
        let style = Style::new().fg(t.tone(tone));
        Self {
            text,
            mark,
            mark_style: style.add_modifier(ratatui::style::Modifier::BOLD),
            style,
        }
    }
}

pub fn render_styled(
    area: Rect,
    buf: &mut Buffer,
    t: &Theme,
    hints: &[Hint],
    badge: Option<(&str, BadgeKind)>,
    right: Option<(&str, Tone)>,
    style: &HintBarStyle,
) -> usize {
    let right = right.map(|(r, tone)| Right::toned(t, r, tone));
    render_right(
        area,
        buf,
        t,
        hints,
        badge,
        right,
        &[],
        style,
        &mut Vec::new(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn render_parts(
    area: Rect,
    buf: &mut Buffer,
    t: &Theme,
    hints: &[Hint],
    badge: Option<(&str, BadgeKind)>,
    right: Option<(&str, Tone)>,
    parts: &[(String, Tone)],
    style: &HintBarStyle,
) -> usize {
    let right = right.map(|(r, tone)| Right::toned(t, r, tone));
    render_right(
        area,
        buf,
        t,
        hints,
        badge,
        right,
        parts,
        style,
        &mut Vec::new(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn render_parts_at(
    area: Rect,
    buf: &mut Buffer,
    t: &Theme,
    hints: &[Hint],
    badge: Option<(&str, BadgeKind)>,
    right: Option<(&str, Tone)>,
    parts: &[(String, Tone)],
    style: &HintBarStyle,
) -> (usize, Vec<(usize, Rect)>) {
    let right = right.map(|(r, tone)| Right::toned(t, r, tone));
    let mut spots = Vec::new();
    let n = render_right(area, buf, t, hints, badge, right, parts, style, &mut spots);
    (n, spots)
}

pub fn key_of(label: &str) -> Option<termrock_core::event::Key> {
    use ratatui::crossterm::event::KeyCode;
    let label = normalize_key(label);
    let mut rest: &str = &label;
    let mut mods = KeyModifiers::NONE;
    loop {
        let (m, tail) = if let Some(t) = rest.strip_prefix("Ctrl+") {
            (KeyModifiers::CONTROL, t)
        } else if let Some(t) = rest.strip_prefix("Alt+") {
            (KeyModifiers::ALT, t)
        } else if let Some(t) = rest.strip_prefix("Shift+") {
            (KeyModifiers::SHIFT, t)
        } else {
            break;
        };
        if tail.is_empty() {
            break;
        }
        mods |= m;
        rest = tail;
    }
    let code = match rest {
        "Esc" => KeyCode::Esc,
        "Enter" | "↵" | "Return" => KeyCode::Enter,
        "Tab" if mods == KeyModifiers::SHIFT => {
            mods = KeyModifiers::NONE;
            KeyCode::BackTab
        }
        "Tab" => KeyCode::Tab,
        "Space" => KeyCode::Char(' '),
        "Delete" => KeyCode::Delete,
        "Insert" | "Ins" => KeyCode::Insert,
        "Backspace" => KeyCode::Backspace,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PgUp" | "PageUp" => KeyCode::PageUp,
        "PgDn" | "PageDown" => KeyCode::PageDown,
        "↑" => KeyCode::Up,
        "↓" => KeyCode::Down,
        "←" => KeyCode::Left,
        "→" => KeyCode::Right,
        _ => {
            if let Some(n) = rest.strip_prefix('F').and_then(|n| n.parse::<u8>().ok())
                && (1..=12).contains(&n)
            {
                KeyCode::F(n)
            } else {
                let mut chars = rest.chars();
                let c = chars.next()?;
                if chars.next().is_some() || c.is_whitespace() {
                    return None;
                }
                if mods.contains(KeyModifiers::CONTROL) {
                    KeyCode::Char(c.to_ascii_lowercase())
                } else {
                    if c.is_ascii_uppercase() {
                        mods |= KeyModifiers::SHIFT;
                    }
                    KeyCode::Char(c)
                }
            }
        }
    };
    Some(termrock_core::event::Key { code, mods })
}

#[allow(clippy::too_many_arguments)]
fn render_right(
    area: Rect,
    buf: &mut Buffer,
    t: &Theme,
    hints: &[Hint],
    badge: Option<(&str, BadgeKind)>,
    right: Option<Right<'_>>,
    parts: &[(String, Tone)],
    style: &HintBarStyle,
    spots: &mut Vec<(usize, Rect)>,
) -> usize {
    let area = area.intersection(*buf.area());
    if area.is_empty() {
        return 0;
    }
    let mut x = area.x + 1;
    if let Some((text, kind)) = badge {
        let b = termrock_text::cells::truncate(
            &format!(" {text} "),
            area.width.saturating_sub(2) as usize,
        );
        if !b.is_empty() {
            buf.set_string(x, area.y, &b, t.badge(kind));
            x = x
                .saturating_add(terminal_extent(termrock_text::cells::width(&b)))
                .saturating_add(2)
                .min(area.right());
        }
    }
    let mut right_w = 0u16;
    if let Some(Right {
        text: r,
        mark,
        mark_style,
        style: st,
    }) = right
    {
        let text = if mark.is_empty() {
            r.to_owned()
        } else {
            format!("{mark} {r}")
        };
        let mut available = area.right().saturating_sub(x + 1) as usize;
        if !hints.is_empty() {
            let share = area.width as usize * style.status_min_share.min(100) as usize / 100;
            let pinned: Vec<usize> = (0..hints.len())
                .filter(|&i| hints[i].is_pinned(style))
                .collect();
            if !pinned.is_empty() {
                let need = cost(hints, &pinned, &vec![false; hints.len()]);
                available = available.min(share.max(available.saturating_sub(need + 2)));
            }
        }
        let text = termrock_text::cells::truncate(&text, available);
        let w = termrock_text::cells::width(&text) as u16;
        if w > 0 {
            buf.set_string(area.right() - w - 1, area.y, &text, st);
            if !mark.is_empty() {
                buf.set_string(area.right() - w - 1, area.y, mark, mark_style);
            }

            let mut px = area.right() - w - 1;
            if !mark.is_empty() {
                px += termrock_text::cells::width(mark) as u16 + 1;
            }
            let end = area.right() - 1;
            for (part, tone) in parts {
                let pw = termrock_text::cells::width(part) as u16;
                let to = px.saturating_add(pw).min(end);
                if to > px {
                    buf.set_style(
                        Rect::new(px, area.y, to - px, 1),
                        ratatui::style::Style::new().fg(t.tone(*tone)),
                    );
                }
                px = px.saturating_add(pw);
            }
            right_w = w + 3;
        }
    }
    let limit = area.right().saturating_sub(right_w);
    let (keep, compact) = fit(hints, limit.saturating_sub(x), style);
    let cut = keep.len() < hints.len();
    if style.align == HintAlign::Center {
        let mut used = terminal_extent(cost(hints, &keep, &compact));
        used = if cut {
            used.saturating_sub(GAP as u16) + 1
        } else {
            used.saturating_sub(GAP as u16)
        };
        let free = area.width.saturating_sub(used);
        let mid = area.x + free / 2;

        x = mid.max(x).min(limit.saturating_sub(used).max(x));
    }
    for &i in &keep {
        let h = &hints[i];
        let key = normalize_key(h.key.as_ref());
        let key = t.glyphs.key_label(&key);
        let kw = termrock_text::cells::width(&key) as u16;
        buf.set_string(x, area.y, &key, t.key_hint_key());
        if !compact[i] {
            buf.set_string(x + kw + 1, area.y, &h.action, t.key_hint_action());
        }
        let hw = terminal_extent(hint_width(h, compact[i]));
        let w = hw.min(area.right().saturating_sub(x));
        spots.push((i, Rect::new(x, area.y, w, 1)));
        x = x.saturating_add(hw).saturating_add(GAP as u16);
    }
    if cut && x < limit {
        buf.set_string(x, area.y, t.glyphs.ellipsis, t.faint());
    }
    keep.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_hint_labels_are_clickable() {
        for label in ["Insert", "Ins"] {
            let key = key_of(label).unwrap_or_else(|| panic!("{label} must parse"));
            assert_eq!(key.code, ratatui::crossterm::event::KeyCode::Insert);
            assert_eq!(key.mods, KeyModifiers::NONE);
        }
        assert!(key_of("Insertion").is_none());
    }
}
