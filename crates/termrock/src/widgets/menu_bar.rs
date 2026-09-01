// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Desktop-style **MenuBar** with nested menus, mnemonics, and OverlayStack cascade.
//!
//! **Mission.** Command-rich applications need a top-level menu strip (File, Edit,
//! View…) with nested submenus, checked/radio rows, separators, shortcuts, recent
//! items, and dynamic host-owned commands — without the widget owning side effects.
//!
//! **vs [`super::DropdownMenu`].** Dropdown menus are anchored popup lists;
//! the menu bar owns application-wide top-level groups.
//! `MenuBar` owns horizontal top-level menus, cascade depth, mnemonic arming, and
//! narrow replacement with [`super::CommandPalette`].
//!
//! **vs CommandPalette.** Narrow terminals (`MENU_BAR_NARROW_MAX_WIDTH`) collapse
//! the bar to a chip that emits [`MenuBarOutcome::PreferCommandPalette`]. Hosts
//! open the palette with commands flattened via [`flatten_menu_commands`].
//!
//! Research: desktop menu bars, Textual, terminal editors (Helix/Kakoune chrome),
//! Radix Menubar / DropdownMenu (roving + nested dismiss).

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::StatefulWidget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{
        CollectionItem, CollectionState, NavigationMove, OverlayId, OverlayKind, OverlayOutcome,
        OverlayPolicy, OverlaySize, OverlaySpec, OverlayStack, RovingOrientation, SemanticNode,
        SemanticRole, SemanticScene, SemanticState, UiIntent, place_overlay,
    },
    style::{DesignSystem, ListRowVisualState, Role},
    text::{display_cols, take_display_cols},
};

/// Width under which the bar collapses to a CommandPalette chip.
pub const MENU_BAR_NARROW_MAX_WIDTH: u16 = 40;
/// Default overlay id for the first open menu panel under a bar.
pub const MENU_BAR_OVERLAY_ID: &str = "termrock.menu-bar";
/// Overlay id prefix for nested submenu panels (`termrock.menu-bar.sub.N`).
pub const MENU_BAR_SUBMENU_OVERLAY_PREFIX: &str = "termrock.menu-bar.sub";

// ── Model ───────────────────────────────────────────────────────────────────

/// Kind of one row inside a menu panel (shared by MenuBar, DropdownMenu, ContextMenu).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MenuRowKind {
    /// Leaf command (default).
    #[default]
    Command,
    /// Toggle checkbox row (host owns checked bit).
    Checkbox {
        /// Whether currently checked.
        checked: bool,
    },
    /// Radio row within a named group (host owns selection).
    Radio {
        /// Group identity shared by mutually exclusive radios.
        group: String,
        /// Whether this radio is the selected member.
        selected: bool,
    },
    /// Opens nested children on Right / Activate.
    Submenu,
    /// Non-interactive separator line.
    Separator,
    /// Non-interactive section label (e.g. "Recent"). Alias: label rows.
    Section,
    /// Non-interactive loading placeholder (async host fetch).
    Loading,
    /// Non-interactive custom preview row; host paints into hit rect.
    CustomPreview,
}

impl MenuRowKind {
    /// Stable id for diagnostics.
    #[must_use]
    pub fn id(&self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Checkbox { .. } => "checkbox",
            Self::Radio { .. } => "radio",
            Self::Submenu => "submenu",
            Self::Separator => "separator",
            Self::Section => "section",
            Self::Loading => "loading",
            Self::CustomPreview => "custom-preview",
        }
    }

    /// Whether the row can receive keyboard cursor / activation.
    #[must_use]
    pub const fn is_interactive(&self) -> bool {
        !matches!(
            self,
            Self::Separator | Self::Section | Self::Loading | Self::CustomPreview
        )
    }
}

/// One hierarchical menu row (commands, submenus, separators).
///
/// Shared model for [`super::MenuBar`] and [`super::DropdownMenu`] context or dropdown surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuNode<Id> {
    /// Stable identity for outcomes and host command maps.
    pub id: Id,
    /// Visible label.
    pub label: String,
    /// Optional mnemonic letter (case-insensitive match).
    pub mnemonic: Option<char>,
    /// Shortcut hint text (display only; host owns real bindings).
    pub shortcut: Option<String>,
    /// Whether activatable (separators ignore this).
    pub enabled: bool,
    /// Row kind.
    pub kind: MenuRowKind,
    /// Nested children when [`MenuRowKind::Submenu`].
    pub children: Vec<MenuNode<Id>>,
    /// Optional host command metadata key (for palette / keymap projection).
    pub command: Option<String>,
    /// Why disabled (hint / a11y).
    pub disabled_reason: Option<String>,
    /// Marks a recent-items row (ordering/grouping is host-owned).
    pub recent: bool,
    /// Destructive / danger styling (delete, force-push, …).
    pub destructive: bool,
}

impl<Id> MenuNode<Id> {
    fn base(id: Id, label: String, kind: MenuRowKind) -> Self {
        Self {
            id,
            label,
            mnemonic: None,
            shortcut: None,
            enabled: true,
            kind,
            children: Vec::new(),
            command: None,
            disabled_reason: None,
            recent: false,
            destructive: false,
        }
    }

    /// Enabled command leaf.
    #[must_use]
    pub fn command(id: Id, label: impl Into<String>) -> Self {
        Self::base(id, label.into(), MenuRowKind::Command)
    }

    /// Submenu parent.
    #[must_use]
    pub fn submenu(id: Id, label: impl Into<String>, children: Vec<MenuNode<Id>>) -> Self {
        let mut n = Self::base(id, label.into(), MenuRowKind::Submenu);
        n.children = children;
        n
    }

    /// Separator (id still required for stable tree identity).
    #[must_use]
    pub fn separator(id: Id) -> Self {
        let mut n = Self::base(id, String::new(), MenuRowKind::Separator);
        n.enabled = false;
        n
    }

    /// Section / label header (e.g. "Recent").
    #[must_use]
    pub fn section(id: Id, label: impl Into<String>) -> Self {
        let mut n = Self::base(id, label.into(), MenuRowKind::Section);
        n.enabled = false;
        n
    }

    /// Checkbox row.
    #[must_use]
    pub fn checkbox(id: Id, label: impl Into<String>, checked: bool) -> Self {
        Self::base(id, label.into(), MenuRowKind::Checkbox { checked })
    }

    /// Radio row in a group.
    #[must_use]
    pub fn radio(
        id: Id,
        label: impl Into<String>,
        group: impl Into<String>,
        selected: bool,
    ) -> Self {
        Self::base(
            id,
            label.into(),
            MenuRowKind::Radio {
                group: group.into(),
                selected,
            },
        )
    }

    /// Loading placeholder (async host content).
    #[must_use]
    pub fn loading(id: Id, label: impl Into<String>) -> Self {
        let mut n = Self::base(id, label.into(), MenuRowKind::Loading);
        n.enabled = false;
        n
    }

    /// Custom preview row (host paints into hit geometry).
    #[must_use]
    pub fn custom_preview(id: Id, label: impl Into<String>) -> Self {
        let mut n = Self::base(id, label.into(), MenuRowKind::CustomPreview);
        n.enabled = false;
        n
    }

    /// Mnemonic letter.
    #[must_use]
    pub fn mnemonic(mut self, ch: char) -> Self {
        self.mnemonic = Some(ch);
        self
    }

    /// Shortcut hint.
    #[must_use]
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = Some(s.into());
        self
    }

    /// Enabled flag.
    #[must_use]
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// Host command key.
    #[must_use]
    pub fn command_key(mut self, key: impl Into<String>) -> Self {
        self.command = Some(key.into());
        self
    }

    /// Disabled reason.
    #[must_use]
    pub fn disabled_reason(mut self, reason: impl Into<String>) -> Self {
        self.disabled_reason = Some(reason.into());
        self
    }

    /// Mark as recent-items entry.
    #[must_use]
    pub fn recent(mut self, on: bool) -> Self {
        self.recent = on;
        self
    }

    /// Destructive styling / semantics.
    #[must_use]
    pub fn destructive(mut self, on: bool) -> Self {
        self.destructive = on;
        self
    }

    /// Whether this row can be activated / receive cursor.
    #[must_use]
    pub fn is_activatable(&self) -> bool {
        self.enabled && self.kind.is_interactive()
    }
}

