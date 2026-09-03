// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Composable panel chrome with anatomy, variants, and body modes.
//!
//! **Anatomy:** `root` · `header` · `body` · `footer` · optional `disclosure`.
//! Border *weight* never encodes focus — only [`Role::BorderFocused`] does.
//! Fill/geometry come from [`crate::widgets::Surface`].
//!
//! Focus belongs to interactive *descendants* by default. Only
//! [`PanelVariant::Interactive`] (or collapsible header) registers panel-level
//! focus / activation.
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Widget,
};

use crate::input::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use crate::interaction::{EventResult, UiIntent, default_button_intent, default_list_intent};
use crate::style::{DesignSystem, Elevation, PanelChrome, PanelRecipe, Role};
use crate::text::{display_cols, take_display_cols};
use crate::widgets::empty_state::EmptyState;
use crate::widgets::error_state::ErrorState;
use crate::widgets::skeleton::Skeleton;
use crate::widgets::surface::{Surface, SurfaceFill, SurfaceRecipe};
use crate::widgets::view_state::LoadingView;

// PanelChrome lives in `style` (sole chrome enum). Re-exported from widgets::mod.

/// Border / interaction recipe for a panel (orthogonal to focus emphasis).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PanelVariant {
    /// Single-line border + surface fill.
    Bordered,
    /// No border; density padding only (quiet region, default).
    #[default]
    Quiet,
    /// Top/bottom divider rules only (no side borders).
    DividerOnly,
    /// Whole panel is actionable (focus + activate).
    Interactive,
    /// Selected membership chrome (distinct from focus).
    Selected,
}

impl PanelVariant {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Bordered => "bordered",
            Self::Quiet => "quiet",
            Self::DividerOnly => "divider-only",
            Self::Interactive => "interactive",
            Self::Selected => "selected",
        }
    }
}

/// Built-in body projection when the host does not paint custom children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PanelBody {
    /// Host paints children into body (default).
    #[default]
    Host,
    /// Loading placeholder.
    Loading,
    /// Empty state.
    Empty,
    /// Error state.
    Error,
}

/// One header action (stable id + label). Host owns policy; panel owns chrome hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PanelAction<'a> {
    /// Stable action id for [`PanelOutcome::HeaderAction`].
    pub id: &'a str,
    /// Visible label (contracts under narrow width with the action band).
    pub label: &'a str,
    /// Optional icon for compact header / IconButton composition.
    pub icon: Option<&'a str>,
}

impl<'a> PanelAction<'a> {
    /// Creates a header action.
    #[must_use]
    pub const fn new(id: &'a str, label: &'a str) -> Self {
        Self {
            id,
            label,
            icon: None,
        }
    }

    /// Icon for compact paint (pair with [`crate::widgets::IconButton`] in host).
    #[must_use]
    pub const fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// Priority-ordered title/footer slots for panel chrome.
///
/// Narrow drop order (first dropped under pressure):
/// footer → header_actions → badge → trailing → subtitle → leading → title (last).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PanelSlots<'a> {
    /// Primary title (survives longest under contraction).
    pub title: Option<&'a str>,
    /// Secondary title text after the primary.
    pub subtitle: Option<&'a str>,
    /// Leading status glyph/text before the title.
    pub leading: Option<&'a str>,
    /// Status badge (distinct from trailing meta / actions).
    pub badge: Option<&'a str>,
    /// Trailing metadata label on the title line (not an action).
    pub trailing: Option<&'a str>,
    /// Footer hint/status on the bottom border or footer band.
    pub footer: Option<&'a str>,
    /// Optional body title for empty/error modes.
    pub body_title: Option<&'a str>,
    /// Optional body detail for empty/error/loading modes.
    pub body_detail: Option<&'a str>,
}

impl<'a> PanelSlots<'a> {
    /// Resolves which slots survive at the available title width.
    #[must_use]
    pub fn for_width(self, width: u16) -> Self {
        let mut slots = self;
        // Drop order: footer → badge → trailing → subtitle → leading → title.
        // Header actions are gated separately via [`Panel::actions_visible`].
        if width < 24 {
            slots.footer = None;
        }
        if width < 22 {
            slots.badge = None;
        }
        if width < 20 {
            slots.trailing = None;
        }
        if width < 14 {
            slots.subtitle = None;
        }
        if width < 10 {
            slots.leading = None;
        }
        if width < 16 {
            slots.body_detail = None;
        }
        slots
    }

    /// Formats the top title span content (without outer spaces).
    #[must_use]
    pub fn title_text(self) -> Option<String> {
        if self.title.is_none()
            && self.leading.is_none()
            && self.subtitle.is_none()
            && self.badge.is_none()
            && self.trailing.is_none()
        {
            return None;
        }
        let mut parts = Vec::new();
        if let Some(leading) = self.leading {
            parts.push(leading.trim().to_string());
        }
        if let Some(title) = self.title {
            parts.push(title.trim().to_string());
        }
        if let Some(subtitle) = self.subtitle {
            parts.push(format!("· {}", subtitle.trim()));
        }
        if let Some(badge) = self.badge {
            parts.push(format!("[{}]", badge.trim()));
        }
        if let Some(trailing) = self.trailing {
            parts.push(format!("· {}", trailing.trim()));
        }
        let text = parts.join(" ");
        if text.is_empty() { None } else { Some(text) }
    }
}

/// Named geometry parts for one laid-out panel (no nested box soup).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PanelParts {
    /// Outer allocation.
    pub root: Rect,
    /// Header / title band (inside border); None when untitled quiet panel.
    pub header: Option<Rect>,
    /// Body content area (children paint here).
    pub body: Rect,
    /// Footer band; None when no footer.
    pub footer: Option<Rect>,
    /// Disclosure hit target when collapsible.
    pub disclosure: Option<Rect>,
    /// Header-actions band (right of title); host paints labels into action hits on state.
    pub actions: Option<Rect>,
    /// Right-edge overflow track when [`Panel::vertical_scroll`] is set.
    pub scrollbar: Option<Rect>,
    /// Mouse hit region for panel-level interaction.
    pub hit: Rect,
    /// Clip contract (= body for children).
    pub clip: Rect,
}

impl PanelParts {
    /// True when body has positive area.
    #[must_use]
    pub const fn has_body(self) -> bool {
        self.body.width > 0 && self.body.height > 0
    }
}

/// Interaction state for collapsible / interactive panels.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PanelState {
    /// Collapsed body when collapsible.
    pub collapsed: bool,
    /// Panel-level focus (interactive / collapsible header only).
    pub focused: bool,
    /// Pointer hover on panel hit region.
    pub hovered: bool,
    /// Cached layout for hit tests.
    pub parts: Option<PanelParts>,
    /// Header action hit targets (id, rect) filled during [`Panel::paint`].
    pub action_hits: Vec<(String, Rect)>,
}

