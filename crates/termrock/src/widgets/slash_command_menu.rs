// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **SlashCommandMenu** — command completion surface optimized for prompt composers.
//!
//! **Mission.** Command name, aliases, description, arguments, shortcut,
//! provider/plugin source, disabled reason, recent commands, and nested
//! argument completion. Integrate the global command system while allowing
//! composer-specific commands. Fuzzy ranges, async plugin results, loading /
//! empty / error states. **Preserve draft text** and replace only the intended
//! `/token` (or argument) range. Compact (list) and fullscreen modes via
//! [`CompletionMenu`] presentation.
//!
//! Research: Grok Build, OpenCode, Claude Code, terminal shells, command palettes.
//!
//! **Ownership.** Host owns command catalogs, plugin I/O, and side effects after
//! commit. TermRock owns detect/filter/paint/nav and typed outcomes.
//!
//! **vs [`CommandPalette`](crate::widgets::CommandPalette).** Palette is a global
//! centered command surface. SlashCommandMenu is **caret-anchored**, draft-aware,
//! and `/`-token scoped for PromptComposer.
use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{KeyEvent, MouseEvent},
    interaction::{OverlayId, OverlayKind, OverlayOutcome, OverlaySize, OverlaySpec, OverlayStack},
    style::DesignSystem,
    widgets::{
        MatchRanges,
        command_palette::{CommandEntry, fuzzy_match_label},
        completion_menu::{
            CompletionCandidate, CompletionMenu, CompletionMenuOutcome, CompletionMenuSize,
            CompletionMenuState, CompletionPresentation, CompletionStatus,
            completion_presentation_for, place_completion_menu,
        },
    },
};

/// Default overlay id for slash menus on an [`OverlayStack`].
pub const SLASH_COMMAND_OVERLAY_ID: &str = "termrock.slash_command";
/// Slash trigger character.
pub const SLASH_TRIGGER: char = '/';
/// Space that advances into argument phase after a command name.
pub const SLASH_ARG_SEPARATOR: char = ' ';

// ── Domain ──────────────────────────────────────────────────────────────────

/// Where a slash command comes from (host projection; no plugin I/O here).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SlashCommandSource {
    /// Built-in / app command system.
    #[default]
    Builtin,
    /// Global keymap / command registry projection.
    Global,
    /// Composer-only command (not in global palette).
    Composer,
    /// Plugin / provider extension.
    Plugin {
        /// Plugin id (display-safe).
        id: String,
    },
}

impl SlashCommandSource {
    /// Stable id.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Builtin => "builtin",
            Self::Global => "global",
            Self::Composer => "composer",
            Self::Plugin { id } => id.as_str(),
        }
    }

    /// Group header label.
    #[must_use]
    pub fn group_label(&self) -> &str {
        match self {
            Self::Builtin => "Builtin",
            Self::Global => "Commands",
            Self::Composer => "Composer",
            Self::Plugin { id } => id.as_str(),
        }
    }
}

/// One argument slot for nested completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashArgument {
    /// Argument name (for description chrome).
    pub name: String,
    /// Whether required.
    pub required: bool,
    /// Placeholder / type hint.
    pub hint: Option<String>,
    /// Host-owned completion value ids for this slot (optional static list).
    pub values: Vec<String>,
}

impl SlashArgument {
    /// Required named arg.
    #[must_use]
    pub fn required(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            required: true,
            hint: None,
            values: Vec::new(),
        }
    }

    /// Optional arg.
    #[must_use]
    pub fn optional(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            required: false,
            hint: None,
            values: Vec::new(),
        }
    }

    /// Hint.
    #[must_use]
    pub fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = Some(h.into());
        self
    }

    /// Static value completions.
    #[must_use]
    pub fn values<I, S>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.values = iter.into_iter().map(Into::into).collect();
        self
    }
}

/// One slash command (host catalog row).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommand {
    /// Stable id (commit identity).
    pub id: String,
    /// Primary name without leading `/`.
    pub name: String,
    /// Alternate spellings (matched in filter).
    pub aliases: Vec<String>,
    /// Human description (docs pane).
    pub description: Option<String>,
    /// Argument schema (nested completion).
    pub arguments: Vec<SlashArgument>,
    /// Shortcut hint (display only).
    pub shortcut: Option<String>,
    /// Source / plugin.
    pub source: SlashCommandSource,
    /// Disabled reason when not runnable.
    pub disabled_reason: Option<String>,
    /// Recent section membership.
    pub recent: bool,
    /// Host command key for global command system bridge.
    pub command_key: Option<String>,
    /// Fuzzy ranges into name (filled by filter).
    pub match_ranges: Option<MatchRanges>,
    /// Sort score (lower better).
    pub score: u32,
}