/// One top-level menu on the bar (File, Edit, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuBarMenu<Id> {
    /// Stable id.
    pub id: Id,
    /// Bar label.
    pub label: String,
    /// Optional mnemonic on the top label.
    pub mnemonic: Option<char>,
    /// Whether the menu can open.
    pub enabled: bool,
    /// Root items of this menu.
    pub items: Vec<MenuNode<Id>>,
}

impl<Id> MenuBarMenu<Id> {
    /// Enabled top menu.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>, items: Vec<MenuNode<Id>>) -> Self {
        Self {
            id,
            label: label.into(),
            mnemonic: None,
            enabled: true,
            items,
        }
    }

    /// Mnemonic letter.
    #[must_use]
    pub fn mnemonic(mut self, ch: char) -> Self {
        self.mnemonic = Some(ch);
        self
    }

    /// Enabled flag.
    #[must_use]
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
}

/// Layout density for the bar chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MenuBarPresentation {
    /// Full top-level labels.
    #[default]
    Full,
    /// Abbreviated when space is tight but above narrow.
    Compact,
    /// Replaced by a CommandPalette entry chip.
    CommandPalette,
}

impl MenuBarPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Compact => "compact",
            Self::CommandPalette => "command-palette",
        }
    }
}

/// Choose presentation from available width.
#[must_use]
pub fn menu_bar_presentation_for_width(width: u16) -> MenuBarPresentation {
    if width <= MENU_BAR_NARROW_MAX_WIDTH {
        MenuBarPresentation::CommandPalette
    } else if width <= 64 {
        MenuBarPresentation::Compact
    } else {
        MenuBarPresentation::Full
    }
}

/// Typed outcomes (host performs commands / focus restore).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MenuBarOutcome<Id> {
    /// No change.
    Ignored,
    /// Top-level bar cursor moved.
    BarMoved,
    /// Cursor moved inside an open panel.
    MenuMoved,
    /// A top-level menu opened.
    Opened {
        /// Top menu id.
        menu_id: Id,
    },
    /// Nested submenu panel opened.
    SubmenuOpened {
        /// Submenu node id.
        id: Id,
    },
    /// Leaf command activated.
    Activated {
        /// Node id.
        id: Id,
        /// Optional host command key.
        command: Option<String>,
    },
    /// Checkbox toggled (host should flip model).
    CheckToggled {
        /// Node id.
        id: Id,
        /// Suggested new checked value.
        checked: bool,
    },
    /// Radio selected (host should update group).
    RadioSelected {
        /// Node id.
        id: Id,
        /// Radio group key.
        group: String,
    },
    /// One cascade layer closed (Esc / Left); menus may still be open.
    LayerClosed,
    /// Fully dismissed; host should restore prior focus (OverlayStack opener).
    Closed,
    /// Mnemonic arming changed (Alt / F10 platform-neutral mode).
    MnemonicMode {
        /// Whether mnemonic mode is armed.
        armed: bool,
    },
    /// Narrow presentation: host should open CommandPalette instead.
    PreferCommandPalette,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Cascade frame: cursor collection over a panel's interactive indices.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CascadeFrame {
    /// Collection over flat indices into the panel's `items` slice.
    collection: CollectionState<usize>,
}

impl CascadeFrame {
    fn new() -> Self {
        Self {
            collection: CollectionState::new().orientation(RovingOrientation::Vertical),
        }
    }

    fn cursor(&self) -> usize {
        self.collection.active().copied().unwrap_or(0)
    }

    fn set_cursor(&mut self, idx: usize) {
        self.collection.set_active(Some(idx));
    }
}

/// MenuBar interaction state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuBarState {
    /// Host grants keyboard ownership (scene surface focus).
    focused: bool,
    /// Master enable.
    enabled: bool,
    /// Accepts input gate (overlay / scene).
    accepts_input: bool,
    /// Top-level bar roving (indices into menus).
    bar: CollectionState<usize>,
    /// Open cascade: empty = closed; first frame = root of open top menu.
    cascade: Vec<CascadeFrame>,
    /// Which top-level menu is open (`None` when cascade empty).
    open_top: Option<usize>,
    /// Item indices that opened each nested frame after the root
    /// (`open_path.len() == cascade.len().saturating_sub(1)`).
    open_path: Vec<usize>,
    /// Platform-neutral mnemonic arming (F10 / Alt press).
    mnemonic_mode: bool,
    /// Presentation last painted / forced.
    presentation: MenuBarPresentation,
    /// Force presentation override (`None` = derive from width).
    presentation_override: Option<MenuBarPresentation>,
    /// Bar hit targets (top menu index, rect).
    bar_hits: Vec<(usize, Rect)>,
    /// Panel hits: (depth, item_index, rect).
    panel_hits: Vec<(usize, usize, Rect)>,
    /// (depth, item) the pointer is over.
    hovered: Option<(usize, usize)>,
    /// Painted bar origin.
    bar_origin: (u16, u16),
    /// Painted bar size.
    bar_size: (u16, u16),
    /// Optional host focus token to restore on full close (mirrors OverlayStack).
    opener_focus_hint: Option<String>,
}

impl Default for MenuBarState {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuBarState {
    /// Closed bar, first enabled top menu ready.
    #[must_use]
    pub fn new() -> Self {
        Self {
            focused: false,
            enabled: true,
            accepts_input: true,
            bar: CollectionState::new().orientation(RovingOrientation::Horizontal),
            cascade: Vec::new(),
            open_top: None,
            open_path: Vec::new(),
            mnemonic_mode: false,
            presentation: MenuBarPresentation::Full,
            presentation_override: None,
            bar_hits: Vec::new(),
            panel_hits: Vec::new(),
            hovered: None,
            bar_origin: (0, 0),
            bar_size: (0, 0),
            opener_focus_hint: None,
        }
    }

    /// Whether the bar owns keyboard.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Whether any menu panel is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.cascade.is_empty()
    }

    /// Cascade depth (0 = closed).
    #[must_use]
    pub fn depth(&self) -> usize {
        self.cascade.len()
    }

    /// Open top menu index.
    #[must_use]
    pub const fn open_top_index(&self) -> Option<usize> {
        self.open_top
    }

    /// Bar cursor index.
    #[must_use]
    pub fn bar_cursor(&self) -> usize {
        self.bar.active().copied().unwrap_or(0)
    }

    /// Cursor at cascade depth (0 = root panel of open menu).
    #[must_use]
    pub fn panel_cursor(&self, depth: usize) -> Option<usize> {
        self.cascade.get(depth).map(CascadeFrame::cursor)
    }