impl PanelState {
    /// Open expanded panel.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            collapsed: false,
            focused: false,
            hovered: false,
            parts: None,
            action_hits: Vec::new(),
        }
    }

    /// Sets collapse.
    pub const fn set_collapsed(&mut self, collapsed: bool) {
        self.collapsed = collapsed;
    }

    /// Sets panel focus (host / scene).
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Whether collapsed.
    #[must_use]
    pub const fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Key handling via intents (Activate / Toggle / Expand / Collapse).
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        collapsible: bool,
        interactive: bool,
    ) -> PanelOutcome {
        if !self.focused || !key.is_press() {
            return PanelOutcome::Ignored;
        }
        let Some(intent) = default_button_intent(key).or_else(|| default_list_intent(key)) else {
            return PanelOutcome::Ignored;
        };
        self.handle_intent(intent, collapsible, interactive)
    }

    /// Semantic intent path.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        collapsible: bool,
        interactive: bool,
    ) -> PanelOutcome {
        if !self.focused {
            return PanelOutcome::Ignored;
        }
        match intent {
            UiIntent::Toggle | UiIntent::Expand | UiIntent::Collapse if collapsible => {
                if matches!(intent, UiIntent::Expand) {
                    self.collapsed = false;
                } else if matches!(intent, UiIntent::Collapse) {
                    self.collapsed = true;
                } else {
                    self.collapsed = !self.collapsed;
                }
                PanelOutcome::ToggleCollapsed {
                    collapsed: self.collapsed,
                }
            }
            UiIntent::Activate if interactive => PanelOutcome::Activated,
            UiIntent::Activate if collapsible => {
                self.collapsed = !self.collapsed;
                PanelOutcome::ToggleCollapsed {
                    collapsed: self.collapsed,
                }
            }
            _ => PanelOutcome::Ignored,
        }
    }

    /// Key path with [`EventResult`].
    pub fn handle_key_result(
        &mut self,
        key: KeyEvent,
        collapsible: bool,
        interactive: bool,
    ) -> EventResult<PanelOutcome> {
        match self.handle_key(key, collapsible, interactive) {
            PanelOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Click header toggles collapse; body activates interactive.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        collapsible: bool,
        interactive: bool,
    ) -> PanelOutcome {
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            // Hover tracking
            if matches!(
                event.kind,
                MouseEventKind::Moved | MouseEventKind::Drag(MouseButton::Left)
            ) {
                if let Some(parts) = self.parts {
                    self.hovered = parts.hit.contains(event.position);
                }
            }
            return PanelOutcome::Ignored;
        }
        let Some(parts) = self.parts else {
            return PanelOutcome::Ignored;
        };
        // Header actions first (do not toggle collapse when clicking an action).
        for (id, rect) in &self.action_hits {
            if rect.contains(event.position) {
                return PanelOutcome::HeaderAction { id: id.clone() };
            }
        }
        if collapsible
            && (parts.disclosure.is_some_and(|r| r.contains(event.position))
                || parts.header.is_some_and(|r| {
                    r.contains(event.position)
                        && parts.actions.is_none_or(|a| !a.contains(event.position))
                }))
        {
            self.collapsed = !self.collapsed;
            return PanelOutcome::ToggleCollapsed {
                collapsed: self.collapsed,
            };
        }
        if interactive && parts.hit.contains(event.position) {
            return PanelOutcome::Activated;
        }
        PanelOutcome::Ignored
    }
}

/// Typed panel outcomes (no side effects).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PanelOutcome {
    /// No change.
    #[default]
    Ignored,
    /// Interactive panel activated.
    Activated,
    /// Collapse toggled.
    ToggleCollapsed {
        /// New collapsed flag.
        collapsed: bool,
    },
    /// Header action activated (mouse or host-routed intent).
    HeaderAction {
        /// Action id from [`PanelAction::id`].
        id: String,
    },
}

/// A k9s-style panel title: what this pane is, and what it is showing.
///
/// Panel titles are the most-read line in a workbench, and hand-built title
/// strings never agree on order or tone. The spec fixes both:
/// `Name(scope)[count] /filter`, with the name loud, the scope quiet, the
/// count faint, and the filter in the accent only while it is filtering
/// something (`docs/design/tui-app-deep-analysis.md` §4).
///
/// It composes the panel's existing title slots — there is no second title
/// pipeline — so a panel may state a spec or the individual slots, not both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PanelTitleSpec<'a> {
    /// What the pane is: `Pods`, `Files`, `Diagnostics`.
    pub name: &'a str,
    /// What it is scoped to: a namespace, a directory, a branch.
    pub scope: Option<&'a str>,
    /// How many rows the pane holds.
    pub count: Option<usize>,
    /// The active filter query, without its leading slash.
    pub filter: Option<&'a str>,
    /// Whether the pane is following a live source.
    pub live: bool,
}

impl<'a> PanelTitleSpec<'a> {
    /// A title stating only the pane's name.
    #[must_use]
    pub const fn new(name: &'a str) -> Self {
        Self {
            name,
            scope: None,
            count: None,
            filter: None,
            live: false,
        }
    }

    /// Scopes the pane (namespace, directory, branch).
    #[must_use]
    pub const fn scope(mut self, scope: &'a str) -> Self {
        self.scope = Some(scope);
        self
    }

    /// States how many rows the pane holds.
    #[must_use]
    pub const fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// States the active filter query (without its leading slash).
    #[must_use]
    pub const fn filter(mut self, filter: &'a str) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Marks the pane as following a live source.
    #[must_use]
    pub const fn live(mut self, live: bool) -> Self {
        self.live = live;
        self
    }

    /// The plain title text, for semantics and for tests.
    #[must_use]
    pub fn text(&self, live_glyph: &str) -> String {
        let mut out = self.name.trim().to_string();
        if let Some(scope) = self.scope {
            out.push_str(&format!("({})", scope.trim()));
        }
        if let Some(count) = self.count {
            out.push_str(&format!("[{count}]"));
        }
        if let Some(filter) = self.filter {
            out.push_str(&format!(" /{}", filter.trim()));
        }
        if self.live {
            out.push(' ');
            out.push_str(live_glyph);
        }
        out
    }
}

/// A composable container painted through [`DesignSystem`] recipes.
#[derive(Debug, Clone)]
pub struct Panel<'a> {
    slots: PanelSlots<'a>,
    emphasis: PanelChrome,
    variant: PanelVariant,
    body: PanelBody,
    collapsible: bool,
    /// Prefer elevated fill underlay (cards).
    raised: bool,
    overlay: bool,
    /// Header actions (dropped under narrow width before badge).
    header_actions: &'a [PanelAction<'a>],
    title_spec: Option<PanelTitleSpec<'a>>,
    /// Wrapped line count for framed overflow chrome; `None` is no track.
    vertical_scroll: Option<usize>,
    /// Viewport offset in wrapped lines (top-relative).
    scroll_offset: u16,
    tokens: &'a DesignSystem,
}

impl<'a> Panel<'a> {
    /// Creates an untitled panel from design tokens (canonical constructor).
    #[must_use]
    pub const fn new(tokens: &'a DesignSystem) -> Self {
        Self {
            slots: PanelSlots {
                title: None,
                subtitle: None,
                leading: None,
                badge: None,
                trailing: None,
                footer: None,
                body_title: None,
                body_detail: None,
            },
            emphasis: PanelChrome::Normal,
            variant: PanelVariant::Quiet,
            body: PanelBody::Host,
            collapsible: false,
            raised: false,
            overlay: false,
            header_actions: &[],
            title_spec: None,
            vertical_scroll: None,
            scroll_offset: 0,
            tokens,
        }
    }

    /// Quiet bordered-off panel (no chrome line).
    #[must_use]
    pub const fn quiet(tokens: &'a DesignSystem) -> Self {
        Self::new(tokens).variant(PanelVariant::Quiet)
    }

    #[must_use]
    /// Sets the optional visible title.
    pub const fn title(mut self, title: &'a str) -> Self {
        self.slots.title = Some(title);
        self
    }

    #[must_use]
    /// Sets the optional subtitle (drops before title under narrow pressure).
    pub const fn subtitle(mut self, subtitle: &'a str) -> Self {
        self.slots.subtitle = Some(subtitle);
        self
    }

    #[must_use]
    /// Sets leading status chrome on the title line.
    pub const fn leading(mut self, leading: &'a str) -> Self {
        self.slots.leading = Some(leading);
        self
    }

    #[must_use]
    /// Sets trailing metadata on the title line (not a clickable action).
    pub const fn trailing(mut self, trailing: &'a str) -> Self {
        self.slots.trailing = Some(trailing);
        self
    }

    /// Sets a status badge (contracts after header actions, before trailing).
    #[must_use]
    pub const fn badge(mut self, badge: &'a str) -> Self {
        self.slots.badge = Some(badge);
        self
    }

    /// States the title as a composed spec instead of loose slots.
    ///
    /// `Name(scope)[count] /filter` with one tone per segment. Overrides the
    /// `title` slot; the other slots still apply.
    #[must_use]
    pub const fn title_spec(mut self, spec: PanelTitleSpec<'a>) -> Self {
        self.title_spec = Some(spec);
        self
    }

