// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Persistent demos for the public catalog's compact interactive primitives.

use ratatui::{Frame, layout::Rect, widgets::StatefulWidget};
use termrock::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent},
    style::{Role, RolePalette},
    widgets::{
        Action, ActivationOutcome, Alert, AlertOutcome, AlertState, AlertTone, Badge, BadgeOutcome,
        BadgeState, BreadcrumbHit, BreadcrumbItem, BreadcrumbSeparator, Breadcrumbs,
        BreadcrumbsMode, BreadcrumbsOutcome, BreadcrumbsState, ButtonGroup, ButtonGroupItem,
        ButtonGroupOutcome, ButtonGroupState, Chip, ChipOutcome, ChipState, IconButton,
        IconButtonState, Link, LinkOutcome, LinkState, NavItem, NavigationList,
        NavigationListOutcome, NavigationListState, RadioGroup, RadioOption, RadioOutcome,
        RadioState, Section, SectionOutcome, SectionState, Tag, TagOutcome, TagState, Toolbar,
        ToolbarItem, ToolbarOutcome, ToolbarState,
    },
};

use super::{StoryInteraction, extended::record};

pub(crate) struct AlertInteractor {
    state: AlertState<&'static str>,
    theme: RolePalette,
    outcome: Option<String>,
}

impl AlertInteractor {
    pub(crate) fn new() -> Self {
        let mut state = AlertState::new();
        state.set_focused(true);
        state.set_action_cursor(Some("retry"));
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }

    fn actions() -> [Action<'static, &'static str>; 2] {
        [
            Action {
                id: "retry",
                label: "Retry",
                enabled: true,
                style: None,
            },
            Action {
                id: "logs",
                label: "View logs",
                enabled: true,
                style: None,
            },
        ]
    }

    fn apply(&mut self, outcome: AlertOutcome<&'static str>) -> bool {
        record(&mut self.outcome, "Alert", outcome)
    }
}