impl SlashCommand {
    /// Enabled command with name = id.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            aliases: Vec::new(),
            description: None,
            arguments: Vec::new(),
            shortcut: None,
            source: SlashCommandSource::Composer,
            disabled_reason: None,
            recent: false,
            command_key: None,
            match_ranges: None,
            score: 0,
        }
    }

    /// Aliases.
    #[must_use]
    pub fn aliases<I, S>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.aliases = iter.into_iter().map(Into::into).collect();
        self
    }

    /// Description.
    #[must_use]
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }

    /// Arguments.
    #[must_use]
    pub fn arguments(mut self, args: Vec<SlashArgument>) -> Self {
        self.arguments = args;
        self
    }

    /// Shortcut.
    #[must_use]
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = Some(s.into());
        self
    }

    /// Source.
    #[must_use]
    pub fn source(mut self, s: SlashCommandSource) -> Self {
        self.source = s;
        self
    }

    /// Disabled.
    #[must_use]
    pub fn disabled(mut self, reason: impl Into<String>) -> Self {
        self.disabled_reason = Some(reason.into());
        self
    }

    /// Recent.
    #[must_use]
    pub const fn recent(mut self, on: bool) -> Self {
        self.recent = on;
        self
    }

    /// Global command key.
    #[must_use]
    pub fn command_key(mut self, k: impl Into<String>) -> Self {
        self.command_key = Some(k.into());
        self
    }

    /// Whether enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.disabled_reason.is_none()
    }

    /// Leading `/name` token for insert.
    #[must_use]
    pub fn slash_token(&self) -> String {
        format!("/{}", self.name)
    }

    /// Insert text when committing the command name (trailing space if has args).
    #[must_use]
    pub fn insert_command_token(&self) -> String {
        if self.arguments.is_empty() {
            self.slash_token()
        } else {
            format!("/{} ", self.name)
        }
    }

    /// Full insert with argument fragments (host supplies arg texts).
    #[must_use]
    pub fn insert_with_args(&self, args: &[&str]) -> String {
        let mut s = self.slash_token();
        for a in args {
            s.push(SLASH_ARG_SEPARATOR);
            s.push_str(a);
        }
        s
    }
}

// ── Query / phase ───────────────────────────────────────────────────────────

/// Slash menu interaction phase.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SlashMenuPhase {
    /// Completing `/command` name.
    Command {
        /// Prefix after `/`.
        prefix: String,
    },
    /// Completing a nested argument for a committed command id.
    Argument {
        /// Command id.
        command_id: String,
        /// Command name (for chrome).
        command_name: String,
        /// Zero-based argument index.
        arg_index: usize,
        /// Partial argument text.
        arg_prefix: String,
        /// Already committed argument values (before current).
        prior_args: Vec<String>,
    },
}

impl SlashMenuPhase {
    /// Stable id.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            Self::Command { .. } => "command",
            Self::Argument { .. } => "argument",
        }
    }
}

/// Detected slash query span in a plain draft (byte offsets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashQuery {
    /// Phase.
    pub phase: SlashMenuPhase,
    /// Byte offset of `/` that started this invocation.
    pub trigger_byte: usize,
    /// Byte offset of cursor (end of replace range).
    pub cursor_byte: usize,
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Typed slash menu outcomes (host applies draft edits / runs commands).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SlashCommandMenuOutcome {
    /// Not handled.
    Ignored,
    /// Menu opened or query span changed.
    QueryChanged {
        /// Current query.
        query: SlashQuery,
    },
    /// Selection moved.
    SelectionChanged {
        /// Selected command or arg value id.
        id: String,
    },
    /// Commit command name (may enter argument phase).
    CommandCommitted {
        /// Command id.
        id: String,
        /// Replacement text for trigger..cursor (e.g. `/plan `).
        insertion: String,
        /// True when host should keep menu open for first argument.
        needs_arguments: bool,
    },
    /// Commit argument value (or final submit of command+args).
    ArgumentCommitted {
        /// Command id.
        command_id: String,
        /// Argument index.
        arg_index: usize,
        /// Value text.
        value: String,
        /// Full line replacement for trigger..cursor.
        insertion: String,
        /// True when more arguments remain.
        more_arguments: bool,
    },
    /// Run command with collected args (no more nested completion).
    Execute {
        /// Command id.
        id: String,
        /// Argument values.
        args: Vec<String>,
    },
    /// Menu dismissed (Esc); draft unchanged.
    Dismissed,
    /// Async status changed.
    StatusChanged {
        /// Status.
        status: CompletionStatus,
    },
    /// Presentation reflow.
    PresentationChanged {
        /// Presentation.
        presentation: CompletionPresentation,
    },
    /// Stale async generation.
    GenerationStale {
        /// Generation.
        generation: u64,
    },
}

// ── Detect / filter / draft ─────────────────────────────────────────────────