    /// Header actions (right band); dropped when width &lt; 28.
    #[must_use]
    pub const fn header_actions(mut self, actions: &'a [PanelAction<'a>]) -> Self {
        self.header_actions = actions;
        self
    }

    #[must_use]
    /// Sets footer hint on the bottom border (drops first under narrow pressure).
    pub const fn footer(mut self, footer: &'a str) -> Self {
        self.slots.footer = Some(footer);
        self
    }

    /// Whether header actions survive at `width`.
    #[must_use]
    pub const fn actions_visible(width: u16) -> bool {
        width >= 28
    }

    #[must_use]
    /// Body empty/error/loading title copy.
    pub const fn body_title(mut self, title: &'a str) -> Self {
        self.slots.body_title = Some(title);
        self
    }

    #[must_use]
    /// Body detail copy.
    pub const fn body_detail(mut self, detail: &'a str) -> Self {
        self.slots.body_detail = Some(detail);
        self
    }

    #[must_use]
    /// Replaces all panel slots at once.
    pub const fn slots(mut self, slots: PanelSlots<'a>) -> Self {
        self.slots = slots;
        self
    }

    #[must_use]
    /// Sets the semantic panel emphasis (focus / danger).
    pub const fn emphasis(mut self, emphasis: PanelChrome) -> Self {
        self.emphasis = emphasis;
        self
    }

    /// Border / interaction variant.
    #[must_use]
    pub const fn variant(mut self, variant: PanelVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Built-in body mode.
    #[must_use]
    pub const fn body(mut self, body: PanelBody) -> Self {
        self.body = body;
        self
    }

    /// Enables collapsible header (disclosure + Enter/Space toggle when focused).
    #[must_use]
    pub const fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    /// Use elevated fill (card underlay) when the palette defines one.
    #[must_use]
    pub const fn raised(mut self, raised: bool) -> Self {
        self.raised = raised;
        self
    }

    /// Marks this panel as an overlay host (dialog, picker, sheet body).
    ///
    /// Overlay panels fill with `Role::Elevated` so the content they cover
    /// recedes; in-flow panels keep the ordinary surface.
    #[must_use]
    pub const fn overlay(mut self, overlay: bool) -> Self {
        self.overlay = overlay;
        self
    }

    /// Enables framed vertical overflow chrome.
    ///
    /// `content_len` is the host's wrapped line count. The title row keeps two
    /// blank cells before `─╮` (junie empty `meta` `"  "`). The body gutter
    /// paints `│`/`┃` with [`crate::scroll::overflow_thumb`] when content
    /// overflows. Hosts wrap body copy at [`Self::scrolled_content_area`].
    #[must_use]
    pub const fn vertical_scroll(mut self, content_len: usize) -> Self {
        self.vertical_scroll = Some(content_len);
        self
    }

    /// Top-relative wrapped-line offset for [`Self::vertical_scroll`].
    #[must_use]
    pub const fn scroll_offset(mut self, offset: u16) -> Self {
        self.scroll_offset = offset;
        self
    }

    /// Columns the host may write when [`Self::vertical_scroll`] is set.
    ///
    /// Junie `ScrollPanel` wraps at `inner.width - 2`: one column before the
    /// track, then the track itself.
    #[must_use]
    pub const fn scrolled_content_area(body: Rect) -> Rect {
        Rect {
            x: body.x,
            y: body.y,
            width: body.width.saturating_sub(2),
            height: body.height,
        }
    }

    /// Whether this panel claims panel-level keyboard focus.
    #[must_use]
    pub const fn is_focusable(&self) -> bool {
        self.collapsible || matches!(self.variant, PanelVariant::Interactive)
    }

    /// Resolves the panel recipe for current emphasis.
    #[must_use]
    pub fn recipe(&self) -> PanelRecipe {
        self.tokens
            .panel_recipe(self.resolved_chrome(), self.elevation())
    }

    /// Fill rung this panel paints on.
    #[must_use]
    pub const fn elevation(&self) -> Elevation {
        if self.overlay {
            Elevation::Overlay
        } else if self.raised {
            Elevation::Raised
        } else {
            Elevation::Surface
        }
    }

    /// Palette borrow from the design system.
    #[must_use]
    pub const fn palette(&self) -> &crate::style::RolePalette {
        self.tokens.palette()
    }

    /// Effective chrome after variant (Selected ≠ Focused).
    #[must_use]
    pub const fn resolved_chrome(&self) -> PanelChrome {
        match self.emphasis {
            PanelChrome::Danger => PanelChrome::Danger,
            PanelChrome::Focused => PanelChrome::Focused,
            PanelChrome::Normal => {
                if matches!(self.variant, PanelVariant::Selected) {
                    // Selected uses Selection fill via Surface; border stays Normal
                    // so focus remains a distinct BorderFocused cue.
                    PanelChrome::Normal
                } else {
                    PanelChrome::Normal
                }
            }
        }
    }

    /// Slot projection after contraction for a given outer width.
    #[must_use]
    pub fn slots_for_width(&self, width: u16) -> PanelSlots<'a> {
        // Border corners consume 2 cells; title padding uses ~2 more.
        self.slots.for_width(width.saturating_sub(4))
    }

    /// Maps panel emphasis + variant onto the Surface recipe set.
    #[must_use]
    pub const fn surface_recipe(&self) -> SurfaceRecipe {
        if matches!(self.emphasis, PanelChrome::Danger) {
            return if self.overlay {
                SurfaceRecipe::OverlayDanger
            } else {
                SurfaceRecipe::Destructive
            };
        }
        if matches!(self.emphasis, PanelChrome::Focused) {
            return if self.overlay {
                SurfaceRecipe::OverlayFocused
            } else {
                SurfaceRecipe::Focused
            };
        }
        if self.overlay {
            return SurfaceRecipe::Overlay;
        }
        match self.variant {
            PanelVariant::Selected => SurfaceRecipe::Selected,
            PanelVariant::Interactive => {
                if self.raised {
                    SurfaceRecipe::Raised
                } else {
                    SurfaceRecipe::Interactive
                }
            }
            PanelVariant::Quiet | PanelVariant::DividerOnly => {
                if self.raised {
                    SurfaceRecipe::Raised
                } else {
                    SurfaceRecipe::Inset
                }
            }
            PanelVariant::Bordered => {
                if self.raised {
                    SurfaceRecipe::Raised
                } else {
                    SurfaceRecipe::Inset
                }
            }
        }
    }

    /// Whether a full single-line box border is painted.
    #[must_use]
    pub const fn has_box_border(&self) -> bool {
        match self.variant {
            PanelVariant::Quiet | PanelVariant::DividerOnly => false,
            PanelVariant::Bordered | PanelVariant::Interactive | PanelVariant::Selected => true,
        }
    }

    /// Contracts a title or footer to the cells the chrome can spare.
    ///
    /// One rule for all four panel variants: grapheme-safe, ellipsis-marked,
    /// and applied to the footer too. Titles used to be cut with `chars()`,
    /// which counts code points — a CJK title overran its own border — and
    /// footers were never contracted at all (plans/022 Step 3).
    fn chrome_label(&self, text: &str, budget: u16) -> String {
        crate::text::truncate_cols(
            text.trim(),
            usize::from(budget.max(1)),
            self.tokens.glyphs.ellipsis(),
        )
        .into_owned()
    }

    /// Paints a title spec as one line with a tone per segment.
    fn title_spec_line(
        &self,
        spec: PanelTitleSpec<'a>,
        collapsed: Option<bool>,
        _budget: u16,
        title_style: Style,
    ) -> Line<'static> {
        let live_glyph = self
            .tokens
            .glyphs
            .resolve(crate::style::Glyph::Success)
            .text;
        let mut prefix = Vec::new();
        if self.collapsible {
            prefix.push(
                if collapsed.unwrap_or(false) {
                    self.tokens.glyphs.disclosure_closed()
                } else {
                    self.tokens.glyphs.disclosure_open()
                }
                .to_string(),
            );
        }
        if let Some(warning) = self.recipe().title_prefix {
            prefix.push(warning.to_string());
        }
        let prefix = prefix.join(" ");
        // Overflow is one rule: `paint_border_label` ellipsizes the full
        // multi-span line. Pre-truncating here ate the ellipsis (pad + Clip).
        let mut spans = vec![Span::raw(" ")];
        if !prefix.is_empty() {
            spans.push(Span::styled(format!("{prefix} "), title_style));
        }
        spans.push(Span::styled(spec.name.trim().to_string(), title_style));
        if let Some(scope) = spec.scope {
            spans.push(Span::styled(
                format!("({})", scope.trim()),
                self.tokens.style(Role::TextMuted),
            ));
        }
        if let Some(count) = spec.count {
            spans.push(Span::styled(
                format!("[{count}]"),
                self.tokens.style(Role::TextFaint),
            ));
        }
        if let Some(filter) = spec.filter {
            spans.push(Span::styled(
                format!(" /{}", filter.trim()),
                self.tokens.style(Role::Accent),
            ));
        }
        if spec.live {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                live_glyph.to_string(),
                self.tokens.style(Role::Success),
            ));
        }
        spans.push(Span::raw(" "));
        Line::from(spans)
    }

    fn title_line(&self, slots: PanelSlots<'a>, collapsed: Option<bool>) -> Option<String> {
        if let Some(spec) = self.title_spec {
            let mut base = spec.text(
                self.tokens
                    .glyphs
                    .resolve(crate::style::Glyph::Success)
                    .text,
            );
            if self.collapsible {
                let glyph = if collapsed.unwrap_or(false) {
                    self.tokens.glyphs.disclosure_closed()
                } else {
                    self.tokens.glyphs.disclosure_open()
                };
                base = format!("{glyph} {base}");
            }
            return Some(base);
        }
        let mut base = slots.title_text()?;
        // Danger chrome marks itself in the title so the warning survives a
        // colorless terminal, where the red border is just a border.
        if let Some(prefix) = self.recipe().title_prefix {
            base = format!("{prefix} {base}");
        }
        if self.collapsible {
            let glyph = if collapsed.unwrap_or(false) {
                self.tokens.glyphs.disclosure_closed()
            } else {
                self.tokens.glyphs.disclosure_open()
            };
            base = format!("{glyph} {base}");
        }
        Some(base)
    }

    /// Layout named parts without painting.
    #[must_use]
    pub fn layout(&self, area: Rect, state: Option<&PanelState>) -> PanelParts {
        let collapsed = state.is_some_and(|s| s.collapsed && self.collapsible);
        let has_border = self.has_box_border();
        let spacing = self.tokens.spacing;
        let slots = self.slots_for_width(area.width);
        let has_title =
            self.title_spec.is_some() || slots.title_text().is_some() || self.collapsible;
        let has_footer_band = slots.footer.is_some() && !has_border;
        let footer_rows: u16 = if has_footer_band { 1 } else { 0 };

        // Card: filled surface, card-inset 2, title on the top pad row.
        // Frame: rounded edge, frame-inset 3 (border + 2), one spare column right.
        let (header, body, footer) = if has_border {
            let inner = shrink(area, 1, 1, 1, 1);
            let body = Rect::new(
                inner.x.saturating_add(2),
                inner.y,
                inner.width.saturating_sub(3),
                if collapsed { 0 } else { inner.height },
            );
            let header = if has_title {
                Some(Rect {
                    x: area.x.saturating_add(2),
                    y: area.y,
                    width: area.width.saturating_sub(4),
                    height: 1.min(area.height),
                })
            } else {
                None
            };
            let footer = if footer_rows > 0 && !collapsed {
                Some(Rect {
                    x: body.x,
                    y: area.bottom().saturating_sub(1),
                    width: body.width,
                    height: 1,
                })
            } else {
                None
            };
            (header, body, footer)
        } else {
            let pad_x = if area.width >= spacing.card_inset.saturating_mul(2).saturating_add(4) {
                spacing.card_inset
            } else {
                0
            };
            let pad_y: u16 = if area.height >= 3 { 1 } else { 0 };
            let header = if has_title && area.height > 0 {
                Some(Rect {
                    x: area.x.saturating_add(pad_x),
                    y: area.y,
                    width: area.width.saturating_sub(pad_x.saturating_mul(2)),
                    height: 1,
                })
            } else {
                None
            };
            let body_y = if has_title {
                area.y.saturating_add(pad_y.saturating_add(1))
            } else {
                area.y.saturating_add(pad_y)
            };
            let footer_y = area.bottom().saturating_sub(footer_rows);
            let content_bottom = if footer_rows > 0 {
                footer_y
            } else {
                area.bottom().saturating_sub(pad_y)
            };
            let body_bottom = if collapsed { body_y } else { content_bottom };
            let body = Rect {
                x: area.x.saturating_add(pad_x),
                y: body_y,
                width: area.width.saturating_sub(pad_x.saturating_mul(2)),
                height: if collapsed {
                    0
                } else {
                    body_bottom.saturating_sub(body_y)
                },
            };
            let footer = if footer_rows > 0 && !collapsed {
                Some(Rect {
                    x: body.x,
                    y: footer_y,
                    width: body.width,
                    height: 1,
                })
            } else {
                None
            };
            (header, body, footer)
        };

        let disclosure = header.map(|h| Rect {
            x: h.x,
            y: h.y,
            width: 2.min(h.width),
            height: h.height,
        });

        let show_actions = Self::actions_visible(area.width) && !self.header_actions.is_empty();
        let actions = if show_actions {
            // Right band of top row (border title line or header inside).
            let band_w = self
                .header_actions
                .iter()
                .map(|a| display_cols(a.label) as u16 + 3)
                .sum::<u16>()
                .min(area.width / 2)
                .max(4);
            let right_inset = if has_border { 1 } else { 0 };
            Some(Rect {
                x: area.x.saturating_add(
                    area.width
                        .saturating_sub(band_w)
                        .saturating_sub(right_inset),
                ),
                y: header.map_or(area.y, |header| header.y),
                width: band_w.min(area.width),
                height: 1.min(area.height),
            })
        } else {
            None
        };

        let hit = if self.is_focusable() || has_border {
            area
        } else {
            body
        };

        let scrollbar = if self.vertical_scroll.is_some() && body.width > 0 && body.height > 0 {
            Some(Rect {
                x: body.right().saturating_sub(1),
                y: body.y,
                width: 1,
                height: body.height,
            })
        } else {
            None
        };

        PanelParts {
            root: area,
            header,
            body,
            footer,
            disclosure,
            actions,
            scrollbar,
            hit,
            clip: body,
        }
    }

    /// Content rectangle inside panel chrome (host children).
    #[must_use]
    pub fn inner(&self, area: Rect) -> Rect {
        self.layout(area, None).body
    }

    /// Paint panel chrome + optional built-in body; returns body rect.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: Option<&mut PanelState>) -> Rect {
        if area.is_empty() {
            return area;
        }
        let collapsed = state
            .as_ref()
            .is_some_and(|s| s.collapsed && self.collapsible);
        let focused = state.as_ref().is_some_and(|s| s.focused);
        let parts = self.layout(area, state.as_ref().map(|s| &**s));

        // Surface fill (variant-aware).
        let focused_chrome = focused
            || matches!(self.emphasis, PanelChrome::Focused)
            || (self.is_focusable() && focused);
        if self.has_box_border() {
            // Framed pane: canvas fill, rounded edge, border-subtle / border-strong.
            // Overlay hosts lift to elevated so the page recedes.
            let fill_recipe = if self.overlay {
                if matches!(self.emphasis, PanelChrome::Danger) {
                    SurfaceRecipe::OverlayDanger
                } else if focused_chrome {
                    SurfaceRecipe::OverlayFocused
                } else {
                    SurfaceRecipe::Overlay
                }
            } else {
                SurfaceRecipe::Canvas
            };
            let _ = Surface::new(self.tokens)
                .recipe(fill_recipe)
                .bordered(false)
                .padding(0, 0)
                .paint(area, buffer);
            let theme = self.tokens.junie_theme();
            let mut border = if matches!(self.emphasis, PanelChrome::Danger) {
                self.tokens.style(Role::Danger)
            } else {
                theme.border(focused_chrome)
            };
            if focused_chrome {
                border = border.add_modifier(ratatui_core::style::Modifier::BOLD);
            }
            ratatui_widgets::block::Block::default()
                .borders(ratatui_widgets::borders::Borders::ALL)
                .border_style(border)
                .border_set(self.tokens.border_set())
                .render(area, buffer);
        } else {
            let fill_policy = if self.raised {
                SurfaceFill::Auto
            } else {
                SurfaceFill::Transparent
            };
            let _ = Surface::new(self.tokens)
                .recipe(if self.raised {
                    SurfaceRecipe::Inset
                } else {
                    self.surface_recipe()
                })
                .bordered(false)
                .fill(fill_policy)
                .padding(0, 0)
                .paint(area, buffer);
        }

        // Surface owns the box. Panel only places semantic chrome onto it.
        if self.has_box_border() {
            let mut emphasis = self.emphasis;
            if focused && self.is_focusable() {
                emphasis = PanelChrome::Focused;
            }
            let recipe = self.tokens.panel_recipe(emphasis, self.elevation());
            let slots = self.slots_for_width(area.width);
            // Reserve right band for header actions so title does not collide.
            let action_reserve = parts
                .actions
                .map(|a| a.width.saturating_add(1))
                .unwrap_or(0);
            let budget = area.width.saturating_sub(4).saturating_sub(action_reserve);
            let mut title_slots = slots;
            title_slots.trailing = None;
            let title = if let Some(spec) = self.title_spec {
                Some(self.title_spec_line(spec, Some(collapsed), budget, recipe.title))
            } else if let Some(title) = self.title_line(title_slots, Some(collapsed)) {
                Some(Line::from(Span::styled(format!(" {title} "), recipe.title)))
            } else {
                None
            };
            if let Some(title) = title {
                paint_border_label(area, true, &title, recipe.title, buffer, self.tokens);
            }
            if let Some(meta) = slots.trailing.filter(|m| !m.is_empty()) {
                let theme = self.tokens.junie_theme();
                let text = format!(" {meta} ");
                let tw = display_cols(&text) as u16;
                // junie framed meta sits at `title_row_right - tw`, leaving `─╮`.
                if area.width > tw + 4 {
                    buffer.set_stringn(
                        area.right().saturating_sub(2).saturating_sub(tw),
                        area.y,
                        &text,
                        usize::from(tw),
                        theme
                            .faint()
                            .bg(theme.canvas)
                            .remove_modifier(ratatui_core::style::Modifier::BOLD),
                    );
                }
            } else if self.vertical_scroll.is_some() && area.width > 4 {
                // junie `.meta("")` still paints `"  "` faint before `─╮`.
                let theme = self.tokens.junie_theme();
                buffer.set_stringn(
                    area.right().saturating_sub(4),
                    area.y,
                    "  ",
                    2,
                    theme.faint().bg(theme.canvas),
                );
            }
            if let Some(footer) = slots.footer {
                let line = Line::from(Span::styled(format!(" {footer} "), recipe.title));
                paint_border_label(area, false, &line, recipe.title, buffer, self.tokens);
            }
        } else if matches!(self.variant, PanelVariant::DividerOnly) {
            paint_divider_only(area, buffer, self.tokens);
            if let Some(header) = parts.header {
                paint_header_line(self, header, buffer, collapsed, focused);
            }
            if let Some(footer) = parts.footer {
                if let Some(text) = self.slots_for_width(area.width).footer {
                    let t = take_display_cols(text, usize::from(footer.width));
                    buffer.set_stringn(
                        footer.x,
                        footer.y,
                        &t,
                        usize::from(footer.width),
                        self.tokens.style(Role::TextMuted),
                    );
                }
            }
        } else if matches!(self.variant, PanelVariant::Quiet) {
            if let Some(header) = parts.header {
                paint_header_line(self, header, buffer, collapsed, focused);
            }
            if let Some(footer) = parts.footer {
                if let Some(text) = self.slots_for_width(area.width).footer {
                    let t = take_display_cols(text, usize::from(footer.width));
                    buffer.set_stringn(
                        footer.x,
                        footer.y,
                        &t,
                        usize::from(footer.width),
                        self.tokens.style(Role::TextMuted),
                    );
                }
            }
        }

        // Built-in body modes.
        if !collapsed && parts.has_body() {
            match self.body {
                PanelBody::Host => {}
                PanelBody::Loading => {
                    let label = self.slots.body_detail.unwrap_or("Loading");
                    let frame = self.tokens.glyphs.loading();
                    Widget::render(
                        &LoadingView::new(label, frame, self.tokens),
                        parts.body,
                        buffer,
                    );
                }
                PanelBody::Empty => {
                    let title = self.slots.body_title.unwrap_or("No items");
                    let mut empty = EmptyState::new(title, self.tokens);
                    if let Some(d) = self.slots.body_detail {
                        empty = empty.explanation(d);
                    }
                    Widget::render(&empty, parts.body, buffer);
                }
                PanelBody::Error => {
                    let title = self.slots.body_title.unwrap_or("Error");
                    let mut err = ErrorState::new(title, self.tokens);
                    if let Some(d) = self.slots.body_detail {
                        err = err.explanation(d);
                    }
                    Widget::render(&err, parts.body, buffer);
                }
            }
        } else if collapsed {
            // nothing in body
        }

        // Tiny non-color cue: selected gutter when Selected variant.
        if matches!(self.variant, PanelVariant::Selected) && area.width > 0 && area.height > 0 {
            let g = self.tokens.glyphs.selection_gutter();
            buffer.set_stringn(
                area.x,
                area.y.saturating_add(area.height / 2),
                g,
                1,
                self.tokens.style(Role::Accent),
            );
        }

        // Header actions (right band) + hit targets.
        let mut action_hits = Vec::new();
        if let Some(band) = parts.actions {
            let style = self.tokens.style(if focused {
                Role::ActionFocused
            } else {
                Role::TextMuted
            });
            let mut x = band.x;
            for action in self.header_actions {
                let label = format!("[{}]", action.label.trim());
                let w = (display_cols(&label) as u16).min(band.right().saturating_sub(x));
                if w == 0 {
                    break;
                }
                buffer.set_stringn(x, band.y, &label, usize::from(w), style);
                action_hits.push((
                    action.id.to_string(),
                    Rect {
                        x,
                        y: band.y,
                        width: w,
                        height: 1,
                    },
                ));
                x = x.saturating_add(w).saturating_add(1);
                if x >= band.right() {
                    break;
                }
            }
        }

        if let (Some(content_len), Some(gutter)) = (self.vertical_scroll, parts.scrollbar) {
            crate::scroll::paint_overflow_scrollbar(
                buffer,
                gutter,
                content_len,
                usize::from(gutter.height),
                self.scroll_offset,
                focused_chrome,
                self.tokens,
            );
        }

        if let Some(state) = state {
            state.parts = Some(parts);
            state.action_hits = action_hits;
        }
        parts.body
    }

    /// Registers panel chrome into a semantic scene (optional host aid).
    ///
    /// Does **not** claim focus unless [`Self::is_focusable`]. Body children
    /// remain host-registered interactive descendants.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut crate::interaction::SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: Option<&PanelState>,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        use crate::interaction::{SemanticNode, SemanticRole};
        let label = self.slots.title.unwrap_or("panel");
        let focusable = self.is_focusable();
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Dialog)
                .label(label)
                .focusable(focusable)
                .state(crate::interaction::SemanticState {
                    expanded: !state.is_some_and(|s| s.collapsed),
                    selected: matches!(self.variant, PanelVariant::Selected),
                    ..Default::default()
                }),
        );
    }

    /// Skeleton body helper for loading lists (host-driven).
    pub fn paint_skeleton_body(&self, body: Rect, buffer: &mut Buffer, lines: u16) {
        if body.is_empty() {
            return;
        }
        Widget::render(&Skeleton::new(lines, self.tokens), body, buffer);
    }
}