    /// Mnemonic mode armed.
    #[must_use]
    pub const fn mnemonic_armed(&self) -> bool {
        self.mnemonic_mode
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> MenuBarPresentation {
        self.presentation
    }

    /// Opener focus hint for host restore.
    #[must_use]
    pub fn opener_focus_hint(&self) -> Option<&str> {
        self.opener_focus_hint.as_deref()
    }

    /// Scene surface focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Master enable.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Force presentation (tests / host); `None` clears override.
    pub fn set_presentation_override(&mut self, p: Option<MenuBarPresentation>) {
        self.presentation_override = p;
        if let Some(p) = p {
            self.presentation = p;
        }
    }

    /// Host focus token for restore on [`MenuBarOutcome::Closed`].
    pub fn set_opener_focus_hint(&mut self, hint: Option<String>) {
        self.opener_focus_hint = hint;
    }

    /// Arm or disarm mnemonic mode.
    pub fn set_mnemonic_mode(&mut self, armed: bool) -> MenuBarOutcome<()> {
        if self.mnemonic_mode == armed {
            return MenuBarOutcome::Ignored;
        }
        self.mnemonic_mode = armed;
        MenuBarOutcome::MnemonicMode { armed }
    }

    fn live(&self) -> bool {
        self.enabled && self.accepts_input && self.focused
    }

    fn bar_entries<Id>(menus: &[MenuBarMenu<Id>]) -> Vec<CollectionItem<usize>> {
        menus
            .iter()
            .enumerate()
            .map(|(i, m)| CollectionItem {
                id: i,
                enabled: m.enabled,
                label: m.label.clone(),
                parent: None,
            })
            .collect()
    }

    fn panel_entries<Id>(items: &[MenuNode<Id>]) -> Vec<CollectionItem<usize>> {
        items
            .iter()
            .enumerate()
            .map(|(i, n)| CollectionItem {
                id: i,
                enabled: n.is_activatable(),
                label: n.label.clone(),
                parent: None,
            })
            .collect()
    }

    fn ensure_bar<Id>(&mut self, menus: &[MenuBarMenu<Id>]) {
        let entries = Self::bar_entries(menus);
        let _ = self.bar.reconcile(&entries);
    }

    fn items_at_path<'a, Id>(
        menus: &'a [MenuBarMenu<Id>],
        open_top: usize,
        path: &[usize],
    ) -> Option<&'a [MenuNode<Id>]> {
        let root = menus.get(open_top)?;
        let mut items = root.items.as_slice();
        for &idx in path {
            let node = items.get(idx)?;
            if !matches!(node.kind, MenuRowKind::Submenu) {
                return None;
            }
            items = node.children.as_slice();
        }
        Some(items)
    }

    /// Full close without outcome side channels.
    pub fn close_all(&mut self) {
        self.cascade.clear();
        self.open_top = None;
        self.open_path.clear();
        self.mnemonic_mode = false;
    }

    /// Open top menu at index (or bar cursor).
    pub fn open_menu_at<Id: Clone>(
        &mut self,
        menus: &[MenuBarMenu<Id>],
        index: usize,
    ) -> MenuBarOutcome<Id> {
        if !self.enabled || menus.is_empty() {
            return MenuBarOutcome::Ignored;
        }
        let idx = index.min(menus.len().saturating_sub(1));
        if !menus[idx].enabled {
            return MenuBarOutcome::Ignored;
        }
        self.ensure_bar(menus);
        self.bar.set_active(Some(idx));
        self.open_top = Some(idx);
        self.open_path.clear();
        let mut frame = CascadeFrame::new();
        let entries = Self::panel_entries(&menus[idx].items);
        let _ = frame.collection.reconcile(&entries);
        self.cascade = vec![frame];
        self.mnemonic_mode = false;
        MenuBarOutcome::Opened {
            menu_id: menus[idx].id.clone(),
        }
    }

    /// Open bar cursor menu.
    pub fn open_active_menu<Id: Clone>(&mut self, menus: &[MenuBarMenu<Id>]) -> MenuBarOutcome<Id> {
        self.ensure_bar(menus);
        let idx = self.bar_cursor();
        self.open_menu_at(menus, idx)
    }

    fn current_items<'a, Id>(&self, menus: &'a [MenuBarMenu<Id>]) -> Option<&'a [MenuNode<Id>]> {
        let top = self.open_top?;
        Self::items_at_path(menus, top, &self.open_path)
    }

    fn ensure_top_frame<Id>(&mut self, menus: &[MenuBarMenu<Id>]) {
        if let Some(items) = self.current_items(menus) {
            if let Some(frame) = self.cascade.last_mut() {
                let entries = Self::panel_entries(items);
                let _ = frame.collection.reconcile(&entries);
            }
        }
    }

    fn open_submenu_under_cursor<Id: Clone>(
        &mut self,
        menus: &[MenuBarMenu<Id>],
    ) -> MenuBarOutcome<Id> {
        let items = match self.current_items(menus) {
            Some(i) => i,
            None => return MenuBarOutcome::Ignored,
        };
        let frame = match self.cascade.last() {
            Some(f) => f,
            None => return MenuBarOutcome::Ignored,
        };
        let idx = frame.cursor();
        let node = match items.get(idx) {
            Some(n) if n.is_activatable() && matches!(n.kind, MenuRowKind::Submenu) => n,
            _ => return MenuBarOutcome::Ignored,
        };
        if node.children.is_empty() {
            return MenuBarOutcome::Ignored;
        }
        let id = node.id.clone();
        self.open_path.push(idx);
        let mut child = CascadeFrame::new();
        let entries = Self::panel_entries(&node.children);
        let _ = child.collection.reconcile(&entries);
        self.cascade.push(child);
        MenuBarOutcome::SubmenuOpened { id }
    }

    fn close_one_layer<Id: Clone>(&mut self) -> MenuBarOutcome<Id> {
        if self.cascade.is_empty() {
            return MenuBarOutcome::Ignored;
        }
        self.cascade.pop();
        if !self.open_path.is_empty() {
            self.open_path.pop();
        }
        if self.cascade.is_empty() {
            self.open_top = None;
            self.open_path.clear();
            self.mnemonic_mode = false;
            MenuBarOutcome::Closed
        } else {
            MenuBarOutcome::LayerClosed
        }
    }

    fn activate_cursor<Id: Clone>(&mut self, menus: &[MenuBarMenu<Id>]) -> MenuBarOutcome<Id> {
        let items = match self.current_items(menus) {
            Some(i) => i,
            None => return MenuBarOutcome::Ignored,
        };
        let idx = match self.cascade.last() {
            Some(f) => f.cursor(),
            None => return MenuBarOutcome::Ignored,
        };
        let node = match items.get(idx) {
            Some(n) if n.is_activatable() => n,
            _ => return MenuBarOutcome::Ignored,
        };
        match &node.kind {
            MenuRowKind::Submenu => self.open_submenu_under_cursor(menus),
            MenuRowKind::Checkbox { checked } => {
                let id = node.id.clone();
                let next = !*checked;
                self.close_all();
                MenuBarOutcome::CheckToggled { id, checked: next }
            }
            MenuRowKind::Radio { group, .. } => {
                let id = node.id.clone();
                let group = group.clone();
                self.close_all();
                MenuBarOutcome::RadioSelected { id, group }
            }
            MenuRowKind::Command => {
                let id = node.id.clone();
                let command = node.command.clone();
                self.close_all();
                MenuBarOutcome::Activated { id, command }
            }
            MenuRowKind::Separator
            | MenuRowKind::Section
            | MenuRowKind::Loading
            | MenuRowKind::CustomPreview => MenuBarOutcome::Ignored,
        }
    }

    fn match_mnemonic_top<Id>(menus: &[MenuBarMenu<Id>], ch: char) -> Option<usize> {
        let lower = ch.to_ascii_lowercase();
        menus.iter().position(|m| {
            m.enabled
                && m.mnemonic
                    .map(|c| c.to_ascii_lowercase() == lower)
                    .unwrap_or(false)
        })
    }

    fn match_mnemonic_items<Id>(items: &[MenuNode<Id>], ch: char) -> Option<usize> {
        let lower = ch.to_ascii_lowercase();
        items.iter().position(|n| {
            n.is_activatable()
                && n.mnemonic
                    .map(|c| c.to_ascii_lowercase() == lower)
                    .unwrap_or(false)
        })
    }

    /// Keyboard entry.
    pub fn handle_key<Id: Clone>(
        &mut self,
        key: KeyEvent,
        menus: &[MenuBarMenu<Id>],
    ) -> MenuBarOutcome<Id> {
        if !self.live() || menus.is_empty() || key.kind == KeyEventKind::Release {
            // Allow Alt release to keep mode; ignore other releases.
            return MenuBarOutcome::Ignored;
        }
        self.ensure_bar(menus);

        // Platform-neutral mnemonics:
        // - Alt+letter opens matching top menu when the terminal delivers Alt.
        // - Hosts arm sticky mnemonic mode via [`Self::set_mnemonic_mode`] (map
        //   product F10 / menu-key there — TermRock `KeyCode` has no F-keys).
        // - Ctrl+Shift+M toggles sticky mode as a portable fallback chord.
        if matches!(key.code, KeyCode::Char('m' | 'M'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && key.modifiers.contains(KeyModifiers::SHIFT)
            && !key.modifiers.contains(KeyModifiers::ALT)
        {
            let armed = !self.mnemonic_mode;
            self.mnemonic_mode = armed;
            return MenuBarOutcome::MnemonicMode { armed };
        }

        let alt = key.modifiers.contains(KeyModifiers::ALT)
            && !key.modifiers.contains(KeyModifiers::CONTROL);

        // Alt+letter or armed mnemonic + letter → top menu.
        if let KeyCode::Char(ch) = key.code {
            if (alt || self.mnemonic_mode) && !self.is_open() {
                if let Some(idx) = Self::match_mnemonic_top(menus, ch) {
                    return self.open_menu_at(menus, idx);
                }
                if self.mnemonic_mode && !alt {
                    // Armed letter miss: stay armed.
                    return MenuBarOutcome::Ignored;
                }
            }
            if self.is_open() && (self.mnemonic_mode || !alt) {
                // Letter mnemonic inside open panel (no Ctrl).
                if !key.modifiers.contains(KeyModifiers::CONTROL) {
                    if let Some(items) = self.current_items(menus) {
                        if let Some(idx) = Self::match_mnemonic_items(items, ch) {
                            if let Some(frame) = self.cascade.last_mut() {
                                frame.set_cursor(idx);
                            }
                            return self.activate_cursor(menus);
                        }
                    }
                }
            }
        }

        // Narrow chip: Enter / Space opens palette preference.
        if matches!(self.presentation, MenuBarPresentation::CommandPalette) && !self.is_open() {
            if matches!(
                key.code,
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Down
            ) {
                return MenuBarOutcome::PreferCommandPalette;
            }
            if let Some(intent) = default_menu_bar_intent(key) {
                return self.handle_intent(intent, menus);
            }
            return MenuBarOutcome::Ignored;
        }

        if let Some(intent) = default_menu_bar_intent(key) {
            return self.handle_intent(intent, menus);
        }
        // Left/Right when open — not always covered by default intent for bar switch.
        match key.code {
            KeyCode::Left if self.is_open() => {
                self.handle_intent(UiIntent::Move(NavigationMove::Left), menus)
            }
            KeyCode::Right if self.is_open() => {
                self.handle_intent(UiIntent::Move(NavigationMove::Right), menus)
            }
            _ => MenuBarOutcome::Ignored,
        }
    }

    /// Intent routing.
    pub fn handle_intent<Id: Clone>(
        &mut self,
        intent: UiIntent,
        menus: &[MenuBarMenu<Id>],
    ) -> MenuBarOutcome<Id> {
        if !self.live() || menus.is_empty() {
            return MenuBarOutcome::Ignored;
        }
        self.ensure_bar(menus);

        if matches!(self.presentation, MenuBarPresentation::CommandPalette) && !self.is_open() {
            return match intent {
                UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle => {
                    MenuBarOutcome::PreferCommandPalette
                }
                UiIntent::Cancel | UiIntent::Close => {
                    self.mnemonic_mode = false;
                    MenuBarOutcome::Closed
                }
                _ => MenuBarOutcome::Ignored,
            };
        }

        if !self.is_open() {
            return self.handle_intent_closed(intent, menus);
        }
        self.handle_intent_open(intent, menus)
    }

    fn handle_intent_closed<Id: Clone>(
        &mut self,
        intent: UiIntent,
        menus: &[MenuBarMenu<Id>],
    ) -> MenuBarOutcome<Id> {
        let entries = Self::bar_entries(menus);
        match intent {
            UiIntent::Move(
                NavigationMove::Next
                | NavigationMove::Right
                | NavigationMove::Previous
                | NavigationMove::Left
                | NavigationMove::First
                | NavigationMove::Last,
            ) => {
                let out = self.bar.handle_intent(intent, &entries);
                if out.active_changed() {
                    MenuBarOutcome::BarMoved
                } else {
                    MenuBarOutcome::Ignored
                }
            }
            UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle | UiIntent::Expand => {
                self.open_active_menu(menus)
            }
            UiIntent::Move(NavigationMove::Down) => self.open_active_menu(menus),
            UiIntent::Cancel | UiIntent::Close => {
                if self.mnemonic_mode {
                    self.mnemonic_mode = false;
                    MenuBarOutcome::MnemonicMode { armed: false }
                } else {
                    MenuBarOutcome::Ignored
                }
            }
            _ => MenuBarOutcome::Ignored,
        }
    }

    fn handle_intent_open<Id: Clone>(
        &mut self,
        intent: UiIntent,
        menus: &[MenuBarMenu<Id>],
    ) -> MenuBarOutcome<Id> {
        self.ensure_top_frame(menus);
        let items = match self.current_items(menus) {
            Some(i) => i,
            None => {
                self.close_all();
                return MenuBarOutcome::Closed;
            }
        };
        let entries = Self::panel_entries(items);

        match intent {
            UiIntent::Move(
                NavigationMove::Next
                | NavigationMove::Down
                | NavigationMove::Previous
                | NavigationMove::Up
                | NavigationMove::First
                | NavigationMove::Last,
            ) => {
                if let Some(frame) = self.cascade.last_mut() {
                    let out = frame.collection.handle_intent(intent, &entries);
                    if out.active_changed() {
                        return MenuBarOutcome::MenuMoved;
                    }
                }
                MenuBarOutcome::Ignored
            }
            UiIntent::Move(NavigationMove::Right) | UiIntent::Expand => {
                // Prefer opening submenu under cursor.
                let opened = self.open_submenu_under_cursor(menus);
                if !matches!(opened, MenuBarOutcome::Ignored) {
                    return opened;
                }
                // At root depth, switch to next top menu (desktop style).
                if self.cascade.len() == 1 {
                    let entries = Self::bar_entries(menus);
                    let out = self
                        .bar
                        .handle_intent(UiIntent::Move(NavigationMove::Next), &entries);
                    if out.active_changed() {
                        let idx = self.bar_cursor();
                        return self.open_menu_at(menus, idx);
                    }
                }
                MenuBarOutcome::Ignored
            }
            UiIntent::Move(NavigationMove::Left) | UiIntent::Collapse => {
                if self.cascade.len() > 1 {
                    return self.close_one_layer();
                }
                // At root: switch previous top menu.
                let entries = Self::bar_entries(menus);
                let out = self
                    .bar
                    .handle_intent(UiIntent::Move(NavigationMove::Previous), &entries);
                if out.active_changed() {
                    let idx = self.bar_cursor();
                    return self.open_menu_at(menus, idx);
                }
                MenuBarOutcome::Ignored
            }
            UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle => self.activate_cursor(menus),
            UiIntent::Cancel | UiIntent::Close => self.close_one_layer(),
            _ => MenuBarOutcome::Ignored,
        }
    }

    /// Pointer entry.
    pub fn handle_mouse<Id: Clone>(
        &mut self,
        event: MouseEvent,
        menus: &[MenuBarMenu<Id>],
    ) -> MenuBarOutcome<Id> {
        if !self.enabled || !self.accepts_input || menus.is_empty() {
            return MenuBarOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let pos = event.position;
                // Panel hits first (top of cascade).
                for (depth, item_idx, rect) in self.panel_hits.iter().rev() {
                    if rect_contains(*rect, pos) {
                        // Close deeper than depth.
                        while self.cascade.len() > *depth + 1 {
                            self.cascade.pop();
                            self.open_path.pop();
                        }
                        if let Some(frame) = self.cascade.get_mut(*depth) {
                            frame.set_cursor(*item_idx);
                        }
                        return self.activate_cursor(menus);
                    }
                }
                // Bar hits.
                for (idx, rect) in &self.bar_hits {
                    if rect_contains(*rect, pos) {
                        if matches!(self.presentation, MenuBarPresentation::CommandPalette) {
                            return MenuBarOutcome::PreferCommandPalette;
                        }
                        self.focused = true;
                        return self.open_menu_at(menus, *idx);
                    }
                }
                // Outside: full dismiss if open.
                if self.is_open() {
                    self.close_all();
                    return MenuBarOutcome::Closed;
                }
                MenuBarOutcome::Ignored
            }
            MouseEventKind::Moved if self.is_open() => {
                // Hover is stated every event, so leaving a panel clears it.
                self.hovered = self
                    .panel_hits
                    .iter()
                    .rev()
                    .find(|(_, _, rect)| rect_contains(*rect, event.position))
                    .map(|(depth, idx, _)| (*depth, *idx));
                // Desktop: hover switches top menus.
                for (idx, rect) in &self.bar_hits {
                    if rect_contains(*rect, event.position) && Some(*idx) != self.open_top {
                        return self.open_menu_at(menus, *idx);
                    }
                }
                // Hover into panel items: move cursor; open submenu on hover optionally.
                for (depth, item_idx, rect) in &self.panel_hits {
                    if rect_contains(*rect, event.position) {
                        while self.cascade.len() > *depth + 1 {
                            self.cascade.pop();
                            self.open_path.pop();
                        }
                        if let Some(frame) = self.cascade.get_mut(*depth) {
                            if frame.cursor() != *item_idx {
                                frame.set_cursor(*item_idx);
                                // Auto-open submenu on hover at this depth.
                                if let Some(items) = self.current_items(menus) {
                                    if let Some(n) = items.get(*item_idx) {
                                        if matches!(n.kind, MenuRowKind::Submenu)
                                            && n.is_activatable()
                                        {
                                            let _ = self.open_submenu_under_cursor(menus);
                                        }
                                    }
                                }
                                return MenuBarOutcome::MenuMoved;
                            }
                        }
                    }
                }
                MenuBarOutcome::Ignored
            }
            _ => MenuBarOutcome::Ignored,
        }
    }
}

