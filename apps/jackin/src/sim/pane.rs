use std::collections::VecDeque;
use std::sync::Arc;

use junie_tui::core::event::Key;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::Tone;
use junie_tui::widgets::viewport::{Line, Span, TextViewport};
use junie_tui::widgets::viewport_ops::ViewportOps;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

use crate::domain::account::AccountId;
use crate::domain::agent::Agent;
use crate::domain::instance::AgentState;
use crate::domain::vocabulary::PaneWords;
use crate::panes::{
    PaneBody, PaneKey, PaneKindTag, PaneNote, PaneRequest, PaneSpawn, PasteOutcome,
};

pub const SCROLLBACK: usize = 2_000;

pub trait SessionScript: std::fmt::Debug + Send + Sync {
    fn script(&self, agent: Option<Agent>, workspace: &str) -> Vec<Step>;

    fn respond(
        &self,
        proc: &mut AgentProcess,
        input: &str,
        now_ms: i64,
        workspace: &str,
    ) -> Vec<Line>;
}

#[derive(Debug, Clone)]
pub enum Step {
    Emit(i64, Line),
    State(AgentState),

    Touch(&'static str),

    Await,
}

#[derive(Debug, Clone)]
pub struct AgentProcess {
    pub agent: Option<Agent>,
    pub account: Option<AccountId>,
    pub state: AgentState,
    pub script: Vec<Step>,
    pub pc: usize,
    pub next_at_ms: i64,

    scheduled: bool,
    scripts: Arc<dyn SessionScript>,
    pub reply_queue: VecDeque<(i64, Line)>,
    pub reply_cursor: usize,
    pub prompt: String,
    pub cursor_hidden: bool,
    pub awaiting_permission: bool,
    pub touched: Vec<String>,
}

fn sp(text: &str, tone: Tone) -> Span {
    Span::new(text, tone)
}

impl AgentProcess {
    pub fn new(
        agent: Option<Agent>,
        account: Option<AccountId>,
        workspace: &str,
        start_ms: i64,
        scripts: Arc<dyn SessionScript>,
    ) -> Self {
        let prompt = match agent {
            Some(Agent::ClaudeCode) => "❯ ".to_owned(),
            Some(Agent::Codex) => "❯ ".to_owned(),
            Some(Agent::OpenCode) | Some(Agent::GrokBuild) => "> ".to_owned(),
            Some(_) => "› ".to_owned(),
            None => format!("{workspace} ❯ "),
        };
        Self {
            agent,
            account,
            state: if agent.is_some() {
                AgentState::Working
            } else {
                AgentState::Unknown
            },
            script: scripts.script(agent, workspace),
            pc: 0,
            next_at_ms: start_ms,
            scheduled: false,
            scripts,
            reply_queue: VecDeque::new(),
            reply_cursor: 0,
            prompt,
            cursor_hidden: false,
            awaiting_permission: false,
            touched: vec![],
        }
    }

    pub fn boot_all(&mut self, out: &mut Vec<Line>) {
        while self.pc < self.script.len() {
            match &self.script[self.pc] {
                Step::Emit(_, l) => out.push(l.clone()),
                Step::State(s) => self.state = *s,
                Step::Touch(p) => self.touched.push((*p).to_owned()),
                Step::Await => {
                    self.pc += 1;
                    self.awaiting_permission = self.state == AgentState::Blocked;
                    return;
                }
            }
            self.pc += 1;
        }
    }

    pub fn awaiting(&self) -> bool {
        self.pc > 0
            && matches!(self.script.get(self.pc - 1), Some(Step::Await))
            && self.reply_queue.is_empty()
    }

