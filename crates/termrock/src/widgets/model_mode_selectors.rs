// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **ModelSelector** and **AgentModeSelector** — compact selectors for model,
//! reasoning effort, agent mode, and execution policy.
//!
//! **Mission.** Current choice, provider, capabilities, cost/latency/context
//! metadata, availability, warnings, and recent choices. **Separate** model
//! selection from mode selection while allowing composed presentation. Contract
//! to concise status text in the composer; expand into searchable selection.
//! Show consequential changes clearly (permissions / cost). Provider data is
//! **application-owned** — no provider SDK or catalog fetch.
//!
//! Research: Amp, OpenCode, Grok Build, model pickers in AI tools.
//!
//! **vs [`ModeRibbon`](crate::widgets::ModeRibbon).** ModeRibbon is a generic
//! workbench strip. AgentModeSelector owns safety-mode semantics (FullAuto
//! warning) and composer badge contraction.
//!
//! **vs [`Select`](crate::widgets::Select).** Select is a form field. These
//! selectors are composer/status chrome with metadata rows and consequence cues.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{NavigationMove, UiIntent, default_button_intent, default_list_intent},
    style::{ButtonRecipeVariant, ControlState, DesignSystem, ListRowVisualState, Role},
    text::{display_cols, take_display_cols},
    widgets::{
        agent_blocks::WorkbenchMode,
        prompt_composer::{ModeIndicator, ModelIndicator},
        select::SelectOption,
    },
};

/// Overlay id hint for model popover.
pub const MODEL_SELECTOR_OVERLAY_ID: &str = "termrock.model_select";
/// Overlay id hint for mode popover.
pub const MODE_SELECTOR_OVERLAY_ID: &str = "termrock.mode_select";
/// Max recent model ids retained.
pub const MODEL_RECENT_CAP: usize = 8;

// ── Model domain ────────────────────────────────────────────────────────────

/// Host-projected capability tag (display only).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelCapability {
    /// Stable tag id (e.g. `vision`, `tools`).
    pub id: String,
    /// Short label.
    pub label: String,
}

impl ModelCapability {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// Availability of a model option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ModelAvailability {
    /// Selectable.
    #[default]
    Available,
    /// Temporarily unavailable.
    Unavailable,
    /// Deprecated but selectable with warning.
    Deprecated,
    /// Requires upgrade / entitlement (host).
    Restricted,
}

impl ModelAvailability {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Unavailable => "unavailable",
            Self::Deprecated => "deprecated",
            Self::Restricted => "restricted",
        }
    }

    /// Whether user may confirm.
    #[must_use]
    pub const fn is_selectable(self) -> bool {
        matches!(self, Self::Available | Self::Deprecated)
    }
}

/// Optional reasoning effort knob (host maps to API).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ReasoningEffort {
    /// Host default.
    #[default]
    Default,
    /// Lower latency / cost.
    Low,
    /// Balanced.
    Medium,
    /// Higher reasoning.
    High,
    /// Maximum (host-defined).
    Max,
}

impl ReasoningEffort {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Max => "max",
        }
    }

    /// Compact status token.
    #[must_use]
    pub const fn short(self) -> &'static str {
        match self {
            Self::Default => "def",
            Self::Low => "low",
            Self::Medium => "med",
            Self::High => "high",
            Self::Max => "max",
        }
    }
}

/// One host-projected model row (no provider I/O).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelOption {
    /// Stable model id.
    pub id: String,
    /// Display name.
    pub label: String,
    /// Provider name (display).
    pub provider: Option<String>,
    /// Capability tags.
    pub capabilities: Vec<ModelCapability>,
    /// Context window size (tokens); 0 = unknown.
    pub context_tokens: u64,
    /// Relative cost hint (host string, e.g. `$` / `$$`).
    pub cost_hint: Option<String>,
    /// Latency hint (host string).
    pub latency_hint: Option<String>,
    /// Availability.
    pub availability: ModelAvailability,
    /// Warning text (deprecation, high cost).
    pub warning: Option<String>,
    /// Group header key.
    pub group: Option<String>,
    /// Recent flag (also set by selector state).
    pub recent: bool,
    /// Description / detail line.
    pub detail: Option<String>,
}

impl ModelOption {
    /// Available model.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            provider: None,
            capabilities: Vec::new(),
            context_tokens: 0,
            cost_hint: None,
            latency_hint: None,
            availability: ModelAvailability::Available,
            warning: None,
            group: None,
            recent: false,
            detail: None,
        }
    }

    /// Provider.
    #[must_use]
    pub fn provider(mut self, p: impl Into<String>) -> Self {
        self.provider = Some(p.into());
        self
    }

    /// Capabilities.
    #[must_use]
    pub fn capabilities(mut self, caps: Vec<ModelCapability>) -> Self {
        self.capabilities = caps;
        self
    }

    /// Context window.
    #[must_use]
    pub const fn context_tokens(mut self, n: u64) -> Self {
        self.context_tokens = n;
        self
    }

    /// Cost hint.
    #[must_use]
    pub fn cost_hint(mut self, c: impl Into<String>) -> Self {
        self.cost_hint = Some(c.into());
        self
    }

    /// Latency hint.
    #[must_use]
    pub fn latency_hint(mut self, l: impl Into<String>) -> Self {
        self.latency_hint = Some(l.into());
        self
    }

    /// Availability.
    #[must_use]
    pub const fn availability(mut self, a: ModelAvailability) -> Self {
        self.availability = a;
        self
    }

    /// Warning.
    #[must_use]
    pub fn warning(mut self, w: impl Into<String>) -> Self {
        self.warning = Some(w.into());
        self
    }

    /// Group.
    #[must_use]
    pub fn group(mut self, g: impl Into<String>) -> Self {
        self.group = Some(g.into());
        self
    }

    /// Detail.
    #[must_use]
    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }

    /// Recent.
    #[must_use]
    pub const fn recent(mut self, on: bool) -> Self {
        self.recent = on;
        self
    }

    /// Concise status line for composer chrome.
    #[must_use]
    pub fn status_text(&self, ascii: bool) -> String {
        let mut s = self.label.clone();
        if let Some(p) = &self.provider {
            s.push('/');
            s.push_str(p);
        }
        if self.context_tokens > 0 {
            s.push(' ');
            s.push_str(&format_context(self.context_tokens));
        }
        if let Some(c) = &self.cost_hint {
            s.push(' ');
            s.push_str(c);
        }
        if self.warning.is_some() {
            s.push(if ascii { '!' } else { '⚠' });
        }
        s
    }

    /// Row detail under label (capabilities · latency).
    #[must_use]
    pub fn row_meta(&self) -> String {
        let mut parts = Vec::new();
        if !self.capabilities.is_empty() {
            parts.push(
                self.capabilities
                    .iter()
                    .map(|c| c.label.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
        if let Some(l) = &self.latency_hint {
            parts.push(l.clone());
        }
        if let Some(w) = &self.warning {
            parts.push(w.clone());
        }
        if !self.availability.is_selectable() {
            parts.push(self.availability.id().into());
        }
        parts.join(" · ")
    }
}

/// Filter models by query (label/provider/id/capability).
#[must_use]
pub fn filter_model_options<'a>(options: &'a [ModelOption], query: &str) -> Vec<&'a ModelOption> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return options.iter().collect();
    }
    options
        .iter()
        .filter(|o| {
            let caps = o
                .capabilities
                .iter()
                .map(|c| c.label.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let hay = format!(
                "{} {} {} {}",
                o.id,
                o.label,
                o.provider.as_deref().unwrap_or(""),
                caps
            )
            .to_ascii_lowercase();
            hay.contains(&q)
        })
        .collect()
}

