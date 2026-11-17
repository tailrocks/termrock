use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use termrock::theme::Theme;

use termrock::motion::Motion;

pub const TICK_MS: u64 = 33;

pub const MOTION_SEED: u64 = 0x4A41_434B_494E_5E5E;

#[inline]
pub const fn mix(a: u64, b: u64, c: u64) -> u64 {
    let mut z = a.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ b.wrapping_mul(0xD1B5_4A32_D192_ED03)
        ^ c.wrapping_mul(0x2545_F491_4F6C_DD1D)
        ^ MOTION_SEED;
    z ^= z >> 30;
    z = z.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z ^= z >> 27;
    z = z.wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    z
}

pub const fn pct(h: u64) -> u64 {
    h % 100
}

const POOL: &[u8; 78] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz@#$%&*<>{}[]|/\\~";

pub fn glyph(x: u64, y: u64, epoch: u64) -> char {
    POOL[(mix(x, y, epoch) % 78) as usize] as char
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Ladder(u8),
    Accent,
}

fn ladder_color(t: &Theme, i: u8) -> Color {
    match i {
        0 => t.text_ghost,
        1 => t.text_faint,
        2 => t.text_muted,
        3 => t.text_secondary,
        _ => t.text_primary,
    }
}

pub fn style(t: &Theme, tone: Tone, dim: u8) -> Option<Style> {
    match tone {
        Tone::Ladder(i) => {
            if dim > i {
                None
            } else {
                Some(Style::new().fg(ladder_color(t, i - dim)).bg(t.canvas))
            }
        }
        Tone::Accent => match dim {
            0 => Some(Style::new().fg(t.accent).bg(t.canvas)),
            1 => Some(Style::new().fg(t.accent_hover).bg(t.canvas)),
            2 => Some(Style::new().fg(t.accent_pressed).bg(t.canvas)),
            _ => None,
        },
    }
}

fn put(buf: &mut Buffer, x: u16, y: u16, ch: char, st: Style) {
    if let Some(cell) = buf.cell_mut((x, y)) {
        let mut s = [0u8; 4];
        cell.set_symbol(ch.encode_utf8(&mut s));
        cell.set_style(st);
    }
}

pub fn fill_canvas(buf: &mut Buffer, area: Rect, t: &Theme) {
    termrock::ui::ctx::fill(buf, area, Style::new().bg(t.canvas).fg(t.text_primary));
}

pub fn dim_buffer(buf: &mut Buffer, area: Rect, steps: u8, t: &Theme) {
    if steps == 0 {
        return;
    }
    let ladder = [
        t.text_ghost,
        t.text_faint,
        t.text_muted,
        t.text_secondary,
        t.text_primary,
    ];
    for pos in area.positions() {
        let Some(cell) = buf.cell_mut(pos) else {
            continue;
        };
        let st = cell.style();
        let fg = st.fg.unwrap_or(t.text_primary);
        let idx = ladder.iter().position(|c| *c == fg);
        let new_fg = match idx {
            Some(i) => {
                if (steps as usize) > i {
                    None
                } else {
                    Some(ladder[i - steps as usize])
                }
            }
            None if fg == t.accent || fg == t.success || fg == t.focus => match steps {
                1 => Some(t.accent_hover),
                2 => Some(t.accent_pressed),
                _ => None,
            },
            None if fg == t.error || fg == t.warning => {
                if steps >= 4 {
                    None
                } else {
                    Some(ladder[4 - steps as usize - 1])
                }
            }
            None => {
                if steps >= 3 {
                    None
                } else {
                    Some(t.text_faint)
                }
            }
        };
        let bg = st.bg.unwrap_or(t.canvas);
        let new_bg = if bg == t.canvas {
            t.canvas
        } else if bg == t.accent {
            if steps <= 2 { t.accent_bg } else { t.canvas }
        } else if steps >= 2 {
            t.canvas
        } else {
            t.surface
        };
        match new_fg {
            Some(c) => {
                let mut ns = Style::new().fg(c).bg(new_bg);
                if st.add_modifier.contains(Modifier::BOLD) {
                    ns = ns.add_modifier(Modifier::BOLD);
                }
                cell.set_style(ns);
            }
            None => {
                cell.set_symbol(" ");
                cell.set_style(Style::new().fg(new_bg).bg(new_bg));
            }
        }
    }
}

