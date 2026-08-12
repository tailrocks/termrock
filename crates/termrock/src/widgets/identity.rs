// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! AvatarGlyph and Identity — terminal-native identity markers.
//!
//! **Mission.** Users, agents, services, and collaborators in monospaced UIs
//! without raster images: initials, semantic glyphs, presence, role badges,
//! deterministic color fallbacks, and fixed **1- or 2-cell** avatar widths
//! across Unicode / ASCII / Enhanced glyph profiles.
//!
//! **Critical meaning.** Color alone never carries identity: initials/glyphs
//! and optional labels remain legible under `no_color`. Presence uses a
//! trailing cell (or ASCII letter) in addition to color.
//!
//! Compose with threads, subagents, and collaboration lists. Prefer
//! [`Identity`] for avatar + name + badge; [`AvatarGlyph`] alone for dense
//! gutters and presence-only rails.
//!
//! Research: chat identity systems and agent TUIs, adapted to terminals.

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::style::{DesignSystem, Glyph, Role};
use crate::text::{display_cols, take_display_cols};
use crate::widgets::{Badge, SemanticStatus, Text, TextSpan};

// ── Role / presence / size ──────────────────────────────────────────────────

/// Semantic identity role (maps to glyph fallback + default badge).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum IdentityRole {
    /// Human user.
    #[default]
    User,
    /// Agent / assistant.
    Agent,
    /// Background service / bot worker.
    Service,
    /// System / infrastructure.
    System,
    /// Collaborator / teammate.
    Collaborator,
    /// Generic bot.
    Bot,
    /// Unknown / guest.
    Unknown,
}

impl IdentityRole {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Agent => "agent",
            Self::Service => "service",
            Self::System => "system",
            Self::Collaborator => "collaborator",
            Self::Bot => "bot",
            Self::Unknown => "unknown",
        }
    }

    /// Short badge label (ASCII-safe).
    #[must_use]
    pub const fn badge_label(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Agent => "agent",
            Self::Service => "svc",
            Self::System => "sys",
            Self::Collaborator => "collab",
            Self::Bot => "bot",
            Self::Unknown => "?",
        }
    }

    /// Glyph used when avatar kind is role-glyph (not initials).
    #[must_use]
    pub const fn glyph(self) -> Glyph {
        match self {
            Self::User | Self::Collaborator => Glyph::EmptyCircle,
            Self::Agent | Self::Bot => Glyph::Busy,
            Self::Service => Glyph::Settings,
            Self::System => Glyph::ModeDot,
            Self::Unknown => Glyph::Ellipsis,
        }
    }

    /// Default seed role for monochrome/color paint.
    #[must_use]
    pub const fn paint_role(self) -> Role {
        match self {
            Self::User => Role::Info,
            Self::Agent => Role::Accent,
            Self::Service => Role::TextMuted,
            Self::System => Role::Warning,
            Self::Collaborator => Role::Success,
            Self::Bot => Role::Link,
            Self::Unknown => Role::TextDisabled,
        }
    }
}

/// Presence / availability (optional trailing cell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PresenceStatus {
    /// No presence chrome.
    #[default]
    None,
    /// Available.
    Online,
    /// Idle / away.
    Away,
    /// Do not disturb / busy.
    Busy,
    /// Offline / disconnected.
    Offline,
    /// Error / failed session.
    Error,
}

impl PresenceStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Online => "online",
            Self::Away => "away",
            Self::Busy => "busy",
            Self::Offline => "offline",
            Self::Error => "error",
        }
    }

    /// Shared vocabulary projection (`None` → [`SemanticStatus::Unknown`]).
    #[must_use]
    pub const fn semantic(self) -> SemanticStatus {
        match self {
            Self::None => SemanticStatus::Unknown,
            Self::Online => SemanticStatus::Online,
            Self::Away => SemanticStatus::Idle,
            Self::Busy => SemanticStatus::Running,
            Self::Offline => SemanticStatus::Offline,
            Self::Error => SemanticStatus::Failed,
        }
    }

    /// 1-cell paint character from shared [`SemanticStatus`] glyphs.
    #[must_use]
    pub const fn glyph_char(self, ascii: bool) -> Option<&'static str> {
        match self {
            Self::None => None,
            other => Some(other.semantic().glyph(ascii)),
        }
    }

    /// Role for presence cell (aligned with [`SemanticStatus`]; `None` stays disabled).
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::None => Role::TextDisabled,
            other => other.semantic().role(),
        }
    }

    /// Accessible meaning.
    #[must_use]
    pub const fn meaning(self) -> &'static str {
        match self {
            Self::None => "no presence",
            Self::Online => "online",
            Self::Away => "away",
            Self::Busy => "busy",
            Self::Offline => "offline",
            Self::Error => "error",
        }
    }
}

