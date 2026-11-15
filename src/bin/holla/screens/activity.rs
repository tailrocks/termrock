use crate::screens::Holla;
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::shell::Page as ShellPage;
use junie_tui::theme::Tone;
use junie_tui::ui::context::ContextAnchor;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::text::width;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::chips::{Chip, ChipAction, ChipBar};
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::menu::MenuItem;
use junie_tui::widgets::page_header::PageHeader;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::statusbar::StatusItem;
use junie_tui::widgets::viewport::{Line, Mark, Span, TextViewport, ViewportAction, line_text};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use crate::domain::activity::{
    ActivityKind, ActivityState, InputError, KILL_AFTER_TICKS, LineTone, RETAIN_LINES, key_bytes,
};
use crate::screens::{Cx, Go, ModalResult, Screen, StatusBits, open_context, ticks_label};
use crate::sim::world::{STOPPING_REFUSAL, World};
use junie_tui::ui::layer::LayerTag;

pub const VIEW: WidgetId = WidgetId::of("activity.view");
pub const CHIPS: WidgetId = WidgetId::of("activity.chips");
const FOLLOW_UP: WidgetId = WidgetId::of("activity.follow");

pub const MENU: WidgetId = WidgetId::of("activity.menu");

pub struct ActivityTab {
    pub id: String,
    view: TextViewport,
    chips: ChipBar,
    pub attached: bool,

    pub input: bool,

    synced: usize,
    synced_dropped: usize,
    synced_hidden: Vec<String>,

    synced_last: Option<String>,
    synced_services: Vec<String>,
    follow_ups: Vec<Button>,
    find_query: Option<String>,
    find_matches: Vec<(usize, std::ops::Range<usize>)>,
    find_at: usize,
}

impl ActivityTab {
    pub fn new(id: &str) -> Self {
        let mut chips = ChipBar::new(CHIPS);
        chips.set_add_label(None);
        Self {
            id: id.into(),
            view: TextViewport::new(VIEW).max_lines(RETAIN_LINES).wrap(true),
            chips,
            attached: false,
            input: false,
            synced: 0,
            synced_dropped: 0,
            synced_hidden: vec![],
            synced_last: None,
            synced_services: vec![],
            follow_ups: vec![],
            find_query: None,
            find_matches: vec![],
            find_at: 0,
        }
    }

    fn line_of(svc_w: usize, entry: &(Option<String>, String, LineTone)) -> Line {
        let (svc, text, tone) = entry;
        let mut line: Line = vec![];
        if let Some(s) = svc {
            line.push(Span::new(format!("{:<w$}  ", s, w = svc_w), Tone::Secondary).bold());
        }

        if *tone == LineTone::Error {
            line.push(Span::new("! ", Tone::Error).bold());
        }
        let tone = match tone {
            LineTone::Normal | LineTone::Error => Tone::Normal,
            LineTone::Muted => Tone::Muted,
            LineTone::Warning => Tone::Warning,
            LineTone::Success => Tone::Success,
        };
        line.push(Span::new(text.clone(), tone));
        line
    }