/// Project to Select rows (host may open Select overlay).
#[must_use]
pub fn models_to_select_options(options: &[ModelOption]) -> Vec<SelectOption<String>> {
    options
        .iter()
        .map(|o| {
            let mut row = SelectOption::option(o.id.clone(), o.label.clone())
                .disabled(!o.availability.is_selectable());
            let meta = o.row_meta();
            if !meta.is_empty() {
                row = row.description(meta);
            }
            row
        })
        .collect()
}

/// Bridge to PromptComposer model badge.
#[must_use]
pub fn model_to_indicator(option: &ModelOption) -> ModelIndicator {
    ModelIndicator {
        label: option.label.clone(),
    }
}

// ── Agent mode domain ───────────────────────────────────────────────────────

/// Standard agent safety / autonomy modes (host may subset or extend via Custom).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AgentModeKind {
    /// Ask-only / suggest.
    Ask,
    /// Plan without apply.
    Plan,
    /// Edit with confirmation.
    #[default]
    Edit,
    /// Auto-approve within policy.
    Auto,
    /// Full autonomy — **warning** chrome required.
    FullAuto,
    /// Host-defined mode id carried separately.
    Custom,
}

impl AgentModeKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Plan => "plan",
            Self::Edit => "edit",
            Self::Auto => "auto",
            Self::FullAuto => "full-auto",
            Self::Custom => "custom",
        }
    }

    /// Compact badge label.
    #[must_use]
    pub const fn short(self) -> &'static str {
        match self {
            Self::Ask => "ASK",
            Self::Plan => "PLAN",
            Self::Edit => "EDIT",
            Self::Auto => "AUTO",
            Self::FullAuto => "FULL",
            Self::Custom => "MODE",
        }
    }

    /// Whether FullAuto-class warning applies.
    #[must_use]
    pub const fn is_warning(self) -> bool {
        matches!(self, Self::FullAuto)
    }

    /// Parse common short labels.
    #[must_use]
    pub fn from_short(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "ASK" => Some(Self::Ask),
            "PLAN" => Some(Self::Plan),
            "EDIT" => Some(Self::Edit),
            "AUTO" => Some(Self::Auto),
            "FULL" | "FULLAUTO" | "FULL-AUTO" => Some(Self::FullAuto),
            _ => None,
        }
    }
}

/// Execution policy display (host-owned meaning).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ExecutionPolicyKind {
    /// Default host policy.
    #[default]
    Default,
    /// Read-only tools.
    ReadOnly,
    /// Workspace write.
    WorkspaceWrite,
    /// Network allowed.
    Network,
    /// Unrestricted (warning).
    Unrestricted,
}

impl ExecutionPolicyKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::Network => "network",
            Self::Unrestricted => "unrestricted",
        }
    }

    /// Warning chrome.
    #[must_use]
    pub const fn is_warning(self) -> bool {
        matches!(self, Self::Unrestricted | Self::Network)
    }

    /// Short badge.
    #[must_use]
    pub const fn short(self) -> &'static str {
        match self {
            Self::Default => "pol",
            Self::ReadOnly => "ro",
            Self::WorkspaceWrite => "ww",
            Self::Network => "net",
            Self::Unrestricted => "all",
        }
    }
}

/// One mode option (host projects labels and consequences).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentModeOption {
    /// Stable id (often equals kind.id() or custom).
    pub id: String,
    /// Kind for warning chrome.
    pub kind: AgentModeKind,
    /// Full label.
    pub label: String,
    /// Compact badge.
    pub short_label: String,
    /// Permission / safety consequence description (shown on change).
    pub consequence: Option<String>,
    /// Cost / autonomy warning.
    pub warning: Option<String>,
    /// Enabled.
    pub enabled: bool,
    /// Optional execution policy attached to this mode.
    pub execution_policy: Option<ExecutionPolicyKind>,
}

impl AgentModeOption {
    /// From standard kind with default labels.
    #[must_use]
    pub fn from_kind(kind: AgentModeKind) -> Self {
        let (label, consequence) = match kind {
            AgentModeKind::Ask => ("Ask", Some("Suggest only; no apply")),
            AgentModeKind::Plan => ("Plan", Some("Plan without tool writes")),
            AgentModeKind::Edit => ("Edit", Some("Edits require confirmation")),
            AgentModeKind::Auto => ("Auto", Some("Auto-approve within policy")),
            AgentModeKind::FullAuto => (
                "Full auto",
                Some("High autonomy — review permissions carefully"),
            ),
            AgentModeKind::Custom => ("Custom", None),
        };
        Self {
            id: kind.id().into(),
            kind,
            label: label.into(),
            short_label: kind.short().into(),
            consequence: consequence.map(str::to_string),
            warning: if kind.is_warning() {
                Some("elevated permissions".into())
            } else {
                None
            },
            enabled: true,
            execution_policy: None,
        }
    }

    /// Custom mode.
    #[must_use]
    pub fn custom(
        id: impl Into<String>,
        label: impl Into<String>,
        short: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: AgentModeKind::Custom,
            label: label.into(),
            short_label: short.into(),
            consequence: None,
            warning: None,
            enabled: true,
            execution_policy: None,
        }
    }

    /// Consequence text.
    #[must_use]
    pub fn consequence(mut self, c: impl Into<String>) -> Self {
        self.consequence = Some(c.into());
        self
    }

    /// Warning.
    #[must_use]
    pub fn warning(mut self, w: impl Into<String>) -> Self {
        self.warning = Some(w.into());
        self
    }

    /// Enabled.
    #[must_use]
    pub const fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// Execution policy.
    #[must_use]
    pub const fn execution_policy(mut self, p: ExecutionPolicyKind) -> Self {
        self.execution_policy = Some(p);
        self
    }

    /// Whether warning role applies.
    #[must_use]
    pub fn needs_warning_role(&self) -> bool {
        self.kind.is_warning()
            || self.warning.is_some()
            || self
                .execution_policy
                .is_some_and(ExecutionPolicyKind::is_warning)
    }

    /// Compact status for composer.
    #[must_use]
    pub fn status_text(&self) -> String {
        self.short_label.clone()
    }
}

/// Default mode ladder (Ask → FullAuto).
#[must_use]
pub fn default_agent_modes() -> Vec<AgentModeOption> {
    vec![
        AgentModeOption::from_kind(AgentModeKind::Ask),
        AgentModeOption::from_kind(AgentModeKind::Plan),
        AgentModeOption::from_kind(AgentModeKind::Edit),
        AgentModeOption::from_kind(AgentModeKind::Auto),
        AgentModeOption::from_kind(AgentModeKind::FullAuto),
    ]
}