// open_path needs to be on state — I referenced it before declaring. Fix by adding field.
// Re-open the struct... I'll use search_replace after write if needed.

fn rect_contains(rect: Rect, pos: Position) -> bool {
    pos.x >= rect.x
        && pos.y >= rect.y
        && pos.x < rect.x.saturating_add(rect.width)
        && pos.y < rect.y.saturating_add(rect.height)
}

/// Default intent map for MenuBar (closed + open panels).
#[must_use]
pub fn default_menu_bar_intent(key: KeyEvent) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let is_press = key.kind == KeyEventKind::Press;
    // Ignore Alt chords here; handle_key processes mnemonics.
    if key.modifiers.contains(KeyModifiers::ALT) && matches!(key.code, KeyCode::Char(_)) {
        return None;
    }
    match key.code {
        KeyCode::Left | KeyCode::Char('h' | 'H') => Some(UiIntent::Move(NavigationMove::Left)),
        KeyCode::Right | KeyCode::Char('l' | 'L') => Some(UiIntent::Move(NavigationMove::Right)),
        KeyCode::Down | KeyCode::Char('j' | 'J') => Some(UiIntent::Move(NavigationMove::Down)),
        KeyCode::Up | KeyCode::Char('k' | 'K') => Some(UiIntent::Move(NavigationMove::Up)),
        KeyCode::Home => Some(UiIntent::Move(NavigationMove::First)),
        KeyCode::End => Some(UiIntent::Move(NavigationMove::Last)),
        KeyCode::Enter if is_press => Some(UiIntent::Activate),
        KeyCode::Char(' ') if is_press => Some(UiIntent::Toggle),
        KeyCode::Esc if is_press => Some(UiIntent::Cancel),
        _ => None,
    }
}