fn center(area: Rect) -> (u16, u16) {
    (
        area.x + area.width / 2,
        area.y + (area.height / 2).saturating_sub(1),
    )
}

fn draw_text(buf: &mut Buffer, area: Rect, text: &str, y: u16, tone: Tone, t: &Theme) {
    let n = text.chars().count() as u16;
    let x0 = area.x + area.width.saturating_sub(n) / 2;
    if let Some(st) = style(t, tone, 0) {
        for (i, ch) in text.chars().enumerate() {
            put(buf, x0 + i as u16, y, ch, st);
        }
    }
}

fn draw_hint(buf: &mut Buffer, area: Rect, key: &str, action: &str, t: &Theme) {
    let text = format!("{key} {action}");
    let n = text.chars().count() as u16;
    if area.width < n + 4 {
        return;
    }
    let x = area.right().saturating_sub(n + 2);
    let y = area.bottom().saturating_sub(1);

    for xx in x.saturating_sub(1)..area.right() {
        put(buf, xx, y, ' ', Style::new().bg(t.canvas));
    }
    let ks = Style::new()
        .fg(t.text_muted)
        .bg(t.canvas)
        .add_modifier(Modifier::BOLD);
    let as_ = Style::new().fg(t.text_faint).bg(t.canvas);
    for (i, ch) in key.chars().enumerate() {
        put(buf, x + i as u16, y, ch, ks);
    }
    for (i, ch) in action.chars().enumerate() {
        put(
            buf,
            x + key.chars().count() as u16 + 1 + i as u16,
            y,
            ch,
            as_,
        );
    }
}

fn draw_pill_bottom(buf: &mut Buffer, area: Rect, t: &Theme) {
    let lockup = termrock::widgets::brand::Lockup::new(crate::app::BRAND_MARK);
    let w = lockup.width();
    if area.height < 4 || area.width < w + 2 {
        return;
    }
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.bottom().saturating_sub(2);
    lockup.render(x, y, buf, t);
}

fn draw_typed(buf: &mut Buffer, area: Rect, text: &str, y: u16, shown: usize, t: &Theme) {
    let n = text.chars().count() as u16;
    let x0 = area.x + area.width.saturating_sub(n) / 2;
    if let Some(st) = style(t, Tone::Ladder(4), 0) {
        for (i, ch) in text.chars().take(shown).enumerate() {
            put(buf, x0 + i as u16, y, ch, st);
        }
    }
}

fn draw_glitched(buf: &mut Buffer, area: Rect, text: &str, y: u16, j: u64, tone: Tone, t: &Theme) {
    let n = text.chars().count() as u16;
    let x0 = area.x + area.width.saturating_sub(n) / 2;
    let Some(st) = style(t, tone, 0) else { return };
    let pass = j / GLITCH_PASS_TICKS;
    for (i, ch) in text.chars().enumerate() {
        let x = x0 + i as u16;
        let shown = if pass < GLITCH_PASSES && mix(x as u64, y as u64, pass).is_multiple_of(3) {
            glyph(x as u64, y as u64, pass)
        } else {
            ch
        };
        put(buf, x, y, shown, st);
    }
}

pub const GLITCH_PASS_TICKS: u64 = 2;
pub const GLITCH_PASSES: u64 = 5;

pub const WARP_TICKS: u64 = 95;