/// Project modes to ModeRibbon rows.
#[must_use]
pub fn modes_to_ribbon<'a>(
    modes: &'a [AgentModeOption],
    selected_id: Option<&str>,
) -> Vec<WorkbenchMode<'a, &'a str>> {
    modes
        .iter()
        .map(|m| WorkbenchMode {
            id: m.id.as_str(),
            label: m.short_label.as_str(),
            active: selected_id == Some(m.id.as_str()),
            enabled: m.enabled,
        })
        .collect()
}

/// Bridge to PromptComposer mode badge.
#[must_use]
pub fn mode_to_indicator(mode: &AgentModeOption) -> ModeIndicator {
    ModeIndicator {
        label: mode.short_label.clone(),
        warning: mode.needs_warning_role(),
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Model selector outcomes (host applies selection / opens overlays).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelSelectorOutcome {
    /// Ignored.
    Ignored,
    /// Expanded list opened.
    Opened,
    /// List closed without commit.
    Closed,
    /// Cursor moved in list.
    HighlightChanged {
        /// Model id.
        id: String,
    },
    /// Search query changed.
    SearchChanged {
        /// Query.
        query: String,
    },
    /// Confirmed model (may carry consequence warning for host dialog).
    Confirmed {
        /// Model id.
        id: String,
        /// Warning text if cost/deprecation.
        warning: Option<String>,
    },
    /// Reasoning effort changed (optional secondary control).
    ReasoningChanged {
        /// Effort.
        effort: ReasoningEffort,
    },
    /// Compact status activated (open request).
    ActivateCompact,
}

/// Agent mode selector outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AgentModeSelectorOutcome {
    /// Ignored.
    Ignored,
    /// Menu opened.
    Opened,
    /// Menu closed without change.
    Closed,
    /// Highlight in menu.
    HighlightChanged {
        /// Mode id.
        id: String,
    },
    /// Mode confirmed.
    ModeChanged {
        /// Mode id.
        id: String,
        /// Kind for chrome.
        kind: AgentModeKind,
        /// Permission consequence text.
        consequence: Option<String>,
        /// Whether host should show confirm dialog (FullAuto).
        needs_confirm: bool,
    },
    /// Execution policy badge activated.
    PolicyChanged {
        /// Policy.
        policy: ExecutionPolicyKind,
        /// Warning.
        warning: bool,
    },
    /// Compact badge activated.
    ActivateCompact,
}

// ── ModelSelector state / paint ─────────────────────────────────────────────

/// Presentation of the model control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ModelSelectorPresentation {
    /// Single-line status for composer.
    #[default]
    Compact,
    /// Expanded searchable list.
    Expanded,
}

impl ModelSelectorPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Expanded => "expanded",
        }
    }
}

/// Model selector state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSelectorState {
    /// Committed model id.
    selected: Option<String>,
    /// List highlight id.
    pub highlight: Option<String>,
    /// Presentation.
    pub presentation: ModelSelectorPresentation,
    /// Search draft when expanded.
    pub search: String,
    /// Optional reasoning effort.
    pub reasoning: ReasoningEffort,
    /// Focus / accepts input.
    accepts_input: bool,
    /// Recent model ids (MRU).
    recent: Vec<String>,
    /// Focused for keyboard.
    focused: bool,
}

impl Default for ModelSelectorState {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelSelectorState {
    /// Empty selection.
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected: None,
            highlight: None,
            presentation: ModelSelectorPresentation::Compact,
            search: String::new(),
            reasoning: ReasoningEffort::Default,
            accepts_input: true,
            recent: Vec::new(),
            focused: true,
        }
    }

    /// With initial model id.
    #[must_use]
    pub fn with_selected(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            selected: Some(id.clone()),
            highlight: Some(id),
            ..Self::new()
        }
    }

    /// Selected id.
    #[must_use]
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Highlight id.
    #[must_use]
    pub fn highlight(&self) -> Option<&str> {
        self.highlight.as_deref()
    }

    /// Accepts input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Gate (does not clear selection).
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Set selected without outcome.
    pub fn set_selected(&mut self, id: Option<String>) {
        self.selected = id.clone();
        self.highlight = id;
    }

    /// Recent ids.
    #[must_use]
    pub fn recent(&self) -> &[String] {
        &self.recent
    }

    /// Open expanded list.
    pub fn open(&mut self) -> ModelSelectorOutcome {
        if !self.accepts_input {
            return ModelSelectorOutcome::Ignored;
        }
        if self.presentation == ModelSelectorPresentation::Expanded {
            return ModelSelectorOutcome::Ignored;
        }
        self.presentation = ModelSelectorPresentation::Expanded;
        self.highlight = self.selected.clone();
        self.search.clear();
        ModelSelectorOutcome::Opened
    }

    /// Close without commit.
    pub fn close(&mut self) -> ModelSelectorOutcome {
        if self.presentation == ModelSelectorPresentation::Compact {
            return ModelSelectorOutcome::Ignored;
        }
        self.presentation = ModelSelectorPresentation::Compact;
        self.search.clear();
        self.highlight = self.selected.clone();
        ModelSelectorOutcome::Closed
    }

    /// Confirm highlight as selection.
    pub fn confirm(&mut self, options: &[ModelOption]) -> ModelSelectorOutcome {
        let Some(id) = self.highlight.clone() else {
            return ModelSelectorOutcome::Ignored;
        };
        let opt = options.iter().find(|o| o.id == id);
        if let Some(o) = opt {
            if !o.availability.is_selectable() {
                return ModelSelectorOutcome::Ignored;
            }
            self.selected = Some(id.clone());
            self.push_recent(&id);
            self.presentation = ModelSelectorPresentation::Compact;
            self.search.clear();
            return ModelSelectorOutcome::Confirmed {
                id,
                warning: o.warning.clone().or_else(|| {
                    if matches!(o.availability, ModelAvailability::Deprecated) {
                        Some("deprecated model".into())
                    } else {
                        None
                    }
                }),
            };
        }
        ModelSelectorOutcome::Ignored
    }

    fn push_recent(&mut self, id: &str) {
        self.recent.retain(|x| x != id);
        self.recent.insert(0, id.to_string());
        if self.recent.len() > MODEL_RECENT_CAP {
            self.recent.pop();
        }
    }

    /// Visible options (search + recent bias mark).
    #[must_use]
    pub fn visible<'a>(&self, options: &'a [ModelOption]) -> Vec<&'a ModelOption> {
        let mut v: Vec<&ModelOption> = filter_model_options(options, &self.search);
        // sort: recent first when no search
        if self.search.trim().is_empty() {
            v.sort_by_key(|o| {
                let recent_rank = self
                    .recent
                    .iter()
                    .position(|r| r == &o.id)
                    .map(|i| i as u32)
                    .unwrap_or(100);
                (recent_rank, o.label.as_str())
            });
        }
        v
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, options: &[ModelOption]) -> ModelSelectorOutcome {
        if !self.accepts_input || !self.focused || key.kind != KeyEventKind::Press {
            return ModelSelectorOutcome::Ignored;
        }
        match self.presentation {
            ModelSelectorPresentation::Compact => {
                if matches!(default_button_intent(key), Some(UiIntent::Activate)) {
                    return self.open();
                }
                match key.code {
                    KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.open()
                    }
                    KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.reasoning = cycle_reasoning(self.reasoning);
                        ModelSelectorOutcome::ReasoningChanged {
                            effort: self.reasoning,
                        }
                    }
                    _ => ModelSelectorOutcome::Ignored,
                }
            }
            ModelSelectorPresentation::Expanded => {
                if let Some(intent) = default_list_intent(key) {
                    match intent {
                        UiIntent::Cancel => return self.close(),
                        UiIntent::Activate => return self.confirm(options),
                        UiIntent::Move(NavigationMove::Next) => {
                            let visible = self.visible(options);
                            self.move_highlight(&visible, 1);
                            return ModelSelectorOutcome::HighlightChanged {
                                id: self.highlight.clone().unwrap_or_default(),
                            };
                        }
                        UiIntent::Move(NavigationMove::Previous) => {
                            let visible = self.visible(options);
                            self.move_highlight(&visible, -1);
                            return ModelSelectorOutcome::HighlightChanged {
                                id: self.highlight.clone().unwrap_or_default(),
                            };
                        }
                        // Space remains search text in this editable surface.
                        _ => {}
                    }
                }
                let visible = self.visible(options);
                if visible.is_empty() {
                    // still allow typing search
                    return self.handle_search_char(key);
                }
                match key.code {
                    KeyCode::Backspace => {
                        self.search.pop();
                        ModelSelectorOutcome::SearchChanged {
                            query: self.search.clone(),
                        }
                    }
                    KeyCode::Char(c)
                        if !key.modifiers.contains(KeyModifiers::CONTROL)
                            && !key.modifiers.contains(KeyModifiers::ALT)
                            && !c.is_control() =>
                    {
                        self.search.push(c);
                        // reset highlight to first match
                        let vis = self.visible(options);
                        self.highlight = vis.first().map(|o| o.id.clone());
                        ModelSelectorOutcome::SearchChanged {
                            query: self.search.clone(),
                        }
                    }
                    _ => ModelSelectorOutcome::Ignored,
                }
            }
        }
    }

    fn handle_search_char(&mut self, key: KeyEvent) -> ModelSelectorOutcome {
        match key.code {
            KeyCode::Backspace => {
                self.search.pop();
                ModelSelectorOutcome::SearchChanged {
                    query: self.search.clone(),
                }
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL) && !c.is_control() =>
            {
                self.search.push(c);
                ModelSelectorOutcome::SearchChanged {
                    query: self.search.clone(),
                }
            }
            _ => ModelSelectorOutcome::Ignored,
        }
    }

    fn move_highlight(&mut self, visible: &[&ModelOption], delta: isize) {
        if visible.is_empty() {
            return;
        }
        let cur = self
            .highlight
            .as_ref()
            .and_then(|id| visible.iter().position(|o| &o.id == id))
            .unwrap_or(0);
        let next = if delta < 0 {
            cur.saturating_sub(1)
        } else {
            (cur + 1).min(visible.len() - 1)
        };
        self.highlight = Some(visible[next].id.clone());
    }

    /// Mouse: click compact opens; click row confirms.
    pub fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        area: Rect,
        options: &[ModelOption],
        row_hits: &[(String, Rect)],
    ) -> ModelSelectorOutcome {
        if !self.accepts_input || mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return ModelSelectorOutcome::Ignored;
        }
        if self.presentation == ModelSelectorPresentation::Compact {
            if area.contains(mouse.position) {
                return self.open();
            }
            return ModelSelectorOutcome::Ignored;
        }
        for (id, rect) in row_hits {
            if rect.contains(mouse.position) {
                self.highlight = Some(id.clone());
                return self.confirm(options);
            }
        }
        ModelSelectorOutcome::Ignored
    }
}

