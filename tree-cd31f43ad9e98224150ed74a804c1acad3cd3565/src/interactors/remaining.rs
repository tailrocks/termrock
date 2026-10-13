// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Persistent demos for the remaining primary routes with public interaction APIs.

use ratatui::{Frame, layout::Rect, widgets::Widget};
use termrock::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind},
    style::{Role, RolePalette},
    widgets::{
        Action, Button, ButtonState, CheckpointTimeline, CheckpointTimelineState, DiffHunk,
        DiffLine, DiffReview, DiffReviewFileRow, DiffReviewState, Drawer, DrawerOutcome,
        DrawerState, EmptyStateOutcome, EmptyStateState, ErrorStateOutcome, ErrorStateState,
        FullscreenViewer, FullscreenViewerOutcome, FullscreenViewerState, KeyValueList,
        KeyValueListState, KeyValueTable, KeyValueTableOutcome, KeyValueTableState, KvEntry,
        KvStatus, KvtField, LoadState, ModeRibbon, ModeRibbonOutcome, ModeRibbonState,
        OfflineBanner, OfflineSurface, PreviewCard, PreviewCardState, ReconnectingState, Sheet,
        SourceContext, ViewerContentKind, WorkbenchMode, example_checkpoints, example_empty_search,
        example_error_network, example_file_preview, example_reconnecting_agent,
    },
};

use super::{StoryInteraction, extended::record};
use crate::stories::diff_review_sample;