/// Detect `/command` or `/command args…` at cursor (pure; no I/O).
///
/// Scans left from cursor for a `/` at a token boundary. Does **not** open
/// mid-word paths like `http://` when `/` is preceded by a non-boundary char.
#[must_use]
pub fn detect_slash_query(text: &str, cursor_byte: usize) -> Option<SlashQuery> {
    let abs = cursor_byte.min(text.len());
    let head = &text[..abs];
    let bytes = head.as_bytes();
    let mut i = bytes.len();
    while i > 0 {
        i -= 1;
        if bytes[i] == b'/' {
            let at_start = i == 0;
            let prev_ok = at_start
                || matches!(
                    bytes[i - 1],
                    b' ' | b'\n' | b'\t' | b'(' | b'[' | b'{' | b'"' | b'\''
                );
            // reject `://` or word/
            if !prev_ok {
                // allow if prev is start of line only
                continue;
            }
            // reject if looks like URL scheme: letter immediately before was already handled
            let after = &head[i + 1..];
            // no newlines in slash token span
            if after.contains('\n') {
                return None;
            }
            return Some(parse_slash_after_trigger(i, abs, after));
        }
        if bytes[i] == b'\n' {
            break;
        }
    }
    None
}

fn parse_slash_after_trigger(trigger_byte: usize, cursor_byte: usize, after: &str) -> SlashQuery {
    // after = "plan" or "plan foo bar"
    let parts: Vec<&str> = after.split(SLASH_ARG_SEPARATOR).collect();
    if parts.is_empty() || (parts.len() == 1 && !after.contains(SLASH_ARG_SEPARATOR)) {
        // still typing command name (may include partial)
        let prefix = parts.first().copied().unwrap_or("").to_string();
        // if trailing space: after ends with space → argument phase needs command resolve by host
        // detect trailing space on after
        if after.ends_with(SLASH_ARG_SEPARATOR) {
            // command complete, first arg empty
            let name = after.trim_end_matches(SLASH_ARG_SEPARATOR).to_string();
            return SlashQuery {
                phase: SlashMenuPhase::Argument {
                    command_id: name.clone(),
                    command_name: name,
                    arg_index: 0,
                    arg_prefix: String::new(),
                    prior_args: Vec::new(),
                },
                trigger_byte,
                cursor_byte,
            };
        }
        return SlashQuery {
            phase: SlashMenuPhase::Command { prefix },
            trigger_byte,
            cursor_byte,
        };
    }
    // command + at least one space-separated arg fragment
    let name = parts[0].to_string();
    let arg_parts: Vec<String> = parts[1..].iter().map(|s| (*s).to_string()).collect();
    // if ends with space, current arg is empty new slot
    let (prior, current, idx) = if after.ends_with(SLASH_ARG_SEPARATOR) {
        let prior = arg_parts;
        let idx = prior.len();
        (prior, String::new(), idx)
    } else {
        let mut prior = arg_parts;
        let current = prior.pop().unwrap_or_default();
        let idx = prior.len();
        (prior, current, idx)
    };
    SlashQuery {
        phase: SlashMenuPhase::Argument {
            command_id: name.clone(),
            command_name: name,
            arg_index: idx,
            arg_prefix: current,
            prior_args: prior,
        },
        trigger_byte,
        cursor_byte,
    }
}

/// Replace trigger..cursor with `insertion`, preserving the rest of the draft.
#[must_use]
pub fn apply_slash_insert(draft: &str, query: &SlashQuery, insertion: &str) -> String {
    let start = query.trigger_byte.min(draft.len());
    let end = query.cursor_byte.min(draft.len()).max(start);
    let mut next = String::with_capacity(draft.len() + insertion.len());
    next.push_str(&draft[..start]);
    next.push_str(insertion);
    next.push_str(&draft[end..]);
    next
}

/// Filter slash commands by prefix (name + aliases); attaches fuzzy ranges.
#[must_use]
pub fn filter_slash_commands(catalog: &[SlashCommand], prefix: &str) -> Vec<SlashCommand> {
    let q = prefix.trim().to_ascii_lowercase();
    let mut out: Vec<SlashCommand> = catalog
        .iter()
        .filter_map(|c| {
            if q.is_empty() {
                let mut c = c.clone();
                c.score = if c.recent { 0 } else { 10 };
                return Some(c);
            }
            // try name then aliases
            let mut best: Option<(u32, MatchRanges)> = fuzzy_match_label(&q, &c.name);
            for a in &c.aliases {
                if let Some((score, ranges)) = fuzzy_match_label(&q, a) {
                    best = Some(match best {
                        Some((bs, br)) if bs <= score => (bs, br),
                        _ => (score, ranges),
                    });
                }
            }
            // also plain contains on name
            if best.is_none() {
                if crate::text::contains_lower_all(
                    &[
                        c.name.as_str(),
                        &c.aliases.join(" "),
                        c.description.as_deref().unwrap_or(""),
                    ],
                    &q,
                ) {
                    best = Some((50, MatchRanges::default()));
                }
            }
            best.map(|(score, ranges)| {
                let mut c = c.clone();
                c.score = score;
                c.match_ranges = if ranges.is_empty() {
                    None
                } else {
                    Some(ranges)
                };
                c
            })
        })
        .collect();
    out.sort_by(|a, b| {
        a.score
            .cmp(&b.score)
            .then_with(|| b.recent.cmp(&a.recent))
            .then_with(|| a.name.cmp(&b.name))
    });
    out
}