fn cycle_reasoning(r: ReasoningEffort) -> ReasoningEffort {
    match r {
        ReasoningEffort::Default => ReasoningEffort::Low,
        ReasoningEffort::Low => ReasoningEffort::Medium,
        ReasoningEffort::Medium => ReasoningEffort::High,
        ReasoningEffort::High => ReasoningEffort::Max,
        ReasoningEffort::Max => ReasoningEffort::Default,
    }
}

/// Model selector paint.
#[derive(Debug, Clone, Copy)]
pub struct ModelSelector<'a> {
    options: &'a [ModelOption],
    system: &'a DesignSystem,
    /// Show reasoning effort in compact status.
    show_reasoning: bool,
}

impl<'a> ModelSelector<'a> {
    /// Options + system.
    #[must_use]
    pub const fn new(options: &'a [ModelOption], system: &'a DesignSystem) -> Self {
        Self {
            options,
            system,
            show_reasoning: false,
        }
    }

    /// ASCII.
    #[must_use]
    /// Include reasoning token in compact line.
    pub const fn show_reasoning(mut self, on: bool) -> Self {
        self.show_reasoning = on;
        self
    }

    /// Compact status string for external chrome.
    #[must_use]
    pub fn compact_status(&self, state: &ModelSelectorState) -> String {
        let base = state
            .selected()
            .and_then(|id| self.options.iter().find(|o| o.id == id))
            .map(|o| o.status_text(false))
            .unwrap_or_else(|| "model?".into());
        if self.show_reasoning {
            format!("{base}{}{}", { " · " }, state.reasoning.short())
        } else {
            base
        }
    }