    fn sync(&mut self, w: &World) {
        let Some(a) = w.activity(&self.id) else {
            return;
        };
        if self.input && !a.accepts_input() {
            self.input = false;
        }
        let services = a.services();
        if self.chips.chips().len() != services.len() || self.synced_services != services {
            let chips = services
                .iter()
                .map(|s| {
                    let mut c = Chip::new(s);
                    c.removable = false;
                    c.enabled = !a.hidden_services.contains(s);
                    c
                })
                .collect();
            self.chips.set_chips(chips);
            self.synced_services = services.clone();
        } else {
            for (i, s) in services.iter().enumerate() {
                let enabled = !a.hidden_services.contains(s);
                self.chips.update_chip(i, |c| c.enabled = enabled);
            }
        }
        let svc_w = services.iter().map(|s| width(s)).max().unwrap_or(0);
        let visible = |e: &(Option<String>, String, LineTone)| {
            e.0.as_ref().is_none_or(|s| !a.hidden_services.contains(s))
        };
        let mut changed = false;

        let delta = a.dropped.saturating_sub(self.synced_dropped);
        let rebuild = self.synced_hidden != a.hidden_services
            || a.output.len() + delta < self.synced
            || a.dropped < self.synced_dropped;
        if !rebuild && delta > 0 {
            self.synced = self.synced.saturating_sub(delta);
            self.synced_dropped = a.dropped;
        }
        if rebuild {
            let follow = self.view.follow;
            self.view.set_lines(
                a.output
                    .iter()
                    .filter(|e| visible(e))
                    .map(|e| Self::line_of(svc_w, e))
                    .collect(),
            );
            self.view.follow = follow;
            self.synced = a.output.len();
            self.synced_hidden = a.hidden_services.clone();
            self.synced_dropped = a.dropped;
            changed = true;
        } else {
            if self.synced > 0
                && let Some(last) = a.output.get(self.synced - 1)
                && self.synced_last.as_deref() != Some(last.1.as_str())
                && visible(last)
            {
                self.view.replace_last(Self::line_of(svc_w, last));
                changed = true;
            }
            for e in &a.output[self.synced..] {
                if visible(e) {
                    self.view.push(Self::line_of(svc_w, e));
                    changed = true;
                }
            }
            self.synced = a.output.len();
        }
        self.synced_last = a.output.last().map(|e| e.1.clone());
        if changed && self.find_query.is_some() {
            self.run_find();
        }
        if self.follow_ups.len() != a.follow_ups.len() {
            self.follow_ups = a
                .follow_ups
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    Button::subtle(FOLLOW_UP.child(i), f).disabled(!follow_up_supported(f))
                })
                .collect();
        }
    }

    fn run_find(&mut self) {
        let q = self.find_query.clone().unwrap_or_default();
        let before = self.find_matches.get(self.find_at).cloned();
        self.find_matches.clear();
        if q.is_empty() {
            self.view.clear_marks();
            return;
        }
        let texts: Vec<String> = self.view.lines().map(|l| line_text(l)).collect();
        for (i, l) in texts.iter().enumerate() {
            for r in junie_tui::ui::text::find_ranges(l, &q, false) {
                self.find_matches.push((i, r));
            }
        }

        self.find_at = before
            .and_then(|b| self.find_matches.iter().position(|m| *m == b))
            .unwrap_or(0);
        self.apply_marks();
    }

    fn apply_marks(&mut self) {
        let marks: Vec<Mark> = self
            .find_matches
            .iter()
            .enumerate()
            .map(|(k, (line, r))| Mark {
                line: *line,
                range: r.clone(),
                current: k == self.find_at,
            })
            .collect();
        self.view.set_marks(marks);
        if let Some((line, _)) = self.find_matches.get(self.find_at) {
            self.view.set_follow(false);
            self.view.reveal_line(*line);
        }
    }

    fn send(&mut self, bytes: &[u8], w: &mut World, cx: &mut Cx) -> Outcome {
        let tick = w.tick;
        let Some(sent) = w.edit_activity(&self.id, |a| a.send_input(bytes, tick)) else {
            return Outcome::Ignored;
        };
        match sent {
            Ok(()) => {
                if bytes == [0x03] {
                    cx.status("Sent ^C · stopping · SIGKILL follows if it lingers");
                    self.input = false;
                } else if bytes == [0x04] {
                    cx.status("Sent end of input");
                }
                Outcome::Changed
            }
            Err(e) => {
                self.input = false;
                cx.status(match e {
                    InputError::Finished => "The program has finished · nothing reads input",
                    InputError::NotStarted => "Queued · it has no stdin until it starts",
                    InputError::NoStdin => "No stdin · a detached monitor takes no input",
                });
                Outcome::Changed
            }
        }
    }

    fn toggle_stream(&mut self, name: String, w: &mut World) {
        w.edit_activity(&self.id, |a| {
            if a.hidden_services.contains(&name) {
                a.hidden_services.retain(|s| s != &name);
            } else {
                a.hidden_services.push(name);
            }
        });
    }

    fn follow_up(&self, label: &str, w: &World, cx: &mut Cx) {
        let l = label.to_lowercase();
        if l.starts_with("find the process on port") {
            let port = l.rsplit(' ').next().unwrap_or("5173").to_owned();
            cx.go(Go::Run {
                item: "system.port".into(),
                args: vec![("port".into(), port)],
            });
        } else if l.starts_with("retry") {
            cx.go(Go::Restart(self.id.clone()));
        } else if l.starts_with("open the mise") {
            cx.go(Go::Run {
                item: "file.mise".into(),
                args: vec![],
            });
        } else if l.starts_with("run tests again") {
            let id = w
                .items()
                .into_iter()
                .find(|i| i.id.ends_with(":test") || i.id == "mise.task.test")
                .map(|i| i.id);
            match id {
                Some(id) => cx.go(Go::Run {
                    item: id,
                    args: vec![],
                }),
                None => cx.status("No test task here"),
            }
        } else if l.starts_with("review") {
            cx.go(Go::Run {
                item: "git.review".into(),
                args: vec![],
            });
        } else if l.starts_with("upgrade mise") || l.starts_with("mise upgrade") {
            cx.go(Go::Run {
                item: "upgrade.mise".into(),
                args: vec![],
            });
        } else if let Some(rest) = l.strip_prefix("run ") {
            let id = w
                .items()
                .into_iter()
                .find(|i| i.label.to_lowercase() == rest || i.id == rest)
                .map(|i| i.id);
            match id {
                Some(id) => cx.go(Go::Run {
                    item: id,
                    args: vec![],
                }),
                None => cx.status(format!("{label}: no such action here")),
            }
        }
    }
}