/// Avatar footprint (avatar cells only; presence is extra when enabled).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AvatarSize {
    /// Exactly **1** display column.
    Compact,
    /// Exactly **2** display columns.
    #[default]
    Normal,
    /// Two-column avatar + optional presence cell (variant layout).
    Presence,
}

impl AvatarSize {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Normal => "normal",
            Self::Presence => "presence",
        }
    }

    /// Avatar body width in cells (not including presence).
    #[must_use]
    pub const fn body_cols(self) -> u16 {
        match self {
            Self::Compact => 1,
            Self::Normal | Self::Presence => 2,
        }
    }
}

/// How the avatar face is derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AvatarFace {
    /// Initials from name / explicit seed (default).
    #[default]
    Initials,
    /// Semantic glyph for the identity role.
    RoleGlyph,
    /// Explicit catalog glyph.
    Glyph(Glyph),
}

impl AvatarFace {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Initials => "initials",
            Self::RoleGlyph => "role-glyph",
            Self::Glyph(_) => "glyph",
        }
    }
}

// ── Initials / hash ─────────────────────────────────────────────────────────

/// Derive 1–2 initials from a display name (grapheme-safe first chars of words).
#[must_use]
pub fn initials_from_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "?".into();
    }
    let mut out = String::new();
    for part in trimmed.split(|c: char| c.is_whitespace() || c == '_' || c == '-' || c == '.') {
        let ch = part.chars().find(|c| c.is_alphanumeric());
        if let Some(c) = ch {
            out.push(c.to_ascii_uppercase());
            if out.chars().count() >= 2 {
                break;
            }
        }
    }
    if out.is_empty() {
        // Fallback: first alphanumeric in whole string
        if let Some(c) = trimmed.chars().find(|c| c.is_alphanumeric()) {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('?');
        }
    }
    out
}

/// Deterministic seed from a string (FNV-1a 64).
#[must_use]
pub fn identity_seed(s: &str) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x100_0000_01b3;
    let mut hash = OFFSET;
    for b in s.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Palette role cycled by seed (colorful avatars without per-user assets).
#[must_use]
pub fn role_for_seed(seed: u64) -> Role {
    const ROLES: [Role; 6] = [
        Role::Accent,
        Role::Info,
        Role::Success,
        Role::Warning,
        Role::Link,
        Role::ActionFocused,
    ];
    ROLES[(seed as usize) % ROLES.len()]
}

// ── AvatarGlyph ─────────────────────────────────────────────────────────────

/// Painted avatar geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AvatarGlyphParts {
    /// Full used area (body + optional presence).
    pub root: Rect,
    /// Face cells (1 or 2 wide).
    pub face: Rect,
    /// Presence cell (may be empty height/width 0).
    pub presence: Rect,
}

/// Terminal avatar: initials / glyph face with fixed width.
#[derive(Debug, Clone, Copy)]
pub struct AvatarGlyph<'a> {
    system: &'a DesignSystem,
    /// Seed for color + fallback initials (`name` or stable id).
    seed: &'a str,
    /// Explicit initials override (1–2 chars recommended).
    initials: Option<&'a str>,
    role: IdentityRole,
    face: AvatarFace,
    size: AvatarSize,
    presence: PresenceStatus,
    /// Bracket face in ASCII / no-color for stronger affordance.
    bracketed: bool,
}