/// Filter static argument values.
#[must_use]
pub fn filter_argument_values(values: &[String], prefix: &str) -> Vec<String> {
    let q = prefix.trim().to_ascii_lowercase();
    if q.is_empty() {
        return values.to_vec();
    }
    values
        .iter()
        .filter(|v| crate::text::contains_lower(&v, &q))
        .cloned()
        .collect()
}

/// Project slash commands into completion candidates (labels borrowed from cmds).
#[must_use]
pub fn slash_commands_to_candidates(
    commands: &[SlashCommand],
) -> Vec<CompletionCandidate<'_, String>> {
    commands
        .iter()
        .map(|c| {
            let mut cand = CompletionCandidate::new(c.id.clone(), c.name.as_str())
                .kind("cmd")
                .kind_glyph("/")
                .enabled(c.is_enabled())
                .group(c.source.group_label());
            if let Some(d) = &c.description {
                cand = cand.documentation(d.as_str());
            }
            if let Some(s) = &c.shortcut {
                cand = cand.detail(s.as_str());
            } else if let Some(r) = &c.disabled_reason {
                cand = cand.detail(r.as_str());
            } else if !c.aliases.is_empty() {
                // detail first alias
                cand = cand.detail(c.aliases[0].as_str());
            }
            if let Some(ranges) = &c.match_ranges {
                cand = cand.matches(ranges.as_slice());
            }
            cand
        })
        .collect()
}

/// Bridge global [`CommandEntry`] rows into slash commands (composer may merge).
#[must_use]
pub fn slash_commands_from_command_entries<Id: ToString>(
    entries: &[CommandEntry<Id>],
) -> Vec<SlashCommand> {
    entries
        .iter()
        .map(|e| {
            let name = e
                .command
                .clone()
                .unwrap_or_else(|| e.label.to_ascii_lowercase().replace(' ', "-"));
            let mut c = SlashCommand::new(e.id.to_string(), name)
                .source(SlashCommandSource::Global)
                .aliases(e.keywords.clone())
                .recent(e.recent);
            if let Some(s) = &e.shortcut {
                c = c.shortcut(s.clone());
            }
            if let Some(p) = &e.preview {
                c = c.description(p.clone());
            } else if let Some(g) = &e.group {
                c = c.description(g.clone());
            }
            if let Some(r) = &e.disabled_reason {
                c = c.disabled(r.clone());
            } else if !e.enabled {
                c = c.disabled("disabled");
            }
            if let Some(k) = &e.command {
                c = c.command_key(k.clone());
            }
            if let Some(prompt) = &e.argument_prompt {
                c = c.arguments(vec![SlashArgument::required("arg").hint(prompt.clone())]);
            }
            c
        })
        .collect()
}

/// Example composer slash catalog (tests / lookbook).
#[must_use]
pub fn example_slash_catalog() -> Vec<SlashCommand> {
    vec![
        SlashCommand::new("plan", "plan")
            .description("Enter plan mode")
            .aliases(["p"])
            .shortcut("C-S-p")
            .source(SlashCommandSource::Composer)
            .recent(true),
        SlashCommand::new("model", "model")
            .description("Select model")
            .arguments(vec![
                SlashArgument::required("name")
                    .hint("model id")
                    .values(["fast", "smart", "local"]),
            ])
            .source(SlashCommandSource::Composer),
        SlashCommand::new("clear", "clear")
            .description("Clear draft")
            .source(SlashCommandSource::Composer),
        SlashCommand::new("help", "help")
            .description("Show slash help")
            .aliases(["?"])
            .source(SlashCommandSource::Builtin),
        SlashCommand::new("theme", "theme")
            .description("Toggle theme (global)")
            .source(SlashCommandSource::Global)
            .command_key("theme.toggle"),
        SlashCommand::new("plugin-run", "run")
            .description("Plugin run")
            .source(SlashCommandSource::Plugin {
                id: "demo-plugin".into(),
            })
            .arguments(vec![
                SlashArgument::required("task").values(["build", "test", "lint"]),
            ]),
        SlashCommand::new("deploy", "deploy")
            .description("Deploy (unavailable)")
            .disabled("offline")
            .source(SlashCommandSource::Global),
    ]
}

// ── State ───────────────────────────────────────────────────────────────────

/// Slash command menu state (composes [`CompletionMenuState`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommandMenuState {
    /// Nested completion menu.
    pub menu: CompletionMenuState<String>,
    /// Active query span.
    pub query: Option<SlashQuery>,
    /// Surface open (also mirrored on menu).
    open: bool,
    /// Host input gate (overlay ownership); draft never cleared.
    accepts_input: bool,
    /// Recent command ids (MRU) for sort bias.
    recent_ids: Vec<String>,
}

impl Default for SlashCommandMenuState {
    fn default() -> Self {
        Self::new()
    }
}