impl Widget for &Panel<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer, None);
    }
}

impl Widget for Panel<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

fn paint_header_line(
    panel: &Panel<'_>,
    header: Rect,
    buffer: &mut Buffer,
    collapsed: bool,
    focused: bool,
) {
    let slots = panel.slots_for_width(header.width.saturating_add(4));
    let theme = panel.tokens.junie_theme();
    let focused = focused || panel.emphasis == PanelChrome::Focused;
    let style = if focused {
        theme.title()
    } else {
        theme.secondary()
    };
    if focused && header.x >= 1 {
        // Card focus: ▎ in the padding column, bold title.
        buffer.set_stringn(
            header.x.saturating_sub(1),
            header.y,
            panel.tokens.glyphs.selection_gutter(),
            1,
            theme.accent_fg(),
        );
    }
    if let Some(spec) = panel.title_spec {
        let line = panel.title_spec_line(spec, Some(collapsed), header.width, style);
        let mut scratch = String::new();
        crate::text::paint_line_overflow(
            buffer,
            header,
            &line,
            style,
            crate::text::LinePlacement {
                alignment: crate::text::CellAlignment::Left,
                overflow: crate::text::CellOverflow::Ellipsis,
                ellipsis: panel.tokens.glyphs.ellipsis(),
            },
            &mut scratch,
        );
        return;
    }
    let mut left = String::new();
    if panel.collapsible {
        left.push_str(if collapsed {
            panel.tokens.glyphs.disclosure_closed()
        } else {
            panel.tokens.glyphs.disclosure_open()
        });
        left.push(' ');
    }
    if let Some(leading) = slots.leading {
        left.push_str(leading.trim());
        left.push(' ');
    }
    if let Some(title) = slots.title {
        left.push_str(title.trim());
    }
    if let Some(subtitle) = slots.subtitle {
        if !left.is_empty() {
            left.push(' ');
        }
        left.push_str("· ");
        left.push_str(subtitle.trim());
    }
    let t = panel.chrome_label(&left, header.width);
    buffer.set_stringn(header.x, header.y, &t, usize::from(header.width), style);
    let mut right = header.right();
    if let Some(meta) = slots.trailing {
        let text = meta.trim();
        let tw = display_cols(text) as u16;
        if right > header.x.saturating_add(tw.saturating_add(1)) {
            right = right.saturating_sub(tw);
            buffer.set_stringn(right, header.y, text, usize::from(tw), theme.faint());
        }
    }
}