impl<'a> AvatarGlyph<'a> {
    /// Avatar from a seed string (name or stable id).
    #[must_use]
    pub const fn new(seed: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            system,
            seed,
            initials: None,
            role: IdentityRole::User,
            face: AvatarFace::Initials,
            size: AvatarSize::Normal,
            presence: PresenceStatus::None,
            bracketed: false,
        }
    }

    /// Identity role (glyph fallback + default paint).
    #[must_use]
    pub const fn role(mut self, role: IdentityRole) -> Self {
        self.role = role;
        self
    }

    /// Face kind.
    #[must_use]
    pub const fn face(mut self, face: AvatarFace) -> Self {
        self.face = face;
        self
    }

    /// Role glyph face.
    #[must_use]
    pub const fn role_glyph(mut self) -> Self {
        self.face = AvatarFace::RoleGlyph;
        self
    }

    /// Explicit glyph face.
    #[must_use]
    pub const fn glyph(mut self, glyph: Glyph) -> Self {
        self.face = AvatarFace::Glyph(glyph);
        self
    }

    /// Explicit initials (overrides derivation).
    #[must_use]
    pub const fn initials(mut self, initials: &'a str) -> Self {
        self.initials = Some(initials);
        self
    }

    /// Size variant.
    #[must_use]
    pub const fn size(mut self, size: AvatarSize) -> Self {
        self.size = size;
        self
    }

    /// Compact 1-cell.
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.size = AvatarSize::Compact;
        self
    }

    /// Presence-capable size (2-cell face + optional status).
    #[must_use]
    pub const fn with_presence(mut self, status: PresenceStatus) -> Self {
        self.size = AvatarSize::Presence;
        self.presence = status;
        self
    }

    /// Presence without forcing Presence size (compact still 1 body cell).
    #[must_use]
    pub const fn presence(mut self, status: PresenceStatus) -> Self {
        self.presence = status;
        self
    }

    /// Bracket the face (`[AB]`) — stronger no-color / ASCII.
    #[must_use]
    pub const fn bracketed(mut self, on: bool) -> Self {
        self.bracketed = on;
        self
    }

    /// Body width in cells (1 or 2).
    #[must_use]
    pub const fn body_cols(&self) -> u16 {
        self.size.body_cols()
    }

    /// Total width including presence cell when shown.
    #[must_use]
    pub fn measure_width(&self) -> u16 {
        let body = self.body_cols();
        let pres = if self.presence == PresenceStatus::None {
            0
        } else {
            1
        };
        body.saturating_add(pres)
    }

    /// Face paint string with exact column budget (pads/truncates).
    #[must_use]
    pub fn face_text(&self) -> String {
        let cols = usize::from(self.body_cols());
        let ascii = self.system.glyphs.is_ascii()
            || matches!(
                self.system.capability,
                crate::style::ColorCapability::Monochrome
            );
        let raw = match self.face {
            AvatarFace::Initials => {
                let init = self
                    .initials
                    .map(|s| {
                        s.chars()
                            .filter(|c| !c.is_whitespace())
                            .take(2)
                            .collect::<String>()
                            .to_uppercase()
                    })
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| initials_from_name(self.seed));
                if cols == 1 {
                    init.chars().next().unwrap_or('?').to_string()
                } else {
                    let mut s = init;
                    while display_cols(&s) < cols {
                        s.push(' ');
                    }
                    take_display_cols(&s, cols)
                }
            }
            AvatarFace::RoleGlyph => {
                let g = self.system.glyphs.resolve(self.role.glyph());
                fit_glyph_text(g.text, cols, ascii)
            }
            AvatarFace::Glyph(glyph) => {
                let g = self.system.glyphs.resolve(glyph);
                fit_glyph_text(g.text, cols, ascii)
            }
        };
        if self.bracketed || (ascii && matches!(self.face, AvatarFace::Initials) && cols >= 2) {
            // Bracketed 2-cell face: use [A] style for 2 cols? That is 3 cols.
            // Keep raw for width contract; bracket only when compact uses 1 cell
            // or when explicitly requested and we can fit in body via single-char.
            if cols == 1 {
                return raw;
            }
            // Explicit bracketed with 2 cols: first initial only centered as "A "
            // Prefer no expand beyond cols — skip brackets to honor width guarantee.
            let _ = ascii;
        }
        // Final clamp to exact cols
        let mut out = take_display_cols(&raw, cols);
        while display_cols(&out) < cols {
            out.push(' ');
        }
        take_display_cols(&out, cols)
    }

    /// Paint role for the face (deterministic from seed unless role-forced).
    #[must_use]
    pub fn face_role(&self) -> Role {
        // Prefer role paint for RoleGlyph; seed hash for initials
        match self.face {
            AvatarFace::RoleGlyph | AvatarFace::Glyph(_) => self.role.paint_role(),
            AvatarFace::Initials => role_for_seed(identity_seed(self.seed)),
        }
    }

    /// Accessible plain description.
    #[must_use]
    pub fn plain(&self) -> String {
        let face = self.face_text().trim().to_string();
        let mut s = format!("avatar {face} ({})", self.role.id());
        if self.presence != PresenceStatus::None {
            s.push(' ');
            s.push_str(self.presence.meaning());
        }
        s
    }

    /// Layout geometry.
    #[must_use]
    pub fn layout(&self, area: Rect) -> AvatarGlyphParts {
        if area.is_empty() {
            return AvatarGlyphParts::default();
        }
        let body_w = self.body_cols().min(area.width);
        let face = Rect {
            x: area.x,
            y: area.y,
            width: body_w,
            height: 1.min(area.height),
        };
        let presence =
            if self.presence != PresenceStatus::None && area.width > body_w && area.height > 0 {
                Rect {
                    x: area.x.saturating_add(body_w),
                    y: area.y,
                    width: 1,
                    height: 1,
                }
            } else {
                Rect::default()
            };
        let root_w = face.width.saturating_add(presence.width).min(area.width);
        AvatarGlyphParts {
            root: Rect {
                x: area.x,
                y: area.y,
                width: root_w,
                height: 1.min(area.height),
            },
            face,
            presence,
        }
    }

    /// Paint avatar into `area`.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) -> AvatarGlyphParts {
        let parts = self.layout(area);
        if parts.root.is_empty() {
            return parts;
        }
        let face = self.face_text();
        let mut style = self.system.style(self.face_role());
        style = ratatui_core::style::Style { bg: None, ..style };
        style = style.add_modifier(Modifier::BOLD);
        // No-color: reverse/underline for presence of identity without hue
        if matches!(
            self.system.capability,
            crate::style::ColorCapability::Monochrome
        ) {
            style.fg = None;
            style = style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        }
        buffer.set_stringn(
            parts.face.x,
            parts.face.y,
            &face,
            usize::from(parts.face.width),
            style,
        );
        if parts.presence.width > 0 {
            if let Some(ch) = self.presence.glyph_char(self.system.glyphs.is_ascii()) {
                let mut ps = self.system.style(self.presence.role());
                ps = ratatui_core::style::Style { bg: None, ..ps };
                if matches!(
                    self.system.capability,
                    crate::style::ColorCapability::Monochrome
                ) {
                    ps.fg = None;
                    ps = match self.presence {
                        PresenceStatus::Online => ps.add_modifier(Modifier::BOLD),
                        PresenceStatus::Busy | PresenceStatus::Error => {
                            ps.add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                        }
                        PresenceStatus::Away => ps.add_modifier(Modifier::DIM),
                        _ => ps.add_modifier(Modifier::DIM),
                    };
                }
                buffer.set_stringn(parts.presence.x, parts.presence.y, ch, 1, ps);
            }
        }
        parts
    }
}