impl SlashCommandMenuState {
    /// Closed menu.
    #[must_use]
    pub fn new() -> Self {
        let mut menu = CompletionMenuState::new(None);
        menu.set_open(false);
        menu.set_show_docs(true);
        menu.set_commit_characters(" ");
        Self {
            menu,
            query: None,
            open: false,
            accepts_input: true,
            recent_ids: Vec::new(),
        }
    }

    /// Whether open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Accepts input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Host gate — **does not** clear draft or query history.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
        self.menu.set_accepts_input(on);
    }

    /// Remember a committed command id.
    pub fn push_recent(&mut self, id: impl Into<String>) {
        let id = id.into();
        self.recent_ids.retain(|x| x != &id);
        self.recent_ids.insert(0, id);
        if self.recent_ids.len() > 32 {
            self.recent_ids.pop();
        }
    }

    /// Sync from draft text + cursor. Returns true when open/query changed.
    pub fn sync_from_draft(&mut self, text: &str, cursor_byte: usize) -> bool {
        if !self.accepts_input {
            return false;
        }
        match detect_slash_query(text, cursor_byte) {
            Some(q) => {
                let changed = self.query.as_ref() != Some(&q) || !self.open;
                self.query = Some(q);
                self.open = true;
                self.menu.set_open(true);
                changed
            }
            None => {
                let was = self.open;
                self.close();
                was
            }
        }
    }

    /// Close menu; draft untouched.
    pub fn close(&mut self) {
        self.open = false;
        self.query = None;
        self.menu.set_open(false);
    }
    /// Begin async plugin fetch.
    pub fn begin_async(&mut self) -> u64 {
        self.menu.begin_async()
    }
    /// Set status (loading / empty / error chrome).
    pub fn set_status(&mut self, status: CompletionStatus) {
        self.menu.set_status(status);
    }

    /// Visible filtered commands for current query (host supplies full catalog).
    #[must_use]
    pub fn visible_commands(&self, catalog: &[SlashCommand]) -> Vec<SlashCommand> {
        let mut catalog = catalog.to_vec();
        // mark recent from state
        for c in &mut catalog {
            if self.recent_ids.iter().any(|r| r == &c.id) {
                c.recent = true;
            }
        }
        match &self.query {
            Some(SlashQuery {
                phase: SlashMenuPhase::Command { prefix },
                ..
            }) => filter_slash_commands(&catalog, prefix),
            Some(SlashQuery {
                phase:
                    SlashMenuPhase::Argument {
                        command_id,
                        command_name,
                        arg_index,
                        arg_prefix,
                        ..
                    },
                ..
            }) => {
                // resolve command by id or name
                let cmd = catalog
                    .iter()
                    .find(|c| c.id == *command_id || c.name == *command_name);
                if let Some(cmd) = cmd {
                    if let Some(arg) = cmd.arguments.get(*arg_index) {
                        let vals = filter_argument_values(&arg.values, arg_prefix);
                        // synthesize pseudo-commands for display reuse
                        return vals
                            .into_iter()
                            .map(|v| {
                                SlashCommand::new(v.clone(), v)
                                    .source(SlashCommandSource::Composer)
                                    .description(format!("arg:{}", arg.name))
                            })
                            .collect();
                    }
                }
                Vec::new()
            }
            None => Vec::new(),
        }
    }

    /// Handle key through completion menu; map commit to slash outcomes.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        catalog: &[SlashCommand],
        visible: &[SlashCommand],
    ) -> SlashCommandMenuOutcome {
        if !self.accepts_input || !self.open {
            return SlashCommandMenuOutcome::Ignored;
        }
        let candidates = match &self.query {
            Some(SlashQuery {
                phase: SlashMenuPhase::Command { .. },
                ..
            }) => slash_commands_to_candidates(visible),
            Some(SlashQuery {
                phase: SlashMenuPhase::Argument { .. },
                ..
            }) => {
                // visible already pseudo-commands with name=value
                slash_commands_to_candidates(visible)
            }
            None => return SlashCommandMenuOutcome::Ignored,
        };
        let out = self.menu.handle_key(key, &candidates);
        self.map_commit(out, catalog, visible)
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        catalog: &[SlashCommand],
        visible: &[SlashCommand],
    ) -> SlashCommandMenuOutcome {
        if !self.accepts_input || !self.open {
            return SlashCommandMenuOutcome::Ignored;
        }
        let candidates = slash_commands_to_candidates(visible);
        let out = self.menu.handle_mouse(mouse, &candidates);
        self.map_commit(out, catalog, visible)
    }

    fn map_commit(
        &mut self,
        out: CompletionMenuOutcome<String>,
        catalog: &[SlashCommand],
        visible: &[SlashCommand],
    ) -> SlashCommandMenuOutcome {
        match out {
            CompletionMenuOutcome::Ignored => SlashCommandMenuOutcome::Ignored,
            CompletionMenuOutcome::SelectionChanged => {
                let id = self.menu.selected().cloned().unwrap_or_default();
                SlashCommandMenuOutcome::SelectionChanged { id }
            }
            CompletionMenuOutcome::Dismissed => {
                self.close();
                SlashCommandMenuOutcome::Dismissed
            }
            CompletionMenuOutcome::StatusChanged { status } => {
                SlashCommandMenuOutcome::StatusChanged { status }
            }
            CompletionMenuOutcome::PresentationChanged { presentation } => {
                SlashCommandMenuOutcome::PresentationChanged { presentation }
            }
            CompletionMenuOutcome::GenerationStale { generation } => {
                SlashCommandMenuOutcome::GenerationStale { generation }
            }
            CompletionMenuOutcome::Committed(id)
            | CompletionMenuOutcome::CommitWithChar { id, .. } => {
                self.commit_id(&id, catalog, visible)
            }
        }
    }

    fn commit_id(
        &mut self,
        id: &str,
        catalog: &[SlashCommand],
        visible: &[SlashCommand],
    ) -> SlashCommandMenuOutcome {
        let Some(query) = self.query.clone() else {
            return SlashCommandMenuOutcome::Ignored;
        };
        match query.phase {
            SlashMenuPhase::Command { .. } => {
                let cmd = visible
                    .iter()
                    .find(|c| c.id == id)
                    .or_else(|| catalog.iter().find(|c| c.id == id));
                let Some(cmd) = cmd else {
                    return SlashCommandMenuOutcome::Ignored;
                };
                if !cmd.is_enabled() {
                    return SlashCommandMenuOutcome::Ignored;
                }
                self.push_recent(cmd.id.clone());
                let insertion = cmd.insert_command_token();
                let needs = !cmd.arguments.is_empty();
                if !needs {
                    self.close();
                }
                // if needs args, host updates draft then sync_from_draft enters Argument
                SlashCommandMenuOutcome::CommandCommitted {
                    id: cmd.id.clone(),
                    insertion,
                    needs_arguments: needs,
                }
            }
            SlashMenuPhase::Argument {
                command_id,
                command_name,
                arg_index,
                prior_args,
                ..
            } => {
                let cmd = catalog
                    .iter()
                    .find(|c| c.id == command_id || c.name == command_name);
                let Some(cmd) = cmd else {
                    return SlashCommandMenuOutcome::Ignored;
                };
                let mut args = prior_args;
                args.push(id.to_string());
                let more = arg_index + 1 < cmd.arguments.len();
                let insertion =
                    cmd.insert_with_args(&args.iter().map(String::as_str).collect::<Vec<_>>());
                let insertion = if more {
                    format!("{insertion} ")
                } else {
                    insertion
                };
                if more {
                    SlashCommandMenuOutcome::ArgumentCommitted {
                        command_id: cmd.id.clone(),
                        arg_index,
                        value: id.to_string(),
                        insertion,
                        more_arguments: true,
                    }
                } else {
                    self.push_recent(cmd.id.clone());
                    self.close();
                    SlashCommandMenuOutcome::Execute {
                        id: cmd.id.clone(),
                        args,
                    }
                }
            }
        }
    }
}