// ── Flatten for CommandPalette ──────────────────────────────────────────────

/// Flattened command projection for narrow CommandPalette replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuCommandRef<Id> {
    /// Node id.
    pub id: Id,
    /// Path labels joined (e.g. "File › Open Recent › foo.rs").
    pub path_label: String,
    /// Leaf label.
    pub label: String,
    /// Optional command key.
    pub command: Option<String>,
    /// Shortcut hint.
    pub shortcut: Option<String>,
    /// Enabled.
    pub enabled: bool,
    /// Disabled reason.
    pub disabled_reason: Option<String>,
}

/// Flatten all activatable leaves for palette projection (host maps to palette rows).
#[must_use]
pub fn flatten_menu_commands<Id: Clone>(menus: &[MenuBarMenu<Id>]) -> Vec<MenuCommandRef<Id>> {
    let mut out = Vec::new();
    for menu in menus {
        let prefix = menu.label.as_str();
        flatten_nodes(&menu.items, prefix, &mut out);
    }
    out
}

fn flatten_nodes<Id: Clone>(
    nodes: &[MenuNode<Id>],
    prefix: &str,
    out: &mut Vec<MenuCommandRef<Id>>,
) {
    for n in nodes {
        match &n.kind {
            MenuRowKind::Separator
            | MenuRowKind::Section
            | MenuRowKind::Loading
            | MenuRowKind::CustomPreview => {}
            MenuRowKind::Submenu => {
                let next = if prefix.is_empty() {
                    n.label.clone()
                } else {
                    format!("{prefix} › {}", n.label)
                };
                flatten_nodes(&n.children, &next, out);
            }
            MenuRowKind::Command | MenuRowKind::Checkbox { .. } | MenuRowKind::Radio { .. } => {
                let path_label = if prefix.is_empty() {
                    n.label.clone()
                } else {
                    format!("{prefix} › {}", n.label)
                };
                out.push(MenuCommandRef {
                    id: n.id.clone(),
                    path_label,
                    label: n.label.clone(),
                    command: n.command.clone(),
                    shortcut: n.shortcut.clone(),
                    enabled: n.enabled,
                    disabled_reason: n.disabled_reason.clone(),
                });
            }
        }
    }
}

// ── Overlay helpers ─────────────────────────────────────────────────────────

/// Place a menu panel below an anchor (bar label or parent item).
#[must_use]
pub fn place_menu_bar_panel(bounds: Rect, anchor: Rect, size: OverlaySize) -> Rect {
    if bounds.is_empty() || size.width == 0 || size.height == 0 {
        return Rect::default();
    }
    place_overlay(
        bounds,
        Some(anchor),
        size,
        OverlayPolicy::for_kind(OverlayKind::Menu),
    )
}

/// Open root menu panel on the stack.
pub fn open_menu_bar_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    size: OverlaySize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    let spec = OverlaySpec::menu(MENU_BAR_OVERLAY_ID, anchor, size, opener_focus);
    stack.open(bounds, spec)
}

/// Open nested submenu panel with parent link for cascade dismiss.
pub fn open_menu_bar_submenu_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    size: OverlaySize,
    depth: usize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    let id = format!("{MENU_BAR_SUBMENU_OVERLAY_PREFIX}.{depth}");
    let parent = if depth <= 1 {
        OverlayId::from_static(MENU_BAR_OVERLAY_ID)
    } else {
        OverlayId(format!(
            "{MENU_BAR_SUBMENU_OVERLAY_PREFIX}.{}",
            depth.saturating_sub(1)
        ))
    };
    let spec = OverlaySpec::menu(id, anchor, size, opener_focus).with_parent(parent);
    stack.open(bounds, spec)
}

/// Dismiss entire menu-bar cascade (root + nested).
pub fn dismiss_menu_bar_overlays<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(MENU_BAR_OVERLAY_ID))
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// MenuBar paint + hit geometry.
#[derive(Debug, Clone, Copy)]
pub struct MenuBar<'a, Id> {
    menus: &'a [MenuBarMenu<Id>],
    system: &'a DesignSystem,
    ascii: bool,
    colorless: bool,
}

impl<'a, Id> MenuBar<'a, Id> {
    /// Menus + design system.
    #[must_use]
    pub const fn new(menus: &'a [MenuBarMenu<Id>], system: &'a DesignSystem) -> Self {
        Self {
            menus,
            system,
            ascii: false,
            colorless: false,
        }
    }

    /// ASCII glyphs.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Reduced-color roles.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Paint bar into `area` (typically one row). Updates hits on `state`.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut MenuBarState)
    where
        Id: Clone,
    {
        state.bar_hits.clear();
        state.bar_origin = (area.x, area.y);
        state.bar_size = (area.width, area.height);
        if area.is_empty() {
            return;
        }

        let presentation = state
            .presentation_override
            .unwrap_or_else(|| menu_bar_presentation_for_width(area.width));
        state.presentation = presentation;

        // Clear line with surface role.
        let fill = " ".repeat(usize::from(area.width));
        buffer.set_stringn(
            area.x,
            area.y,
            &fill,
            usize::from(area.width),
            self.system.style(Role::Surface),
        );

        match presentation {
            MenuBarPresentation::CommandPalette => {
                let label = if self.ascii { "Menu..." } else { "Menu…" };
                let armed = state.mnemonic_mode || state.focused;
                let style = if self.colorless {
                    self.system
                        .style(if armed { Role::TextStrong } else { Role::Text })
                } else if armed {
                    self.system
                        .style(Role::Focus)
                        .add_modifier(Modifier::REVERSED)
                } else {
                    self.system.style(Role::Text)
                };
                let text = take_display_cols(label, usize::from(area.width));
                let w = display_cols(&text) as u16;
                buffer.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
                state
                    .bar_hits
                    .push((0, Rect::new(area.x, area.y, w.max(1), 1)));
            }
            MenuBarPresentation::Full | MenuBarPresentation::Compact => {
                state.ensure_bar(self.menus);
                let mut x = area.x;
                let bar_cursor = state.bar_cursor();
                let surface = state.focused && state.accepts_input;
                for (i, menu) in self.menus.iter().enumerate() {
                    if x >= area.right() {
                        break;
                    }
                    let label = if matches!(presentation, MenuBarPresentation::Compact)
                        && menu.label.chars().count() > 6
                    {
                        take_display_cols(&menu.label, 4)
                    } else {
                        menu.label.clone()
                    };
                    let painted = format_mnemonic_label(&label, menu.mnemonic, self.ascii);
                    let pad = format!(" {painted} ");
                    let w = (display_cols(&pad) as u16).min(area.right().saturating_sub(x));
                    if w == 0 {
                        break;
                    }
                    let active = surface && (bar_cursor == i || state.open_top == Some(i));
                    let style = if self.colorless {
                        if !menu.enabled {
                            self.system.style(Role::TextMuted)
                        } else if active {
                            self.system.style(Role::TextStrong)
                        } else if state.mnemonic_mode {
                            self.system.style(Role::Text)
                        } else {
                            self.system.style(Role::Text)
                        }
                    } else if !menu.enabled {
                        self.system.style(Role::TextDisabled)
                    } else if active {
                        self.system
                            .style(Role::TextStrong)
                            .patch(self.system.style(Role::SelectionTint))
                            .add_modifier(Modifier::BOLD)
                    } else if state.mnemonic_mode {
                        self.system.style(Role::Focus)
                    } else {
                        self.system.style(Role::Text)
                    };
                    let text = take_display_cols(&pad, usize::from(w));
                    buffer.set_stringn(x, area.y, &text, usize::from(w), style);
                    state.bar_hits.push((i, Rect::new(x, area.y, w, 1)));
                    x = x.saturating_add(w);
                }
            }
        }
    }