    pub fn tick(&mut self, now_ms: i64) -> Vec<Line> {
        let mut out = vec![];
        while let Some((due, _)) = self.reply_queue.front() {
            if *due <= now_ms {
                let (_, l) = self.reply_queue.pop_front().unwrap();
                out.push(l);
            } else {
                break;
            }
        }
        if self.reply_queue.is_empty()
            && self.state == AgentState::Working
            && self.awaiting()
            && !self.awaiting_permission
        {
            self.state = AgentState::Done;
        }
        while self.pc < self.script.len() {
            match self.script[self.pc].clone() {
                Step::Emit(delay, l) => {
                    if !self.scheduled {
                        self.next_at_ms += delay;
                        self.scheduled = true;
                    }
                    if now_ms < self.next_at_ms {
                        break;
                    }
                    self.scheduled = false;
                    out.push(l);
                    self.pc += 1;
                }
                Step::State(s) => {
                    self.state = s;
                    self.pc += 1;
                }
                Step::Touch(p) => {
                    self.touched.push(p.to_owned());
                    self.pc += 1;
                }
                Step::Await => {
                    self.awaiting_permission = self.state == AgentState::Blocked;
                    self.pc += 1;
                    break;
                }
            }
        }
        out
    }

    pub fn on_input(&mut self, input: &str, now_ms: i64, workspace: &str) -> Vec<Line> {
        let scripts = self.scripts.clone();
        scripts.respond(self, input, now_ms, workspace)
    }
}

#[derive(Debug)]
pub struct ScriptPane {
    proc: AgentProcess,
    term: TextViewport,
    input: String,
    workspace: String,
}

impl ScriptPane {
    pub fn new(request: &PaneRequest, spawn: &PaneSpawn, scripts: Arc<dyn SessionScript>) -> Self {
        let mut term = TextViewport::new(WidgetId::of("capsule.pane").child(spawn.id as usize))
            .max_lines(SCROLLBACK);
        term.follow = true;
        Self {
            proc: AgentProcess::new(
                request.agent(),
                request.account().cloned(),
                &spawn.workspace,
                spawn.now_ms,
                scripts,
            ),
            term,
            input: String::new(),
            workspace: spawn.workspace.clone(),
        }
    }

    pub fn booted(mut self) -> Self {
        let mut out = vec![];
        self.proc.boot_all(&mut out);
        self.push_lines(out);
        self.refresh_prompt();
        self
    }

    pub fn with_history_above(mut self, mut history: Vec<Line>) -> Self {
        history.extend(self.term.lines().cloned());
        self.term.set_lines(history);
        self
    }

    fn prompt_line(&self) -> Line {
        vec![
            sp(&self.proc.prompt, Tone::Secondary),
            sp(&self.input, Tone::Normal),
        ]
    }

    fn refresh_prompt(&mut self) {
        let waiting = self.proc.awaiting() || !self.input.is_empty();
        if waiting {
            let last_is_prompt = self
                .term
                .last()
                .is_some_and(|l| l.first().is_some_and(|s| s.text == self.proc.prompt));
            let pl = self.prompt_line();
            if last_is_prompt {
                self.term.replace_last(pl);
            } else {
                self.term.push(pl);
            }
            let n = self.term.len().saturating_sub(1);
            let col = junie_tui::ui::text::width(&self.proc.prompt)
                + junie_tui::ui::text::width(&self.input);
            self.term.caret = Some(junie_tui::widgets::viewport::CellPos { line: n, col });
        } else {
            self.term.caret = None;
        }
        self.term.caret_visible = !self.proc.cursor_hidden;
    }

    fn push_lines(&mut self, lines: Vec<Line>) {
        if lines.is_empty() {
            return;
        }

        if self
            .term
            .last()
            .is_some_and(|l| l.first().is_some_and(|s| s.text == self.proc.prompt))
        {
            self.term.pop();
        }
        for l in lines {
            if l.first().is_some_and(|s| s.text == "\u{0}clear") {
                self.term.clear();
                continue;
            }
            if l.first().is_some_and(|s| s.text == "⠋ ") {
                self.term.push(l);
                continue;
            }

            if self
                .term
                .last()
                .is_some_and(|x| x.first().is_some_and(|s| s.text == "⠋ "))
            {
                self.term.pop();
            }
            self.term.push(l);
        }
        self.refresh_prompt();
    }

    fn type_char(&mut self, c: char) {
        self.term.set_follow(true);
        self.input.push(c);
        self.refresh_prompt();
    }

    fn backspace(&mut self) {
        self.term.set_follow(true);
        self.input.pop();
        self.refresh_prompt();
    }