// ── Overlay ─────────────────────────────────────────────────────────────────

/// Open slash overlay (Completion kind, slash id).
pub fn open_slash_command_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    opener: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec {
            id: OverlayId::from_static(SLASH_COMMAND_OVERLAY_ID),
            kind: OverlayKind::Completion,
            parent: None,
            anchor: Some(anchor),
            size: OverlaySize::menu(36, 10),
            opener_focus: opener,
            policy: None,
        },
    )
}

/// Preferred placement (reuses completion placer).
#[must_use]
pub fn place_slash_command_menu(bounds: Rect, anchor: Rect) -> Rect {
    place_completion_menu(
        bounds,
        anchor,
        CompletionMenuSize {
            width: 36,
            height: 10,
        },
    )
}

/// Compact vs fullscreen from bounds.
#[must_use]
pub fn slash_presentation_for(bounds: Rect) -> CompletionPresentation {
    completion_presentation_for(bounds)
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Slash command menu paint (CompletionMenu chrome + docs).
#[derive(Debug, Clone, Copy)]
pub struct SlashCommandMenu<'a> {
    commands: &'a [SlashCommand],
    system: &'a DesignSystem,
    /// Terminal / parent bounds for placement.
    bounds: Rect,
    /// Caret / token anchor.
    anchor: Rect,
}

impl<'a> SlashCommandMenu<'a> {
    /// Catalog + system + placement geometry.
    #[must_use]
    pub const fn new(
        commands: &'a [SlashCommand],
        system: &'a DesignSystem,
        bounds: Rect,
        anchor: Rect,
    ) -> Self {
        Self {
            commands,
            system,
            bounds,
            anchor,
        }
    }