    /// Paint compact or expanded.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut ModelSelectorState,
    ) -> Vec<(String, Rect)> {
        let mut hits = Vec::new();
        if area.is_empty() {
            return hits;
        }
        match state.presentation {
            ModelSelectorPresentation::Compact => {
                let line = format!("{} {}", { "⚙" }, self.compact_status(state));
                let control_state = if !state.accepts_input {
                    ControlState::Disabled
                } else if state.focused {
                    ControlState::Focused
                } else {
                    ControlState::Default
                };
                let recipe = self.system.button_recipe(
                    ButtonRecipeVariant::Quiet,
                    control_state,
                    self.system.junie_theme().surface,
                );
                buffer.set_style(area, recipe.fill);
                buffer.set_stringn(
                    area.x,
                    area.y,
                    take_display_cols(&line, usize::from(area.width)),
                    usize::from(area.width),
                    recipe.label,
                );
            }
            ModelSelectorPresentation::Expanded => {
                let mut y = area.y;
                // search line
                if area.height > 0 {
                    let control_state = if !state.accepts_input {
                        ControlState::Disabled
                    } else if state.focused {
                        ControlState::Focused
                    } else {
                        ControlState::Default
                    };
                    let recipe = self.system.input_recipe(control_state, false, false);
                    let search_area = Rect::new(area.x, y, area.width, 1);
                    buffer.set_style(search_area, recipe.fill);
                    if area.width > 0 {
                        let prompt = { "›" };
                        buffer.set_stringn(area.x, y, prompt, 1, recipe.cursor);
                    }
                    let value_x = area.x.saturating_add(1).min(area.right());
                    let value_w = area.width.saturating_sub(1);
                    let (q, style) = if state.search.is_empty() {
                        ("Search models", recipe.placeholder)
                    } else {
                        (state.search.as_str(), recipe.value)
                    };
                    buffer.set_stringn(
                        value_x,
                        y,
                        take_display_cols(q, usize::from(value_w)),
                        usize::from(value_w),
                        style,
                    );
                    if state.focused && !state.search.is_empty() {
                        let caret_x = value_x
                            .saturating_add(display_cols(&state.search) as u16)
                            .min(area.right().saturating_sub(1));
                        buffer.set_stringn(caret_x, y, " ", 1, recipe.cursor);
                    }
                    y = y.saturating_add(1);
                }
                let visible = state.visible(self.options);
                if visible.is_empty() {
                    if y < area.bottom() {
                        buffer.set_stringn(
                            area.x,
                            y,
                            take_display_cols("No models", usize::from(area.width)),
                            usize::from(area.width),
                            self.system.style(Role::TextMuted),
                        );
                    }
                    return hits;
                }
                for o in visible {
                    if y >= area.bottom() {
                        break;
                    }
                    let selected = state.highlight.as_deref() == Some(o.id.as_str());
                    let mark = if selected { "›" } else { " " };
                    let warn = if o.warning.is_some() || !o.availability.is_selectable() {
                        "⚠"
                    } else {
                        ""
                    };
                    let committed = state.selected.as_deref() == Some(o.id.as_str());
                    let checked = if committed { "✓" } else { " " };
                    let line = format!(
                        "{mark}{checked} {} {}{}",
                        o.label,
                        if o.context_tokens > 0 {
                            format_context(o.context_tokens)
                        } else {
                            String::new()
                        },
                        warn
                    );
                    let recipe = self.system.resolve_list_row(ListRowVisualState {
                        selected,
                        focused: selected && state.focused,
                        hovered: false,
                        enabled: o.availability.is_selectable(),
                        loading: false,
                        checked: committed,
                        ..ListRowVisualState::default()
                    });
                    let rect = Rect::new(area.x, y, area.width, 1);
                    if recipe.use_tint {
                        buffer.set_style(rect, recipe.tint);
                    }
                    let style = if o.warning.is_some() && o.availability.is_selectable() {
                        recipe.label.patch(self.system.style(Role::Warning))
                    } else {
                        recipe.label
                    };
                    buffer.set_stringn(
                        area.x,
                        y,
                        take_display_cols(&line, usize::from(area.width)),
                        usize::from(area.width),
                        style,
                    );
                    hits.push((
                        o.id.clone(),
                        Rect {
                            x: area.x,
                            y,
                            width: area.width,
                            height: 1,
                        },
                    ));
                    y = y.saturating_add(1);
                    // meta line if room
                    let meta = { o.row_meta() };
                    if !meta.is_empty() && y < area.bottom() && area.height > 4 {
                        buffer.set_stringn(
                            area.x.saturating_add(2),
                            y,
                            take_display_cols(&meta, usize::from(area.width.saturating_sub(2))),
                            usize::from(area.width.saturating_sub(2)),
                            recipe.secondary,
                        );
                        y = y.saturating_add(1);
                    }
                }
            }
        }
        hits
    }

    /// Render alias.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut ModelSelectorState) {
        let _ = self.paint(area, buffer, state);
    }
}

// ── AgentModeSelector ───────────────────────────────────────────────────────

/// Mode selector presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AgentModePresentation {
    /// Compact badge for composer.
    #[default]
    Compact,
    /// Full ribbon of modes.
    Ribbon,
    /// Expandable list menu.
    Menu,
}

impl AgentModePresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Ribbon => "ribbon",
            Self::Menu => "menu",
        }
    }
}

/// Agent mode selector state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentModeSelectorState {
    selected: Option<String>,
    /// Menu/ribbon highlight id.
    pub highlight: Option<String>,
    /// Compact badge, ribbon, or list menu.
    pub presentation: AgentModePresentation,
    /// Optional execution policy independent of mode.
    pub policy: Option<ExecutionPolicyKind>,
    accepts_input: bool,
    focused: bool,
}