impl StoryInteraction for AlertInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let actions = Self::actions();
        if !self.state.is_visible() {
            frame.buffer_mut().set_stringn(
                area.x,
                area.y,
                "Alert dismissed · press O to show it again",
                usize::from(area.width),
                system.style(Role::TextMuted),
            );
            return;
        }
        Alert::new("Deploy failed", &system)
            .tone(AlertTone::Danger)
            .body("Rollout aborted at step 3.")
            .details("timeout waiting for health check")
            .source("pipeline #42")
            .actions(&actions)
            .paint(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !self.state.is_visible()
            && key.kind == KeyEventKind::Press
            && key.modifiers == KeyModifiers::NONE
            && matches!(key.code, KeyCode::Char('o' | 'O'))
        {
            self.state.show();
            self.state.set_focused(true);
            self.state.set_action_cursor(Some("retry"));
            self.outcome = Some("Alert: Shown".into());
            return true;
        }
        let actions = Self::actions();
        let outcome = self.state.handle_key_with(key, &actions, true);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let actions = Self::actions();
        let outcome = self.state.handle_mouse(mouse, &actions, true);
        self.apply(outcome)
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        if !self.state.is_visible() {
            return vec!["O show alert"];
        }
        vec![
            "←→ choose action",
            "Enter activate",
            "D toggle details",
            "Esc dismiss",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct BadgeInteractor {
    state: BadgeState,
    theme: RolePalette,
    outcome: Option<String>,
}

impl BadgeInteractor {
    pub(crate) fn new() -> Self {
        let mut state = BadgeState::new();
        state.set_focused(true);
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: BadgeOutcome) -> bool {
        record(&mut self.outcome, "Badge", outcome)
    }
}

impl StoryInteraction for BadgeInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let _ = Badge::new("interactive", &system).interactive(true).paint(
            area,
            frame.buffer_mut(),
            Some(&mut self.state),
        );
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let outcome = Badge::new("interactive", &system)
            .interactive(true)
            .handle_key(&mut self.state, key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let before = self.state;
        let outcome = Badge::new("interactive", &system)
            .interactive(true)
            .handle_mouse(&mut self.state, mouse);
        self.apply(outcome) || self.state != before
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec!["hover", "click toggle", "Enter toggle"]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct BreadcrumbsInteractor {
    state: BreadcrumbsState,
    hits: Vec<(BreadcrumbHit<&'static str>, Rect)>,
    theme: RolePalette,
    outcome: Option<String>,
}

impl BreadcrumbsInteractor {
    pub(crate) fn new() -> Self {
        let mut state = BreadcrumbsState::new()
            .with_editable(true)
            .with_separator(BreadcrumbSeparator::Slash);
        state.set_focused(true);
        state.set_focus_index(2);
        Self {
            state,
            hits: Vec::new(),
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn items() -> [BreadcrumbItem<&'static str>; 5] {
        [
            BreadcrumbItem::new("home", "home"),
            BreadcrumbItem::new("projects", "projects"),
            BreadcrumbItem::new("termrock", "termrock"),
            BreadcrumbItem::new("src", "src"),
            BreadcrumbItem::new("widgets", "widgets").current(true),
        ]
    }
    fn apply(&mut self, outcome: BreadcrumbsOutcome<&'static str>) -> bool {
        record(&mut self.outcome, "Breadcrumbs", outcome)
    }
}

impl StoryInteraction for BreadcrumbsInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let items = Self::items();
        self.hits = Breadcrumbs::new(&items, &system)
            .separator(BreadcrumbSeparator::Slash)
            .paint(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key, &Self::items());
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse, &Self::items(), &self.hits);
        self.apply(outcome)
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "←→ move",
            "Enter navigate/edit",
            "Ctrl+E edit path",
            "click segment",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        matches!(self.state.mode(), BreadcrumbsMode::Editable)
    }
}

pub(crate) struct ButtonGroupInteractor {
    state: ButtonGroupState<&'static str>,
    theme: RolePalette,
    outcome: Option<String>,
}

impl ButtonGroupInteractor {
    pub(crate) fn new() -> Self {
        let mut state = ButtonGroupState::new();
        state.set_surface_focused(true);
        state.cursor = Some("save");
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn items() -> [ButtonGroupItem<'static, &'static str>; 3] {
        [
            ButtonGroupItem::new("cancel", "Cancel"),
            ButtonGroupItem::destructive("delete", "Delete"),
            ButtonGroupItem::primary("save", "Save"),
        ]
    }
    fn apply(&mut self, outcome: ButtonGroupOutcome<&'static str>) -> bool {
        record(&mut self.outcome, "ButtonGroup", outcome)
    }
}

impl StoryInteraction for ButtonGroupInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let items = Self::items();
        let _ = ButtonGroup::new(&items, &system).paint(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let items = Self::items();
        let outcome = ButtonGroup::new(&items, &system).handle_key(&mut self.state, key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let items = Self::items();
        let before = self.state.clone();
        let outcome = ButtonGroup::new(&items, &system).handle_mouse(&mut self.state, mouse);
        self.apply(outcome) || self.state != before
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "←→ choose",
            "Space activate",
            "Enter submit Save",
            "click action",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct ChipInteractor {
    state: ChipState,
    visible: bool,
    theme: RolePalette,
    outcome: Option<String>,
}

impl ChipInteractor {
    pub(crate) fn new() -> Self {
        let mut state = ChipState::new(true);
        state.set_focused(true);
        Self {
            state,
            visible: true,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: ChipOutcome<&'static str>) -> bool {
        record(&mut self.outcome, "Chip", outcome)
    }
}

impl StoryInteraction for ChipInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        if !self.visible {
            frame.buffer_mut().set_stringn(
                area.x,
                area.y,
                "Chip removed · press R to restore",
                usize::from(area.width),
                system.style(Role::TextMuted),
            );
            return;
        }
        let _ = Chip::new("rust", "rust", &system).removable(true).paint(
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !self.visible {
            if key.kind == KeyEventKind::Press
                && key.modifiers == KeyModifiers::NONE
                && matches!(key.code, KeyCode::Char('r' | 'R'))
            {
                self.state = ChipState::new(true);
                self.state.set_focused(true);
                self.visible = true;
                self.outcome = Some("Chip: Restored".into());
                return true;
            }
            return false;
        }
        let system = crate::design::lookbook_system(self.theme.clone());
        let outcome = Chip::new("rust", "rust", &system)
            .removable(true)
            .handle_key(&mut self.state, key);
        if matches!(outcome, ChipOutcome::Remove(_)) {
            self.visible = false;
        }
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let outcome = Chip::new("rust", "rust", &system)
            .removable(true)
            .handle_mouse(&mut self.state, mouse);
        if matches!(outcome, ChipOutcome::Remove(_)) {
            self.visible = false;
        }
        self.apply(outcome)
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        if !self.visible {
            return vec!["R restore chip"];
        }
        vec![
            "Enter/Space toggle",
            "←→ choose remove",
            "Delete remove",
            "click",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct TagInteractor {
    state: TagState,
    visible: bool,
    theme: RolePalette,
    outcome: Option<String>,
}

impl TagInteractor {
    pub(crate) fn new() -> Self {
        let mut state = TagState::new();
        state.set_focused(true);
        Self {
            state,
            visible: true,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: TagOutcome<&'static str>) -> bool {
        record(&mut self.outcome, "Tag", outcome)
    }
}

impl StoryInteraction for TagInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        if !self.visible {
            frame.buffer_mut().set_stringn(
                area.x,
                area.y,
                "Tag removed · press R to restore",
                usize::from(area.width),
                system.style(Role::TextMuted),
            );
            return;
        }
        let _ = Tag::removable_tag("attachment", "paste-body.txt", &system).paint(
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !self.visible {
            if key.kind == KeyEventKind::Press
                && key.modifiers == KeyModifiers::NONE
                && matches!(key.code, KeyCode::Char('r' | 'R'))
            {
                self.state = TagState::new();
                self.state.set_focused(true);
                self.visible = true;
                self.outcome = Some("Tag: Restored".into());
                return true;
            }
            return false;
        }
        let system = crate::design::lookbook_system(self.theme.clone());
        let outcome = Tag::removable_tag("attachment", "paste-body.txt", &system)
            .handle_key(&mut self.state, key);
        if matches!(outcome, TagOutcome::Remove(_)) {
            self.visible = false;
        }
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let outcome = Tag::removable_tag("attachment", "paste-body.txt", &system)
            .handle_mouse(&mut self.state, mouse);
        if matches!(outcome, TagOutcome::Remove(_)) {
            self.visible = false;
        }
        self.apply(outcome)
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        if !self.visible {
            return vec!["R restore tag"];
        }
        vec![
            "←→ choose remove",
            "Enter activate",
            "Delete remove",
            "click",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct IconButtonInteractor {
    state: IconButtonState,
    theme: RolePalette,
    outcome: Option<String>,
}

impl IconButtonInteractor {
    pub(crate) fn new() -> Self {
        let mut state = IconButtonState::new();
        state.activation.set_accepts_input(true);
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
}

impl StoryInteraction for IconButtonInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let _ = IconButton::new("★", "Favorite", &system)
            .ascii_glyph("*")
            .toggle(true)
            .paint(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        if matches!(outcome, ActivationOutcome::Activated) {
            self.state.set_pressed(!self.state.pressed);
        }
        record(&mut self.outcome, "IconButton", outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let before = self.state;
        let outcome = self.state.handle_mouse(mouse);
        if matches!(outcome, ActivationOutcome::Activated) {
            self.state.set_pressed(!self.state.pressed);
        }
        record(&mut self.outcome, "IconButton", outcome) || self.state != before
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec!["hover", "click toggle", "Enter/Space toggle"]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct RadioGroupInteractor {
    state: RadioState<&'static str>,
    theme: RolePalette,
    outcome: Option<String>,
}

impl RadioGroupInteractor {
    pub(crate) fn new() -> Self {
        let mut state = RadioState::new(Some("plan"));
        state.set_surface_focused(true);
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn options() -> [RadioOption<'static, &'static str>; 3] {
        [
            RadioOption::new("plan", "Plan").description("Inspect before changing files"),
            RadioOption::new("build", "Build").description("Implement and verify changes"),
            RadioOption::new("ask", "Ask").description("Request missing direction"),
        ]
    }
    fn apply(&mut self, outcome: RadioOutcome<&'static str>) -> bool {
        record(&mut self.outcome, "RadioGroup", outcome)
    }
}

impl StoryInteraction for RadioGroupInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let options = Self::options();
        let _ = RadioGroup::new(&options, &system)
            .legend("Agent mode")
            .paint(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let options = Self::options();
        let outcome = RadioGroup::new(&options, &system).handle_key(&mut self.state, key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let options = Self::options();
        let before = self.state.clone();
        let outcome = RadioGroup::new(&options, &system).handle_mouse(&mut self.state, mouse);
        self.apply(outcome) || self.state != before
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ choose",
            "typeahead",
            "Enter/Space select",
            "click option",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct SectionInteractor {
    state: SectionState,
    theme: RolePalette,
    outcome: Option<String>,
}

impl SectionInteractor {
    pub(crate) fn new() -> Self {
        let mut state = SectionState::new();
        state.set_focused(true);
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: SectionOutcome) -> bool {
        record(&mut self.outcome, "Section", outcome)
    }
}

impl StoryInteraction for SectionInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let body = Section::new("Network", &system)
            .description("Proxy and connection policy")
            .status("live")
            .collapsible(true)
            .emphasized()
            .paint(area, frame.buffer_mut(), Some(&mut self.state));
        if !self.state.is_collapsed() && !body.is_empty() {
            frame.buffer_mut().set_stringn(
                body.x,
                body.y,
                "proxy: system",
                usize::from(body.width),
                system.style(Role::Text),
            );
        }
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key, true);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse, true);
        self.apply(outcome)
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "Enter/Space toggle",
            "← collapse",
            "→ expand",
            "click header",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct ToolbarInteractor {
    state: ToolbarState<&'static str>,
    wrap_pressed: bool,
    area: Rect,
    theme: RolePalette,
    outcome: Option<String>,
}

impl ToolbarInteractor {
    pub(crate) fn new() -> Self {
        let mut state = ToolbarState::new();
        state.set_surface_focused(true);
        state.set_cursor(Some("save"));
        Self {
            state,
            wrap_pressed: false,
            area: Rect::default(),
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn items(&self) -> [ToolbarItem<'static, &'static str>; 5] {
        [
            ToolbarItem::action("save", "Save").icon("S").hint("Ctrl+S"),
            ToolbarItem::action("open", "Open").icon("O"),
            ToolbarItem::separator("separator"),
            ToolbarItem::toggle("wrap", "Wrap", self.wrap_pressed).icon("W"),
            ToolbarItem::action("find", "Find").icon("F").hint("Ctrl+F"),
        ]
    }
    fn apply(&mut self, outcome: ToolbarOutcome<&'static str>) -> bool {
        if let ToolbarOutcome::Toggled {
            id: "wrap",
            pressed,
        } = outcome
        {
            self.wrap_pressed = pressed;
        }
        record(&mut self.outcome, "Toolbar", outcome)
    }
}

impl StoryInteraction for ToolbarInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        self.area = area;
        let system = crate::design::lookbook_system(self.theme.clone());
        let items = self.items();
        Toolbar::new(&items, &system).overflow_id("more").render(
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let items = self.items();
        let outcome = Toolbar::new(&items, &system)
            .overflow_id("more")
            .handle_key(&mut self.state, key, self.area);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let items = self.items();
        let outcome = Toolbar::new(&items, &system)
            .overflow_id("more")
            .handle_mouse(&mut self.state, mouse);
        self.apply(outcome)
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "←→ choose",
            "Enter/Space activate",
            "click action",
            "resize for overflow",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct NavigationListInteractor {
    state: NavigationListState<&'static str>,
    expanded: bool,
    theme: RolePalette,
    outcome: Option<String>,
}

impl NavigationListInteractor {
    pub(crate) fn new() -> Self {
        let mut state = NavigationListState::new(Some("inbox"));
        state.set_focused(true);
        state.set_route_and_focus("inbox");
        Self {
            state,
            expanded: true,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn items(&self) -> Vec<NavItem<&'static str>> {
        vec![
            NavItem::group("mail", "Mail").expanded(self.expanded),
            NavItem::new("inbox", "Inbox").depth(1).badge("12"),
            NavItem::new("starred", "Starred").depth(1),
            NavItem::new("archive", "Archive").depth(1),
        ]
    }
    fn apply(&mut self, outcome: NavigationListOutcome<&'static str>) -> bool {
        if let NavigationListOutcome::ExpandToggled {
            id: "mail",
            expanded,
        } = outcome
        {
            self.expanded = expanded;
        }
        record(&mut self.outcome, "NavigationList", outcome)
    }
}

impl StoryInteraction for NavigationListInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let items = self.items();
        NavigationList::new(&items, &system).paint(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key, &self.items());
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse, &self.items());
        self.apply(outcome)
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ move",
            "Enter open",
            "←→ collapse/expand",
            "/ filter",
            "click row",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        self.state.is_filter_active()
    }
}

pub(crate) struct LinkInteractor {
    state: LinkState,
    theme: RolePalette,
    outcome: Option<String>,
}

impl LinkInteractor {
    pub(crate) fn new() -> Self {
        let mut state = LinkState::new();
        state.set_focused(true);
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: LinkOutcome) -> bool {
        record(&mut self.outcome, "Link", outcome)
    }
}

impl StoryInteraction for LinkInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let _ = Link::url("TermRock documentation", "https://termrock.dev", &system).paint(
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let outcome = Link::url("TermRock documentation", "https://termrock.dev", &system)
            .handle_key(&mut self.state, key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let before = self.state.clone();
        let outcome = Link::url("TermRock documentation", "https://termrock.dev", &system)
            .handle_mouse(&mut self.state, mouse);
        self.apply(outcome) || self.state != before
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec!["hover", "click open", "Enter open", "C copy destination"]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}