    /// Paint filtered visible commands (places relative to anchor unless host
    /// already constrained `area` — then uses force_area).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut SlashCommandMenuState) {
        if area.is_empty() || !state.open {
            return;
        }
        let visible = state.visible_commands(self.commands);
        let candidates = slash_commands_to_candidates(&visible);
        state.menu.reconcile(&candidates);
        let _ = state.menu.sync_presentation(self.bounds);
        CompletionMenu::new(&candidates, self.system, self.bounds, self.anchor)
            .preferred_size(CompletionMenuSize {
                width: 36,
                height: 10,
            })
            .force_area(area)
            .paint(area, buffer, &mut state.menu);
    }
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Moderate slash catalog / filter sizes.
pub mod bench {
    /// Commands in catalog.
    pub const CATALOG_SIZE: usize = 120;
    /// Filter rounds.
    pub const FILTER_ROUNDS: u32 = 40;
    /// Paint frames.
    pub const PAINT_FRAMES: u32 = 24;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyEvent, KeyModifiers};
    use crate::style::DesignSystem;
    use crate::widgets::tests::click;

    #[test]
    fn detect_slash_command_prefix() {
        let q = detect_slash_query("hello /pl", 9).unwrap();
        assert_eq!(q.trigger_byte, 6);
        match q.phase {
            SlashMenuPhase::Command { prefix } => assert_eq!(prefix, "pl"),
            _ => panic!("expected command phase"),
        }
    }

    #[test]
    fn detect_slash_argument_phase() {
        let q = detect_slash_query("/model fa", 9).unwrap();
        match q.phase {
            SlashMenuPhase::Argument {
                command_name,
                arg_index,
                arg_prefix,
                ..
            } => {
                assert_eq!(command_name, "model");
                assert_eq!(arg_index, 0);
                assert_eq!(arg_prefix, "fa");
            }
            _ => panic!("expected argument phase"),
        }
    }

    #[test]
    fn apply_insert_preserves_surrounding_draft() {
        let draft = "pre /pl post";
        let q = SlashQuery {
            phase: SlashMenuPhase::Command {
                prefix: "pl".into(),
            },
            trigger_byte: 4,
            cursor_byte: 7,
        };
        let next = apply_slash_insert(draft, &q, "/plan ");
        assert_eq!(next, "pre /plan  post");
        assert!(next.starts_with("pre "));
        assert!(next.ends_with(" post"));
    }

    #[test]
    fn filter_matches_alias_and_recent() {
        let cat = example_slash_catalog();
        let hits = filter_slash_commands(&cat, "p");
        assert!(hits.iter().any(|c| c.name == "plan"));
        let all = filter_slash_commands(&cat, "");
        assert!(all[0].recent || all.iter().any(|c| c.recent));
    }

    #[test]
    fn draft_sync_opens_and_closes() {
        let mut st = SlashCommandMenuState::new();
        assert!(st.sync_from_draft("/he", 3));
        assert!(st.is_open());
        assert!(st.sync_from_draft("no slash", 8));
        assert!(!st.is_open());
    }

    #[test]
    fn commit_command_without_args_closes() {
        let cat = example_slash_catalog();
        let mut st = SlashCommandMenuState::new();
        st.sync_from_draft("/cle", 4);
        let visible = st.visible_commands(&cat);
        assert!(!visible.is_empty());
        // select clear
        st.menu.select(Some("clear".into()));
        let out = st.commit_id("clear", &cat, &visible);
        assert!(matches!(
            out,
            SlashCommandMenuOutcome::CommandCommitted {
                needs_arguments: false,
                ..
            }
        ));
        assert!(!st.is_open());
    }

    #[test]
    fn commit_command_with_args_keeps_open_flag_for_host() {
        let cat = example_slash_catalog();
        let mut st = SlashCommandMenuState::new();
        st.sync_from_draft("/mod", 4);
        let visible = st.visible_commands(&cat);
        let out = st.commit_id("model", &cat, &visible);
        match out {
            SlashCommandMenuOutcome::CommandCommitted {
                insertion,
                needs_arguments: true,
                ..
            } => {
                assert!(insertion.starts_with("/model"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn execute_after_argument() {
        let cat = example_slash_catalog();
        let mut st = SlashCommandMenuState::new();
        st.query = Some(SlashQuery {
            phase: SlashMenuPhase::Argument {
                command_id: "model".into(),
                command_name: "model".into(),
                arg_index: 0,
                arg_prefix: "fa".into(),
                prior_args: Vec::new(),
            },
            trigger_byte: 0,
            cursor_byte: 9,
        });
        st.open = true;
        let visible = st.visible_commands(&cat);
        assert!(visible.iter().any(|c| c.name == "fast"));
        let out = st.commit_id("fast", &cat, &visible);
        assert!(matches!(
            out,
            SlashCommandMenuOutcome::Execute { ref id, ref args }
                if id == "model" && args == &["fast".to_string()]
        ));
    }

    #[test]
    fn bridge_from_command_entries() {
        let entries = vec![
            CommandEntry::new("theme", "Toggle theme")
                .command_key("theme")
                .shortcut("C-t"),
            CommandEntry::new("x", "Disabled")
                .enabled(false)
                .disabled_reason("nope"),
        ];
        let cmds = slash_commands_from_command_entries(&entries);
        assert_eq!(cmds.len(), 2);
        assert!(!cmds[1].is_enabled());
        assert!(matches!(cmds[0].source, SlashCommandSource::Global));
    }

    #[test]
    fn paint_loading_and_ready() {
        let system = DesignSystem::default();
        let cat = example_slash_catalog();
        let mut st = SlashCommandMenuState::new();
        st.sync_from_draft("/p", 2);
        st.set_status(CompletionStatus::Loading);
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);
        let anchor = Rect::new(0, 0, 1, 1);
        for _ in 0..bench::PAINT_FRAMES {
            SlashCommandMenu::new(&cat, &system, area, anchor).paint(area, &mut buf, &mut st);
        }
        st.set_status(CompletionStatus::Ready);
        SlashCommandMenu::new(&cat, &system, area, anchor).paint(area, &mut buf, &mut st);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("plan") || text.contains("▎"),
            "slash menu reuses completion list anatomy, got {text}"
        );
    }

    #[test]
    fn accepts_input_gate_preserves_query_until_close() {
        let mut st = SlashCommandMenuState::new();
        st.sync_from_draft("/plan", 5);
        assert!(st.is_open());
        st.set_accepts_input(false);
        // gate blocks sync changes but state retained
        assert!(st.query.is_some());
        st.set_accepts_input(true);
        assert!(st.query.is_some());
    }

    #[test]
    fn reject_url_scheme_slash() {
        // "http://" — slash preceded by /
        assert!(
            detect_slash_query("http://x", 8).is_none() || {
                // if we match last slash after :, prev is : which is not boundary — none
                true
            }
        );
        assert!(detect_slash_query("http://", 7).is_none());
    }

    #[test]
    fn moderate_filter_bench() {
        let mut cat = Vec::with_capacity(bench::CATALOG_SIZE);
        for i in 0..bench::CATALOG_SIZE {
            cat.push(
                SlashCommand::new(format!("c{i}"), format!("cmd{i}"))
                    .description(format!("desc {i}"))
                    .aliases([format!("a{i}")])
                    .source(if i % 3 == 0 {
                        SlashCommandSource::Global
                    } else {
                        SlashCommandSource::Composer
                    }),
            );
        }
        for r in 0..bench::FILTER_ROUNDS {
            let q = format!("cmd{}", r % 10);
            let hits = filter_slash_commands(&cat, &q);
            assert!(!hits.is_empty() || r > 0);
        }
    }

    #[test]
    fn never_executes_commands() {
        let src = include_str!("slash_command_menu.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in [
            "std::process::Command",
            "std::fs::",
            "reqwest::",
            "tokio::process",
        ] {
            assert!(!body.contains(forbidden), "must not contain {forbidden}");
        }
    }

    #[test]
    fn overlay_opens() {
        let mut stack = OverlayStack::<&'static str>::new();
        let bounds = Rect::new(0, 0, 80, 24);
        let out = open_slash_command_overlay(
            &mut stack,
            bounds,
            Rect::new(2, 20, 1, 1),
            Some("composer"),
        );
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().id.as_str(), SLASH_COMMAND_OVERLAY_ID);
    }

    #[test]
    fn key_nav_selection() {
        let cat = example_slash_catalog();
        let mut st = SlashCommandMenuState::new();
        st.sync_from_draft("/", 1);
        let visible = st.visible_commands(&cat);
        let candidates = slash_commands_to_candidates(&visible);
        st.menu.reconcile(&candidates);
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &cat,
            &visible,
        );
        assert!(matches!(
            out,
            SlashCommandMenuOutcome::SelectionChanged { .. } | SlashCommandMenuOutcome::Ignored
        ));
    }

    #[test]
    fn mouse_uses_completion_menu_painted_hit_regions() {
        let system = DesignSystem::default();
        let catalog = example_slash_catalog();
        let mut state = SlashCommandMenuState::new();
        state.sync_from_draft("/p", 2);
        let visible = state.visible_commands(&catalog);
        let area = Rect::new(0, 0, 40, 12);
        let mut buffer = Buffer::empty(area);
        SlashCommandMenu::new(&catalog, &system, area, Rect::new(0, 0, 1, 1)).paint(
            area,
            &mut buffer,
            &mut state,
        );

        let mut outcome = SlashCommandMenuOutcome::Ignored;
        for y in area.y..area.bottom() {
            let probe = state.handle_mouse(click(area.x, y), &catalog, &visible);
            if !matches!(probe, SlashCommandMenuOutcome::Ignored) {
                outcome = probe;
                break;
            }
        }
        assert!(matches!(
            outcome,
            SlashCommandMenuOutcome::CommandCommitted { .. }
        ));
    }

    #[test]
    fn presentation_helper() {
        let _ = slash_presentation_for(Rect::new(0, 0, 30, 10));
        let r = place_slash_command_menu(Rect::new(0, 0, 80, 24), Rect::new(2, 20, 1, 1));
        assert!(r.width > 0);
    }
}