impl Default for AgentModeSelectorState {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentModeSelectorState {
    /// Default Edit selected.
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected: Some(AgentModeKind::Edit.id().into()),
            highlight: Some(AgentModeKind::Edit.id().into()),
            presentation: AgentModePresentation::Compact,
            policy: None,
            accepts_input: true,
            focused: true,
        }
    }

    /// With mode id.
    #[must_use]
    pub fn with_selected(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            selected: Some(id.clone()),
            highlight: Some(id),
            ..Self::new()
        }
    }

    /// Selected.
    #[must_use]
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Set selected silently.
    pub fn set_selected(&mut self, id: Option<String>) {
        self.selected = id.clone();
        self.highlight = id;
    }

    /// Open menu.
    pub fn open_menu(&mut self) -> AgentModeSelectorOutcome {
        if !self.accepts_input {
            return AgentModeSelectorOutcome::Ignored;
        }
        if self.presentation == AgentModePresentation::Menu {
            return AgentModeSelectorOutcome::Ignored;
        }
        self.presentation = AgentModePresentation::Menu;
        self.highlight = self.selected.clone();
        AgentModeSelectorOutcome::Opened
    }

    /// Close menu.
    pub fn close_menu(&mut self) -> AgentModeSelectorOutcome {
        if self.presentation == AgentModePresentation::Compact
            || self.presentation == AgentModePresentation::Ribbon
        {
            // from menu only
            if self.presentation != AgentModePresentation::Menu {
                return AgentModeSelectorOutcome::Ignored;
            }
        }
        if self.presentation != AgentModePresentation::Menu {
            return AgentModeSelectorOutcome::Ignored;
        }
        self.presentation = AgentModePresentation::Compact;
        self.highlight = self.selected.clone();
        AgentModeSelectorOutcome::Closed
    }

    /// Confirm highlight.
    pub fn confirm(&mut self, modes: &[AgentModeOption]) -> AgentModeSelectorOutcome {
        let Some(id) = self.highlight.clone() else {
            return AgentModeSelectorOutcome::Ignored;
        };
        let Some(mode) = modes.iter().find(|m| m.id == id) else {
            return AgentModeSelectorOutcome::Ignored;
        };
        if !mode.enabled {
            return AgentModeSelectorOutcome::Ignored;
        }
        self.selected = Some(id.clone());
        if self.presentation == AgentModePresentation::Menu {
            self.presentation = AgentModePresentation::Compact;
        }
        AgentModeSelectorOutcome::ModeChanged {
            id,
            kind: mode.kind,
            consequence: mode.consequence.clone(),
            needs_confirm: mode.needs_warning_role(),
        }
    }

    /// Cycle modes (ribbon/compact chord).
    pub fn cycle(&mut self, modes: &[AgentModeOption], delta: isize) -> AgentModeSelectorOutcome {
        let enabled: Vec<&AgentModeOption> = modes.iter().filter(|m| m.enabled).collect();
        if enabled.is_empty() {
            return AgentModeSelectorOutcome::Ignored;
        }
        let cur = self
            .selected
            .as_ref()
            .and_then(|id| enabled.iter().position(|m| &m.id == id))
            .unwrap_or(0);
        let next = if delta < 0 {
            if cur == 0 { enabled.len() - 1 } else { cur - 1 }
        } else {
            (cur + 1) % enabled.len()
        };
        self.highlight = Some(enabled[next].id.clone());
        self.confirm(modes)
    }

    /// Keys.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        modes: &[AgentModeOption],
    ) -> AgentModeSelectorOutcome {
        if !self.accepts_input || !self.focused || key.kind != KeyEventKind::Press {
            return AgentModeSelectorOutcome::Ignored;
        }
        match self.presentation {
            AgentModePresentation::Compact => {
                if matches!(default_button_intent(key), Some(UiIntent::Activate)) {
                    return self.open_menu();
                }
                match key.code {
                    KeyCode::Left | KeyCode::Char('h') => self.cycle(modes, -1),
                    KeyCode::Right | KeyCode::Char('l') => self.cycle(modes, 1),
                    KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.open_menu()
                    }
                    KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // cycle policy
                        let next =
                            cycle_policy(self.policy.unwrap_or(ExecutionPolicyKind::Default));
                        self.policy = Some(next);
                        AgentModeSelectorOutcome::PolicyChanged {
                            policy: next,
                            warning: next.is_warning(),
                        }
                    }
                    _ => AgentModeSelectorOutcome::Ignored,
                }
            }
            AgentModePresentation::Ribbon => {
                if let Some(intent) = default_list_intent(key) {
                    match intent {
                        UiIntent::Activate => {
                            // confirm current highlight if set
                            if self.highlight.is_none() {
                                self.highlight = self.selected.clone();
                            }
                            return self.confirm(modes);
                        }
                        UiIntent::Toggle => return self.open_menu(),
                        _ => {}
                    }
                }
                match key.code {
                    KeyCode::Left | KeyCode::Char('h') => self.cycle(modes, -1),
                    KeyCode::Right | KeyCode::Char('l') => self.cycle(modes, 1),
                    _ => AgentModeSelectorOutcome::Ignored,
                }
            }
            AgentModePresentation::Menu => {
                let enabled: Vec<&AgentModeOption> = modes.iter().filter(|m| m.enabled).collect();
                match default_list_intent(key) {
                    Some(UiIntent::Cancel) => self.close_menu(),
                    Some(UiIntent::Activate | UiIntent::Toggle) => self.confirm(modes),
                    Some(UiIntent::Move(NavigationMove::Next)) => {
                        self.move_hl(&enabled, 1);
                        AgentModeSelectorOutcome::HighlightChanged {
                            id: self.highlight.clone().unwrap_or_default(),
                        }
                    }
                    Some(UiIntent::Move(NavigationMove::Previous)) => {
                        self.move_hl(&enabled, -1);
                        AgentModeSelectorOutcome::HighlightChanged {
                            id: self.highlight.clone().unwrap_or_default(),
                        }
                    }
                    _ => AgentModeSelectorOutcome::Ignored,
                }
            }
        }
    }

    fn move_hl(&mut self, enabled: &[&AgentModeOption], delta: isize) {
        if enabled.is_empty() {
            return;
        }
        let cur = self
            .highlight
            .as_ref()
            .and_then(|id| enabled.iter().position(|m| &m.id == id))
            .unwrap_or(0);
        let next = if delta < 0 {
            cur.saturating_sub(1)
        } else {
            (cur + 1).min(enabled.len() - 1)
        };
        self.highlight = Some(enabled[next].id.clone());
    }

    /// Mouse hits: ribbon segments or menu rows.
    pub fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        modes: &[AgentModeOption],
        hits: &[(String, Rect)],
    ) -> AgentModeSelectorOutcome {
        if !self.accepts_input || mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return AgentModeSelectorOutcome::Ignored;
        }
        for (id, rect) in hits {
            if rect.contains(mouse.position) {
                self.highlight = Some(id.clone());
                return self.confirm(modes);
            }
        }
        if self.presentation == AgentModePresentation::Compact {
            return self.open_menu();
        }
        AgentModeSelectorOutcome::Ignored
    }
}

fn cycle_policy(p: ExecutionPolicyKind) -> ExecutionPolicyKind {
    match p {
        ExecutionPolicyKind::Default => ExecutionPolicyKind::ReadOnly,
        ExecutionPolicyKind::ReadOnly => ExecutionPolicyKind::WorkspaceWrite,
        ExecutionPolicyKind::WorkspaceWrite => ExecutionPolicyKind::Network,
        ExecutionPolicyKind::Network => ExecutionPolicyKind::Unrestricted,
        ExecutionPolicyKind::Unrestricted => ExecutionPolicyKind::Default,
    }
}

/// Agent mode selector paint.
#[derive(Debug, Clone, Copy)]
pub struct AgentModeSelector<'a> {
    modes: &'a [AgentModeOption],
    system: &'a DesignSystem,
}

impl<'a> AgentModeSelector<'a> {
    /// Modes + system.
    #[must_use]
    pub const fn new(modes: &'a [AgentModeOption], system: &'a DesignSystem) -> Self {
        Self { modes, system }
    }

    /// ASCII.
    #[must_use]
    /// Compact status text.
    pub fn compact_status(&self, state: &AgentModeSelectorState) -> String {
        let mode = state
            .selected()
            .and_then(|id| self.modes.iter().find(|m| m.id == id))
            .map(|m| m.status_text())
            .unwrap_or_else(|| "MODE".into());
        if let Some(p) = state.policy {
            format!("{mode}{}{}", { "·" }, p.short())
        } else {
            mode
        }
    }