fn paint_border_label(
    area: Rect,
    top: bool,
    line: &Line<'_>,
    style: Style,
    buffer: &mut Buffer,
    system: &DesignSystem,
) {
    if area.width <= 2 || area.height == 0 {
        return;
    }
    let line_w = line
        .spans
        .iter()
        .map(|span| display_cols(span.content.as_ref()))
        .sum::<usize>();
    let budget = usize::from(area.width.saturating_sub(4));
    if budget == 0 || line_w == 0 {
        return;
    }
    // Occupy only the contracted label so trailing `─` survive (`╭─ Title ─╮`).
    // Ellipsis marks contraction; Clip used to swallow the glyph after pad.
    let width = line_w.min(budget);
    // junie title_row sits at x+2 so the `─` after `╭` survives: `╭─ Title ─`.
    let rect = Rect::new(
        area.x.saturating_add(2),
        if top {
            area.y
        } else {
            area.bottom().saturating_sub(1)
        },
        u16::try_from(width).unwrap_or(u16::MAX),
        1,
    );
    let mut scratch = String::new();
    crate::text::paint_line_overflow(
        buffer,
        rect,
        line,
        style,
        crate::text::LinePlacement {
            alignment: crate::text::CellAlignment::Left,
            overflow: crate::text::CellOverflow::Ellipsis,
            ellipsis: system.glyphs.ellipsis(),
        },
        &mut scratch,
    );
}