pub(crate) struct EmptyStateInteractor {
    state: EmptyStateState,
    theme: RolePalette,
    outcome: Option<String>,
}
impl EmptyStateInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: EmptyStateState::new(),
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: EmptyStateOutcome) -> bool {
        record(&mut self.outcome, "EmptyState", outcome)
    }
}
impl StoryInteraction for EmptyStateInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        example_empty_search(&system).paint_with_state(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let outcome = example_empty_search(&system).handle_key(key, &mut self.state);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let outcome = example_empty_search(&system).handle_mouse(mouse, area, &mut self.state);
        self.apply(outcome)
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "Tab choose action",
            "Enter/Space activate",
            "1/2 shortcut",
            "click action",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct ErrorStateInteractor {
    state: ErrorStateState,
    theme: RolePalette,
    outcome: Option<String>,
}
impl ErrorStateInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: ErrorStateState::new(),
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: ErrorStateOutcome) -> bool {
        record(&mut self.outcome, "ErrorState", outcome)
    }
}
impl StoryInteraction for ErrorStateInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        example_error_network(&system).paint_with_state(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let outcome = example_error_network(&system).handle_key(key, &mut self.state);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let outcome = example_error_network(&system).handle_mouse(mouse, area, &mut self.state);
        self.apply(outcome)
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "R retry",
            "D details",
            "C copy diagnostics",
            "Tab choose action",
            "Enter activate",
            "click action",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct DrawerInteractor {
    state: DrawerState,
    trigger: ButtonState,
    sheet: bool,
    area: Rect,
    theme: RolePalette,
    outcome: Option<String>,
}
impl DrawerInteractor {
    pub(crate) fn drawer() -> Self {
        Self::new(false)
    }
    pub(crate) fn sheet() -> Self {
        Self::new(true)
    }
    fn new(sheet: bool) -> Self {
        let mut trigger = ButtonState::new();
        trigger.activation.set_accepts_input(true);
        Self {
            state: if sheet {
                DrawerState::sheet()
            } else {
                DrawerState::new()
            },
            trigger,
            sheet,
            area: Rect::default(),
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: DrawerOutcome) -> bool {
        record(
            &mut self.outcome,
            if self.sheet { "Sheet" } else { "Drawer" },
            outcome,
        )
    }
}
impl StoryInteraction for DrawerInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        self.area = area;
        let system = crate::design::lookbook_system(self.theme.clone());
        if !self.state.is_open() {
            let _ = Button::new(
                if self.sheet {
                    "Open sheet"
                } else {
                    "Open drawer"
                },
                &system,
            )
            .paint(
                Rect::new(area.x, area.y, area.width.min(18), 1.min(area.height)),
                frame.buffer_mut(),
                &mut self.trigger,
            );
            return;
        }
        if self.sheet {
            Sheet::new("Details", &system)
                .footer(Some("Esc close · drag handle"))
                .paint(area, frame.buffer_mut(), &mut self.state);
        } else {
            Drawer::new("Inspector", &system)
                .footer(Some("Esc close · [ ] resize"))
                .paint(area, frame.buffer_mut(), &mut self.state);
        }
        let body = self.state.body_area();
        if !body.is_empty() {
            frame.buffer_mut().set_stringn(
                body.x,
                body.y,
                "filters · details",
                usize::from(body.width),
                system.style(Role::Text),
            );
        }
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.state.is_open() {
            let outcome = self.state.handle_key(key);
            return self.apply(outcome);
        }
        let outcome = self.trigger.handle_key(key);
        if matches!(outcome, termrock::widgets::ActivationOutcome::Activated) {
            let drawer = self.state.request_open(self.area);
            return self.apply(drawer);
        }
        false
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        if self.state.is_open() {
            let outcome = self.state.handle_mouse(mouse);
            return self.apply(outcome);
        }
        let before = self.trigger;
        let outcome = self.trigger.handle_mouse(mouse);
        if matches!(outcome, termrock::widgets::ActivationOutcome::Activated) {
            let drawer = self.state.request_open(self.area);
            return self.apply(drawer);
        }
        self.trigger != before
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        if self.state.is_open() {
            if self.sheet {
                vec!["Esc close", "drag handle resize"]
            } else {
                vec!["Esc close", "[ ] resize", "drag handle resize"]
            }
        } else {
            vec!["Enter open", "click trigger"]
        }
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn handle_preview_escape(&mut self, key: KeyEvent) -> bool {
        if self.state.is_open() {
            self.handle_key(key)
        } else {
            false
        }
    }
}

pub(crate) struct FullscreenViewerInteractor {
    state: FullscreenViewerState<&'static str>,
    trigger: ButtonState,
    theme: RolePalette,
    outcome: Option<String>,
}
impl FullscreenViewerInteractor {
    pub(crate) fn new() -> Self {
        let mut trigger = ButtonState::new();
        trigger.activation.set_accepts_input(true);
        let mut state = FullscreenViewerState::new();
        state.zoom_mut().set_content_kind(ViewerContentKind::Code);
        Self {
            state,
            trigger,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn actions() -> [Action<'static, &'static str>; 2] {
        [
            Action {
                id: "copy",
                label: "Copy",
                enabled: true,
                style: None,
            },
            Action {
                id: "raw",
                label: "Raw",
                enabled: true,
                style: None,
            },
        ]
    }
    fn apply(&mut self, outcome: FullscreenViewerOutcome<&'static str>) -> bool {
        record(&mut self.outcome, "FullscreenViewer", outcome)
    }
}
impl StoryInteraction for FullscreenViewerInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        if !self.state.is_open() {
            let _ = Button::new("Open fullscreen viewer", &system).paint(
                Rect::new(area.x, area.y, area.width.min(24), 1.min(area.height)),
                frame.buffer_mut(),
                &mut self.trigger,
            );
            return;
        }
        let actions = Self::actions();
        FullscreenViewer::new(&system, &actions).paint(area, frame.buffer_mut(), &mut self.state);
        let body = self.state.body_area();
        if !body.is_empty() {
            frame.buffer_mut().set_stringn(
                body.x,
                body.y,
                "fn main() { /* interactive viewer */ }",
                usize::from(body.width),
                system.style(Role::Text),
            );
        }
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !self.state.is_open() {
            let outcome = self.trigger.handle_key(key);
            if matches!(outcome, termrock::widgets::ActivationOutcome::Activated) {
                let opened = self
                    .state
                    .enter_fullscreen(SourceContext::new("main.rs"), "main.rs");
                return self.apply(opened);
            }
            return false;
        }
        let actions = Self::actions();
        let outcome = self.state.handle_key(key, &actions);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        if self.state.is_open() {
            return false;
        }
        let before = self.trigger;
        let outcome = self.trigger.handle_mouse(mouse);
        if matches!(outcome, termrock::widgets::ActivationOutcome::Activated) {
            let opened = self
                .state
                .enter_fullscreen(SourceContext::new("main.rs"), "main.rs");
            return self.apply(opened);
        }
        self.trigger != before
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        if self.state.is_open() {
            vec![
                "Esc close/demote",
                "/ search",
                "? help",
                "Tab cycle chrome",
                "Enter action",
            ]
        } else {
            vec!["Enter open", "click trigger"]
        }
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        self.state.search_open()
    }
    fn handle_preview_escape(&mut self, key: KeyEvent) -> bool {
        if self.state.is_open() {
            self.handle_key(key)
        } else {
            false
        }
    }
}

pub(crate) struct ModeRibbonInteractor {
    state: ModeRibbonState<&'static str>,
    theme: RolePalette,
    outcome: Option<String>,
}
impl ModeRibbonInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: ModeRibbonState::new(Some("plan")),
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: ModeRibbonOutcome<&'static str>) -> bool {
        record(&mut self.outcome, "ModeRibbon", outcome)
    }
}
impl StoryInteraction for ModeRibbonInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let selected = self.state.selected().copied();
        let modes = [
            WorkbenchMode {
                id: "plan",
                label: "Plan",
                active: selected == Some("plan"),
                enabled: true,
            },
            WorkbenchMode {
                id: "build",
                label: "Build",
                active: selected == Some("build"),
                enabled: true,
            },
            WorkbenchMode {
                id: "ask",
                label: "Ask",
                active: selected == Some("ask"),
                enabled: true,
            },
        ];
        Widget::render(ModeRibbon::new(&modes, &system), area, frame.buffer_mut());
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let selected = self.state.selected().copied();
        let modes = [
            WorkbenchMode {
                id: "plan",
                label: "Plan",
                active: selected == Some("plan"),
                enabled: true,
            },
            WorkbenchMode {
                id: "build",
                label: "Build",
                active: selected == Some("build"),
                enabled: true,
            },
            WorkbenchMode {
                id: "ask",
                label: "Ask",
                active: selected == Some("ask"),
                enabled: true,
            },
        ];
        let outcome = self.state.handle_key(&modes, key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, _mouse: MouseEvent, _area: Rect) -> bool {
        false
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec!["←→ change mode", "Enter request mode"]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct OfflineBannerInteractor {
    state: ReconnectingState,
    area: Rect,
    full: bool,
    theme: RolePalette,
    outcome: Option<String>,
}
impl OfflineBannerInteractor {
    pub(crate) fn new() -> Self {
        Self::banner()
    }
    pub(crate) fn banner() -> Self {
        Self::with_presentation(false)
    }
    pub(crate) fn surface() -> Self {
        Self::with_presentation(true)
    }
    fn with_presentation(full: bool) -> Self {
        let mut state = example_reconnecting_agent();
        if full {
            state.set_presentation(termrock::widgets::ConnectivityPresentation::Full);
        }
        Self {
            state,
            area: Rect::default(),
            full,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
}
impl StoryInteraction for OfflineBannerInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        self.area = area;
        let system = crate::design::lookbook_system(self.theme.clone());
        if self.full {
            OfflineSurface::new(&system).paint(area, frame.buffer_mut(), &mut self.state);
        } else {
            OfflineBanner::new(&self.state, &system).paint(area, frame.buffer_mut());
        }
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.state.banner_dismissed()
            && key.kind == KeyEventKind::Press
            && key.modifiers == KeyModifiers::NONE
            && matches!(key.code, KeyCode::Char('b' | 'B'))
        {
            self.state.begin_reconnect(1);
            self.outcome = Some("OfflineBanner: Shown".into());
            return true;
        }
        let outcome = self.state.handle_key(key);
        record(&mut self.outcome, "OfflineBanner", outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse, self.area);
        record(&mut self.outcome, "OfflineBanner", outcome)
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        if self.state.banner_dismissed() {
            vec!["B show banner"]
        } else {
            vec![
                "R retry",
                "O work offline",
                "Q view queue",
                "Esc dismiss",
                "click retry",
            ]
        }
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct PreviewCardInteractor {
    state: PreviewCardState,
    area: Rect,
    theme: RolePalette,
    outcome: Option<String>,
}
impl PreviewCardInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: PreviewCardState::with_delay(std::time::Duration::ZERO),
            area: Rect::default(),
            theme: RolePalette::default(),
            outcome: None,
        }
    }
}
impl StoryInteraction for PreviewCardInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        self.area = area;
        let system = crate::design::lookbook_system(self.theme.clone());
        if self.state.is_visible() {
            let (content, _, _) = example_file_preview();
            PreviewCard::new(content, &system).paint(area, frame.buffer_mut(), &mut self.state);
        } else {
            frame.buffer_mut().set_stringn(
                area.x,
                area.y,
                "Hover here to preview main.rs",
                usize::from(area.width),
                system.style(Role::TextMuted),
            );
        }
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = if !self.state.is_pinned()
            && key.kind == KeyEventKind::Press
            && key.modifiers == KeyModifiers::NONE
            && matches!(key.code, KeyCode::Char('p' | 'P'))
        {
            self.state.pin()
        } else {
            self.state.handle_key(key)
        };
        record(&mut self.outcome, "PreviewCard", outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        if !matches!(mouse.kind, MouseEventKind::Moved | MouseEventKind::Drag(_)) {
            return false;
        }
        let hovering = self.area.contains(mouse.position);
        let outcome = self.state.tick_hover(1, hovering);
        record(&mut self.outcome, "PreviewCard", outcome)
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        if self.state.is_pinned() {
            vec!["Enter open", "P/Esc unpin"]
        } else {
            vec!["hover show/hide", "P pin"]
        }
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct CheckpointTimelineInteractor {
    state: CheckpointTimelineState,
    theme: RolePalette,
    outcome: Option<String>,
}

impl CheckpointTimelineInteractor {
    pub(crate) fn new() -> Self {
        let mut state = CheckpointTimelineState::new();
        state.set_checkpoints(example_checkpoints());
        state.focused = true;
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
}

impl StoryInteraction for CheckpointTimelineInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        CheckpointTimeline::new(&system).paint(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        record(&mut self.outcome, "CheckpointTimeline", outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse);
        record(&mut self.outcome, "CheckpointTimeline", outcome)
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ select",
            "Enter preview/confirm",
            "R restore",
            "W rewind",
            "C compare",
            "F follow",
            "click row",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct DiffReviewInteractor {
    state: DiffReviewState,
    lines: Vec<DiffLine<'static>>,
    hunks: [DiffHunk; 2],
    files: [DiffReviewFileRow<'static>; 2],
    theme: RolePalette,
    outcome: Option<String>,
}

impl DiffReviewInteractor {
    pub(crate) fn new() -> Self {
        let (lines, hunks, files) = diff_review_sample();
        Self {
            state: DiffReviewState::new(),
            lines,
            hunks,
            files,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
}

impl StoryInteraction for DiffReviewInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        DiffReview::new(&self.lines, &system)
            .hunks(&self.hunks)
            .files(&self.files)
            .title("PR · interactive review")
            .render(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self
            .state
            .handle_key_lines(key, &self.lines, &self.hunks, &self.files);
        record(&mut self.outcome, "DiffReview", outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self
            .state
            .handle_mouse_lines(mouse, &self.lines, &self.hunks, &self.files);
        record(&mut self.outcome, "DiffReview", outcome)
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ navigate",
            "Tab region",
            "Space select",
            "A approve",
            "X reject",
            "C comment",
            "/ search",
            "click row",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        self.state.comment_draft.is_some() || self.state.view.search.is_some()
    }
}

fn key_value_entries() -> [KvEntry<'static, &'static str>; 7] {
    [
        KvEntry::group_header("id", "Identity"),
        KvEntry::pair("name", "Name", "termrock")
            .copyable()
            .annotation("crate"),
        KvEntry::pair("status", "Status", "active").status(KvStatus::Success),
        KvEntry::pair("token", "Token", "super-secret")
            .secret()
            .copyable(),
        KvEntry::pair("docs", "Docs", "handbook")
            .href("https://example.invalid")
            .annotation("external"),
        KvEntry::group_header("build", "Build"),
        KvEntry::pair("target", "Target", "aarch64-apple-darwin").depth(1),
    ]
}

pub(crate) struct KeyValueListInteractor {
    state: KeyValueListState<&'static str>,
    theme: RolePalette,
    outcome: Option<String>,
}

impl KeyValueListInteractor {
    pub(crate) fn new() -> Self {
        let mut state = KeyValueListState::new();
        state.set_focused(true);
        state.cursor = Some("status");
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
}

impl StoryInteraction for KeyValueListInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let entries = key_value_entries();
        let _ = KeyValueList::reading(&entries, &system).paint(
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let entries = key_value_entries();
        let outcome = KeyValueList::reading(&entries, &system).handle_key(&mut self.state, key);
        record(&mut self.outcome, "KeyValueList", outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let entries = key_value_entries();
        let outcome = KeyValueList::reading(&entries, &system).handle_mouse(&mut self.state, mouse);
        record(&mut self.outcome, "KeyValueList", outcome)
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ select",
            "R/Space reveal secret",
            "C copy",
            "Enter open link",
            "wheel scroll",
            "click row",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct KeyValueTableInteractor {
    state: KeyValueTableState<&'static str>,
    content_type: String,
    theme: RolePalette,
    outcome: Option<String>,
}

impl KeyValueTableInteractor {
    pub(crate) fn new() -> Self {
        let mut state = KeyValueTableState::new().with_cursor("h");
        state.load = LoadState::Ready { count: 5 };
        Self {
            state,
            content_type: "application/json".into(),
            theme: RolePalette::default(),
            outcome: None,
        }
    }
}

fn key_value_table_fields(content_type: &str) -> [KvtField<'_, &'static str>; 6] {
    [
        KvtField::group("g", "Request"),
        KvtField::pair("m", "method", "GET")
            .value_type("string")
            .source("line")
            .copyable()
            .depth(1),
        KvtField::pair("h", "host", "api.tailrocks.dev")
            .value_type("host")
            .source("header")
            .copyable()
            .depth(1),
        KvtField::pair("a", "authorization", "Bearer sk-live-secret")
            .value_type("secret")
            .source("header")
            .secret()
            .copyable()
            .depth(1),
        KvtField::pair("c", "content-type", content_type)
            .value_type("mime")
            .source("header")
            .editable()
            .depth(1),
        KvtField::pair("u", "user-agent", "termrock/0.11")
            .value_type("string")
            .source("header")
            .depth(1),
    ]
}

impl StoryInteraction for KeyValueTableInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = crate::design::lookbook_system(self.theme.clone());
        let content_type = self.content_type.clone();
        let fields = key_value_table_fields(&content_type);
        KeyValueTable::new(&fields, &system).render(area, frame.buffer_mut(), &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let content_type = self.content_type.clone();
        let fields = key_value_table_fields(&content_type);
        let outcome = KeyValueTable::new(&fields, &system).handle_key(&mut self.state, key);
        if let KeyValueTableOutcome::EditCommitted { text, .. } = &outcome {
            self.content_type.clone_from(text);
        }
        record(&mut self.outcome, "KeyValueTable", outcome)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let system = crate::design::lookbook_system(self.theme.clone());
        let content_type = self.content_type.clone();
        let fields = key_value_table_fields(&content_type);
        let outcome = KeyValueTable::new(&fields, &system).handle_mouse(&mut self.state, mouse);
        record(&mut self.outcome, "KeyValueTable", outcome)
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ select",
            "E edit",
            "R reveal",
            "C copy",
            "/ filter",
            "D compare",
            "wheel scroll",
            "click row",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        self.state.editing || self.state.filter.is_some()
    }
}