const fn xorshift(seed: &mut u64) -> u64 {
    if *seed == 0 {
        *seed = 0xDEAD_BEEF_CAFE_1337;
    }
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Star {
    angle: f32,
    radius: f32,
    speed: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarpCell {
    pub ch: char,
    pub level: u8,
    pub accent: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Starfield {
    seed: u64,
    stars: Vec<Star>,
    cols: u16,
    rows: u16,
    cells: Vec<Option<WarpCell>>,

    pub frame: u64,
}

fn edge_radius(angle: f32, cx: f32, cy: f32) -> f32 {
    let dx = (angle.cos() * 2.0).abs();
    let dy = angle.sin().abs();
    let rx = if dx > 1e-3 { cx / dx } else { f32::MAX };
    let ry = if dy > 1e-3 { cy / dy } else { f32::MAX };
    rx.min(ry).max(1.0)
}

impl Starfield {
    pub fn new(cols: u16, rows: u16, salt: u64) -> Self {
        use std::f32::consts::PI;
        let rows = rows.max(1);
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15 ^ salt;
        let (cx, cy) = (cols as f32 / 2.0, rows as f32 / 2.0);
        let n = (cols as usize * rows as usize / 4).clamp(80, 2400);
        let stars = (0..n)
            .map(|_| {
                let angle = (xorshift(&mut seed) % 36000) as f32 / 36000.0 * 2.0 * PI;
                Star {
                    angle,
                    radius: (xorshift(&mut seed) % 1000) as f32 / 1000.0
                        * edge_radius(angle, cx, cy),
                    speed: 0.5 + (xorshift(&mut seed) % 100) as f32 / 100.0,
                }
            })
            .collect();
        Self {
            seed,
            stars,
            cols,
            rows,
            cells: vec![None; cols as usize * rows as usize],
            frame: 0,
        }
    }

    pub fn advance(&mut self, accelerating: bool, f: u64) {
        use std::f32::consts::PI;
        let (cols, rows) = (self.cols as usize, self.rows as usize);
        self.cells.iter_mut().for_each(|c| *c = None);
        let cx = cols as f32 / 2.0;
        let cy = rows as f32 / 2.0;
        let max_r = (cx / 2.0).hypot(cy).max(1.0);
        let t = f as f32 / WARP_TICKS as f32;
        let warp_factor = if accelerating {
            0.2 + t * t * 5.0
        } else {
            0.2 + (1.0 - t).powi(2) * 5.0
        };
        let entry_fade = (f as f32 / 8.0).min(1.0);
        for i in 0..self.stars.len() {
            let mut star = self.stars[i];
            let prev = star.radius;
            star.radius += star.speed * warp_factor;
            let (dx, dy) = (star.angle.cos() * 2.0, star.angle.sin());
            let head_x = cx + dx * star.radius;
            let head_y = cy + dy * star.radius;
            if head_x < 0.0 || head_x >= cols as f32 || head_y < 0.0 || head_y >= rows as f32 {
                star.angle = (xorshift(&mut self.seed) % 36000) as f32 / 36000.0 * 2.0 * PI;
                star.radius = (xorshift(&mut self.seed) % 60) as f32 / 100.0;
                star.speed = 0.5 + (xorshift(&mut self.seed) % 100) as f32 / 100.0;
                self.stars[i] = star;
                continue;
            }
            let steps = ((1.0 + warp_factor * 1.4) as usize).max(1);
            for s in 0..=steps {
                let rr = prev + (star.radius - prev) * (s as f32 / steps as f32);
                let x = (cx + dx * rr).round();
                let y = (cy + dy * rr).round();
                if x < 0.0 || y < 0.0 {
                    continue;
                }
                let (xu, yu) = (x as usize, y as usize);
                if xu >= cols || yu >= rows {
                    continue;
                }
                let frac = (rr / max_r).clamp(0.0, 1.0);
                let streak = frac > 0.66 && warp_factor > 2.5;
                let ch = if frac > 0.66 {
                    if streak { '─' } else { '*' }
                } else if frac > 0.33 {
                    '+'
                } else {
                    '·'
                };
                let bright = (frac * 0.7 + warp_factor / 5.2 * 0.3).clamp(0.0, 1.0) * entry_fade;
                let level = (bright * 4.999) as u8;

                self.cells[yu * cols + xu] = Some(WarpCell {
                    ch,
                    level,
                    accent: streak && s == steps && bright > 0.7,
                });
            }
            self.stars[i] = star;
        }
        self.frame = f + 1;
    }

    pub fn size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    pub fn paint(&self, buf: &mut Buffer, area: Rect, dim: u8, t: &Theme) {
        for y in 0..self.rows.min(area.height) {
            for x in 0..self.cols.min(area.width) {
                let Some(c) = self.cells[y as usize * self.cols as usize + x as usize] else {
                    continue;
                };
                let tone = if c.accent {
                    Tone::Accent
                } else {
                    Tone::Ladder(c.level)
                };
                if c.level == 0 && !c.accent {
                    continue;
                }
                if let Some(st) = style(t, tone, dim) {
                    put(buf, area.x + x, area.y + y, c.ch, st);
                }
            }
        }
    }
}

fn sync_field(
    field: &mut Option<Starfield>,
    area: Rect,
    salt: u64,
    accelerating: bool,
    frame: u64,
) {
    let fresh = !matches!(field, Some(f) if f.size() == (area.width, area.height));
    if fresh {
        *field = Some(Starfield::new(area.width, area.height, salt));
    }
    if let Some(f) = field.as_mut() {
        if f.frame > frame + 1 {
            *f = Starfield::new(area.width, area.height, salt);
        }
        while f.frame <= frame {
            let step = f.frame;
            f.advance(accelerating, step);
        }
    }
}

pub const PHRASES: [(&str, u64, u64); 3] = [
    ("Stand up, operator…", 60, 950),
    ("Host stays outside…", 55, 950),
    ("Follow the green.", 50, 850),
];
pub const CAPTION: &str = "Knock, knock, operator.";
pub const CAPTION_HOLD_MS: u64 = 850;

pub const fn phrase_ticks(chars: u64, char_ms: u64, hold_ms: u64) -> u64 {
    (chars * char_ms + hold_ms).div_ceil(TICK_MS)
}
pub const P1_LEN: u64 = phrase_ticks(19, 60, 950);
pub const P2_LEN: u64 = phrase_ticks(19, 55, 950);
pub const P3_LEN: u64 = phrase_ticks(17, 50, 850);
pub const KNOCK_START: u64 = P1_LEN + P2_LEN + P3_LEN;
pub const KNOCK_LEN: u64 = GLITCH_PASSES * GLITCH_PASS_TICKS + CAPTION_HOLD_MS.div_ceil(TICK_MS);
pub const WARP_START: u64 = KNOCK_START + KNOCK_LEN;
pub const INTRO_END: u64 = WARP_START + WARP_TICKS;
pub const REDUCED_HOLD: u64 = 45;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroPhase {
    Phrases,
    Warp,
    Done,
}

impl IntroPhase {
    pub fn of(tick: u64) -> Self {
        if tick < WARP_START {
            IntroPhase::Phrases
        } else if tick < INTRO_END {
            IntroPhase::Warp
        } else {
            IntroPhase::Done
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntroState {
    pub tick: u64,
    pub mode: Motion,
    field: Option<Starfield>,
}

impl IntroState {
    pub fn new(mode: Motion, frame: u64) -> Self {
        Self {
            tick: frame,
            mode,
            field: None,
        }
    }

    pub fn phase(&self) -> IntroPhase {
        match self.mode {
            Motion::Reduced => {
                if self.tick < REDUCED_HOLD {
                    IntroPhase::Phrases
                } else {
                    IntroPhase::Done
                }
            }
            _ => IntroPhase::of(self.tick),
        }
    }

    pub fn is_done(&self) -> bool {
        self.phase() == IntroPhase::Done
    }

    pub fn on_tick(&mut self) -> bool {
        if self.mode == Motion::Paused || self.is_done() {
            return false;
        }
        self.tick += 1;
        true
    }

    pub fn skip(&mut self) {
        match self.mode {
            Motion::Reduced => self.tick = REDUCED_HOLD,
            _ => {
                self.tick = match IntroPhase::of(self.tick) {
                    IntroPhase::Phrases => WARP_START,
                    _ => INTRO_END,
                }
            }
        }
    }
}

fn phrase_at(tick: u64) -> Option<(usize, usize)> {
    let mut start = 0;
    for (i, (text, char_ms, hold_ms)) in PHRASES.iter().enumerate() {
        let n = text.chars().count() as u64;
        let len = phrase_ticks(n, *char_ms, *hold_ms);
        if tick < start + len {
            let k = tick - start;
            let shown = ((k * TICK_MS) / char_ms).min(n) as usize;
            return Some((i, shown));
        }
        start += len;
    }
    None
}

pub fn render_intro(buf: &mut Buffer, area: Rect, state: &mut IntroState, t: &Theme) {
    if area.is_empty() {
        return;
    }
    let (_, cy) = center(area);
    let cy = cy + 1;
    match state.mode {
        Motion::Reduced => {
            fill_canvas(buf, area, t);
            draw_text(buf, area, CAPTION, cy, Tone::Ladder(4), t);
            draw_pill_bottom(buf, area, t);
            draw_hint(buf, area, "Enter", "Continue", t);
            return;
        }
        Motion::Full | Motion::Paused => {}
    }
    let tick = state.tick;
    match IntroPhase::of(tick) {
        IntroPhase::Phrases => {
            fill_canvas(buf, area, t);
            if tick < KNOCK_START {
                if let Some((i, shown)) = phrase_at(tick) {
                    draw_typed(buf, area, PHRASES[i].0, cy, shown, t);
                }
            } else {
                draw_glitched(
                    buf,
                    area,
                    CAPTION,
                    cy,
                    tick - KNOCK_START,
                    Tone::Ladder(4),
                    t,
                );
            }
            draw_pill_bottom(buf, area, t);
            draw_hint(buf, area, "Enter", "Skip", t);
        }
        IntroPhase::Warp => {
            fill_canvas(buf, area, t);
            let f = tick - WARP_START;
            sync_field(&mut state.field, area, 0, true, f);
            if let Some(field) = &state.field {
                field.paint(buf, area, 0, t);
            }
            draw_hint(buf, area, "Enter", "Skip", t);
        }
        IntroPhase::Done => fill_canvas(buf, area, t),
    }
}

pub const OUT_WARP: u64 = WARP_TICKS;

pub const OUT_CAPTION: u64 = GLITCH_PASSES * GLITCH_PASS_TICKS + 2_400u64.div_ceil(TICK_MS);
const OUTRO_SALT: u64 = 0x5F5F_4F55_5452_4F5F;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutroPhase {
    Warp,
    Caption,
    Done,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutroState {
    pub tick: u64,
    pub elapsed_secs: Option<u64>,
    pub mode: Motion,
    field: Option<Starfield>,
}

pub fn format_universe_duration(secs: u64) -> String {
    fn unit(n: u64, name: &str) -> String {
        format!("{n} {name}{}", if n == 1 { "" } else { "s" })
    }
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    if days > 0 {
        format!("{} {}", unit(days, "day"), unit(hours, "hour"))
    } else if hours > 0 {
        format!("{} {}", unit(hours, "hour"), unit(minutes, "minute"))
    } else if minutes > 0 {
        format!("{} {}", unit(minutes, "minute"), unit(seconds, "second"))
    } else {
        unit(seconds, "second")
    }
}

impl OutroState {
    pub fn new(mode: Motion, elapsed_secs: Option<u64>, frame: u64) -> Self {
        Self {
            tick: frame,
            elapsed_secs,
            mode,
            field: None,
        }
    }

    pub fn end(&self) -> u64 {
        match self.mode {
            Motion::Reduced => REDUCED_HOLD,
            _ => {
                if self.elapsed_secs.is_some() {
                    OUT_WARP + OUT_CAPTION
                } else {
                    OUT_WARP
                }
            }
        }
    }

    pub fn phase(&self) -> OutroPhase {
        match self.mode {
            Motion::Reduced => {
                if self.tick < REDUCED_HOLD {
                    OutroPhase::Caption
                } else {
                    OutroPhase::Done
                }
            }
            _ => {
                if self.tick < OUT_WARP {
                    OutroPhase::Warp
                } else if self.tick < self.end() {
                    OutroPhase::Caption
                } else {
                    OutroPhase::Done
                }
            }
        }
    }

    pub fn is_done(&self) -> bool {
        self.phase() == OutroPhase::Done
    }

    pub fn on_tick(&mut self) -> bool {
        if self.mode == Motion::Paused || self.is_done() {
            return false;
        }
        self.tick += 1;
        true
    }

    pub fn skip(&mut self) {
        self.tick = match self.phase() {
            OutroPhase::Warp => OUT_WARP,
            _ => self.end(),
        };
    }

    pub fn caption(&self, place: &str) -> Option<String> {
        self.elapsed_secs
            .map(|s| format!("You were in {place} for {}", format_universe_duration(s)))
    }
}

pub fn render_outro(buf: &mut Buffer, area: Rect, state: &mut OutroState, place: &str, t: &Theme) {
    if area.is_empty() {
        return;
    }
    let (_, cy) = center(area);
    let cy = cy + 1;
    match state.phase() {
        OutroPhase::Warp => {
            fill_canvas(buf, area, t);
            let f = state.tick;
            sync_field(&mut state.field, area, OUTRO_SALT, false, f);
            if let Some(field) = &state.field {
                let dim = if f >= OUT_WARP - 12 {
                    ((f - (OUT_WARP - 12)) / 4) as u8
                } else {
                    0
                };
                field.paint(buf, area, dim, t);
            }
            draw_hint(buf, area, "Enter", "Skip", t);
        }
        OutroPhase::Caption => {
            fill_canvas(buf, area, t);
            match (state.mode, state.caption(place)) {
                (Motion::Reduced, Some(text)) => {
                    draw_text(buf, area, &text, cy, Tone::Ladder(4), t)
                }
                (Motion::Reduced, None) => {}
                (_, Some(text)) => {
                    let j = state.tick - OUT_WARP;
                    draw_glitched(buf, area, &text, cy, j, Tone::Ladder(4), t);
                }
                (_, None) => {}
            }
            draw_pill_bottom(buf, area, t);
            draw_hint(buf, area, "Enter", "Close", t);
        }
        OutroPhase::Done => fill_canvas(buf, area, t),
    }
}

pub fn paint_atmosphere(
    buf: &mut Buffer,
    area: Rect,
    exclude: &[Rect],
    t_local: u64,
    running: bool,
    frozen: bool,
    t: &Theme,
) {
    for x in area.left()..area.right() {
        if pct(mix(x as u64, 11, 0)) >= 18 {
            continue;
        }
        let m = mix(x as u64, 12, 0);
        let period_t = 2 + m % 2;
        let trail = 6 + (m >> 8) % 5;
        let gap = 6 + (m >> 16) % 19;
        let period = area.height as u64 + trail + gap;
        let phase = (m >> 24) % period;
        let signal = (m >> 40).is_multiple_of(10);
        let head = (t_local / period_t + phase) % period;
        let head_y = head as i64 - gap as i64;
        for y in area.top()..area.bottom() {
            if exclude.iter().any(|r| r.contains((x, y).into())) {
                continue;
            }
            let age = head_y - (y - area.y) as i64;
            if !(0..=3).contains(&age) {
                continue;
            }
            let tone = if age == 0 {
                if signal && running && !frozen && t_local >= 15 {
                    Tone::Accent
                } else {
                    Tone::Ladder(1)
                }
            } else {
                Tone::Ladder(0)
            };
            let tone = if t_local < 15 { Tone::Ladder(0) } else { tone };
            if let Some(st) = style(t, tone, 0) {
                put(buf, x, y, glyph(x as u64, y as u64, t_local >> 3), st);
            }
        }
    }
}

pub const HANDOFF_LEN: u64 = 12;

pub fn handoff_stage(h: u64) -> HandoffStage {
    match h {
        0..=3 => HandoffStage::CockpitDim((h + 1) as u8),
        4..=5 => HandoffStage::Canvas,
        6..=10 => HandoffStage::CapsuleDim((10 - h) as u8),
        _ => HandoffStage::Capsule,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandoffStage {
    CockpitDim(u8),
    Canvas,
    CapsuleDim(u8),
    Capsule,
}