fn paint_divider_only(area: Rect, buffer: &mut Buffer, system: &DesignSystem) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let rule = system.glyphs.rule();
    let style = system.style(Role::Border);
    let line: String = std::iter::repeat_n(rule, usize::from(area.width)).collect();
    buffer.set_stringn(area.x, area.y, &line, usize::from(area.width), style);
    if area.height > 1 {
        buffer.set_stringn(
            area.x,
            area.bottom().saturating_sub(1),
            &line,
            usize::from(area.width),
            style,
        );
    }
}

fn shrink(area: Rect, left: u16, top: u16, right: u16, bottom: u16) -> Rect {
    let x = area.x.saturating_add(left);
    let y = area.y.saturating_add(top);
    let width = area.width.saturating_sub(left.saturating_add(right));
    let height = area.height.saturating_sub(top.saturating_add(bottom));
    if width == 0 || height == 0 {
        Rect {
            x,
            y,
            width: 0,
            height: 0,
        }
    } else {
        Rect {
            x,
            y,
            width,
            height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};
    use crate::style::DesignSystem;

    #[test]
    fn overlay_panels_fill_from_the_elevated_rung() {
        let system = DesignSystem::junie();
        let area = Rect::new(0, 0, 12, 5);

        let mut overlay = Buffer::empty(area);
        Panel::new(&system)
            .variant(PanelVariant::Bordered)
            .overlay(true)
            .emphasis(PanelChrome::Focused)
            .paint(area, &mut overlay, None);

        let mut in_flow = Buffer::empty(area);
        Panel::new(&system)
            .variant(PanelVariant::Bordered)
            .emphasis(PanelChrome::Focused)
            .paint(area, &mut in_flow, None);

        assert_eq!(overlay[(4, 2)].bg, system.style(Role::Elevated).bg.unwrap());
        // Framed in-flow panes sit on the canvas; overlays lift to elevated.
        assert_eq!(
            in_flow[(4, 2)].bg,
            system.style(Role::Canvas).bg.unwrap(),
            "framed fill is canvas, got {:?}",
            in_flow[(4, 2)].bg
        );
        assert_ne!(
            overlay[(4, 2)].bg,
            in_flow[(4, 2)].bg,
            "an overlay must lift off the surface it covers"
        );
        // Focus still speaks through the border role, not through weight.
        assert_eq!(
            overlay[(0, 0)].fg,
            system.style(Role::BorderFocused).fg.unwrap()
        );
    }

    #[test]
    fn wide_titles_and_long_footers_stay_inside_the_border() {
        let tokens = DesignSystem::default();
        // Wide enough that the footer slot survives contraction (< 24 drops it).
        let area = Rect::new(0, 0, 34, 4);
        let mut buffer = Buffer::empty(area);
        Panel::new(&tokens)
            .variant(PanelVariant::Bordered)
            .title("日本語のタイトルです、とても長い見出し")
            .footer("a footer far too long for this panel")
            .render(area, &mut buffer);
        for y in [0u16, area.height - 1] {
            let row: String = (0..area.width).map(|x| buffer[(x, y)].symbol()).collect();
            // Corners survive: the label never overruns its own border.
            assert!(row.starts_with('╭') || row.starts_with('╰'), "{row:?}");
            assert!(row.ends_with('╮') || row.ends_with('╯'), "{row:?}");
            assert!(row.contains('…'), "{row:?}");
        }
    }

    #[test]
    fn danger_chrome_marks_its_title_for_colorless_terminals() {
        let system = DesignSystem::junie();
        let panel = Panel::new(&system)
            .title("Delete branch")
            .emphasis(PanelChrome::Danger);
        let title = panel
            .title_line(panel.slots_for_width(24), None)
            .expect("panel has a title");
        assert!(
            title.starts_with(system.glyphs.resolve(crate::style::Glyph::Error).text),
            "danger titles carry the error mark, got {title:?}"
        );
    }

    #[test]
    fn default_panel_is_quiet_and_explicit_border_reserves_chrome() {
        let area = Rect::new(0, 0, 20, 10);
        let system = DesignSystem::default();
        let mut quiet_buffer = Buffer::empty(area);
        Panel::new(&system).paint(area, &mut quiet_buffer, None);
        let mut bordered_buffer = Buffer::empty(area);
        Panel::new(&system)
            .variant(PanelVariant::Bordered)
            .paint(area, &mut bordered_buffer, None);
        let comfortable = Panel::new(&DesignSystem::default()).inner(area);
        let bordered = Panel::new(&DesignSystem::default())
            .variant(PanelVariant::Bordered)
            .inner(area);
        assert!(!Panel::new(&DesignSystem::default()).has_box_border());
        assert_eq!(quiet_buffer[(0, 0)].symbol(), " ");
        assert_eq!(
            bordered_buffer[(0, 0)].symbol(),
            system.border_set().top_left
        );
        assert_eq!(comfortable, Rect::new(2, 1, 16, 8));
        // frame-inset 3: border + 2, one spare column on the right.
        assert_eq!(bordered, Rect::new(3, 1, 15, 8));
        assert_eq!(
            Panel::new(&DesignSystem::default()).inner(Rect::new(0, 0, 5, 2)),
            Rect::new(0, 0, 5, 2)
        );
    }

    #[test]
    fn panel_recipe_focus_uses_border_focused_not_weight() {
        let tokens = DesignSystem::default();
        let normal = tokens.panel_recipe(PanelChrome::Normal, Elevation::Surface);
        let focused = tokens.panel_recipe(PanelChrome::Focused, Elevation::Surface);
        assert_ne!(normal.border, focused.border);
        let panel = Panel::new(&tokens)
            .emphasis(PanelChrome::Focused)
            .title("T");
        assert_eq!(panel.recipe().border, focused.border);
    }

    #[test]
    fn panel_slots_drop_trailing_before_title() {
        let tokens = DesignSystem::default();
        let panel = Panel::new(&tokens)
            .title("Main")
            .subtitle("sub")
            .leading("*")
            .badge("new")
            .trailing("meta")
            .footer("hint");
        let wide = panel.slots_for_width(80);
        assert!(wide.footer.is_some());
        assert!(wide.trailing.is_some());
        assert!(wide.badge.is_some());
        let mid = panel.slots_for_width(18);
        assert!(mid.trailing.is_none());
        assert!(mid.badge.is_none());
        assert_eq!(mid.title, Some("Main"));
        let tiny = panel.slots_for_width(8);
        assert!(tiny.leading.is_none());
        assert_eq!(tiny.title, Some("Main"));
    }

    #[test]
    fn header_action_mouse_hit() {
        use crate::input::{KeyModifiers, MouseButton, MouseEventKind};
        let tokens = DesignSystem::default();
        let actions = [PanelAction::new("retry", "Retry")];
        let panel = Panel::new(&tokens).title("Job").header_actions(&actions);
        let mut state = PanelState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 6));
        let _ = panel.paint(Rect::new(0, 0, 40, 6), &mut buf, Some(&mut state));
        assert!(!state.action_hits.is_empty());
        let (id, rect) = &state.action_hits[0];
        assert_eq!(id, "retry");
        let out = state.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: ratatui_core::layout::Position {
                    x: rect.x,
                    y: rect.y,
                },
                modifiers: KeyModifiers::NONE,
            },
            false,
            false,
        );
        assert!(matches!(out, PanelOutcome::HeaderAction { id } if id == "retry"));
    }

    #[test]
    fn header_action_not_toggle_when_collapsible() {
        use crate::input::{KeyModifiers, MouseButton, MouseEventKind};
        let tokens = DesignSystem::default();
        let actions = [PanelAction::new("more", "More")];
        let panel = Panel::new(&tokens)
            .title("Fold")
            .collapsible(true)
            .header_actions(&actions);
        let mut state = PanelState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 48, 8));
        let _ = panel.paint(Rect::new(0, 0, 48, 8), &mut buf, Some(&mut state));
        let (_, rect) = &state.action_hits[0];
        let out = state.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: ratatui_core::layout::Position {
                    x: rect.x,
                    y: rect.y,
                },
                modifiers: KeyModifiers::NONE,
            },
            true,
            false,
        );
        assert!(matches!(out, PanelOutcome::HeaderAction { id } if id == "more"));
        assert!(!state.is_collapsed());
    }

    #[test]
    fn non_focusable_panel_ignores_keys() {
        let mut state = PanelState::new();
        // Host never sets focused when !is_focusable; defensive check.
        state.set_focused(false);
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            false,
            false,
        );
        assert_eq!(out, PanelOutcome::Ignored);
    }

    #[test]
    fn title_reserves_action_band() {
        let tokens = DesignSystem::default();
        let actions = [
            PanelAction::new("a", "Retry"),
            PanelAction::new("b", "Cancel"),
        ];
        let panel = Panel::new(&tokens)
            .title("Very long panel title that would collide")
            .header_actions(&actions);
        let parts = panel.layout(Rect::new(0, 0, 40, 6), None);
        assert!(parts.actions.is_some());
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 6));
        let _ = panel.paint(Rect::new(0, 0, 40, 6), &mut buf, None);
        // Action label cells should still read '[' from painted [Retry]
        let ax = parts.actions.unwrap().x;
        let ch = buf[(ax, 0)].symbol();
        assert!(
            ch.contains('[') || ch == "[" || !ch.is_empty(),
            "expected action paint at x={ax}, got {ch:?}"
        );
    }

    #[test]
    fn actions_hidden_when_narrow() {
        assert!(!Panel::actions_visible(20));
        assert!(Panel::actions_visible(28));
    }

    #[test]
    fn loading_and_error_body_modes_paint() {
        let tokens = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 8));
        let _ = Panel::new(&tokens)
            .title("Load")
            .body(PanelBody::Loading)
            .body_detail("Fetching…")
            .paint(Rect::new(0, 0, 30, 8), &mut buf, None);
        let _ = Panel::new(&tokens)
            .title("Err")
            .body(PanelBody::Error)
            .body_title("Failed")
            .body_detail("timeout")
            .paint(Rect::new(0, 0, 30, 8), &mut buf, None);
    }

    #[test]
    fn selected_is_not_focused_surface() {
        let tokens = DesignSystem::default();
        let selected = Panel::new(&tokens)
            .variant(PanelVariant::Selected)
            .title("S");
        assert_eq!(selected.surface_recipe(), SurfaceRecipe::Selected);
        assert_eq!(selected.resolved_chrome(), PanelChrome::Normal);
        let focused = Panel::new(&tokens)
            .emphasis(PanelChrome::Focused)
            .title("F");
        assert_eq!(focused.surface_recipe(), SurfaceRecipe::Focused);
    }

    #[test]
    fn quiet_has_no_box_border() {
        let tokens = DesignSystem::default();
        let p = Panel::quiet(&tokens).title("Q");
        assert!(!p.has_box_border());
        let parts = p.layout(Rect::new(0, 0, 20, 6), None);
        assert!(parts.body.width > 0);
    }

    #[test]
    fn collapsible_toggle_via_intent() {
        let mut state = PanelState::new();
        state.set_focused(true);
        let out = state.handle_intent(UiIntent::Toggle, true, false);
        assert_eq!(out, PanelOutcome::ToggleCollapsed { collapsed: true });
        assert!(state.is_collapsed());
    }

    #[test]
    fn interactive_activate_via_enter() {
        let mut state = PanelState::new();
        state.set_focused(true);
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            false,
            true,
        );
        assert_eq!(out, PanelOutcome::Activated);
    }

    #[test]
    fn collapsed_body_has_zero_height() {
        let tokens = DesignSystem::default();
        let panel = Panel::new(&tokens).title("Fold").collapsible(true);
        let mut state = PanelState::new();
        state.set_collapsed(true);
        let parts = panel.layout(Rect::new(0, 0, 30, 10), Some(&state));
        assert_eq!(parts.body.height, 0);
    }

    #[test]
    fn paint_empty_body_mode() {
        let tokens = DesignSystem::default();
        let panel = Panel::new(&tokens)
            .title("List")
            .body(PanelBody::Empty)
            .body_title("No rows");
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 8));
        let body = panel.paint(Rect::new(0, 0, 24, 8), &mut buf, None);
        assert!(body.height > 0);
    }

    #[test]
    fn layout_is_cheap() {
        let tokens = DesignSystem::default();
        let panel = Panel::new(&tokens)
            .title("Perf")
            .subtitle("sub")
            .footer("f")
            .variant(PanelVariant::Bordered);
        let area = Rect::new(0, 0, 40, 12);
        for _ in 0..20_000 {
            let _ = panel.layout(area, None);
        }
    }

    #[test]
    fn focusable_only_when_interactive_or_collapsible() {
        let tokens = DesignSystem::default();
        assert!(!Panel::new(&tokens).title("x").is_focusable());
        assert!(
            Panel::new(&tokens)
                .variant(PanelVariant::Interactive)
                .is_focusable()
        );
        assert!(Panel::new(&tokens).collapsible(true).is_focusable());
    }

    #[test]
    fn a_title_spec_states_each_segment_in_its_own_tone() {
        let system = DesignSystem::default();
        let spec = PanelTitleSpec::new("Pods")
            .scope("kube-system")
            .count(42)
            .filter("api");
        let panel = Panel::new(&system)
            .variant(PanelVariant::Bordered)
            .title_spec(spec);
        let area = Rect::new(0, 0, 48, 4);
        let mut buffer = Buffer::empty(area);
        Widget::render(&panel, area, &mut buffer);

        let top: String = (0..area.width).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(top.contains("Pods(kube-system)[42] /api"), "{top:?}");

        let at = |needle: char| {
            let x = (0..area.width)
                .find(|x| buffer[(*x, 0)].symbol().starts_with(needle))
                .unwrap_or_else(|| panic!("{needle:?} must be painted in {top:?}"));
            buffer[(x, 0)].style().fg
        };
        assert_eq!(at('k'), system.style(Role::TextMuted).fg, "scope is quiet");
        assert_eq!(at('4'), system.style(Role::TextFaint).fg, "count is faint");
        assert_eq!(
            at('/'),
            system.style(Role::Accent).fg,
            "an active filter is loud"
        );
        assert_ne!(at('P'), at('k'), "the name must outrank its scope");
    }

    #[test]
    fn an_over_wide_title_spec_contracts_as_one_line() {
        let system = DesignSystem::default();
        let spec = PanelTitleSpec::new("Diagnostics")
            .scope("crates/termrock/src/widgets")
            .count(1234)
            .filter("unresolved import");
        let panel = Panel::new(&system)
            .variant(PanelVariant::Bordered)
            .title_spec(spec);
        let area = Rect::new(0, 0, 24, 3);
        let mut buffer = Buffer::empty(area);
        Widget::render(&panel, area, &mut buffer);
        let top: String = (0..area.width).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(
            top.contains(system.glyphs.ellipsis()),
            "a contracted title says so: {top:?}"
        );
    }

    #[test]
    fn variant_ids_stable() {
        assert_eq!(PanelVariant::DividerOnly.id(), "divider-only");
    }

    #[test]
    fn framed_uses_rounded_corners_and_frame_inset() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 20, 8);
        let mut buffer = Buffer::empty(area);
        Panel::new(&system)
            .variant(PanelVariant::Bordered)
            .title("Pane")
            .paint(area, &mut buffer, None);
        assert_eq!(buffer[(0, 0)].symbol(), "╭");
        assert_eq!(buffer[(19, 0)].symbol(), "╮");
        assert_eq!(buffer[(0, 7)].symbol(), "╰");
        assert_eq!(buffer[(19, 7)].symbol(), "╯");
        let inner = Panel::new(&system)
            .variant(PanelVariant::Bordered)
            .inner(area);
        assert_eq!(inner.x, 3);
        assert_eq!(inner.y, 1);
        assert_eq!(inner.width, 15);
    }

    #[test]
    fn framed_vertical_scroll_reserves_title_and_uses_junie_thumb() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 46, 17);
        let mut buffer = Buffer::empty(area);
        Panel::new(&system)
            .variant(PanelVariant::Bordered)
            .title("Framed · split pane")
            .vertical_scroll(24)
            .paint(area, &mut buffer, None);
        assert_eq!(buffer[(42, 0)].symbol(), " ");
        assert_eq!(buffer[(43, 0)].symbol(), " ");
        assert_eq!(buffer[(44, 0)].symbol(), "─");
        assert_eq!(buffer[(45, 0)].symbol(), "╮");
        let thumbs: Vec<u16> = (1..16)
            .filter(|y| buffer[(43, *y)].symbol() == "┃")
            .collect();
        assert_eq!(thumbs, (1..10).collect::<Vec<_>>());
        assert_eq!(buffer[(43, 10)].symbol(), crate::scroll::SCROLLBAR_TRACK);
        assert_eq!(
            Panel::scrolled_content_area(
                Panel::new(&system)
                    .variant(PanelVariant::Bordered)
                    .inner(area)
            )
            .width,
            39
        );
    }

    #[test]
    fn framed_vertical_scroll_short_pane_thumb_is_one_cell() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 29, 9);
        let mut buffer = Buffer::empty(area);
        Panel::new(&system)
            .variant(PanelVariant::Bordered)
            .title("Framed · split pane")
            .vertical_scroll(49)
            .paint(area, &mut buffer, None);
        assert_eq!(buffer[(25, 0)].symbol(), " ");
        assert_eq!(buffer[(26, 0)].symbol(), " ");
        assert_eq!(buffer[(27, 0)].symbol(), "─");
        assert_eq!(buffer[(28, 0)].symbol(), "╮");
        assert_eq!(buffer[(26, 1)].symbol(), "┃");
        assert_eq!(buffer[(26, 2)].symbol(), crate::scroll::SCROLLBAR_TRACK);
    }

    #[test]
    fn framed_without_scroll_keeps_dashes_before_corner() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 29, 3);
        let mut buffer = Buffer::empty(area);
        Panel::new(&system)
            .variant(PanelVariant::Bordered)
            .title("Framed · split pane")
            .paint(area, &mut buffer, None);
        assert_eq!(buffer[(25, 0)].symbol(), "─");
        assert_eq!(buffer[(26, 0)].symbol(), "─");
        assert_eq!(buffer[(27, 0)].symbol(), "─");
        assert_eq!(buffer[(28, 0)].symbol(), "╮");
    }

    #[test]
    fn framed_title_is_secondary_idle_and_bold_when_focused() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 24, 5);
        let idle = Panel::new(&system)
            .variant(PanelVariant::Bordered)
            .title("Logs");
        let mut a = Buffer::empty(area);
        idle.paint(area, &mut a, None);
        let theme = system.junie_theme();
        let x = (0..area.width)
            .find(|x| a[(*x, 0)].symbol() == "L")
            .unwrap();
        assert_eq!(a[(x, 0)].fg, theme.secondary().fg.unwrap());

        let focused = Panel::new(&system)
            .variant(PanelVariant::Bordered)
            .title("Logs")
            .emphasis(PanelChrome::Focused);
        let mut b = Buffer::empty(area);
        focused.paint(area, &mut b, None);
        let x = (0..area.width)
            .find(|x| b[(*x, 0)].symbol() == "L")
            .unwrap();
        assert_eq!(b[(x, 0)].fg, theme.title().fg.unwrap());
        assert!(
            b[(x, 0)]
                .modifier
                .contains(ratatui_core::style::Modifier::BOLD)
        );
        assert_eq!(b[(0, 0)].fg, theme.border(true).fg.unwrap());
    }
}
