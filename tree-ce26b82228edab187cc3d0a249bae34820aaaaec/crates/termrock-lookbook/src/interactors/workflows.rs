// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Persistent demos for multi-step and trust-sensitive workflows.

use ratatui::{Frame, layout::Rect};
use termrock::{
    input::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent},
    keymap::KeyChord,
    style::{DesignSystem, Role, RolePalette},
    widgets::{
        FieldToken, JumpOutcome, JumpOverlay, JumpOverlayState, JumpTarget, KeybindingRecorder,
        KeybindingRecorderOutcome, KeybindingRecorderState, PermissionActionKind,
        PermissionOutcome, PermissionPrompt, PermissionPromptState, PermissionProvenance,
        PermissionRequest, PermissionRisk, ProgressStep, ProgressSteps, ProgressStepsOutcome,
        ProgressStepsState, QuestionFlow, QuestionFlowOutcome, QuestionFlowState, StepItem,
        Stepper, StepperNavPolicy, StepperOutcome, StepperState, TokenField, TokenFieldOutcome,
        TokenFieldState, example_agent_plan_steps, example_onboarding_steps, example_question_set,
    },
};

use super::{StoryInteraction, extended::record};

pub(crate) struct ProgressStepsInteractor {
    state: ProgressStepsState,
    theme: RolePalette,
    outcome: Option<String>,
}
impl ProgressStepsInteractor {
    pub(crate) fn new() -> Self {
        let mut state = ProgressStepsState::interactive();
        state.set_cursor(Some("build".into()));
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn steps() -> Vec<ProgressStep> {
        example_agent_plan_steps()
    }
    fn apply(&mut self, outcome: ProgressStepsOutcome) -> bool {
        record(&mut self.outcome, "ProgressSteps", outcome)
    }
}
impl StoryInteraction for ProgressStepsInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = DesignSystem::from_palette(self.theme.clone());
        let steps = Self::steps();
        ProgressSteps::new(&steps, &system)
            .title("Agent plan")
            .paint(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(&Self::steps(), key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> bool {
        let outcome = self.state.handle_mouse(&Self::steps(), mouse, area, 2);
        self.apply(outcome)
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "↑↓ choose step",
            "Enter activate/retry",
            "click step",
            "Esc blur",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct StepperInteractor {
    state: StepperState,
    theme: RolePalette,
    outcome: Option<String>,
}
impl StepperInteractor {
    pub(crate) fn new() -> Self {
        let items = example_onboarding_steps();
        let mut state = StepperState::with_len(items.len()).policy(StepperNavPolicy::Free);
        state.set_focused(true);
        state.set_current(1, items.len(), true);
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn items() -> Vec<StepItem> {
        example_onboarding_steps()
    }
    fn apply(&mut self, outcome: StepperOutcome) -> bool {
        record(&mut self.outcome, "Stepper", outcome)
    }
}
impl StoryInteraction for StepperInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = DesignSystem::from_palette(self.theme.clone());
        let items = Self::items();
        Stepper::new(&items, &system).paint(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let items = Self::items();
        let outcome = self.state.handle_key(key, &items);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let items = Self::items();
        let outcome = self.state.handle_mouse(mouse, &items);
        self.apply(outcome)
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec!["←→ choose step", "Enter activate", "1–9 jump", "click step"]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}

pub(crate) struct KeybindingRecorderInteractor {
    state: KeybindingRecorderState,
    theme: RolePalette,
    outcome: Option<String>,
}
impl KeybindingRecorderInteractor {
    pub(crate) fn new() -> Self {
        let mut state = KeybindingRecorderState::new("app.save", "Save")
            .with_chords([KeyChord::ctrl(KeyCode::Char('s'))])
            .with_reserved(Vec::new());
        state.set_focused(true);
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: KeybindingRecorderOutcome) -> bool {
        record(&mut self.outcome, "KeybindingRecorder", outcome)
    }
}
impl StoryInteraction for KeybindingRecorderInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = DesignSystem::from_palette(self.theme.clone());
        KeybindingRecorder::new(&system).ascii(true).paint(
            area,
            frame.buffer_mut(),
            &mut self.state,
        );
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, _mouse: MouseEvent, _area: Rect) -> bool {
        false
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        if self.state.is_recording() {
            vec![
                "press chord",
                "Enter commit",
                "Backspace undo",
                "Esc cancel",
            ]
        } else {
            vec!["Enter/Space record", "Delete clear", "R restore default"]
        }
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        self.state.is_recording()
    }
}

pub(crate) struct QuestionFlowInteractor {
    state: QuestionFlowState,
    theme: RolePalette,
    outcome: Option<String>,
}
impl QuestionFlowInteractor {
    pub(crate) fn new() -> Self {
        let mut state = QuestionFlowState::new();
        state.open_set(example_question_set());
        state.focused = true;
        state.set_accepts_input(true);
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: QuestionFlowOutcome) -> bool {
        record(&mut self.outcome, "QuestionFlow", outcome)
    }
}
impl StoryInteraction for QuestionFlowInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        if !self.state.is_open() {
            let system = DesignSystem::from_palette(self.theme.clone());
            frame.buffer_mut().set_stringn(
                area.x,
                area.y,
                "Question flow closed · press O to start again",
                usize::from(area.width),
                system.style(Role::TextMuted),
            );
            return;
        }
        let system = DesignSystem::from_palette(self.theme.clone());
        frame.render_stateful_widget(&QuestionFlow::new(&system), area, &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !self.state.is_open() {
            if key.kind == KeyEventKind::Press
                && key.modifiers == KeyModifiers::NONE
                && matches!(key.code, KeyCode::Char('o' | 'O'))
            {
                self.state.open_set(example_question_set());
                self.state.focused = true;
                self.outcome = Some("QuestionFlow: Opened".into());
                return true;
            }
            return false;
        }
        let outcome = self.state.handle_key(key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse);
        self.apply(outcome)
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        if !self.state.is_open() {
            return vec!["O start question flow"];
        }
        vec![
            "↑↓ choose",
            "Space toggle",
            "type free text",
            "Enter answer/submit",
            "click option",
            "Esc cancel",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        self.state.is_open()
    }
}

pub(crate) struct TokenFieldInteractor {
    state: TokenFieldState<String>,
    theme: RolePalette,
    outcome: Option<String>,
}
impl TokenFieldInteractor {
    pub(crate) fn new() -> Self {
        let mut state = TokenFieldState::new();
        state.set_focused(true);
        let _ = state.push_token(FieldToken::new("1".into(), "alice@example.com"));
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: TokenFieldOutcome<String>) -> bool {
        record(&mut self.outcome, "TokenField", outcome)
    }
}
impl StoryInteraction for TokenFieldInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = DesignSystem::from_palette(self.theme.clone());
        let _ = TokenField::new(&system)
            .label("To")
            .placeholder("Add recipient…")
            .ascii(true)
            .paint(area, frame.buffer_mut(), &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let outcome = self.state.handle_key(key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse);
        self.apply(outcome)
    }
    fn handle_event(&mut self, event: Event, area: Rect) -> bool {
        match event {
            Event::Paste(text) => {
                let outcome = self.state.paste_values(&text);
                self.apply(outcome)
            }
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse, area),
            _ => false,
        }
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        vec![
            "type recipient",
            "Enter/comma add",
            "paste values",
            "←→ choose token",
            "Delete remove",
            "Alt+←→ reorder",
            "click",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        true
    }
}

pub(crate) struct PermissionPromptInteractor {
    state: PermissionPromptState,
    theme: RolePalette,
    outcome: Option<String>,
}
impl PermissionPromptInteractor {
    pub(crate) fn new() -> Self {
        let mut state = PermissionPromptState::new();
        state.enqueue(Self::request());
        Self {
            state,
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn request() -> PermissionRequest {
        PermissionRequest::new("r1", "bash", "workspace")
            .risk(PermissionRisk::High)
            .action_kind(PermissionActionKind::Shell)
            .command("rm -rf build/")
            .expected("remove generated build artifacts")
            .provenance(PermissionProvenance::main_agent("a", "agent"))
    }
    fn apply(&mut self, outcome: PermissionOutcome) -> bool {
        record(&mut self.outcome, "PermissionPrompt", outcome)
    }
}
impl StoryInteraction for PermissionPromptInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        if self.state.is_empty() {
            let system = DesignSystem::from_palette(self.theme.clone());
            frame.buffer_mut().set_stringn(
                area.x,
                area.y,
                "Permission decided · press O for another request",
                usize::from(area.width),
                system.style(Role::TextMuted),
            );
            return;
        }
        let system = DesignSystem::from_palette(self.theme.clone());
        frame.render_stateful_widget(&PermissionPrompt::new(&system), area, &mut self.state);
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.state.is_empty() {
            if key.kind == KeyEventKind::Press
                && key.modifiers == KeyModifiers::NONE
                && matches!(key.code, KeyCode::Char('o' | 'O'))
            {
                self.state.enqueue(Self::request());
                self.outcome = Some("PermissionPrompt: Enqueued".into());
                return true;
            }
            return false;
        }
        let outcome = self.state.handle_key(key);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse);
        self.apply(outcome)
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        if self.state.is_empty() {
            return vec!["O enqueue request"];
        }
        vec![
            "←→ choose decision",
            "↑↓ choose scope",
            "Enter decide",
            "D details",
            "E edit",
            "click action",
            "Esc deny/cancel",
        ]
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
    fn captures_text_input(&self) -> bool {
        false
    }
}

pub(crate) struct JumpOverlayInteractor {
    state: JumpOverlayState,
    targets: Vec<JumpTarget<&'static str>>,
    theme: RolePalette,
    outcome: Option<String>,
}
impl JumpOverlayInteractor {
    pub(crate) fn new() -> Self {
        let mut state = JumpOverlayState::new();
        state.open();
        Self {
            state,
            targets: Vec::new(),
            theme: RolePalette::default(),
            outcome: None,
        }
    }
    fn apply(&mut self, outcome: JumpOutcome<&'static str>) -> bool {
        record(&mut self.outcome, "JumpOverlay", outcome)
    }
}
impl StoryInteraction for JumpOverlayInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = DesignSystem::from_palette(self.theme.clone());
        self.targets = vec![
            JumpTarget::new(
                "files",
                Rect::new(area.x.saturating_add(2), area.y.saturating_add(1), 12, 1),
                "f",
            ),
            JumpTarget::new(
                "main",
                Rect::new(area.x.saturating_add(2), area.y.saturating_add(3), 12, 1),
                "m",
            ),
        ];
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(2),
            area.y.saturating_add(1),
            "Files pane",
            12,
            system.style(termrock::style::Role::Text),
        );
        frame.buffer_mut().set_stringn(
            area.x.saturating_add(2),
            area.y.saturating_add(3),
            "Main pane",
            12,
            system.style(termrock::style::Role::Text),
        );
        if self.state.is_open() {
            frame.render_widget(JumpOverlay::new(&self.targets, &system), area);
        }
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !self.state.is_open()
            && key.kind == termrock::input::KeyEventKind::Press
            && key.modifiers == KeyModifiers::NONE
            && matches!(key.code, KeyCode::Char('j' | 'J'))
        {
            self.state.open();
            self.outcome = Some("JumpOverlay: Opened".into());
            return true;
        }
        let outcome = self.state.handle_key(key, &self.targets);
        self.apply(outcome)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, _area: Rect) -> bool {
        let outcome = self.state.handle_mouse(mouse, &self.targets);
        self.apply(outcome)
    }
    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }
    fn hints(&self) -> Vec<&'static str> {
        if self.state.is_open() {
            vec!["F/M jump", "click target", "Esc dismiss"]
        } else {
            vec!["J reopen jump mode"]
        }
    }
    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }
}