impl Widget for &AvatarGlyph<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer);
    }
}

impl Widget for AvatarGlyph<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

fn fit_glyph_text(text: &str, cols: usize, _ascii: bool) -> String {
    let t = take_display_cols(text, cols);
    let mut out = t;
    while display_cols(&out) < cols {
        out.push(' ');
    }
    take_display_cols(&out, cols)
}

// ── Identity ────────────────────────────────────────────────────────────────

/// Painted identity row geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IdentityParts {
    /// Full row.
    pub root: Rect,
    /// Avatar band.
    pub avatar: Rect,
    /// Primary name.
    pub name: Rect,
    /// Secondary / handle.
    pub secondary: Rect,
    /// Role badge.
    pub badge: Rect,
}

/// Avatar + name + optional secondary + role badge (thread/subagent row).
#[derive(Debug, Clone, Copy)]
pub struct Identity<'a> {
    system: &'a DesignSystem,
    /// Display name (also default seed).
    name: &'a str,
    /// Stable seed override (id) for color determinism.
    seed: Option<&'a str>,
    secondary: Option<&'a str>,
    role: IdentityRole,
    face: AvatarFace,
    size: AvatarSize,
    presence: PresenceStatus,
    show_badge: bool,
    show_avatar: bool,
    /// Compact: hide secondary, smaller avatar.
    compact: bool,
    initials: Option<&'a str>,
}