    fn commit(&mut self, now_ms: i64) {
        self.term.set_follow(true);
        let text = std::mem::take(&mut self.input);
        if self
            .term
            .last()
            .is_some_and(|l| l.first().is_some_and(|s| s.text == self.proc.prompt))
        {
            self.term.pop();
        }
        let out = self.proc.on_input(&text, now_ms, &self.workspace);
        self.push_lines(out);
        self.refresh_prompt();
    }
}

impl PaneBody for ScriptPane {
    fn kind(&self) -> PaneKindTag {
        PaneKindTag::Script
    }

    fn state(&self) -> AgentState {
        self.proc.state
    }

    fn title(&self) -> Option<&str> {
        None
    }

    fn note(&self, _words: &PaneWords) -> Option<PaneNote> {
        None
    }

    fn viewport(&self) -> &dyn ViewportOps {
        &self.term
    }

    fn viewport_mut(&mut self) -> &mut dyn ViewportOps {
        &mut self.term
    }

    fn key(&mut self, key: &Key, now_ms: i64) -> PaneKey {
        match key.code {
            KeyCode::Char(c) if !key.chord() => {
                self.term.clear_selection();
                self.type_char(c);
            }
            KeyCode::Backspace => self.backspace(),
            KeyCode::Enter => self.commit(now_ms),
            KeyCode::Esc => {
                self.term.set_follow(true);
            }
            _ => return PaneKey::Consumed,
        }
        PaneKey::Forwarded
    }

    fn paste(&mut self, text: &str, _now_ms: i64) -> PasteOutcome {
        for c in text.chars() {
            self.type_char(c);
        }
        PasteOutcome::Sent
    }

    fn tick(&mut self, now_ms: i64) -> bool {
        let out = self.proc.tick(now_ms);
        let changed = !out.is_empty();
        self.push_lines(out);
        self.refresh_prompt();
        changed
    }

    fn sync_size(&mut self, inner: Rect) {
        self.term.set_area(inner);
    }

    fn focus_changed(&mut self, _focused: bool, _now_ms: i64) {}

    fn redraw(&mut self) {}

    fn clear(&mut self) {
        self.term.clear();
        self.term.set_follow(true);
        self.refresh_prompt();
    }

    fn touched(&self) -> &[String] {
        &self.proc.touched
    }

    fn sends_prefix(&self) -> bool {
        false
    }

    fn close_needs_confirm(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::instance::InstanceId;
    use crate::panes::PaneSpawn;
    use crate::sim::scripts::SimScripts;
    use junie_tui::core::focus::FocusRing;
    use junie_tui::core::hit::HitRegistry;
    use junie_tui::theme::{ColorLevel, Theme};
    use junie_tui::ui::ctx::{Interaction, RenderCtx};
    use ratatui::buffer::Buffer;

    const INNER: Rect = Rect::new(0, 0, 80, 24);

    fn synced_pane() -> ScriptPane {
        let instance = InstanceId::simulated("jk-pane-test");
        let request = PaneRequest::new(&instance, None, None);
        let spawn = PaneSpawn {
            id: 1,
            workspace: "workspace".into(),
            now_ms: 0,
        };
        let mut pane = ScriptPane::new(&request, &spawn, SimScripts::shared());
        pane.term.set_lines(
            (0..1_000)
                .map(|n| vec![Span::plain(format!("history line {n}"))])
                .collect(),
        );
        pane.sync_size(INNER);
        pane
    }

    fn draw_at(pane: &ScriptPane, area: Rect) {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(INNER);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        pane.viewport()
            .draw(area, &mut buf, &mut ctx, theme.canvas, true);
    }

    #[test]
    fn capsule_pane_draw_takes_the_fast_path() {
        let pane = synced_pane();
        draw_at(&pane, INNER);
        assert_eq!(pane.term.work_counters().draw_fallbacks, 0);
    }

    #[test]
    fn capsule_pane_draw_fallback_counts_a_different_rect() {
        let pane = synced_pane();
        draw_at(&pane, Rect::new(0, 0, 60, 24));
        assert_eq!(pane.term.work_counters().draw_fallbacks, 1);
    }

    #[test]
    fn capsule_pane_collapsed_draw_copies_no_history() {
        let pane = synced_pane();
        draw_at(&pane, Rect::new(0, 0, 0, 0));
        let work = pane.term.work_counters();
        assert_eq!(work.draw_fallbacks, 0);
        assert_eq!(work.draw_cloned_lines, 0);
    }
}