    /// Paint; returns hit regions for mouse.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut AgentModeSelectorState,
    ) -> Vec<(String, Rect)> {
        let mut hits = Vec::new();
        if area.is_empty() {
            return hits;
        }
        match state.presentation {
            AgentModePresentation::Compact => {
                let mode = state
                    .selected()
                    .and_then(|id| self.modes.iter().find(|m| m.id == id));
                let warn = mode.is_some_and(|m| m.needs_warning_role());
                let warning = if warn { "⚠" } else { "" };
                let line = format!(" {} {warning}", self.compact_status(state));
                let control_state = if !state.accepts_input {
                    ControlState::Disabled
                } else if state.focused {
                    ControlState::Focused
                } else {
                    ControlState::Default
                };
                let recipe = self.system.button_recipe(
                    ButtonRecipeVariant::Quiet,
                    control_state,
                    self.system.junie_theme().surface,
                );
                buffer.set_style(area, recipe.fill);
                let style = if warn {
                    recipe.label.patch(self.system.style(Role::Warning))
                } else {
                    recipe.label
                };
                buffer.set_stringn(
                    area.x,
                    area.y,
                    take_display_cols(&line, usize::from(area.width)),
                    usize::from(area.width),
                    style,
                );
            }
            AgentModePresentation::Ribbon => {
                let mut x = area.x;
                for m in self.modes {
                    if x >= area.right() {
                        break;
                    }
                    let active = state.selected.as_deref() == Some(m.id.as_str());
                    let label = format!(" {} ", m.short_label);
                    let w = (display_cols(&label) as u16)
                        .min(area.right().saturating_sub(x))
                        .max(1);
                    let control_state = if !m.enabled || !state.accepts_input {
                        ControlState::Disabled
                    } else if active && state.focused {
                        ControlState::Focused
                    } else {
                        ControlState::Default
                    };
                    let recipe = self.system.button_recipe(
                        ButtonRecipeVariant::Quiet,
                        control_state,
                        self.system.junie_theme().surface,
                    );
                    let rect = Rect::new(x, area.y, w, 1);
                    buffer.set_style(rect, recipe.fill);
                    let mut style = recipe.label;
                    if active {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    if m.needs_warning_role() {
                        style = style.patch(self.system.style(Role::Warning));
                    }
                    buffer.set_stringn(
                        x,
                        area.y,
                        take_display_cols(&label, usize::from(w)),
                        usize::from(w),
                        style,
                    );
                    hits.push((
                        m.id.clone(),
                        Rect {
                            x,
                            y: area.y,
                            width: w,
                            height: 1,
                        },
                    ));
                    x = x.saturating_add(w.saturating_add(1));
                }
            }
            AgentModePresentation::Menu => {
                let mut y = area.y;
                for m in self.modes {
                    if y >= area.bottom() {
                        break;
                    }
                    let selected = state.highlight.as_deref() == Some(m.id.as_str());
                    let mark = if selected { "›" } else { " " };
                    let line = format!("{mark}{} {}", m.short_label, m.label);
                    let committed = state.selected.as_deref() == Some(m.id.as_str());
                    let recipe = self.system.resolve_list_row(ListRowVisualState {
                        selected,
                        focused: selected && state.focused,
                        hovered: false,
                        enabled: m.enabled && state.accepts_input,
                        loading: false,
                        checked: committed,
                        ..ListRowVisualState::default()
                    });
                    let rect = Rect::new(area.x, y, area.width, 1);
                    if recipe.use_tint {
                        buffer.set_style(rect, recipe.tint);
                    }
                    let style = if m.needs_warning_role() {
                        recipe.label.patch(self.system.style(Role::Warning))
                    } else {
                        recipe.label
                    };
                    buffer.set_stringn(
                        area.x,
                        y,
                        take_display_cols(&line, usize::from(area.width)),
                        usize::from(area.width),
                        style,
                    );
                    hits.push((
                        m.id.clone(),
                        Rect {
                            x: area.x,
                            y,
                            width: area.width,
                            height: 1,
                        },
                    ));
                    y = y.saturating_add(1);
                    if let Some(c) = &m.consequence {
                        if y < area.bottom() && selected {
                            buffer.set_stringn(
                                area.x.saturating_add(2),
                                y,
                                take_display_cols(c, usize::from(area.width.saturating_sub(2))),
                                usize::from(area.width.saturating_sub(2)),
                                recipe.secondary,
                            );
                            y = y.saturating_add(1);
                        }
                    }
                }
            }
        }
        hits
    }

    /// Render alias.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut AgentModeSelectorState) {
        let _ = self.paint(area, buffer, state);
    }
}

// ── Composed presentation ───────────────────────────────────────────────────

/// Side-by-side mode + model compact chrome for PromptComposer status.
#[derive(Debug, Clone, Copy)]
pub struct ComposerSelectors<'a> {
    modes: &'a [AgentModeOption],
    models: &'a [ModelOption],
    system: &'a DesignSystem,
}

impl<'a> ComposerSelectors<'a> {
    /// Modes + models.
    #[must_use]
    pub const fn new(
        modes: &'a [AgentModeOption],
        models: &'a [ModelOption],
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            modes,
            models,
            system,
        }
    }

    /// ASCII.
    #[must_use]
    /// Paint compact: `[MODE] · model/provider ctx`.
    pub fn paint_compact(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        mode_state: &AgentModeSelectorState,
        model_state: &ModelSelectorState,
    ) {
        if area.is_empty() {
            return;
        }
        let mode = AgentModeSelector::new(self.modes, self.system).compact_status(mode_state);
        let model = ModelSelector::new(self.models, self.system).compact_status(model_state);
        let warn = mode_state
            .selected()
            .and_then(|id| self.modes.iter().find(|m| m.id == id))
            .is_some_and(|m| m.needs_warning_role());
        let line = format!("{mode}{}{}", { " · " }, model);
        let style = if warn {
            self.system.style(Role::Warning)
        } else {
            self.system.style(Role::TextMuted)
        };
        buffer.set_stringn(
            area.x,
            area.y,
            take_display_cols(&line, usize::from(area.width)),
            usize::from(area.width),
            style,
        );
    }
}

