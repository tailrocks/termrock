use crate::domain::context::ScopeTag;
use crate::domain::effect::Effect;
use crate::domain::exec::Command;
use unicode_segmentation::UnicodeSegmentation;

pub const RETAIN_LINES: usize = 4000;

pub const KILL_AFTER_TICKS: u64 = 10;

pub const TERM_EXIT_TICKS: u64 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityState {
    Queued,
    Running,

    Cancelling,
    Succeeded,
    Failed,

    Stopped,

    Detached,
}

impl ActivityState {
    pub fn label(self) -> &'static str {
        match self {
            ActivityState::Queued => "queued",
            ActivityState::Running => "running",
            ActivityState::Cancelling => "cancelling",
            ActivityState::Succeeded => "succeeded",
            ActivityState::Failed => "failed",
            ActivityState::Stopped => "stopped",
            ActivityState::Detached => "detached",
        }
    }

    pub fn live(self) -> bool {
        matches!(
            self,
            ActivityState::Running
                | ActivityState::Detached
                | ActivityState::Queued
                | ActivityState::Cancelling
        )
    }
    pub fn finished(self) -> bool {
        !self.live()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityKind {
    Task,

    Logs { services: Vec<String> },

    Monitor { tool: String },

    Ssh { alias: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineTone {
    Normal,
    Muted,
    Warning,
    Error,
    Success,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineKind {
    Output,

    Prompt { secret: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptLine {
    pub at: u64,
    pub service: Option<String>,
    pub text: String,
    pub tone: LineTone,
    pub kind: LineKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    pub matches: Option<String>,

    pub lines: Vec<ScriptLine>,

    pub end: Option<(u64, i32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    pub lines: Vec<ScriptLine>,

    pub ends_at: Option<u64>,
    pub exit: i32,

    pub branches: Vec<Vec<Branch>>,

    pub resistant: bool,

    pub spawn_failure: Option<String>,
}

impl Script {
    pub fn running(lines: Vec<ScriptLine>) -> Self {
        Self {
            lines,
            ends_at: None,
            exit: 0,
            branches: vec![],
            resistant: false,
            spawn_failure: None,
        }
    }
    pub fn ending(lines: Vec<ScriptLine>, at: u64, exit: i32) -> Self {
        Self {
            lines,
            ends_at: Some(at),
            exit,
            branches: vec![],
            resistant: false,
            spawn_failure: None,
        }
    }

    pub fn spawn_failed(reason: &str) -> Self {
        Self {
            lines: vec![],
            ends_at: Some(0),
            exit: 127,
            branches: vec![],
            resistant: false,
            spawn_failure: Some(reason.into()),
        }
    }

    pub fn unmodeled(display: &str) -> Self {
        Self::ending(
            vec![
                line(0, &format!("$ {display}")),
                toned(
                    1,
                    &format!("holla: no simulated outcome for `{display}` in this preview"),
                    LineTone::Error,
                ),
            ],
            2,
            127,
        )
    }
    pub fn branches(mut self, b: Vec<Vec<Branch>>) -> Self {
        self.branches = b;
        self
    }
    pub fn resistant(mut self) -> Self {
        self.resistant = true;
        self
    }
}

pub fn line(at: u64, text: &str) -> ScriptLine {
    ScriptLine {
        at,
        service: None,
        text: text.into(),
        tone: LineTone::Normal,
        kind: LineKind::Output,
    }
}

pub fn toned(at: u64, text: &str, tone: LineTone) -> ScriptLine {
    ScriptLine {
        at,
        service: None,
        text: text.into(),
        tone,
        kind: LineKind::Output,
    }
}

pub fn service(at: u64, svc: &str, text: &str) -> ScriptLine {
    ScriptLine {
        at,
        service: Some(svc.into()),
        text: text.into(),
        tone: LineTone::Normal,
        kind: LineKind::Output,
    }
}

pub fn prompt(at: u64, text: &str, secret: bool) -> ScriptLine {
    ScriptLine {
        at,
        service: None,
        text: text.into(),
        tone: LineTone::Normal,
        kind: LineKind::Prompt { secret },
    }
}

pub fn normalize_line(raw: &[u8]) -> String {
    let raw = raw.strip_suffix(b"\n").unwrap_or(raw);
    let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
    String::from_utf8_lossy(raw).into_owned()
}

pub fn split_stream(raw: &[u8]) -> Vec<String> {
    let mut out = vec![];
    let mut start = 0;
    for (i, b) in raw.iter().enumerate() {
        if *b == b'\n' {
            out.push(normalize_line(&raw[start..=i]));
            start = i + 1;
        }
    }
    if start < raw.len() {
        out.push(normalize_line(&raw[start..]));
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputRecord {
    pub at: u64,
    pub bytes: Vec<u8>,
    pub secret: bool,
    pub redacted_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub text: String,
    pub secret: bool,
    pub since_tick: u64,

    pub index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelState {
    pub requested: u64,
    pub kill_sent: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputError {
    Finished,

    NotStarted,

    NoStdin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    pub id: String,
    pub name: String,

    pub origin: String,
    pub kind: ActivityKind,
    pub scope: ScopeTag,
    pub host: String,
    pub state: ActivityState,
    pub started_tick: u64,
    pub ended_tick: Option<u64>,
    pub exit: Option<i32>,

    pub output: Vec<(Option<String>, String, LineTone)>,

    pub dropped: usize,

    pub attachable: bool,
    pub script: Script,

    pub follow_ups: Vec<String>,

    pub insights: Vec<(String, String)>,

    pub hidden_services: Vec<String>,

    pub cursor: usize,

    pub effect: Option<Effect>,

    pub argv: Vec<Command>,

    pub after: Option<String>,
    pub batch: Option<String>,
    pub waiting: Option<Prompt>,

    pub paused_ticks: u64,
    pub stdin: Vec<InputRecord>,

    pub pending: Vec<u8>,

    echo_active: bool,
    pub cancel: Option<CancelState>,
    prompts_seen: usize,
}

impl Activity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: &str,
        name: &str,
        origin: &str,
        kind: ActivityKind,
        scope: ScopeTag,
        host: &str,
        script: Script,
        started_tick: u64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            origin: origin.into(),
            kind,
            scope,
            host: host.into(),
            state: ActivityState::Running,
            started_tick,
            ended_tick: None,
            exit: None,
            output: vec![],
            dropped: 0,
            attachable: false,
            script,
            follow_ups: vec![],
            insights: vec![],
            hidden_services: vec![],
            cursor: 0,
            effect: None,
            argv: vec![],
            after: None,
            batch: None,
            waiting: None,
            paused_ticks: 0,
            stdin: vec![],
            pending: vec![],
            echo_active: false,
            cancel: None,
            prompts_seen: 0,
        }
    }

    fn emit(&mut self, service: Option<String>, text: String, tone: LineTone) {
        self.echo_active = false;
        self.output.push((service, text, tone));
        if self.output.len() > RETAIN_LINES {
            let drop = self.output.len() - RETAIN_LINES;
            self.output.drain(..drop);
            self.dropped += drop;
        }
    }

    pub fn elapsed(&self, tick: u64) -> u64 {
        let wall = tick.saturating_sub(self.started_tick);
        let paused = self.paused_ticks
            + self
                .waiting
                .as_ref()
                .map(|p| tick.saturating_sub(p.since_tick))
                .unwrap_or(0);
        wall.saturating_sub(paused)
    }

    pub fn accepts_input(&self) -> bool {
        matches!(self.kind, ActivityKind::Task | ActivityKind::Ssh { .. })
            && matches!(self.state, ActivityState::Running)
    }

    pub fn advance(&mut self, tick: u64) -> bool {
        match self.state {
            ActivityState::Cancelling => return self.advance_cancel(tick),
            ActivityState::Running | ActivityState::Detached => {}
            _ => return false,
        }
        if let Some(reason) = self.script.spawn_failure.clone() {
            self.emit(
                None,
                format!("holla: cannot start: {reason}"),
                LineTone::Error,
            );
            self.state = ActivityState::Failed;
            self.exit = Some(127);
            self.ended_tick = Some(tick);
            self.script.spawn_failure = None;
            return true;
        }
        if self.waiting.is_some() {
            return false;
        }
        let elapsed = self.elapsed(tick);
        let mut changed = false;
        while let Some(l) = self.script.lines.get(self.cursor).cloned() {
            if l.at > elapsed {
                break;
            }
            self.cursor += 1;
            changed = true;
            match l.kind {
                LineKind::Output => self.emit(l.service, l.text, l.tone),
                LineKind::Prompt { secret } => {
                    self.emit(None, l.text.clone(), LineTone::Normal);
                    let index = self.prompts_seen;
                    self.prompts_seen += 1;
                    self.waiting = Some(Prompt {
                        text: l.text,
                        secret,
                        since_tick: tick,
                        index,
                    });
                    return true;
                }
            }
        }
        if let Some(end) = self.script.ends_at
            && elapsed >= end
            && self.state != ActivityState::Detached
        {
            while let Some(l) = self.script.lines.get(self.cursor).cloned() {
                self.cursor += 1;
                if let LineKind::Output = l.kind {
                    self.emit(l.service, l.text, l.tone);
                }
            }
            self.state = if self.script.exit == 0 {
                ActivityState::Succeeded
            } else {
                ActivityState::Failed
            };
            self.exit = Some(self.script.exit);
            self.ended_tick = Some(self.started_tick + end + self.paused_ticks);
            changed = true;
        }
        changed
    }

    fn advance_cancel(&mut self, tick: u64) -> bool {
        let Some(c) = self.cancel.clone() else {
            return false;
        };
        let since = tick.saturating_sub(c.requested);
        if !self.script.resistant {
            if since >= TERM_EXIT_TICKS {
                self.emit(
                    None,
                    "terminated (SIGTERM) · process group reaped".into(),
                    LineTone::Muted,
                );
                self.finish_cancel(tick, 130);
                return true;
            }
            return false;
        }
        if c.kill_sent.is_none() && since >= KILL_AFTER_TICKS {
            self.emit(
                None,
                "no exit after SIGTERM for 750 ms · SIGKILL to the process group".into(),
                LineTone::Warning,
            );
            self.cancel = Some(CancelState {
                requested: c.requested,
                kill_sent: Some(tick),
            });
            return true;
        }
        if let Some(k) = c.kill_sent
            && tick.saturating_sub(k) >= 2
        {
            self.emit(
                None,
                "killed (SIGKILL) · descendants reaped".into(),
                LineTone::Muted,
            );
            self.finish_cancel(tick, 137);
            return true;
        }
        false
    }

    fn finish_cancel(&mut self, tick: u64, exit: i32) {
        self.state = ActivityState::Stopped;
        self.ended_tick = Some(tick);
        self.exit = Some(exit);
        if let Some(p) = self.waiting.take() {
            self.paused_ticks += tick.saturating_sub(p.since_tick);
        }
    }

    pub fn stop(&mut self, tick: u64) {
        match self.state {
            ActivityState::Queued => {
                self.emit(None, "cancelled before start".into(), LineTone::Muted);
                self.started_tick = tick;
                self.finish_cancel(tick, 130);
            }
            ActivityState::Running | ActivityState::Detached => {
                self.state = ActivityState::Cancelling;
                self.cancel = Some(CancelState {
                    requested: tick,
                    kill_sent: None,
                });
                self.emit(
                    None,
                    "stop requested by holla · SIGTERM to the process group".into(),
                    LineTone::Muted,
                );
            }
            _ => {}
        }
    }

    pub fn start(&mut self, tick: u64) {
        if self.state == ActivityState::Queued {
            self.state = ActivityState::Running;
            self.started_tick = tick;
        }
    }

    pub fn restart(&mut self, tick: u64) {
        self.state = ActivityState::Running;
        self.started_tick = tick;
        self.ended_tick = None;
        self.exit = None;
        self.cursor = 0;
        self.output.clear();
        self.dropped = 0;
        self.waiting = None;
        self.paused_ticks = 0;
        self.stdin.clear();
        self.cancel = None;
        self.prompts_seen = 0;
    }

    pub fn send_input(&mut self, bytes: &[u8], tick: u64) -> Result<(), InputError> {
        match self.state {
            ActivityState::Queued => return Err(InputError::NotStarted),
            ActivityState::Running => {}
            ActivityState::Detached | ActivityState::Cancelling => return Err(InputError::NoStdin),
            _ => return Err(InputError::Finished),
        }
        if !matches!(self.kind, ActivityKind::Task | ActivityKind::Ssh { .. }) {
            return Err(InputError::NoStdin);
        }
        let secret = self.waiting.as_ref().is_some_and(|p| p.secret);
        self.stdin.push(InputRecord {
            at: tick,
            bytes: if secret { vec![] } else { bytes.to_vec() },
            secret,
            redacted_len: bytes.len(),
        });
        if bytes == [0x03] {
            self.pending.clear();
            self.emit(None, "^C".into(), LineTone::Muted);
            self.stop(tick);
            return Ok(());
        }
        if bytes == [0x04] {
            if self.waiting.is_some() {
                let pending = std::mem::take(&mut self.pending);
                let answer = if pending.is_empty() {
                    "\u{4}".to_owned()
                } else {
                    String::from_utf8_lossy(&pending).into_owned()
                };
                self.answer_prompt(answer, Some("^D"), tick);
            } else {
                self.echo_pending(Some("^D"));
                self.pending.clear();
                self.echo_active = false;
            }
            return Ok(());
        }
        let mut echo_dirty = false;
        for &b in bytes {
            match b {
                0x7f | 0x08 => {
                    echo_dirty |= erase_last_input(&mut self.pending);
                }
                b'\r' | b'\n' => {
                    echo_dirty = false;
                    if self.waiting.is_some() {
                        let line = String::from_utf8_lossy(&std::mem::take(&mut self.pending))
                            .into_owned();
                        self.answer_prompt(line, None, tick);
                    } else {
                        self.echo_pending(None);
                        self.pending.clear();
                        self.echo_active = false;
                    }
                }
                _ => {
                    self.pending.push(b);
                    echo_dirty = true;
                }
            }
        }
        if !self.pending.is_empty() || (echo_dirty && (self.echo_active || self.waiting.is_some()))
        {
            self.echo_pending(None);
        }
        Ok(())
    }

    fn echo_pending(&mut self, mark: Option<&str>) {
        let text = String::from_utf8_lossy(&self.pending).into_owned();
        if let Some(p) = self.waiting.clone() {
            let shown = if p.secret { String::new() } else { text };
            if let Some(last) = self.output.last_mut() {
                last.1 = format!("{}{shown}{}", p.text, mark.unwrap_or(""));
            }
            return;
        }
        let shown = format!("{text}{}", mark.unwrap_or(""));
        if self.echo_active
            && let Some(last) = self.output.last_mut()
        {
            last.1 = shown;
        } else {
            self.output.push((None, shown, LineTone::Muted));
            if self.output.len() > RETAIN_LINES {
                let drop = self.output.len() - RETAIN_LINES;
                self.output.drain(..drop);
                self.dropped += drop;
            }
            self.echo_active = true;
        }
    }

    fn answer_prompt(&mut self, answer: String, mark: Option<&str>, tick: u64) {
        let Some(p) = self.waiting.clone() else {
            return;
        };

        if let Some(last) = self.output.last_mut() {
            let shown = if p.secret || answer == "\u{4}" {
                String::new()
            } else {
                answer.clone()
            };
            last.1 = format!("{}{shown}{}", p.text, mark.unwrap_or(""));
        }
        self.paused_ticks += tick.saturating_sub(p.since_tick);
        self.waiting = None;
        let elapsed = self.elapsed(tick);
        let branches = self
            .script
            .branches
            .get(p.index)
            .cloned()
            .unwrap_or_default();
        let chosen = branches
            .iter()
            .find(|b| b.matches.as_deref() == Some(answer.as_str()))
            .or_else(|| branches.iter().find(|b| b.matches.is_none()))
            .cloned();
        if let Some(b) = chosen {
            let mut spliced: Vec<ScriptLine> = b
                .lines
                .iter()
                .map(|l| ScriptLine {
                    at: l.at + elapsed,
                    ..l.clone()
                })
                .collect();
            let rest = self.script.lines.split_off(self.cursor);
            self.script.lines.append(&mut spliced);
            if let Some((ticks, exit)) = b.end {
                // the program exits along this branch: the lines after the
                // prompt are never reached, so none of them is flushed as
                // final output
                self.script.ends_at = Some(elapsed + ticks);
                self.script.exit = exit;
            } else {
                self.script.lines.extend(rest);
            }
        }
    }

    /// Run time on the script clock: the time a prompt waited for the
    /// operator is not counted. The fixture clock advances with runtime
    /// ticks, which arrive on wall time, so a wait on the operator lasts as
    /// long as the operator (or a capture harness) takes to answer; counting
    /// it would make the same scenario and keys show a different duration
    /// from run to run.
    pub fn duration_ticks(&self, now: u64) -> u64 {
        match self.ended_tick {
            Some(end) => end
                .saturating_sub(self.started_tick)
                .saturating_sub(self.paused_ticks),
            None => self.elapsed(now),
        }
    }

    pub fn services(&self) -> Vec<String> {
        match &self.kind {
            ActivityKind::Logs { services } => services.clone(),
            _ => vec![],
        }
    }
}

/// Removes one user-perceived character from the end of `pending`. Returns
/// whether anything was removed. An incomplete trailing UTF-8 scalar goes
/// first, and a malformed byte goes one byte at a time.
fn erase_last_input(pending: &mut Vec<u8>) -> bool {
    if pending.is_empty() {
        return false;
    }
    let keep = match std::str::from_utf8(pending) {
        Ok(text) => text
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(offset, _)| offset),
        Err(error) if error.error_len().is_none() => error.valid_up_to(),
        Err(_) => pending.len() - 1,
    };
    pending.truncate(keep);
    true
}

pub fn key_bytes(key: &termrock::core::event::Key) -> Option<Vec<u8>> {
    use ratatui::crossterm::event::KeyCode;
    let ctrl = key.ctrl();
    Some(match key.code {
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Char(c) if ctrl => {
            let lower = c.to_ascii_lowercase();
            if lower.is_ascii_lowercase() {
                vec![(lower as u8) - b'a' + 1]
            } else if c == '@' || c == ' ' {
                vec![0]
            } else if c == '[' {
                vec![0x1b]
            } else {
                return None;
            }
        }
        KeyCode::Char(c) if !key.alt() => c.to_string().into_bytes(),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task() -> Activity {
        Activity::new(
            "act-test",
            "demo",
            "test",
            ActivityKind::Task,
            ScopeTag::here("/work"),
            "host",
            Script::running(vec![]),
            0,
        )
    }

    fn prompt(text: &str, secret: bool) -> Prompt {
        Prompt {
            text: text.into(),
            secret,
            since_tick: 0,
            index: 0,
        }
    }

    fn last_text(a: &Activity) -> &str {
        a.output.last().map_or("", |e| e.1.as_str())
    }

    fn texts(a: &Activity) -> Vec<&str> {
        a.output.iter().map(|e| e.1.as_str()).collect()
    }

    #[test]
    fn stdin_backspace_removes_unicode_grapheme() {
        let clusters = ["é", "e\u{301}", "中", "👍🏽", "👨\u{200d}👩\u{200d}👧"];
        for cluster in clusters {
            let mut a = task();
            a.send_input(format!("ab{cluster}").as_bytes(), 1).unwrap();
            a.send_input(&[0x7f], 2).unwrap();
            assert_eq!(a.pending, b"ab", "cluster {cluster:?} is erased whole");
            assert_eq!(last_text(&a), "ab");
        }
    }

    #[test]
    fn stdin_backspace_last_character_clears_echo() {
        let mut a = task();
        a.send_input(b"x", 1).unwrap();
        assert_eq!(last_text(&a), "x");
        a.send_input(&[0x7f], 2).unwrap();
        assert!(a.pending.is_empty());
        assert_eq!(texts(&a), vec![""], "the echo row is blank, not stale");
    }

    #[test]
    fn stdin_backspace_empty_is_noop() {
        let mut a = task();
        for tick in 1..4 {
            a.send_input(&[0x7f], tick).unwrap();
        }
        assert!(a.pending.is_empty());
        assert!(a.output.is_empty(), "no blank row for an untouched line");
    }

    #[test]
    fn stdin_backspace_fragmented_utf8_discards_incomplete_tail() {
        let mut a = task();
        a.send_input(&[b'a', 0xE4, 0xB8], 1).unwrap();
        a.send_input(&[0x7f], 2).unwrap();
        assert_eq!(a.pending, b"a", "the incomplete scalar goes, 'a' stays");

        a.send_input(&[0xE4, 0xB8, 0xAD], 3).unwrap();
        assert_eq!(last_text(&a), "a中");
    }

    #[test]
    fn stdin_backspace_malformed_byte_removes_one_byte() {
        let mut a = task();
        a.send_input(&[b'a', 0xFF, b'b'], 1).unwrap();
        a.send_input(&[0x7f], 2).unwrap();
        assert_eq!(a.pending, [b'a', 0xFF]);
    }

    #[test]
    fn stdin_secret_edit_never_displays_input() {
        let mut a = task();
        a.output.push((None, "Password: ".into(), LineTone::Normal));
        a.waiting = Some(prompt("Password: ", true));
        a.send_input(b"hunter2", 1).unwrap();
        a.send_input(&[0x7f, 0x7f], 2).unwrap();
        assert_eq!(a.pending, b"hunte");
        assert_eq!(last_text(&a), "Password: ");
        assert!(a.stdin.iter().all(|r| r.secret && r.bytes.is_empty()));
    }

    #[test]
    fn stdin_edit_submit_same_chunk_has_one_echo_per_line() {
        let mut a = task();
        a.send_input(b"x\ry", 1).unwrap();
        assert_eq!(texts(&a), vec!["x", "y"]);
        assert_eq!(a.pending, b"y");
    }

    #[test]
    fn stdin_empty_prompt_delete_keeps_prompt_text() {
        let mut a = task();
        a.output.push((None, "Name: ".into(), LineTone::Normal));
        a.waiting = Some(prompt("Name: ", false));
        a.send_input(b"a", 1).unwrap();
        assert_eq!(last_text(&a), "Name: a");
        a.send_input(&[0x7f], 2).unwrap();
        assert!(a.pending.is_empty());
        assert_eq!(last_text(&a), "Name: ");
    }
}