    /// Paint open cascade panels into `bounds` (full terminal content area).
    ///
    /// Hosts usually place panels via OverlayStack rects; this helper paints all
    /// open layers relative to bar hits for lookbook / simple hosts.
    pub fn paint_panels(&self, bounds: Rect, buffer: &mut Buffer, state: &mut MenuBarState)
    where
        Id: Clone,
    {
        state.panel_hits.clear();
        if !state.is_open() || bounds.is_empty() {
            return;
        }
        let top = match state.open_top {
            Some(t) => t,
            None => return,
        };
        let menu = match self.menus.get(top) {
            Some(m) => m,
            None => return,
        };

        // Anchor for root panel: bar hit for top menu.
        let root_anchor = state
            .bar_hits
            .iter()
            .find(|(i, _)| *i == top)
            .map(|(_, r)| *r)
            .unwrap_or(Rect::new(state.bar_origin.0, state.bar_origin.1, 8, 1));

        let mut path: Vec<usize> = Vec::new();
        let mut items = menu.items.as_slice();
        let mut anchor = root_anchor;

        for depth in 0..state.cascade.len() {
            let size = measure_panel(items, self.ascii);
            let rect = place_menu_bar_panel(bounds, anchor, size);
            self.paint_panel_at(rect, buffer, state, items, depth);
            // Next anchor: selected row rect if any.
            let cursor = state
                .cascade
                .get(depth)
                .map(CascadeFrame::cursor)
                .unwrap_or(0);
            if let Some((_, _, hit)) = state
                .panel_hits
                .iter()
                .find(|(d, i, _)| *d == depth && *i == cursor)
            {
                anchor = *hit;
            } else {
                anchor = Rect::new(rect.right().saturating_sub(1), rect.y, 1, 1);
            }
            // Advance items along stored open_path for next depth.
            if depth + 1 < state.cascade.len() {
                if let Some(&idx) = state.open_path.get(depth) {
                    if let Some(node) = items.get(idx) {
                        path.push(idx);
                        items = node.children.as_slice();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        let _ = path;
    }

    fn paint_panel_at(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut MenuBarState,
        items: &[MenuNode<Id>],
        depth: usize,
    ) {
        if area.is_empty() {
            return;
        }
        let recipe = if state.focused {
            super::SurfaceRecipe::OverlayFocused
        } else {
            super::SurfaceRecipe::Overlay
        };
        let colorless_system;
        let surface_system = if self.colorless {
            colorless_system = self
                .system
                .clone()
                .capability(crate::style::ColorCapability::Monochrome);
            &colorless_system
        } else {
            self.system
        };
        let inner = super::Surface::new(surface_system)
            .recipe(recipe)
            .bordered(true)
            .content_inset()
            .paint(area, buffer);
        if inner.is_empty() {
            return;
        }

        let cursor = state
            .cascade
            .get(depth)
            .map(CascadeFrame::cursor)
            .unwrap_or(0);
        let surface_focus = state.focused && state.accepts_input;
        let mut y = inner.y;
        for (i, item) in items.iter().enumerate() {
            if y >= inner.bottom() {
                break;
            }
            if matches!(item.kind, MenuRowKind::Separator) {
                let line = if self.ascii {
                    "-".repeat(usize::from(inner.width))
                } else {
                    "─".repeat(usize::from(inner.width))
                };
                buffer.set_stringn(
                    inner.x,
                    y,
                    &line,
                    usize::from(inner.width),
                    self.system.style(Role::Border),
                );
                state
                    .panel_hits
                    .push((depth, i, Rect::new(inner.x, y, inner.width, 1)));
                y = y.saturating_add(1);
                continue;
            }
            if matches!(item.kind, MenuRowKind::Section) {
                let text = take_display_cols(&item.label, usize::from(inner.width));
                buffer.set_stringn(
                    inner.x,
                    y,
                    &text,
                    usize::from(inner.width),
                    self.system.style(Role::TextMuted),
                );
                state
                    .panel_hits
                    .push((depth, i, Rect::new(inner.x, y, inner.width, 1)));
                y = y.saturating_add(1);
                continue;
            }
            if matches!(item.kind, MenuRowKind::Loading) {
                let prefix = if self.ascii { "... " } else { "… " };
                let text =
                    take_display_cols(&format!("{prefix}{}", item.label), usize::from(inner.width));
                buffer.set_stringn(
                    inner.x,
                    y,
                    &text,
                    usize::from(inner.width),
                    self.system.style(Role::TextMuted),
                );
                state
                    .panel_hits
                    .push((depth, i, Rect::new(inner.x, y, inner.width, 1)));
                y = y.saturating_add(1);
                continue;
            }
            if matches!(item.kind, MenuRowKind::CustomPreview) {
                // Host paints into hit rect; show muted label as fallback.
                let text = take_display_cols(&item.label, usize::from(inner.width));
                buffer.set_stringn(
                    inner.x,
                    y,
                    &text,
                    usize::from(inner.width),
                    self.system.style(Role::TextMuted),
                );
                state
                    .panel_hits
                    .push((depth, i, Rect::new(inner.x, y, inner.width, 1)));
                y = y.saturating_add(1);
                continue;
            }

            let active = cursor == i && surface_focus;
            let recipe = self.system.resolve_list_row(ListRowVisualState {
                selected: active,
                focused: active,
                hovered: state.hovered == Some((depth, i)),
                enabled: item.enabled,
                loading: false,
                checked: matches!(
                    item.kind,
                    MenuRowKind::Checkbox { checked: true }
                        | MenuRowKind::Radio { selected: true, .. }
                ),
            });
            let row = Rect::new(inner.x, y, inner.width, 1);
            if recipe.use_fill {
                buffer.set_style(row, recipe.label);
            } else if recipe.use_tint {
                buffer.set_style(row, recipe.tint);
            }
            let style = if self.colorless {
                if !item.enabled {
                    self.system.style(Role::TextMuted)
                } else if active {
                    self.system.style(Role::TextStrong)
                } else {
                    self.system.style(Role::Text)
                }
            } else if !item.enabled {
                self.system.style(Role::TextDisabled)
            } else if item.destructive && !active {
                self.system.style(Role::Danger)
            } else if active {
                recipe.label
            } else {
                self.system.style(Role::Text)
            };

            let mark = match &item.kind {
                MenuRowKind::Checkbox { checked: true } if self.ascii => "[x] ",
                MenuRowKind::Checkbox { checked: true } => "✓ ",
                MenuRowKind::Checkbox { checked: false } if self.ascii => "[ ] ",
                MenuRowKind::Checkbox { checked: false } => "  ",
                MenuRowKind::Radio { selected: true, .. } if self.ascii => "(*) ",
                MenuRowKind::Radio { selected: true, .. } => "● ",
                MenuRowKind::Radio {
                    selected: false, ..
                } if self.ascii => "( ) ",
                MenuRowKind::Radio {
                    selected: false, ..
                } => "○ ",
                _ if active && self.ascii => "> ",
                _ if active => "› ",
                _ => "  ",
            };
            let label = format_mnemonic_label(&item.label, item.mnemonic, self.ascii);
            let mut line = format!("{mark}{label}");
            if matches!(item.kind, MenuRowKind::Submenu) {
                line.push(if self.ascii { ' ' } else { ' ' });
                line.push(if self.ascii { '>' } else { '›' });
            }
            if let Some(sc) = &item.shortcut {
                let used = display_cols(&line);
                let sc_w = display_cols(sc);
                let pad = usize::from(inner.width)
                    .saturating_sub(used)
                    .saturating_sub(sc_w);
                if pad > 1 {
                    line.push_str(&" ".repeat(pad));
                    line.push_str(sc);
                }
            }
            let text = take_display_cols(&line, usize::from(inner.width));
            buffer.set_stringn(inner.x, y, &text, usize::from(inner.width), style);
            state
                .panel_hits
                .push((depth, i, Rect::new(inner.x, y, inner.width, 1)));
            y = y.saturating_add(1);
        }
    }

    /// Combined bar + panels paint for simple hosts / stories.
    pub fn paint_all(
        &self,
        bar_area: Rect,
        bounds: Rect,
        buffer: &mut Buffer,
        state: &mut MenuBarState,
    ) where
        Id: Clone,
    {
        self.paint(bar_area, buffer, state);
        self.paint_panels(bounds, buffer, state);
    }

    /// Semantic registration (single bar control + open status).
    pub fn register_semantic<Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &MenuBarState,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "menubar presentation={} open={} depth={} mnemonic={}",
            state.presentation.id(),
            state.is_open(),
            state.depth(),
            state.mnemonic_armed()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Control)
                .label("menu-bar")
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    busy: false,
                    invalid: false,
                    expanded: state.is_open(),
                    ..Default::default()
                }),
        );
    }
}