/// Example model catalog (lookbook / tests).
#[must_use]
pub fn example_model_catalog() -> Vec<ModelOption> {
    vec![
        ModelOption::new("fast", "fast")
            .provider("local")
            .context_tokens(32_000)
            .cost_hint("$")
            .latency_hint("low")
            .capabilities(vec![ModelCapability::new("tools", "tools")])
            .recent(true),
        ModelOption::new("smart", "smart")
            .provider("cloud")
            .context_tokens(128_000)
            .cost_hint("$$")
            .latency_hint("med")
            .capabilities(vec![
                ModelCapability::new("tools", "tools"),
                ModelCapability::new("vision", "vision"),
            ]),
        ModelOption::new("old", "legacy-v1")
            .provider("cloud")
            .context_tokens(8_000)
            .availability(ModelAvailability::Deprecated)
            .warning("deprecated — prefer smart"),
        ModelOption::new("down", "offline-model")
            .provider("edge")
            .availability(ModelAvailability::Unavailable),
    ]
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn format_context(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{}M", n / 1_000_000)
    } else if n >= 1000 {
        format!("{}k", n / 1000)
    } else {
        format!("{n}")
    }
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Moderate catalog sizes.
pub mod bench {
    /// Models.
    pub const MODEL_COUNT: usize = 64;
    /// Filter rounds.
    pub const FILTER_ROUNDS: u32 = 32;
    /// Paint frames.
    pub const PAINT_FRAMES: u32 = 20;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;

    #[test]
    fn model_filter_and_confirm() {
        let cat = example_model_catalog();
        let mut st = ModelSelectorState::with_selected("fast");
        assert!(matches!(st.open(), ModelSelectorOutcome::Opened));
        st.search = "sma".into();
        let vis = st.visible(&cat);
        assert_eq!(vis.len(), 1);
        assert_eq!(vis[0].id, "smart");
        st.highlight = Some("smart".into());
        let out = st.confirm(&cat);
        assert!(matches!(
            out,
            ModelSelectorOutcome::Confirmed { ref id, .. } if id == "smart"
        ));
        assert_eq!(st.selected(), Some("smart"));
        assert_eq!(st.presentation, ModelSelectorPresentation::Compact);
    }

    #[test]
    fn unavailable_cannot_confirm() {
        let cat = example_model_catalog();
        let mut st = ModelSelectorState::new();
        st.presentation = ModelSelectorPresentation::Expanded;
        st.highlight = Some("down".into());
        assert!(matches!(st.confirm(&cat), ModelSelectorOutcome::Ignored));
    }

    #[test]
    fn esc_closes_without_change() {
        let mut st = ModelSelectorState::with_selected("fast");
        st.open();
        st.highlight = Some("smart".into());
        assert!(matches!(st.close(), ModelSelectorOutcome::Closed));
        assert_eq!(st.selected(), Some("fast"));

        let mut mode = AgentModeSelectorState::with_selected("edit");
        mode.open_menu();
        assert!(matches!(
            mode.handle_key(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &default_agent_modes(),
            ),
            AgentModeSelectorOutcome::Closed
        ));
        assert_eq!(mode.selected(), Some("edit"));
    }

    #[test]
    fn mode_fullauto_needs_confirm_and_warning() {
        let modes = default_agent_modes();
        let full = modes
            .iter()
            .find(|m| m.kind == AgentModeKind::FullAuto)
            .unwrap();
        assert!(full.needs_warning_role());
        let mut st = AgentModeSelectorState::with_selected("edit");
        st.presentation = AgentModePresentation::Menu;
        st.highlight = Some("full-auto".into());
        let out = st.confirm(&modes);
        match out {
            AgentModeSelectorOutcome::ModeChanged {
                needs_confirm: true,
                kind: AgentModeKind::FullAuto,
                consequence: Some(_),
                ..
            } => {}
            other => panic!("unexpected {other:?}"),
        }
        let ind = mode_to_indicator(full);
        assert!(ind.warning);
    }

    #[test]
    fn mode_cycle_and_ribbon() {
        let modes = default_agent_modes();
        let mut st = AgentModeSelectorState::with_selected("ask");
        st.presentation = AgentModePresentation::Ribbon;
        let out = st.cycle(&modes, 1);
        assert!(matches!(
            out,
            AgentModeSelectorOutcome::ModeChanged { ref id, .. } if id == "plan"
        ));
    }

    #[test]
    fn model_indicator_bridge() {
        let m = example_model_catalog()[0].clone();
        let ind = model_to_indicator(&m);
        assert_eq!(ind.label, "fast");
    }

    #[test]
    fn workbench_projection() {
        let modes = default_agent_modes();
        let rows = modes_to_ribbon(&modes, Some("edit"));
        assert!(rows.iter().any(|r| r.active && r.id == "edit"));
    }

    #[test]
    fn select_options_bridge() {
        let opts = models_to_select_options(&example_model_catalog());
        assert!(opts.iter().any(|o| o.id == "down" && o.disabled));
    }

    #[test]
    fn compact_composed_paint() {
        let system = DesignSystem::default();
        let models = example_model_catalog();
        let modes = default_agent_modes();
        let mut ms = ModelSelectorState::with_selected("fast");
        let mut as_ = AgentModeSelectorState::with_selected("full-auto");
        let area = Rect::new(0, 0, 48, 1);
        let mut buf = Buffer::empty(area);
        let _ = ComposerSelectors::new(&modes, &models, &system)
            .paint_compact(area, &mut buf, &as_, &ms);
        // expanded paints
        ms.open();
        as_.open_menu();
        let area2 = Rect::new(0, 0, 40, 10);
        let mut buf2 = Buffer::empty(area2);
        for _ in 0..bench::PAINT_FRAMES {
            ModelSelector::new(&models, &system).paint(area2, &mut buf2, &mut ms);
            AgentModeSelector::new(&modes, &system).paint(area2, &mut buf2, &mut as_);
        }
    }

    #[test]
    fn empty_selector_catalogs_paint_stable_fallbacks() {
        let system = DesignSystem::default();
        let models: [ModelOption; 0] = [];
        let modes: [AgentModeOption; 0] = [];
        let model_state = ModelSelectorState::new();
        let mode_state = AgentModeSelectorState::new();
        let area = Rect::new(0, 0, 32, 1);
        let mut buffer = Buffer::empty(area);

        let _ = ComposerSelectors::new(&modes, &models, &system).paint_compact(
            area,
            &mut buffer,
            &mode_state,
            &model_state,
        );

        let row = (0..area.width)
            .map(|x| buffer[(x, 0)].symbol())
            .collect::<String>();
        assert!(!row.trim().is_empty(), "{row:?}");
    }

    #[test]
    fn accepts_input_gate() {
        let mut st = ModelSelectorState::new();
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &[]),
            ModelSelectorOutcome::Ignored
        ));
        let mut ms = AgentModeSelectorState::new();
        ms.set_accepts_input(false);
        assert!(matches!(
            ms.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &[]),
            AgentModeSelectorOutcome::Ignored
        ));
    }

    #[test]
    fn model_and_mode_mouse_confirm_only_hit_options() {
        let models = example_model_catalog();
        let hit = Rect::new(3, 2, 10, 1);
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: ratatui_core::layout::Position::new(hit.x, hit.y),
            modifiers: KeyModifiers::NONE,
        };
        let mut model = ModelSelectorState::new();
        model.presentation = ModelSelectorPresentation::Expanded;
        assert!(matches!(
            model.handle_mouse(
                mouse,
                Rect::new(0, 0, 30, 8),
                &models,
                &[("smart".into(), hit)],
            ),
            ModelSelectorOutcome::Confirmed { id, .. } if id == "smart"
        ));

        let modes = default_agent_modes();
        let mut mode = AgentModeSelectorState::new();
        mode.presentation = AgentModePresentation::Menu;
        assert!(matches!(
            mode.handle_mouse(mouse, &modes, &[("edit".into(), hit)]),
            AgentModeSelectorOutcome::ModeChanged { id, .. } if id == "edit"
        ));
    }

    #[test]
    fn reasoning_cycle() {
        let mut st = ModelSelectorState::new();
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
            &[],
        );
        assert!(matches!(
            out,
            ModelSelectorOutcome::ReasoningChanged {
                effort: ReasoningEffort::Low
            }
        ));
    }

    #[test]
    fn filter_bench() {
        let mut cat = Vec::with_capacity(bench::MODEL_COUNT);
        for i in 0..bench::MODEL_COUNT {
            cat.push(
                ModelOption::new(format!("m{i}"), format!("model-{i}"))
                    .provider(if i % 2 == 0 { "a" } else { "b" })
                    .context_tokens(1000 * (i as u64 + 1)),
            );
        }
        for r in 0..bench::FILTER_ROUNDS {
            let q = format!("model-{}", r % 10);
            let hits = filter_model_options(&cat, &q);
            assert!(!hits.is_empty());
        }
    }

    #[test]
    fn never_calls_providers() {
        let src = include_str!("model_mode_selectors.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in [
            "reqwest::",
            "async_openai",
            "anthropic",
            "openai::",
            "std::process::Command",
        ] {
            assert!(!body.contains(forbidden), "must not contain {forbidden}");
        }
    }

    #[test]
    fn policy_cycle_warns() {
        let mut st = AgentModeSelectorState::new();
        let mut last = AgentModeSelectorOutcome::Ignored;
        for _ in 0..5 {
            last = st.handle_key(
                KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
                &default_agent_modes(),
            );
        }
        // eventually unrestricted warning
        assert!(matches!(
            last,
            AgentModeSelectorOutcome::PolicyChanged { .. }
        ));
    }
}