impl ActivityTab {
    fn output_menu(&self, anchor: ContextAnchor, w: &World, cx: &mut Cx) -> Outcome {
        let Some(a) = w.activity(&self.id) else {
            return Outcome::Ignored;
        };
        let mut copy = MenuItem::new("Copy selection").shortcut("y");
        if self.view.selected_text().is_none() {
            copy = copy.reason("Select text first");
        }
        let follow = MenuItem::new(if self.view.follow {
            "Pause following"
        } else {
            "Follow the tail"
        })
        .shortcut("f")
        .separator();
        let mut items = vec![copy, MenuItem::new("Find…").shortcut("/"), follow];
        let mut keys = String::from("y/f");
        if a.attachable && a.state.live() {
            items.push(MenuItem::new("Attach").shortcut("Enter"));
            keys.push('\n');
        } else {
            let mut input = MenuItem::new(if a.waiting.is_some() {
                "Answer"
            } else {
                "Type input"
            })
            .shortcut("i");
            if !a.accepts_input() {
                input = input.reason(match a.state {
                    ActivityState::Queued => "Queued · it has no stdin until it starts",
                    s if s.live() => "This program takes no input",
                    _ => "The program has finished · nothing reads input",
                });
            }
            items.push(input);
            keys.push('i');
        }
        let mut stop = MenuItem::new("Stop").shortcut("s").danger();
        match a.state {
            ActivityState::Cancelling => stop = stop.reason("Already stopping"),
            s if !s.live() => stop = stop.reason("Already finished"),
            _ => {}
        }
        let mut restart = MenuItem::new("Restart").shortcut("r").separator();
        if a.state.live() {
            restart = restart.reason("Still running · s stops it first");
        }
        items.extend([stop, restart, MenuItem::new("Close tab").shortcut("x")]);
        keys.push_str("srx");
        open_context(
            cx,
            MENU,
            anchor,
            a.name.clone(),
            items,
            LayerTag::new("activity-output").key(keys),
        )
    }

    fn chip_menu(&self, i: usize, anchor: ContextAnchor, w: &World, cx: &mut Cx) -> Outcome {
        let Some(chip) = self.chips.chips().get(i) else {
            return Outcome::Ignored;
        };
        let hidden = w
            .activity(&self.id)
            .is_some_and(|a| a.hidden_services.contains(&chip.label));
        let items = vec![
            MenuItem::new(if hidden { "Show stream" } else { "Hide stream" }).shortcut("Space"),
        ];
        open_context(
            cx,
            MENU,
            anchor,
            chip.label.clone(),
            items,
            LayerTag::new("activity-chip").key(chip.label.clone()),
        )
    }
}

impl ShellPage<Holla> for ActivityTab {
    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        let tick = w.tick;
        if self.attached {
            if key.ctrl_char(']') {
                self.attached = false;
                cx.status("Detached · the program keeps running in its tab");
                return Outcome::Changed;
            }
            if key.is_char('q') {
                w.edit_activity(&self.id, |a| {
                    a.state = ActivityState::Stopped;
                    a.ended_tick = Some(tick);
                    a.exit = Some(0);
                    a.output
                        .push((None, "program exited".into(), LineTone::Muted));
                });
                self.attached = false;
                cx.status("Program exited · tab kept with its last screen");
                return Outcome::Changed;
            }
            return Outcome::Consumed;
        }
        if self.input && !w.activity(&self.id).is_some_and(|a| a.accepts_input()) {
            self.input = false;
        }
        if self.input {
            if key.code == KeyCode::Esc && key.plain() {
                self.input = false;
                cx.status("Keyboard returned to holla · the program keeps running");
                return Outcome::Changed;
            }
            if key.alt() {
                return Outcome::Ignored;
            }
            return match key_bytes(key) {
                Some(bytes) => self.send(&bytes, w, cx),
                None => Outcome::Consumed,
            };
        }
        if let Some(q) = self.find_query.clone() {
            match key.code {
                KeyCode::Esc => {
                    self.find_query = None;
                    self.find_matches.clear();
                    self.view.clear_marks();
                    return Outcome::Changed;
                }
                KeyCode::Enter | KeyCode::Down if !self.find_matches.is_empty() => {
                    self.find_at = (self.find_at + 1) % self.find_matches.len();
                    self.apply_marks();
                    return Outcome::Changed;
                }
                KeyCode::Up if !self.find_matches.is_empty() => {
                    self.find_at =
                        (self.find_at + self.find_matches.len() - 1) % self.find_matches.len();
                    self.apply_marks();
                    return Outcome::Changed;
                }
                KeyCode::Enter | KeyCode::Down | KeyCode::Up => return Outcome::Consumed,
                KeyCode::Backspace => {
                    let mut q = q;
                    q.pop();
                    self.find_query = Some(q);
                    self.run_find();
                    return Outcome::Changed;
                }
                KeyCode::Char(c) if !key.chord() => {
                    let mut q = q;
                    q.push(c);
                    self.find_query = Some(q);
                    self.run_find();
                    return Outcome::Changed;
                }
                _ => {}
            }
        }