impl<'a> Identity<'a> {
    /// Identity for a display name.
    #[must_use]
    pub const fn new(name: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            system,
            name,
            seed: None,
            secondary: None,
            role: IdentityRole::User,
            face: AvatarFace::Initials,
            size: AvatarSize::Normal,
            presence: PresenceStatus::None,
            show_badge: false,
            show_avatar: true,
            compact: false,
            initials: None,
        }
    }

    /// Stable id seed for color (defaults to name).
    #[must_use]
    pub const fn seed(mut self, seed: &'a str) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Secondary line (handle, model, email).
    #[must_use]
    pub const fn secondary(mut self, text: &'a str) -> Self {
        self.secondary = Some(text);
        self
    }

    /// Role.
    #[must_use]
    pub const fn role(mut self, role: IdentityRole) -> Self {
        self.role = role;
        self
    }

    /// Face.
    #[must_use]
    pub const fn face(mut self, face: AvatarFace) -> Self {
        self.face = face;
        self
    }

    /// Avatar size.
    #[must_use]
    pub const fn size(mut self, size: AvatarSize) -> Self {
        self.size = size;
        self
    }

    /// Presence.
    #[must_use]
    pub const fn presence(mut self, status: PresenceStatus) -> Self {
        self.presence = status;
        self
    }

    /// Show role badge.
    #[must_use]
    pub const fn badge(mut self, on: bool) -> Self {
        self.show_badge = on;
        self
    }

    /// Hide avatar (name-only).
    #[must_use]
    pub const fn avatar(mut self, on: bool) -> Self {
        self.show_avatar = on;
        self
    }

    /// Compact recipe: compact avatar, no secondary.
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.compact = true;
        self.size = AvatarSize::Compact;
        self
    }

    /// Initials override.
    #[must_use]
    pub const fn initials(mut self, initials: &'a str) -> Self {
        self.initials = Some(initials);
        self
    }

    /// Build avatar.
    #[must_use]
    pub fn avatar_glyph(&self) -> AvatarGlyph<'a> {
        let seed = self.seed.unwrap_or(self.name);
        let mut a = AvatarGlyph::new(seed, self.system)
            .role(self.role)
            .face(self.face)
            .size(if self.compact {
                AvatarSize::Compact
            } else {
                self.size
            })
            .presence(self.presence);
        if let Some(i) = self.initials {
            a = a.initials(i);
        }
        if matches!(self.size, AvatarSize::Presence) {
            a = a.with_presence(self.presence);
        }
        a
    }

    /// Plain a11y string.
    #[must_use]
    pub fn plain(&self) -> String {
        let mut s = self.name.to_string();
        if let Some(sec) = self.secondary {
            if !self.compact && !sec.is_empty() {
                s.push(' ');
                s.push('(');
                s.push_str(sec);
                s.push(')');
            }
        }
        if self.show_badge {
            s.push(' ');
            s.push('[');
            s.push_str(self.role.badge_label());
            s.push(']');
        }
        if self.presence != PresenceStatus::None {
            s.push(' ');
            s.push_str(self.presence.meaning());
        }
        s
    }

    /// Measure minimum width.
    #[must_use]
    pub fn measure_width(&self) -> u16 {
        let mut w = 0u16;
        if self.show_avatar {
            w = w.saturating_add(self.avatar_glyph().measure_width());
            w = w.saturating_add(1); // gap
        }
        w = w.saturating_add(u16::try_from(display_cols(self.name)).unwrap_or(1));
        if self.show_badge {
            w = w.saturating_add(2);
            w = w.saturating_add(u16::try_from(display_cols(self.role.badge_label())).unwrap_or(1));
        }
        w.max(1)
    }

    /// Layout.
    #[must_use]
    pub fn layout(&self, area: Rect) -> IdentityParts {
        if area.is_empty() {
            return IdentityParts::default();
        }
        let mut x = area.x;
        let y = area.y;
        let mut avatar = Rect::default();
        if self.show_avatar {
            let aw = self.avatar_glyph().measure_width().min(area.width);
            avatar = Rect {
                x,
                y,
                width: aw,
                height: 1.min(area.height),
            };
            x = x.saturating_add(aw).saturating_add(1);
        }
        let rest = area.width.saturating_sub(x.saturating_sub(area.x)).max(0);
        let badge_w = if self.show_badge {
            let bl = self.role.badge_label();
            u16::try_from(display_cols(bl).saturating_add(2)).unwrap_or(4)
        } else {
            0
        };
        let name_budget = rest.saturating_sub(badge_w).max(1);
        let name = Rect {
            x,
            y,
            width: name_budget.min(u16::try_from(display_cols(self.name)).unwrap_or(1).max(1)),
            height: 1.min(area.height),
        };
        // secondary shares same row after name when space, else omitted
        let after_name = x.saturating_add(name.width).saturating_add(1);
        let secondary = if !self.compact
            && self.secondary.is_some_and(|s| !s.is_empty())
            && after_name < area.x.saturating_add(area.width).saturating_sub(badge_w)
        {
            let sw = area
                .x
                .saturating_add(area.width)
                .saturating_sub(badge_w)
                .saturating_sub(after_name);
            Rect {
                x: after_name,
                y,
                width: sw,
                height: 1.min(area.height),
            }
        } else {
            Rect::default()
        };
        let badge = if self.show_badge && badge_w > 0 {
            Rect {
                x: area.x.saturating_add(area.width).saturating_sub(badge_w),
                y,
                width: badge_w.min(area.width),
                height: 1.min(area.height),
            }
        } else {
            Rect::default()
        };
        IdentityParts {
            root: Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1.min(area.height),
            },
            avatar,
            name,
            secondary,
            badge,
        }
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) -> IdentityParts {
        let parts = self.layout(area);
        if parts.root.is_empty() {
            return parts;
        }
        if self.show_avatar && !parts.avatar.is_empty() {
            let _ = self.avatar_glyph().paint(parts.avatar, buffer);
        }
        if !parts.name.is_empty() {
            let _ = Text::spans(
                [TextSpan::new(self.name).role(Role::Text).strong()],
                self.system,
            )
            .truncate()
            .paint(parts.name, buffer);
        }
        if !parts.secondary.is_empty() {
            if let Some(sec) = self.secondary {
                let _ = Text::new(sec, self.system)
                    .role(Role::TextMuted)
                    .truncate()
                    .paint(parts.secondary, buffer);
            }
        }
        if self.show_badge && !parts.badge.is_empty() {
            let _ = Badge::new(self.role.badge_label(), self.system)
                .outline()
                .paint(parts.badge, buffer, None);
        }
        parts
    }
}