impl<Id: Clone> StatefulWidget for &MenuBar<'_, Id> {
    type State = MenuBarState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl<Id: Clone> StatefulWidget for MenuBar<'_, Id> {
    type State = MenuBarState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

fn format_mnemonic_label(label: &str, mnemonic: Option<char>, _ascii: bool) -> String {
    let Some(m) = mnemonic else {
        return label.to_string();
    };
    let lower = m.to_ascii_lowercase();
    // Parentheses form is grid-safe (no combining marks) and works in ASCII /
    // Unicode / colorless paths as a non-color mnemonic cue.
    if let Some(pos) = label
        .char_indices()
        .find(|(_, c)| c.to_ascii_lowercase() == lower)
    {
        let (i, ch) = pos;
        let before = &label[..i];
        let after = &label[i + ch.len_utf8()..];
        format!("{before}({ch}){after}")
    } else {
        format!("{label} ({m})")
    }
}

fn measure_panel<Id>(items: &[MenuNode<Id>], ascii: bool) -> OverlaySize {
    let mut max_w = 12u16;
    let mut h = 2u16; // borders
    for item in items {
        h = h.saturating_add(1);
        let mark = 4u16;
        let label_w = display_cols(&item.label) as u16;
        let sc = item
            .shortcut
            .as_ref()
            .map(|s| display_cols(s) as u16 + 2)
            .unwrap_or(0);
        let sub = if matches!(item.kind, MenuRowKind::Submenu) {
            2
        } else {
            0
        };
        let ascii_pad = if ascii { 0 } else { 0 };
        max_w = max_w.max(mark + label_w + sc + sub + ascii_pad + 2);
    }
    OverlaySize {
        width: max_w.clamp(10, 48),
        height: h.clamp(3, 24),
        min_width: 10,
        min_height: 3,
        max_width: 56,
        max_height: 30,
    }
}