        for i in 0..self.follow_ups.len() {
            if cx.focus.is(self.follow_ups[i].id) {
                let (o, fired) = self.follow_ups[i].on_key(key);
                if fired {
                    let label = self.follow_ups[i].label.clone();
                    self.follow_up(&label, w, cx);
                    return Outcome::Changed;
                }
                if o.consumed() {
                    return o;
                }
            }
        }
        if cx.focus.is(CHIPS) {
            let (o, ev) = self.chips.on_key(key);
            match ev {
                Some(ChipAction::Toggle(i)) | Some(ChipAction::Activate(i)) => {
                    let name = self.chips.chips()[i].label.clone();
                    self.toggle_stream(name, w);
                    return Outcome::Changed;
                }
                _ => {}
            }
            if o.consumed() {
                return o;
            }
        }
        let (attachable, accepts_input, state) = w
            .activity(&self.id)
            .map(|a| (a.attachable && a.state.live(), a.accepts_input(), a.state))
            .unwrap_or((false, false, ActivityState::Failed));
        match key.code {
            KeyCode::Enter if attachable && key.plain() => {
                self.attached = true;
                let tool = w
                    .activity(&self.id)
                    .map(|a| a.name.clone())
                    .unwrap_or_default();
                if w.activity(&self.id)
                    .is_some_and(|a| a.state == ActivityState::Detached)
                {
                    w.edit_activity(&self.id, |a| a.state = ActivityState::Running);
                }
                cx.status(format!("Attached to {tool}"));
                return Outcome::Changed;
            }
            KeyCode::Char('i') if key.plain() => {
                if attachable {
                    self.attached = true;
                    cx.status("Attached");
                } else if accepts_input {
                    self.input = true;
                    self.view.set_follow(true);
                    cx.status("Typing goes to the program");
                } else {
                    cx.status(match state {
                        ActivityState::Queued => "Queued · it has no stdin until it starts",
                        s if s.live() => "This program takes no input",
                        _ => "The program has finished · nothing reads input",
                    });
                }
                return Outcome::Changed;
            }
            KeyCode::Char('/') if key.plain() => {
                self.find_query = Some(String::new());
                cx.focus.focus(VIEW);
                return Outcome::Changed;
            }
            KeyCode::Char('s') if key.plain() => {
                w.edit_activity(&self.id, |a| match a.state {
                    ActivityState::Cancelling => {
                        let since = a
                            .cancel
                            .as_ref()
                            .map(|c| tick.saturating_sub(c.requested))
                            .unwrap_or(0);
                        cx.status(format!(
                            "Already stopping · SIGTERM sent · SIGKILL in {}",
                            ticks_label(KILL_AFTER_TICKS.saturating_sub(since))
                        ));
                    }
                    s if s.live() => {
                        let name = a.name.clone();
                        a.stop(tick);
                        cx.status(format!("Stopping {name} · SIGTERM sent · output kept"));
                    }
                    _ => cx.status("Already finished"),
                });
                self.input = false;
                return Outcome::Changed;
            }
            KeyCode::Char('r') if key.plain() => {
                if w.stopping() {
                    cx.status(STOPPING_REFUSAL);
                    return Outcome::Changed;
                }
                if w.activity(&self.id).is_some_and(|a| a.state.live()) {
                    cx.status("Still running · s stops it first");
                    return Outcome::Changed;
                }
                w.edit_activity(&self.id, |a| {
                    a.restart(tick);
                    a.advance(tick);
                    cx.status(format!("Restarted {}", a.name));
                });
                self.synced = 0;
                self.synced_dropped = 0;
                self.synced_last = None;
                self.input = false;
                self.view.clear();
                self.find_matches.clear();
                return Outcome::Changed;
            }
            KeyCode::Char('x') if key.plain() => {
                cx.go(Go::CloseTab);
                return Outcome::Changed;
            }
            KeyCode::Char(c) if key.plain() && c.is_ascii_digit() && c != '0' => {
                let i = c as usize - '1' as usize;
                let name = w
                    .activity(&self.id)
                    .map(|a| a.services())
                    .unwrap_or_default()
                    .get(i)
                    .cloned();
                if let Some(name) = name {
                    self.toggle_stream(name, w);
                    return Outcome::Changed;
                }
                return Outcome::Ignored;
            }
            _ => {}
        }
        if cx.focus.is(VIEW) {
            let (o, ev) = self.view.on_key(key);
            match ev {
                Some(ViewportAction::Copy(text)) => cx.copy(text),
                Some(ViewportAction::FollowChanged(f)) => cx.status(if f {
                    "Following the tail"
                } else {
                    "Follow paused · End resumes"
                }),
                _ => {}
            }
            return o;
        }
        Outcome::Ignored
    }

    fn on_paste(&mut self, text: &str, w: &mut World, _cx: &mut Cx) -> Outcome {
        if self.input {
            let tick = w.tick;
            w.edit_activity(&self.id, |a| {
                let _ = a.send_input(text.as_bytes(), tick);
            });
            return Outcome::Changed;
        }
        if let Some(q) = self.find_query.as_mut() {
            q.push_str(text.trim());
            self.run_find();
            return Outcome::Changed;
        }
        Outcome::Consumed
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        let (id, pos) = (ev.id, ev.pos());
        match ev.phase() {
            Some(Phase::Click) => {
                if let Some(r) = self.chips.route(ev) {
                    let (o, actions) = r.into_parts();
                    for act in actions {
                        if let ChipAction::Activate(i) | ChipAction::Toggle(i) = act {
                            let name = self.chips.chips()[i].label.clone();
                            self.toggle_stream(name, w);
                        }
                    }
                    cx.focus.focus(CHIPS);
                    return o.or(Outcome::Changed);
                }
                for i in 0..self.follow_ups.len() {
                    if self.follow_ups[i].id == id {
                        cx.focus.focus(id);
                        if self.follow_ups[i].on_click() {
                            let label = self.follow_ups[i].label.clone();
                            self.follow_up(&label, w, cx);
                        }
                        return Outcome::Changed;
                    }
                }
                if id == VIEW {
                    cx.focus.focus(VIEW);
                }
                self.view
                    .route(ev)
                    .map_or(Outcome::Ignored, |r| r.outcome())
            }
            Some(Phase::Secondary) => {
                if self.is_editing() {
                    return Outcome::Ignored;
                }
                if let Some(i) =
                    (0..self.chips.chips().len()).find(|&i| self.chips.chip_id(i) == id)
                {
                    if cx.context.moves_cursor {
                        self.chips.cursor = i;
                    }
                    cx.focus.focus(CHIPS);
                    return self.chip_menu(i, ContextAnchor::Pointer(pos), w, cx);
                }
                if id == VIEW {
                    cx.focus.focus(VIEW);
                    return self.output_menu(ContextAnchor::Pointer(pos), w, cx);
                }
                Outcome::Ignored
            }

            _ => self
                .view
                .route(ev)
                .map_or(Outcome::Ignored, |r| r.outcome()),
        }
    }

    fn on_layer_result(
        &mut self,
        tag: &LayerTag,
        result: ModalResult,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        let ModalResult::MenuChosen(k) = result else {
            return Outcome::Changed;
        };
        match tag.kind {
            "activity-chip" => {
                self.toggle_stream(tag.key.clone(), w);
                Outcome::Changed
            }
            "activity-output" => {
                let Some(c) = tag.key.chars().nth(k) else {
                    return Outcome::Changed;
                };

                cx.focus.focus(VIEW);
                let key = Key {
                    code: if c == '\n' {
                        KeyCode::Enter
                    } else {
                        KeyCode::Char(c)
                    },
                    mods: KeyModifiers::NONE,
                };
                self.on_key(&key, w, cx).or(Outcome::Changed)
            }
            _ => Outcome::Ignored,
        }
    }

    fn on_tick(&mut self, w: &mut World, _cx: &mut Cx) -> Outcome {
        let before = (self.synced, self.view.revision());
        self.sync(w);
        if before != (self.synced, self.view.revision()) {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn enter(&mut self, w: &mut World, _cx: &mut Cx) {
        self.sync(w);
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        self.sync(w);
        let t = ctx.theme;
        let Some(a) = w.activity(&self.id) else {
            junie_tui::widgets::empty::render(
                area,
                buf,
                t,
                &junie_tui::widgets::empty::EmptyState::new("This activity is gone")
                    .hint("Esc returns to Here"),
                t.canvas,
            );
            return;
        };

        let state_tone = match a.state {
            ActivityState::Failed => Tone::Error,
            ActivityState::Succeeded => Tone::Secondary,
            ActivityState::Cancelling => Tone::Warning,
            ActivityState::Stopped | ActivityState::Detached | ActivityState::Queued => Tone::Muted,
            _ => Tone::Secondary,
        };
        let elapsed = ticks_label(a.duration_ticks(w.tick));
        let mut head = format!(" · {} · {elapsed}", a.state.label());
        if a.state == ActivityState::Queued
            && let Some(after) = &a.after
        {
            let name = w
                .activity(after)
                .map(|p| p.name.clone())
                .unwrap_or(after.clone());
            head.push_str(&format!(" · after {name}"));
        }
        if let Some(e) = a.exit {
            head.push_str(&format!(" · exit {e}"));
        }
        if a.waiting.is_some() {
            head.push_str(" · waiting for input");
        }
        if self.attached {
            head.push_str(" · attached");
        }

        let right = if a.scope.direction == crate::domain::context::Scope::System {
            a.host.clone()
        } else {
            format!(
                "{} · {} · {}",
                a.scope.word,
                w.location.short(&a.scope.runs_in),
                a.host
            )
        };
        let status_tone = if a.waiting.is_some() {
            Tone::Warning
        } else {
            state_tone
        };

        let mut insight = if a.insights.is_empty() {
            format!("started by {}", a.origin)
        } else {
            a.insights
                .iter()
                .map(|(k, v)| format!("{k} {v}"))
                .collect::<Vec<_>>()
                .join(" · ")
        };
        if let Some(b) = &a.batch {
            insight = format!("{insight} · batch {b}");
        }
        if !a.argv.is_empty() {
            insight = format!(
                "$ {} · {insight}",
                a.argv
                    .iter()
                    .map(|c| c.display())
                    .collect::<Vec<_>>()
                    .join(" && ")
            );
        }
        PageHeader::new(&a.name)
            .status(&head, status_tone)
            .right(&right)
            .right_gap(6)
            .subtitle(&insight, Tone::Muted)
            .render(area, buf, t);
        let mut y = area.y + 3;
        if !self.chips.chips().is_empty() {
            self.chips.render(
                Rect::new(area.x + 1, y, area.width.saturating_sub(2), 1),
                buf,
                ctx,
                t.canvas,
            );
            y += 2;
        }

        let follow_w: Vec<u16> = self.follow_ups.iter().map(Button::width).collect();
        let follow_area = Rect::new(area.x + 7, 0, area.width.saturating_sub(7), 1);
        let follow_h: u16 = if !self.follow_ups.is_empty() && !a.state.live() {
            1 + junie_tui::widgets::button::row_lines(follow_area.width, &follow_w, 2)
        } else {
            0
        };
        let pane = Rect::new(
            area.x,
            y,
            area.width,
            area.bottom().saturating_sub(y + follow_h),
        );
        let focused = ctx.interaction.focused(VIEW);
        let mut meta = if self.attached {
            "keys go to the program".to_owned()
        } else if self.input {
            "typing goes to stdin · Esc returns".to_owned()
        } else if let Some(q) = &self.find_query {
            format!(
                "find “{q}” · {} of {}",
                if self.find_matches.is_empty() {
                    0
                } else {
                    self.find_at + 1
                },
                self.find_matches.len()
            )
        } else if self.view.is_at_tail() || self.view.follow {
            "following".to_owned()
        } else {
            format!("▲ {} · f follows", self.view.scrollback_depth())
        };
        if a.dropped > 0 {
            meta = format!(
                "{meta} · {} earlier lines dropped · last {} kept",
                a.dropped, RETAIN_LINES
            );
        }
        let title = match &a.kind {
            ActivityKind::Logs { .. } => "merged logs".to_owned(),
            ActivityKind::Monitor { tool } => format!("{tool} screen"),
            ActivityKind::Ssh { alias } => format!("ssh {alias}"),
            _ => "output".to_owned(),
        };
        let panel = Panel::framed(Some(&title))
            .focused(focused || self.input)
            .meta(&meta);
        let inner = panel.render(pane, buf, t);
        self.view.render(inner, buf, ctx, t.canvas);
        if follow_h > 0 {
            let y = area.bottom().saturating_sub(follow_h - 1);
            buf.set_string(area.x + 1, y, "Next", t.faint());
            let rects = junie_tui::widgets::button::row_layout(
                Rect::new(follow_area.x, y, follow_area.width, follow_h - 1),
                &follow_w,
                2,
            );
            for (b, r) in self.follow_ups.iter_mut().zip(rects) {
                b.render(r, buf, ctx, t.canvas);
            }
        }
        if self.attached {
            ctx.cursor = None;
        }
    }

    fn hints(&self, focus: Option<WidgetId>, w: &World) -> Vec<Hint> {
        if self.attached {
            return vec![hint("Ctrl+]", "Detach"), hint("q", "Exit the program")];
        }
        if self.input {
            return vec![
                hint("Type", "To stdin"),
                hint("Enter", "Send line"),
                hint("Ctrl+D", "End input"),
                hint("Ctrl+C", "Stop"),
                hint("Esc", "Release keyboard"),
            ];
        }
        if self.find_query.is_some() {
            return vec![
                hint("Type", "Find"),
                hint("Enter / ↓", "Next"),
                hint("↑", "Previous"),
                hint("Esc", "Close find"),
            ];
        }
        let a = w.activity(&self.id);
        let live = a.is_some_and(|a| a.state.live());
        let attachable = a.is_some_and(|a| a.attachable && a.state.live());
        let accepts = a.is_some_and(|a| a.accepts_input());
        let waiting = a.is_some_and(|a| a.waiting.is_some());
        let mut v = vec![];
        if focus == Some(CHIPS) {
            v.push(hint("←→", "Stream"));
            v.push(hint("Space", "Show / hide"));
        } else {
            v.push(hint("↑↓", "Scroll"));
            v.push(hint("f", "Follow"));
            v.push(hint("/", "Find"));
            v.push(hint("y", "Copy"));
        }
        if attachable {
            v.push(hint("Enter", "Attach"));
        } else if accepts {
            v.push(hint("i", if waiting { "Answer" } else { "Type input" }));
        }
        if live {
            v.push(hint("s", "Stop"));
        } else {
            v.push(hint("r", "Restart"));
        }
        v.push(hint("x", "Close"));
        v.push(hint("Esc", "Back to Here"));
        v.push(junie_tui::widgets::keyhint::CONTEXT_MENU);
        v
    }

    fn crumb(&self, w: &World) -> String {
        w.activity(&self.id)
            .map(|a| a.name.clone())
            .unwrap_or_default()
    }

    fn is_editing(&self) -> bool {
        self.attached || self.input || self.find_query.is_some()
    }

    fn animating(&self, w: &World) -> bool {
        w.activity(&self.id).is_some_and(|a| a.state.live())
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(VIEW)
    }

    fn on_esc_top(&mut self, _w: &mut World, cx: &mut Cx) -> Outcome {
        cx.go(Go::Here);
        Outcome::Changed
    }
}

impl Screen for ActivityTab {
    fn on_context_key(&mut self, w: &mut World, cx: &mut Cx) -> Outcome {
        if self.is_editing() {
            return Outcome::Ignored;
        }
        let top = Rect::new(self.view.area.x, self.view.area.y, 1, 1);
        if cx.focus.is(CHIPS) {
            let i = self.chips.cursor;
            let anchor = ContextAnchor::Control {
                id: self.chips.chip_id(i),
                fallback: top,
            };
            return self.chip_menu(i, anchor, w, cx);
        }
        if cx.focus.is(VIEW) {
            let at = self.view.context_point();
            return self.output_menu(ContextAnchor::Pointer(at), w, cx);
        }
        Outcome::Ignored
    }

    fn status(&self, w: &World) -> StatusBits {
        let mut bits = StatusBits::default();

        if let Some(a) = w.activity(&self.id) {
            let elapsed = ticks_label(a.duration_ticks(w.tick));
            bits.center = Some(if a.waiting.is_some() {
                StatusItem::new(format!("? input wanted · {elapsed}"), Tone::Warning).priority(7)
            } else {
                match a.state {
                    ActivityState::Running | ActivityState::Detached => {
                        StatusItem::new(elapsed, Tone::Secondary).busy().priority(6)
                    }
                    ActivityState::Cancelling => {
                        StatusItem::new(format!("stopping · {elapsed}"), Tone::Warning)
                            .busy()
                            .priority(6)
                    }
                    ActivityState::Failed => StatusItem::new(
                        format!("! exit {} · {elapsed}", a.exit.unwrap_or(1)),
                        Tone::Error,
                    )
                    .priority(6),
                    _ => StatusItem::new(format!("{} · {elapsed}", a.state.label()), Tone::Muted)
                        .priority(6),
                }
            });
        }
        bits
    }
}

const FOLLOW_UPS: [&str; 8] = [
    "find the process on port",
    "retry",
    "open the mise",
    "run tests again",
    "review",
    "upgrade mise",
    "mise upgrade",
    "run ",
];

fn follow_up_supported(label: &str) -> bool {
    let l = label.to_lowercase();
    FOLLOW_UPS.iter().any(|p| l.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::activity::{Activity, ActivityKind, Script, line};
    use crate::domain::context::ScopeTag;
    use crate::domain::fixtures::world_for;
    use crate::scenario::{Motion, Scenario};
    use junie_tui::core::focus::{Focus, FocusRing};
    use junie_tui::core::hit::HitRegistry;
    use junie_tui::theme::Theme;
    use junie_tui::ui::ctx::Interaction;

    fn press(tab: &mut ActivityTab, w: &mut World, cx: &mut Cx<'_>, code: KeyCode) {
        let key = Key {
            code,
            mods: KeyModifiers::NONE,
        };
        tab.on_key(&key, w, cx);
        tab.sync(w);
    }

    /// A typing tab over a running activity, so keys reach its simulated stdin.
    fn typing_tab() -> (World, ActivityTab) {
        let mut w = world_for(Scenario::DockerCleanup, Motion::Full);
        let tick = w.tick;
        w.activities.push(Activity::new(
            "act-test",
            "demo",
            "test",
            ActivityKind::Task,
            ScopeTag::here("/work"),
            "host",
            Script::running(vec![]),
            tick,
        ));
        let mut tab = ActivityTab::new("act-test");
        tab.input = true;
        (w, tab)
    }

    /// Renders the tab through its real `Screen::render` path into a cell buffer.
    fn render_tab(tab: &mut ActivityTab, w: &World) -> Buffer {
        let area = Rect::new(0, 0, 80, 24);
        let theme = Theme::junie();
        let mut buf = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        tab.render(area, &mut buf, &mut ctx, w);
        buf
    }

    fn cells_showing(buf: &Buffer, symbol: &str) -> Vec<(u16, u16)> {
        let area = buf.area;
        (area.y..area.bottom())
            .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
            .filter(|&(x, y)| buf[(x, y)].symbol() == symbol)
            .collect()
    }

    #[test]
    fn activity_tab_unicode_erase_updates_rendered_cells() {
        let (mut w, mut tab) = typing_tab();
        let mut focus = Focus::default();
        let ring = FocusRing::default();
        let mut cx = Cx::new(&mut focus, &ring);

        press(&mut tab, &mut w, &mut cx, KeyCode::Char('a'));
        press(&mut tab, &mut w, &mut cx, KeyCode::Char('é'));
        let buf = render_tab(&mut tab, &w);
        let glyph = cells_showing(&buf, "é");
        assert_eq!(glyph.len(), 1, "é is drawn once on the echo row");
        let (x, y) = glyph[0];
        assert_eq!(buf[(x - 1, y)].symbol(), "a", "a is drawn before é");

        press(&mut tab, &mut w, &mut cx, KeyCode::Backspace);
        let buf = render_tab(&mut tab, &w);
        assert_eq!(buf[(x, y)].symbol(), " ", "the é cell is blank");
        assert_eq!(buf[(x - 1, y)].symbol(), "a", "a stays after one Backspace");
        assert!(cells_showing(&buf, "é").is_empty(), "no é cell survives");
        assert!(tab.input, "erasing keeps the tab in input mode");

        press(&mut tab, &mut w, &mut cx, KeyCode::Backspace);
        let buf = render_tab(&mut tab, &w);
        assert_eq!(
            buf[(x - 1, y)].symbol(),
            " ",
            "the a cell is blank after the second Backspace"
        );
        assert!(tab.input, "erasing keeps the tab in input mode");
    }

    #[test]
    fn activity_tab_combining_mark_erases_whole_cluster() {
        let (mut w, mut tab) = typing_tab();
        let mut focus = Focus::default();
        let ring = FocusRing::default();
        let mut cx = Cx::new(&mut focus, &ring);

        press(&mut tab, &mut w, &mut cx, KeyCode::Char('e'));
        press(&mut tab, &mut w, &mut cx, KeyCode::Char('\u{301}'));
        let buf = render_tab(&mut tab, &w);
        let cluster = cells_showing(&buf, "e\u{301}");
        assert_eq!(cluster.len(), 1, "e with its combining accent is one cell");
        let (x, y) = cluster[0];

        press(&mut tab, &mut w, &mut cx, KeyCode::Backspace);
        let buf = render_tab(&mut tab, &w);
        assert_eq!(
            buf[(x, y)].symbol(),
            " ",
            "one Backspace erases the whole cluster, leaving no bare e"
        );
        assert!(tab.input, "erasing keeps the tab in input mode");
    }

    fn rendered_text(buf: &Buffer) -> String {
        let area = buf.area;
        (area.y..area.bottom())
            .map(|y| {
                (area.x..area.right())
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// `wrong` ends the run, so `body` is only reached by a run that answers
    /// anything else.
    fn restartable_script() -> Script {
        use crate::domain::activity::{Branch, prompt, toned};
        Script::ending(
            vec![line(0, "start"), prompt(1, "pw: ", false), line(2, "body")],
            3,
            0,
        )
        .branches(vec![vec![
            Branch {
                matches: Some("wrong".into()),
                lines: vec![toned(0, "denied", LineTone::Error)],
                end: Some((1, 1)),
            },
            Branch {
                matches: None,
                lines: vec![],
                end: None,
            },
        ]])
    }

    fn tick_world(w: &mut World, tab: &mut ActivityTab, until: impl Fn(&Activity) -> bool) {
        for _ in 0..20 {
            if w.activity("act-test").is_some_and(&until) {
                return;
            }
            w.tick();
            tab.sync(w);
        }
        panic!("activity never reached the expected state");
    }

    fn type_text(tab: &mut ActivityTab, w: &mut World, cx: &mut Cx<'_>, text: &str) {
        for c in text.chars() {
            press(tab, w, cx, KeyCode::Char(c));
        }
        press(tab, w, cx, KeyCode::Enter);
    }

    #[test]
    fn activity_tab_restart_key_resets_view_to_fresh_run() {
        let mut w = world_for(Scenario::DockerCleanup, Motion::Full);
        w.clock.running = true;
        let tick = w.tick;
        w.activities.push(Activity::new(
            "act-test",
            "demo",
            "test",
            ActivityKind::Task,
            ScopeTag::here("/work"),
            "host",
            restartable_script(),
            tick,
        ));
        let mut tab = ActivityTab::new("act-test");
        let mut focus = Focus::default();
        let ring = FocusRing::default();
        let mut cx = Cx::new(&mut focus, &ring);

        tick_world(&mut w, &mut tab, |a| a.waiting.is_some());
        press(&mut tab, &mut w, &mut cx, KeyCode::Char('i'));
        assert!(tab.input);
        type_text(&mut tab, &mut w, &mut cx, "wrong");
        tick_world(&mut w, &mut tab, |a| a.state.finished());
        assert_eq!(w.activity("act-test").unwrap().state, ActivityState::Failed);
        assert!(rendered_text(&render_tab(&mut tab, &w)).contains("denied"));

        press(&mut tab, &mut w, &mut cx, KeyCode::Char('r'));
        assert!(!tab.input, "no input capture survives the restart");
        assert!(tab.find_matches.is_empty());
        let fresh = rendered_text(&render_tab(&mut tab, &w));
        assert!(!fresh.contains("denied"), "no row of the old run stays");

        tick_world(&mut w, &mut tab, |a| a.waiting.is_some());
        press(&mut tab, &mut w, &mut cx, KeyCode::Char('i'));
        type_text(&mut tab, &mut w, &mut cx, "ok");
        tick_world(&mut w, &mut tab, |a| a.state.finished());

        let a = w.activity("act-test").unwrap();
        assert_eq!(a.state, ActivityState::Succeeded);
        assert_eq!(a.exit, Some(0));
        let after = rendered_text(&render_tab(&mut tab, &w));
        assert!(after.contains("body"), "the original tail runs again");
        assert!(!after.contains("denied"), "only the second run is shown");
    }
}