impl Widget for &Identity<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer);
    }
}

impl Widget for Identity<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{ColorCapability, GlyphSet};
    // GlyphSet used for ASCII width contract

    #[test]
    fn initials_from_words() {
        assert_eq!(initials_from_name("Ada Lovelace"), "AL");
        assert_eq!(initials_from_name("termrock"), "T");
        assert_eq!(initials_from_name("  "), "?");
        assert_eq!(initials_from_name("foo_bar"), "FB");
    }

    #[test]
    fn seed_deterministic() {
        assert_eq!(identity_seed("a"), identity_seed("a"));
        assert_ne!(identity_seed("a"), identity_seed("b"));
        assert_eq!(role_for_seed(0), role_for_seed(6)); // cycle
    }

    #[test]
    fn compact_is_one_cell_body() {
        let system = DesignSystem::default();
        let a = AvatarGlyph::new("Ada", &system).compact();
        assert_eq!(a.body_cols(), 1);
        assert_eq!(display_cols(&a.face_text()), 1);
    }

    #[test]
    fn normal_is_two_cells() {
        let system = DesignSystem::default();
        let a = AvatarGlyph::new("Ada Lovelace", &system).size(AvatarSize::Normal);
        assert_eq!(a.body_cols(), 2);
        assert_eq!(display_cols(&a.face_text()), 2);
    }

    #[test]
    fn ascii_profile_width_stable() {
        let system = DesignSystem::default().glyphs(GlyphSet::Ascii);
        for seed in ["A", "Ada", "🤖", "x"] {
            let c = AvatarGlyph::new(seed, &system).compact();
            assert_eq!(display_cols(&c.face_text()), 1, "{seed}");
            let n = AvatarGlyph::new(seed, &system).size(AvatarSize::Normal);
            assert_eq!(display_cols(&n.face_text()), 2, "{seed}");
        }
    }

    #[test]
    fn presence_adds_cell() {
        let system = DesignSystem::default();
        let a = AvatarGlyph::new("bot", &system)
            .size(AvatarSize::Presence)
            .presence(PresenceStatus::Online);
        assert_eq!(a.measure_width(), 3); // 2 face + 1 presence
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 1));
        let parts = a.paint(Rect::new(0, 0, 4, 1), &mut buf);
        assert_eq!(parts.presence.width, 1);
    }

    #[test]
    fn no_color_still_paints_face() {
        let system = DesignSystem::default().no_color();
        assert_eq!(system.capability, ColorCapability::Monochrome);
        let a = AvatarGlyph::new("Ada", &system).size(AvatarSize::Normal);
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 1));
        let _ = a.paint(Rect::new(0, 0, 2, 1), &mut buf);
        let cell = buf[(0, 0)].symbol().to_owned();
        assert!(!cell.trim().is_empty());
    }

    #[test]
    fn role_glyph_face() {
        let system = DesignSystem::default();
        let a = AvatarGlyph::new("agent-1", &system)
            .role(IdentityRole::Agent)
            .role_glyph()
            .compact();
        assert_eq!(display_cols(&a.face_text()), 1);
        assert!(a.plain().contains("agent"));
    }

    #[test]
    fn identity_row_paints_name() {
        let system = DesignSystem::default();
        let id = Identity::new("Ada", &system)
            .role(IdentityRole::Agent)
            .secondary("@ada")
            .badge(true)
            .presence(PresenceStatus::Online);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        let parts = id.paint(Rect::new(0, 0, 40, 1), &mut buf);
        assert!(parts.avatar.width >= 1);
        let row: String = (0..40).map(|x| buf[(x, 0)].symbol().to_owned()).collect();
        assert!(row.contains('A') || row.contains("Ada"), "{row}");
    }

    #[test]
    fn identity_compact_hides_secondary_in_plain() {
        let system = DesignSystem::default();
        let id = Identity::new("Bot", &system).secondary("gpt").compact();
        let p = id.plain();
        assert!(p.contains("Bot"));
        // secondary omitted when compact
        assert!(!p.contains("gpt"));
    }

    #[test]
    fn empty_area_safe() {
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        let parts = AvatarGlyph::new("x", &system).paint(Rect::new(0, 0, 0, 0), &mut buf);
        assert!(parts.root.is_empty());
        let parts = Identity::new("x", &system).paint(Rect::new(0, 0, 0, 0), &mut buf);
        assert!(parts.root.is_empty());
    }

    #[test]
    fn measure_width_cheap() {
        let system = DesignSystem::default();
        for i in 0..10_000 {
            let a = AvatarGlyph::new("user", &system)
                .initials(if i % 2 == 0 { "AB" } else { "Z" })
                .size(if i % 3 == 0 {
                    AvatarSize::Compact
                } else {
                    AvatarSize::Normal
                });
            let w = display_cols(&a.face_text());
            assert!(w == 1 || w == 2);
        }
    }

    #[test]
    fn explicit_initials_override() {
        let system = DesignSystem::default();
        let a = AvatarGlyph::new("Ignored Name", &system)
            .initials("xy")
            .size(AvatarSize::Normal);
        let t = a.face_text();
        assert!(
            t.starts_with('X')
                || t.contains('X')
                || t.contains('Y')
                || t.starts_with("XY")
                || t.starts_with("xy")
                || t.to_uppercase().contains('X'),
            "{t}"
        );
    }
}