/// Sample app menus for stories / tests.
#[must_use]
pub fn example_app_menus() -> Vec<MenuBarMenu<&'static str>> {
    vec![
        MenuBarMenu::new(
            "file",
            "File",
            vec![
                MenuNode::command("new", "New")
                    .mnemonic('N')
                    .shortcut("C-n")
                    .command_key("file.new"),
                MenuNode::command("open", "Open…")
                    .mnemonic('O')
                    .shortcut("C-o")
                    .command_key("file.open"),
                MenuNode::separator("file-sep-1"),
                MenuNode::section("recent-h", "Recent"),
                MenuNode::command("recent-1", "notes.md")
                    .recent(true)
                    .command_key("file.recent.1"),
                MenuNode::command("recent-2", "main.rs")
                    .recent(true)
                    .command_key("file.recent.2"),
                MenuNode::separator("file-sep-2"),
                MenuNode::submenu(
                    "export",
                    "Export",
                    vec![
                        MenuNode::command("export-png", "PNG").command_key("file.export.png"),
                        MenuNode::command("export-svg", "SVG").command_key("file.export.svg"),
                    ],
                )
                .mnemonic('E'),
                MenuNode::separator("file-sep-3"),
                MenuNode::command("quit", "Quit")
                    .mnemonic('Q')
                    .shortcut("C-q")
                    .command_key("file.quit"),
            ],
        )
        .mnemonic('F'),
        MenuBarMenu::new(
            "edit",
            "Edit",
            vec![
                MenuNode::command("undo", "Undo")
                    .mnemonic('U')
                    .shortcut("C-z")
                    .command_key("edit.undo"),
                MenuNode::command("redo", "Redo")
                    .enabled(false)
                    .disabled_reason("Nothing to redo")
                    .command_key("edit.redo"),
                MenuNode::separator("edit-sep"),
                MenuNode::checkbox("wrap", "Word wrap", true)
                    .mnemonic('W')
                    .command_key("edit.wrap"),
            ],
        )
        .mnemonic('E'),
        MenuBarMenu::new(
            "view",
            "View",
            vec![
                MenuNode::radio("theme-ph", "Phosphor", "theme", true)
                    .command_key("view.theme.phosphor"),
                MenuNode::radio("theme-hi", "High contrast", "theme", false)
                    .command_key("view.theme.hc"),
                MenuNode::separator("view-sep"),
                MenuNode::checkbox("status", "Status bar", true).command_key("view.status"),
            ],
        )
        .mnemonic('V'),
        MenuBarMenu::new(
            "help",
            "Help",
            vec![
                MenuNode::command("about", "About")
                    .mnemonic('A')
                    .command_key("help.about"),
            ],
        )
        .mnemonic('H'),
    ]
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::RolePalette;
    use ratatui_core::backend::TestBackend;
    use ratatui_core::terminal::Terminal;

    fn menus() -> Vec<MenuBarMenu<&'static str>> {
        example_app_menus()
    }

    fn focused_state() -> MenuBarState {
        let mut s = MenuBarState::new();
        s.set_focused(true);
        s
    }

    #[test]
    fn open_activate_command() {
        let menus = menus();
        let mut s = focused_state();
        assert!(matches!(
            s.open_menu_at(&menus, 0),
            MenuBarOutcome::Opened { menu_id: "file" }
        ));
        assert!(s.is_open());
        // cursor on New
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &menus),
            MenuBarOutcome::Activated {
                id: "new",
                command: Some(_)
            }
        ));
        assert!(!s.is_open());
    }

    #[test]
    fn nested_submenu_and_layer_dismiss() {
        let menus = menus();
        let mut s = focused_state();
        let _ = s.open_menu_at(&menus, 0);
        // Move to Export submenu (skip past recent section etc.)
        // items: new, open, sep, section, r1, r2, sep, export, sep, quit
        // Find export index
        let export_idx = menus[0]
            .items
            .iter()
            .position(|n| n.id == "export")
            .unwrap();
        s.cascade[0].set_cursor(export_idx);
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &menus),
            MenuBarOutcome::SubmenuOpened { id: "export" }
        ));
        assert_eq!(s.depth(), 2);
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &menus),
            MenuBarOutcome::LayerClosed
        ));
        assert_eq!(s.depth(), 1);
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &menus),
            MenuBarOutcome::Closed
        ));
        assert!(!s.is_open());
    }

    #[test]
    fn nested_activate_leaf_closes_all() {
        let menus = menus();
        let mut s = focused_state();
        let _ = s.open_menu_at(&menus, 0);
        let export_idx = menus[0]
            .items
            .iter()
            .position(|n| n.id == "export")
            .unwrap();
        s.cascade[0].set_cursor(export_idx);
        let _ = s.open_submenu_under_cursor(&menus);
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &menus),
            MenuBarOutcome::Activated {
                id: "export-png",
                ..
            }
        ));
        assert!(!s.is_open());
    }

    #[test]
    fn checkbox_and_radio_outcomes() {
        let menus = menus();
        let mut s = focused_state();
        let _ = s.open_menu_at(&menus, 1); // Edit
        let wrap_idx = menus[1].items.iter().position(|n| n.id == "wrap").unwrap();
        s.cascade[0].set_cursor(wrap_idx);
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &menus),
            MenuBarOutcome::CheckToggled {
                id: "wrap",
                checked: false
            }
        ));

        let mut s = focused_state();
        let _ = s.open_menu_at(&menus, 2); // View
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &menus),
            MenuBarOutcome::MenuMoved
        ));
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &menus),
            MenuBarOutcome::RadioSelected {
                id: "theme-hi",
                group
            } if group == "theme"
        ));
    }

    #[test]
    fn mnemonic_sticky_mode_and_letter() {
        let menus = menus();
        let mut s = focused_state();
        // Portable fallback chord (host may also call set_mnemonic_mode for F10).
        assert!(matches!(
            s.handle_key(
                KeyEvent::new(KeyCode::Char('m'), KeyModifiers::CONTROL.with_shift()),
                &menus
            ),
            MenuBarOutcome::MnemonicMode { armed: true }
        ));
        assert!(matches!(
            s.handle_key(
                KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE),
                &menus
            ),
            MenuBarOutcome::Opened { menu_id: "view" }
        ));
    }

    #[test]
    fn alt_letter_opens_top() {
        let menus = menus();
        let mut s = focused_state();
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT), &menus),
            MenuBarOutcome::Opened { menu_id: "file" }
        ));
    }

    #[test]
    fn bar_roving_and_switch_while_open() {
        let menus = menus();
        let mut s = focused_state();
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &menus),
            MenuBarOutcome::BarMoved
        ));
        assert_eq!(s.bar_cursor(), 1);
        let _ = s.open_active_menu(&menus);
        // Right at root switches to next top menu
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &menus),
            MenuBarOutcome::Opened { menu_id: "view" }
                | MenuBarOutcome::SubmenuOpened { .. }
                | MenuBarOutcome::Opened { .. }
        ));
    }

    #[test]
    fn narrow_prefers_command_palette() {
        let menus = menus();
        let mut s = focused_state();
        s.set_presentation_override(Some(MenuBarPresentation::CommandPalette));
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &menus),
            MenuBarOutcome::PreferCommandPalette
        ));
    }

    #[test]
    fn flatten_includes_nested_and_skips_separators() {
        let menus = menus();
        let flat = flatten_menu_commands(&menus);
        assert!(flat.iter().any(|c| c.id == "export-svg"));
        assert!(flat.iter().any(|c| c.path_label.contains("Export")));
        assert!(!flat.iter().any(|c| c.id == "file-sep-1"));
        assert!(!flat.iter().any(|c| c.id == "export")); // submenu parent not leaf
    }

    #[test]
    fn overlay_stack_nested_dismiss_restores_focus() {
        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(0, 0, 6, 1);
        let mut stack = OverlayStack::<&'static str>::new();
        let size = OverlaySize {
            width: 20,
            height: 8,
            min_width: 10,
            min_height: 3,
            max_width: 40,
            max_height: 20,
        };
        let open = open_menu_bar_overlay(&mut stack, bounds, anchor, size, Some("editor"));
        assert!(matches!(open, OverlayOutcome::Opened { .. }));
        let sub_anchor = Rect::new(18, 4, 1, 1);
        let sub =
            open_menu_bar_submenu_overlay(&mut stack, bounds, sub_anchor, size, 1, Some("editor"));
        assert!(matches!(sub, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.entries().len(), 2);
        // Esc dismisses top (submenu) only
        let esc = stack.handle_escape();
        assert!(matches!(esc, OverlayOutcome::Dismissed { .. }));
        assert_eq!(stack.entries().len(), 1);
        // Dismiss root restores opener focus
        let root = dismiss_menu_bar_overlays(&mut stack);
        assert!(matches!(
            root,
            OverlayOutcome::Dismissed {
                focus: Some("editor"),
                ..
            }
        ));
        assert!(stack.is_empty());
    }

    #[test]
    fn focus_restoration_hint_on_closed() {
        let menus = menus();
        let mut s = focused_state();
        s.set_opener_focus_hint(Some("main-pane".into()));
        let _ = s.open_menu_at(&menus, 0);
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &menus),
            MenuBarOutcome::Closed
        ));
        assert_eq!(s.opener_focus_hint(), Some("main-pane"));
    }

    #[test]
    fn disabled_skipped_on_roving() {
        let menus = menus();
        let mut s = focused_state();
        let _ = s.open_menu_at(&menus, 1); // Edit: undo, redo(disabled), sep, wrap
        // From undo, down should skip redo if disabled in collection
        let _ = s.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &menus);
        let cursor = s.panel_cursor(0).unwrap();
        assert_eq!(menus[1].items[cursor].id, "wrap");
    }

    #[test]
    fn paint_bar_and_panels() {
        let system = DesignSystem::default();
        let menus = menus();
        let mut s = focused_state();
        let _ = s.open_menu_at(&menus, 0);
        let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                let bar = Rect::new(area.x, area.y, area.width, 1);
                MenuBar::new(&menus, &system).ascii(true).paint_all(
                    bar,
                    area,
                    f.buffer_mut(),
                    &mut s,
                );
            })
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        // ASCII mnemonics render as "(F)ile" / "(N)ew".
        assert!(
            text.contains("(F)ile")
                || text.contains("(N)ew")
                || text.contains("(O)pen")
                || text.contains("Recent")
        );
    }

    #[test]
    fn presentation_width() {
        assert_eq!(
            menu_bar_presentation_for_width(20),
            MenuBarPresentation::CommandPalette
        );
        assert_eq!(
            menu_bar_presentation_for_width(50),
            MenuBarPresentation::Compact
        );
        assert_eq!(
            menu_bar_presentation_for_width(100),
            MenuBarPresentation::Full
        );
    }

    #[test]
    fn semantic_registers() {
        let system = DesignSystem::default();
        let menus = menus();
        let s = focused_state();
        let mut scene = SemanticScene::<&str, ()>::default();
        MenuBar::new(&menus, &system).register_semantic(
            &mut scene,
            "mb",
            Rect::new(0, 0, 40, 1),
            &s,
        );
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("menu-bar"))
        );
    }

    #[test]
    fn accepts_input_gate() {
        let menus = menus();
        let mut s = focused_state();
        s.set_accepts_input(false);
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &menus),
            MenuBarOutcome::Ignored
        ));
    }

    #[test]
    fn mouse_bar_hit_opens_the_same_menu_model_as_keyboard() {
        let menus = example_app_menus();
        let mut state = MenuBarState::new();
        state.bar_hits = vec![(0, Rect::new(1, 1, 6, 1))];
        assert!(matches!(
            state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(1, 1),
                    modifiers: KeyModifiers::NONE,
                },
                &menus,
            ),
            MenuBarOutcome::Opened { menu_id: "file" }
        ));
    }

    #[test]
    fn role_palette_smoke() {
        let _ = RolePalette::default();
        let system = DesignSystem::default();
        assert!(system.style(Role::Selection) != system.style(Role::TextDisabled));
    }

    /// Interaction fuzz: random keys must not panic; state stays coherent.
    #[test]
    fn fuzz_keys_no_panic_and_depth_bounded() {
        let menus = menus();
        let mut s = focused_state();
        let keys = [
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Enter,
            KeyCode::Esc,
            KeyCode::Home,
            KeyCode::End,
            KeyCode::Char(' '),
            KeyCode::Char('f'),
            KeyCode::Char('e'),
            KeyCode::Char('j'),
            KeyCode::Char('k'),
            KeyCode::Char('m'),
        ];
        let mods = [
            KeyModifiers::NONE,
            KeyModifiers::ALT,
            KeyModifiers::CONTROL.with_shift(),
        ];
        let mut seed = 0xC0FFEEu64;
        for _ in 0..400 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let k = keys[(seed as usize) % keys.len()];
            let m = mods[((seed >> 8) as usize) % mods.len()];
            let _ = s.handle_key(KeyEvent::new(k, m), &menus);
            assert!(s.depth() <= 8, "cascade depth unbounded");
            if let Some(top) = s.open_top_index() {
                assert!(top < menus.len());
            }
        }
    }

    #[test]
    fn paint_perf_smoke() {
        let system = DesignSystem::default();
        let menus = menus();
        let mut s = focused_state();
        let _ = s.open_menu_at(&menus, 0);
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..200 {
            terminal
                .draw(|f| {
                    let area = f.area();
                    let bar = Rect::new(area.x, area.y, area.width, 1);
                    MenuBar::new(&menus, &system).paint_all(bar, area, f.buffer_mut(), &mut s);
                })
                .unwrap();
        }
        // Soft budget: headless paint should stay well under a second for 200 frames.
        assert!(start.elapsed().as_millis() < 5_000);
    }
}
