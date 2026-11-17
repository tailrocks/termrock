use crate::screens::Jackin;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use termrock::core::event::{Key, Outcome};
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::pointer::{MouseTarget, PointerEvent};
use termrock::core::refusal::{Control, raise};
use termrock::shell::Page;
use termrock::theme::Tone;
use termrock::ui::ctx::{RenderCtx, fill, legible_on_fill};
use termrock::ui::scroll_view::{Axes, ScrollView};
use termrock::ui::text::{fit, truncate, width, wrap};
use termrock::widgets::button::{Button, row_layout_right};
use termrock::widgets::choice::Checkbox;
use termrock::widgets::dialog::Dialog;
use termrock::widgets::keyhint::{Hint, hint};
use termrock::widgets::option_list::Item;
use termrock::widgets::progress::spinner_frame;
use termrock::widgets::props::Prop;
use termrock::widgets::segments::Segment;
use termrock::widgets::tabs::{TabAction, TabItem, Tabs};

use super::config::{ConfigTabs, Doc, Scope, Tab as CfgTab, mode_label};
use super::modals::InfoDialog;
use super::{Cx, Go, LayerTag, ModalResult, Screen};
use crate::backend::Effect;
use crate::domain::agent::{Agent, AuthMode};
use crate::domain::vocabulary::fill_in;
use crate::domain::workspace::RoleName;
use crate::world::{GlobalConfig, Msg, SaveFailure, SaveSuccess, TrustKind, World, job};
use termrock::ui::text::plural;

pub const TABS: WidgetId = WidgetId::of("settings.tabs");
pub const COAUTHOR: WidgetId = WidgetId::of("settings.coauthor");
pub const DCO: WidgetId = WidgetId::of("settings.dco");
pub const TRUST: WidgetId = WidgetId::of("settings.trust");
pub const AGENTS: WidgetId = WidgetId::of("settings.agents");
pub const CANCEL: WidgetId = WidgetId::of("settings.cancel");
pub const SAVE: WidgetId = WidgetId::of("settings.save");
const BASE: WidgetId = WidgetId::of("settings.cfg");

const TAB_NAMES: [&str; 5] = ["General", "Mounts", "Environments", "Agents", "Trust"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StTab {
    General,
    Mounts,
    Environments,
    Agents,
    Trust,
}

impl StTab {
    const ALL: [StTab; 5] = [
        StTab::General,
        StTab::Mounts,
        StTab::Environments,
        StTab::Agents,
        StTab::Trust,
    ];
    fn cfg(self) -> Option<CfgTab> {
        match self {
            StTab::Mounts => Some(CfgTab::Mounts),
            StTab::Environments => Some(CfgTab::Environments),
            _ => None,
        }
    }
}

pub struct SettingsScreen {
    pub original: GlobalConfig,
    pub pending: GlobalConfig,
    pub cfg: ConfigTabs,
    tabs: Tabs,
    tab: StTab,
    coauthor: Checkbox,
    dco: Checkbox,
    trust_cursor: usize,
    agents_cursor: usize,

    agents_view: ScrollView,
    cancel: Button,
    save: Button,
    saving: bool,
    row_status: Option<String>,
}

impl SettingsScreen {
    pub fn new(w: &World) -> Self {
        let original = w.global.clone();
        let pending = original.clone();
        let registry: Vec<RoleName> = w.roles.iter().map(|r| r.name.clone()).collect();
        let cfg = ConfigTabs::new(
            Scope::Global,
            Doc::from_global(&original),
            Doc::from_global(&pending),
            BASE,
            registry,
        );
        let commits = w
            .surfaces
            .fixed
            .as_ref()
            .map_or(Control::Live, |f| Control::fixed(f.commits.clone()));
        Self {
            coauthor: Checkbox::new(
                COAUTHOR,
                "Add Co-authored-by trailer",
                pending.coauthor_trailer,
            )
            .control(commits.clone()),
            dco: Checkbox::new(DCO, "Sign off commits (DCO)", pending.dco_signoff).control(commits),
            original,
            pending,
            cfg,
            tabs: Tabs::new(TABS, &TAB_NAMES),
            tab: StTab::General,
            trust_cursor: 0,
            agents_cursor: 0,
            agents_view: ScrollView::new(AGENTS.sub("view")).axes(Axes::VERTICAL),
            cancel: Button::subtle(CANCEL, "Cancel"),
            save: Button::primary(SAVE, "Save…"),
            saving: false,
            row_status: None,
        }
    }

    fn sync_pending(&mut self) {
        self.pending.coauthor_trailer = self.coauthor.checked;
        self.pending.dco_signoff = self.dco.checked;
        self.cfg.pending.apply_to_global(&mut self.pending);
    }

    pub fn change_count(&self) -> usize {
        self.pending.change_count(&self.original)
    }

    fn general_dirty(&self) -> bool {
        self.pending.coauthor_trailer != self.original.coauthor_trailer
            || self.pending.dco_signoff != self.original.dco_signoff
    }

    fn trust_dirty(&self) -> bool {
        self.pending.trust != self.original.trust
    }

    fn agents_dirty(&self) -> bool {
        self.pending.agent_modes != self.original.agent_modes
    }

    fn set_tab(&mut self, i: usize, cx: &mut Cx) {
        self.tab = StTab::ALL[i.min(4)];
        self.tabs.set_active(i);
        cx.focus.focus(TABS);
    }

    fn body_focus(&self) -> WidgetId {
        match self.tab {
            StTab::General => COAUTHOR,
            StTab::Trust => TRUST,
            StTab::Agents => AGENTS,
            t => self.cfg.list_id(t.cfg().unwrap()),
        }
    }

    fn open_preview(&mut self, w: &World, cx: &mut Cx) {
        self.sync_pending();
        let n = self.change_count();
        if n == 0 {
            cx.status("Nothing to save");
            return;
        }
        if self.cfg.tab_error(CfgTab::Environments) {
            self.set_tab(2, cx);
            cx.error("An environment key is invalid");
            return;
        }
        let blockers = self.cfg.blockers();
        let sensitive: Vec<String> = self
            .cfg
            .pending
            .mounts
            .iter()
            .filter(|m| is_sensitive(m.source_label()))
            .map(|m| m.source_label().to_owned())
            .collect();
        let mut facts = vec![
            Prop::new(
                "Scope",
                match &w.surfaces.vocabulary.global_config {
                    Some(path) => format!(
                        "global config · {}",
                        w.tilde(&fill_in(
                            path,
                            &[("home", &w.home), ("config", &w.config_file())]
                        ))
                    ),
                    None => "global config".to_owned(),
                },
            ),
            Prop::new("Changes", plural(n, "change", "changes")),
        ];
        if self.general_dirty() {
            facts.push(Prop::new(
                "Commits",
                format!(
                    "co-author trailer {} · DCO sign-off {}",
                    on_off(self.pending.coauthor_trailer),
                    on_off(self.pending.dco_signoff)
                ),
            ));
        }
        facts.extend(self.cfg.summary_facts());
        if self.agents_dirty() {
            let changed: Vec<String> = Agent::ALL
                .iter()
                .filter(|a| self.pending.agent_modes.get(a) != self.original.agent_modes.get(a))
                .map(|a| {
                    format!(
                        "{} → {}",
                        a.label(),
                        mode_label(
                            self.pending
                                .agent_modes
                                .get(a)
                                .copied()
                                .unwrap_or(AuthMode::Sync)
                        )
                    )
                })
                .collect();
            facts.push(Prop::new("Agents", changed.join(" · ")));
        }
        if self.trust_dirty() {
            let changed: Vec<String> = self
                .pending
                .trust
                .iter()
                .zip(&self.original.trust)
                .filter(|(a, b)| a.trusted != b.trusted)
                .map(|(a, _)| {
                    format!(
                        "{} → {}",
                        a.source,
                        if a.trusted { "trusted" } else { "untrusted" }
                    )
                })
                .collect();
            facts.push(Prop::new("Trust", changed.join(" · ")));
        }
        for s in &sensitive {
            facts.push(
                Prop::new(
                    "Sensitive",
                    format!("{s} · sensitive global mount path detected"),
                )
                .tone(Tone::Warning),
            );
        }
        for b in &blockers {
            facts.push(Prop::new("Blocker", b.clone()).tone(Tone::Error));
        }
        let mut code = self.cfg.diff_lines();
        for a in Agent::ALL {
            let (o, p) = (
                self.original
                    .agent_modes
                    .get(&a)
                    .copied()
                    .unwrap_or(AuthMode::Sync),
                self.pending
                    .agent_modes
                    .get(&a)
                    .copied()
                    .unwrap_or(AuthMode::Sync),
            );
            if o != p {
                code.push(format!(
                    "~ agent_mode {} {} → {}",
                    a.label(),
                    mode_label(o),
                    mode_label(p)
                ));
            }
        }
        if self.pending.coauthor_trailer != self.original.coauthor_trailer {
            code.push(format!(
                "~ coauthor_trailer {} → {}",
                self.original.coauthor_trailer, self.pending.coauthor_trailer
            ));
        }
        if self.pending.dco_signoff != self.original.dco_signoff {
            code.push(format!(
                "~ dco_signoff {} → {}",
                self.original.dco_signoff, self.pending.dco_signoff
            ));
        }
        for (a, b) in self.pending.trust.iter().zip(&self.original.trust) {
            if a.trusted != b.trusted {
                code.push(format!(
                    "~ trust {} {} → {}",
                    a.source, b.trusted, a.trusted
                ));
            }
        }
        let (confirm, tag) = if !blockers.is_empty() {
            (
                Button::primary(WidgetId::of("settings.preview.cleanup"), "Clean up & save…"),
                "preview.cleanup",
            )
        } else if !sensitive.is_empty() {
            (
                Button::primary(WidgetId::of("settings.preview.save"), "Save anyway"),
                "preview",
            )
        } else {
            (
                Button::primary(WidgetId::of("settings.preview.save"), "Save"),
                "preview",
            )
        };
        let d = Dialog::facts(
            WidgetId::of("settings.preview"),
            "Save settings",
            facts,
            code,
            None,
            confirm,
        );
        cx.open(d, LayerTag::new(tag));
    }

    fn start_save(&mut self, w: &mut World, cx: &mut Cx) {
        self.sync_pending();
        self.saving = true;
        self.save.busy = true;
        w.perform(Effect::SaveGlobal {
            base: Box::new(self.original.clone()),
            global: Box::new(self.pending.clone()),
        });
        cx.progress(job::SETTINGS_SAVE, "Saving settings…");
        cx.focus.focus(SAVE);
    }

    fn finish_save(
        &mut self,
        result: &Result<SaveSuccess, SaveFailure>,
        w: &mut World,
        cx: &mut Cx,
    ) {
        self.saving = false;
        self.save.busy = false;
        let saved = match result {
            Ok(saved) => saved,
            Err(error) => {
                let mut props = vec![
                    Prop::new("Error", error.error.as_str()),
                    Prop::new("State", "your edits are intact · nothing was written"),
                ];
                if let Some(next) = &error.next {
                    props.push(Prop::new("Next", next.as_str()));
                }
                let d = InfoDialog::new(WidgetId::of("settings.savefail"), "Settings error", props)
                    .error()
                    .width(66);
                cx.open(d, LayerTag::new("savefail"));
                return;
            }
        };
        w.apply((*saved.snapshot).clone());
        self.original = self.pending.clone();
        cx.status(match saved.warning {
            Some(warning) => format!("{} · Settings saved", warning.status()),
            None => "Settings saved".to_owned(),
        });
        cx.go(Go::Manager);
    }

    fn leave(&mut self, cx: &mut Cx) {
        self.sync_pending();
        if self.change_count() > 0 {
            let d =
                super::modals::unsaved_dialog(WidgetId::of("settings.exit"), self.change_count());
            cx.open(d, LayerTag::new("exit"));
        } else {
            cx.go(Go::Manager);
        }
    }

    fn render_general(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        if area.is_empty() {
            return;
        }
        let t = ctx.theme;
        let bg = t.canvas;
        let x = area.x + 2;
        let mut y = area.y;
        let field = |y: u16| Rect::new(x, y, 40, 1).intersection(area);
        buf.set_string(
            x,
            y,
            "Commits",
            t.secondary().bg(bg).add_modifier(Modifier::BOLD),
        );
        y += 1;
        if y >= area.bottom() {
            return;
        }
        self.coauthor.render(field(y), buf, ctx, bg);
        if self.pending.coauthor_trailer != self.original.coauthor_trailer {
            buf.set_string(
                x + 41,
                y,
                "•",
                ratatui::style::Style::new().fg(t.warning).bg(bg),
            );
        }
        y += 1;
        if y >= area.bottom() {
            return;
        }
        self.dco.render(field(y), buf, ctx, bg);
        if self.pending.dco_signoff != self.original.dco_signoff {
            buf.set_string(
                x + 41,
                y,
                "•",
                ratatui::style::Style::new().fg(t.warning).bg(bg),
            );
        }
        y += 2;
        if y >= area.bottom() {
            return;
        }
        let note = match &w.surfaces.fixed {
            Some(f) => f.commits.text.as_str(),
            None => "Two independent flags; both apply to every Workspace.",
        };
        buf.set_string(
            x,
            y,
            termrock::ui::reveal::cut(Rect::new(x, y, area.width.saturating_sub(4), 1), note),
            t.faint().bg(bg),
        );
        y += 2;
        if y >= area.bottom() {
            return;
        }
        buf.set_string(
            x,
            y,
            "Trailers",
            t.secondary().bg(bg).add_modifier(Modifier::BOLD),
        );
        y += 1;
        if y >= area.bottom() {
            return;
        }
        buf.set_string(
            x,
            y,
            termrock::ui::reveal::cut(
                Rect::new(x, y, area.width.saturating_sub(4), 1),
                &w.surfaces.vocabulary.trailer_sample,
            ),
            t.muted().bg(bg),
        );
    }

    fn render_trust(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let bg = t.canvas;
        let focused = ctx.interaction.focused(TRUST);
        let trusted = self.pending.trust.iter().filter(|r| r.trusted).count();
        let untrusted = self.pending.trust.len() - trusted;
        buf.set_string(
            area.x + 2,
            area.y,
            "Role sources",
            t.secondary().bg(bg).add_modifier(Modifier::BOLD),
        );
        let meta = format!(
            "{} · {}",
            plural(trusted, "trusted", "trusted"),
            plural(untrusted, "untrusted", "untrusted")
        );
        buf.set_string(
            area.right().saturating_sub(width(&meta) as u16 + 2),
            area.y,
            &meta,
            t.faint().bg(bg),
        );
        let body = Rect::new(
            area.x,
            area.y + 1,
            area.width,
            area.height.saturating_sub(1),
        );
        ctx.control(TRUST, body, false);
        let src_w = area.width.saturating_sub(30).clamp(16, 40) as usize;
        for (i, row) in self.pending.trust.iter().enumerate() {
            let y = body.y + i as u16;
            if y >= body.bottom() {
                break;
            }
            let rid = TRUST.child(i);
            let mut s = ctx.state(rid);
            s.focused = focused && i == self.trust_cursor;
            s.selected = i == self.trust_cursor;
            let st = t.row(s, bg);
            let rect = Rect::new(body.x, y, body.width, 1);
            fill(buf, rect, st);
            buf.set_string(
                rect.x,
                y,
                t.gutter_symbol(s),
                t.gutter(s, st.bg.unwrap_or(bg), false),
            );
            buf.set_string(
                rect.x + 1,
                y,
                if s.selected { "›" } else { " " },
                st.fg(if s.focused {
                    t.accent
                } else {
                    t.text_secondary
                }),
            );
            let changed = self
                .original
                .trust
                .get(i)
                .is_some_and(|o| o.trusted != row.trusted);
            buf.set_string(
                rect.x + 3,
                y,
                if changed { "•" } else { " " },
                st.fg(t.warning),
            );
            buf.set_string(
                rect.x + 5,
                y,
                fit(&truncate(&row.source, src_w), src_w),
                if s.selected {
                    st.add_modifier(Modifier::BOLD)
                } else {
                    st
                },
            );
            let x = rect.x + 7 + src_w as u16;
            buf.set_string(x, y, fit(row.kind.label(), 5), st.fg(t.text_muted));
            let mark = if row.trusted {
                "[✓] trusted"
            } else {
                "[ ] untrusted"
            };
            buf.set_string(
                x + 6,
                y,
                mark,
                st.fg(if row.trusted {
                    t.text_primary
                } else {
                    t.warning
                }),
            );
            let roles = plural(row.roles, "role", "roles");
            if x + 22 + (width(&roles) as u16) < rect.right() {
                buf.set_string(x + 21, y, &roles, st.fg(t.text_faint));
            }

            legible_on_fill(buf, rect, t, bg);
            ctx.clickable(rid, rect);
        }
        let ny = body.y + self.pending.trust.len() as u16 + 1;
        if let Some(f) = &w.surfaces.fixed {
            if ny < body.bottom() {
                buf.set_string(
                    area.x + 2,
                    ny,
                    termrock::ui::reveal::cut(
                        Rect::new(area.x + 2, ny, area.width.saturating_sub(4), 1),
                        &f.trust.text,
                    ),
                    t.faint().bg(bg),
                );
            }
        } else if ny + 1 < body.bottom() {
            buf.set_string(
                area.x + 2,
                ny,
                termrock::ui::reveal::cut(
                    Rect::new(area.x + 2, ny, area.width.saturating_sub(4), 1),
                    "An untrusted source blocks + Load role until it is trusted here or",
                ),
                t.faint().bg(bg),
            );
            buf.set_string(
                area.x + 2,
                ny + 1,
                termrock::ui::reveal::cut(
                    Rect::new(area.x + 2, ny + 1, area.width.saturating_sub(4), 1),
                    "in the trust dialog that the load opens.",
                ),
                t.faint().bg(bg),
            );
        }
    }

    fn render_agents(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let natural = |a: ratatui::layout::Size| {
            let notes: usize = agent_notes(w)
                .iter()
                .map(|n| wrap(n, a.width.saturating_sub(4).max(1) as usize).len())
                .sum();
            ratatui::layout::Size::new(a.width, 3 + Agent::ALL.len() as u16 + notes as u16)
        };
        let size = self.agents_view.fit(area.as_size(), natural);
        let mut frame = self.agents_view.begin(area, size, buf, ctx);
        let inner = frame.area();
        self.draw_agents(inner, frame.buffer(buf), ctx, w);
        self.agents_view.end(frame, buf, ctx);
    }

    fn draw_agents(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let bg = t.canvas;
        let focused = ctx.interaction.focused(AGENTS);
        buf.set_string(
            area.x + 2,
            area.y,
            "Agent runtime mode",
            t.secondary().bg(bg).add_modifier(Modifier::BOLD),
        );
        let meta = "accounts are registered in Accounts (c)";
        if area.width > 50 {
            buf.set_string(
                area.right().saturating_sub(width(meta) as u16 + 2),
                area.y,
                meta,
                t.faint().bg(bg),
            );
        }
        let head = format!("{:<14}{:<13}{}", "Agent", "Mode", "Registry default");
        buf.set_string(area.x + 5, area.y + 1, &head, t.muted().bg(bg));
        let body = Rect::new(
            area.x,
            area.y + 2,
            area.width,
            area.height.saturating_sub(2),
        );
        ctx.control(AGENTS, body, false);
        for (i, a) in Agent::ALL.iter().enumerate() {
            let y = body.y + i as u16;
            if y >= body.bottom() {
                break;
            }
            let rid = AGENTS.child(i);
            let mut s = ctx.state(rid);
            s.focused = focused && i == self.agents_cursor;
            s.selected = i == self.agents_cursor;
            let st = t.row(s, bg);
            let rect = Rect::new(body.x, y, body.width, 1);
            fill(buf, rect, st);
            buf.set_string(
                rect.x,
                y,
                t.gutter_symbol(s),
                t.gutter(s, st.bg.unwrap_or(bg), false),
            );
            buf.set_string(
                rect.x + 1,
                y,
                if s.selected { "›" } else { " " },
                st.fg(if s.focused {
                    t.accent
                } else {
                    t.text_secondary
                }),
            );
            let mode = self
                .pending
                .agent_modes
                .get(a)
                .copied()
                .unwrap_or(AuthMode::Sync);
            let changed = self
                .original
                .agent_modes
                .get(a)
                .copied()
                .unwrap_or(AuthMode::Sync)
                != mode;
            buf.set_string(
                rect.x + 3,
                y,
                if changed { "•" } else { " " },
                st.fg(t.warning),
            );
            buf.set_string(
                rect.x + 5,
                y,
                fit(a.label(), 13),
                if s.selected {
                    st.add_modifier(Modifier::BOLD)
                } else {
                    st
                },
            );
            buf.set_string(
                rect.x + 19,
                y,
                fit(mode_label(mode), 12),
                st.fg(t.text_secondary),
            );
            let default = match w.accounts.default_for(a.provider()) {
                Some(acc) => format!("★ {}", acc.title()),
                None => match w.accounts.discovered_current(a.provider()) {
                    Some(acc) => format!("discovered · {}", acc.source.safe_detail()),
                    None => "no account registered".into(),
                },
            };
            let dx = rect.x + 32;
            let detail = if mode == AuthMode::Ignore {
                w.surfaces.vocabulary.agent_no_credentials.clone()
            } else {
                default
            };
            buf.set_string(
                dx,
                y,
                termrock::ui::reveal::cut(
                    Rect::new(dx, y, rect.right().saturating_sub(dx + 1), 1),
                    &detail,
                ),
                st.fg(t.text_muted),
            );
            legible_on_fill(buf, rect, t, bg);
            ctx.clickable(rid, rect);
        }

        let mut ny = body.y + Agent::ALL.len() as u16 + 1;
        for note in agent_notes(w) {
            for line in wrap(note, area.width.saturating_sub(4).max(1) as usize) {
                if ny < area.bottom() {
                    buf.set_string(area.x + 2, ny, &line, t.faint().bg(bg));
                }
                ny += 1;
            }
        }
    }

    fn agents_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        let n = Agent::ALL.len();
        let a = Agent::ALL[self.agents_cursor.min(n - 1)];
        if let Some(f) = &w.surfaces.fixed
            && key.plain()
            && matches!(
                key.code,
                KeyCode::Char(' ') | KeyCode::Enter | KeyCode::Char('d')
            )
        {
            raise(if a == f.launchable {
                &f.agent_launchable
            } else {
                &f.agent_other
            });
            return Outcome::Consumed;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.agents_cursor = self.agents_cursor.saturating_sub(1)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.agents_cursor = (self.agents_cursor + 1).min(n - 1)
            }
            KeyCode::Home | KeyCode::Char('g') => self.agents_cursor = 0,
            KeyCode::End | KeyCode::Char('G') => self.agents_cursor = n - 1,
            KeyCode::Char(' ') | KeyCode::Enter => {
                let modes = a.auth_modes();
                let cur = self
                    .pending
                    .agent_modes
                    .get(&a)
                    .copied()
                    .unwrap_or(AuthMode::Sync);
                let i = modes.iter().position(|m| *m == cur).unwrap_or(0);
                let next = modes[(i + 1) % modes.len()];
                self.pending.agent_modes.insert(a, next);
                cx.status(format!(
                    "{} · {} · save to apply",
                    a.label(),
                    mode_label(next)
                ));
            }
            KeyCode::Char('d') => {
                self.pending.agent_modes.insert(a, AuthMode::Sync);
                cx.status(format!("{} · back to sync · save to apply", a.label()));
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Changed
    }

    fn trust_key(&mut self, key: &Key, w: &World, cx: &mut Cx) -> Outcome {
        let n = self.pending.trust.len();
        if n == 0 {
            return Outcome::Ignored;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.trust_cursor = self.trust_cursor.saturating_sub(1)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.trust_cursor = (self.trust_cursor + 1).min(n - 1)
            }
            KeyCode::Home | KeyCode::Char('g') => self.trust_cursor = 0,
            KeyCode::End | KeyCode::Char('G') => self.trust_cursor = n - 1,
            KeyCode::Char(' ') | KeyCode::Enter if w.surfaces.fixed.is_some() => {
                if let Some(f) = &w.surfaces.fixed {
                    raise(&f.trust);
                }
                return Outcome::Consumed;
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                let r = &mut self.pending.trust[self.trust_cursor];
                r.trusted = !r.trusted;
                cx.status(format!(
                    "{} · {} · save to apply",
                    r.source,
                    if r.trusted { "trusted" } else { "untrusted" }
                ));
            }
            KeyCode::Char('o') => {
                let r = &self.pending.trust[self.trust_cursor];
                let template = if r.kind == TrustKind::Git && r.source.contains("github.com") {
                    &w.surfaces.vocabulary.trust_browser
                } else {
                    &w.surfaces.vocabulary.trust_reveal
                };
                let text = fill_in(template, &[("source", r.source.as_str())]);
                if w.surfaces.fixed.is_some() {
                    cx.sticky(text);
                } else {
                    cx.status(text);
                }
            }
            _ => return Outcome::Ignored,
        }
        Outcome::Changed
    }
}

fn on_off(b: bool) -> &'static str {
    if b { "on" } else { "off" }
}

fn is_sensitive(path: &str) -> bool {
    let p = path.trim_start_matches('~');
    p == "/"
        || p.starts_with("/.ssh")
        || p.starts_with("/.aws")
        || p.starts_with("/.gnupg")
        || p.starts_with("/etc")
        || p.is_empty()
        || p.starts_with("/.config/op")
}

impl Page<Jackin> for SettingsScreen {
    fn enter(&mut self, _w: &mut World, cx: &mut Cx) {
        cx.focus.focus(TABS);
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(TABS)
    }

    fn animating(&self, _w: &World) -> bool {
        self.saving
    }

    fn on_tick(&mut self, _w: &mut World, _cx: &mut Cx) -> Outcome {
        if self.saving {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn on_msg(&mut self, msg: &Msg, w: &mut World, cx: &mut Cx) -> Outcome {
        if let Msg::GlobalSaved { result } = msg
            && self.saving
        {
            self.finish_save(result, w, cx);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        if self.saving {
            return Outcome::Consumed;
        }
        if self.cfg.menu_open() {
            let o = self.cfg.menu_key(key, w, cx);
            self.sync_pending();

            if o != Outcome::Ignored {
                return o;
            }
        }
        let focus = cx.focus.current();
        if key.ctrl() && key.code == KeyCode::Char('s') {
            self.open_preview(w, cx);
            return Outcome::Changed;
        }
        match key.code {
            KeyCode::Char('[') => {
                let i = (self.tabs.active + 4) % 5;
                self.set_tab(i, cx);
                return Outcome::Changed;
            }
            KeyCode::Char(']') => {
                let i = (self.tabs.active + 1) % 5;
                self.set_tab(i, cx);
                return Outcome::Changed;
            }
            _ => {}
        }
        if focus == Some(TABS) {
            if matches!(
                key.code,
                KeyCode::Enter | KeyCode::Down | KeyCode::Char('j')
            ) {
                cx.focus.focus(self.body_focus());
                return Outcome::Changed;
            }
            let (o, ev) = self.tabs.on_key(key);
            if let Some(TabAction::Activated(i)) = ev {
                self.tab = StTab::ALL[i.min(4)];
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
            match key.code {
                KeyCode::Esc => {
                    self.leave(cx);
                    return Outcome::Changed;
                }
                _ => return Outcome::Ignored,
            }
        }
        if focus == Some(CANCEL) {
            let (o, fired) = self.cancel.on_key(key);
            if fired {
                self.leave(cx);
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
        }
        if focus == Some(SAVE) {
            let (o, fired) = self.save.on_key(key);
            if fired {
                self.open_preview(w, cx);
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
        }
        if matches!(focus, Some(CANCEL) | Some(SAVE)) {
            match key.code {
                KeyCode::Left | KeyCode::Char('h') => {
                    cx.focus.focus(CANCEL);
                    return Outcome::Changed;
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    cx.focus.focus(SAVE);
                    return Outcome::Changed;
                }
                KeyCode::Esc | KeyCode::Up | KeyCode::Char('k') => {
                    cx.focus.focus(self.body_focus());
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        let body = match self.tab {
            StTab::General if focus == Some(COAUTHOR) => {
                let o = self.coauthor.on_key(key);
                if o.consumed() {
                    self.sync_pending();
                    o
                } else {
                    match key.code {
                        KeyCode::Down | KeyCode::Char('j') => {
                            cx.focus.focus(DCO);
                            Outcome::Changed
                        }
                        _ => Outcome::Ignored,
                    }
                }
            }
            StTab::General if focus == Some(DCO) => {
                let o = self.dco.on_key(key);
                if o.consumed() {
                    self.sync_pending();
                    o
                } else {
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            cx.focus.focus(COAUTHOR);
                            Outcome::Changed
                        }
                        _ => Outcome::Ignored,
                    }
                }
            }
            StTab::Trust if focus == Some(TRUST) => self.trust_key(key, w, cx),
            StTab::Agents if focus == Some(AGENTS) => self.agents_key(key, w, cx),
            t if t.cfg().is_some() && self.cfg.owns(t.cfg().unwrap(), focus) => {
                let o = self.cfg.on_key(t.cfg().unwrap(), key, w, cx);
                if o.consumed() {
                    self.sync_pending();
                }
                o
            }
            _ => Outcome::Ignored,
        };
        if body.consumed() {
            return body;
        }
        match key.code {
            KeyCode::Esc => {
                cx.focus.focus(TABS);
                Outcome::Changed
            }
            KeyCode::Char('c') if key.plain() && self.tab == StTab::Agents => {
                let sel = Agent::ALL
                    .get(self.agents_cursor)
                    .and_then(|a| w.accounts.default_for(a.provider()))
                    .map(|a| a.id.clone());
                cx.go(Go::Accounts { select: sel });
                Outcome::Changed
            }
            _ => Outcome::Ignored,
        }
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        if let Some(r) = self.agents_view.pointer(ev) {
            return r.outcome();
        }
        let click = ev.is(Phase::Click);
        if self.saving && (click || ev.is(Phase::Secondary)) {
            return Outcome::Consumed;
        }
        if click && self.cfg.menu_open() {
            let o = self.cfg.menu_click(ev.id, w, cx);
            self.sync_pending();
            return o;
        }
        if let Some(r) = self.tabs.route(ev) {
            let (o, actions) = r.into_parts();
            for action in actions {
                if let TabAction::Activated(i) = action {
                    self.tab = StTab::ALL[i.min(4)];
                }
            }
            if !click {
                return o;
            }
            cx.focus.focus(TABS);
            return o.or(Outcome::Changed);
        }
        if click {
            return self.click(ev, w, cx);
        }
        match self.tab.cfg() {
            Some(t) => self.cfg.pointer(t, ev, w, cx),
            None => Outcome::Ignored,
        }
    }

    fn on_layer_result(
        &mut self,
        tag: &LayerTag,
        result: ModalResult,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        if let Some(o) = self.cfg.on_modal(tag, result.clone(), w, cx) {
            self.sync_pending();
            return o;
        }
        match (tag.kind, result) {
            (
                "preview",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                self.start_save(w, cx);
                Outcome::Changed
            }
            (
                "preview.cleanup",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                self.cfg.clean_up();
                cx.status("Cleaned up isolated state · saving");
                self.start_save(w, cx);
                Outcome::Changed
            }
            ("preview" | "preview.cleanup", _) => {
                cx.status("Save aborted · settings unchanged");
                cx.focus.focus(SAVE);
                Outcome::Changed
            }
            (
                "exit",
                ModalResult::Dialog {
                    action: Some(2), ..
                },
            ) => {
                self.open_preview(w, cx);
                Outcome::Changed
            }
            (
                "exit",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                cx.status(format!(
                    "Discarded {}",
                    plural(self.change_count(), "change", "changes")
                ));
                cx.go(Go::Manager);
                Outcome::Changed
            }
            ("savefail", _) => {
                cx.focus.focus(SAVE);
                Outcome::Changed
            }
            _ => Outcome::Changed,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        let bg = t.canvas;
        self.sync_pending();
        let items: Vec<TabItem> = StTab::ALL
            .iter()
            .zip(TAB_NAMES)
            .map(|(tab, name)| {
                let mut it = TabItem::new(name);
                it.dirty = match tab {
                    StTab::General => self.general_dirty(),
                    StTab::Trust => self.trust_dirty(),
                    StTab::Agents => self.agents_dirty(),
                    t => self.cfg.tab_dirty(t.cfg().unwrap()),
                };
                it.error = match tab {
                    StTab::General | StTab::Trust | StTab::Agents => false,
                    t => self.cfg.tab_error(t.cfg().unwrap()),
                };
                it
            })
            .collect();
        let active = self.tabs.active;
        self.tabs.set_items(items);
        self.tabs.set_active(active);
        self.tabs.render(
            Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), 2),
            buf,
            ctx,
            bg,
        );
        let body = Rect::new(
            area.x + 1,
            area.y + 4,
            area.width.saturating_sub(2),
            area.height.saturating_sub(6),
        );
        self.row_status = None;
        match self.tab {
            StTab::General => self.render_general(body, buf, ctx, w),
            StTab::Trust => self.render_trust(body, buf, ctx, w),
            StTab::Agents => self.render_agents(body, buf, ctx, w),
            t => {
                self.row_status = self.cfg.render(t.cfg().unwrap(), body, buf, ctx, w);
            }
        }
        let n = self.change_count();
        self.save.disabled = n == 0 && !self.saving;
        self.save.label = if self.saving {
            format!("{} Saving…", spinner_frame(w.now_ms() as u64 / 80))
        } else {
            "Save…".into()
        };
        let widths = [self.cancel.width(), self.save.width()];
        let lines = termrock::widgets::button::row_lines(area.width.saturating_sub(4), &widths, 3);
        let rects = row_layout_right(
            Rect::new(
                area.x,
                area.bottom().saturating_sub(lines),
                area.width.saturating_sub(4),
                lines,
            ),
            &widths,
            3,
        );
        self.cancel.render(rects[0], buf, ctx, bg);
        self.save.render(rects[1], buf, ctx, bg);
        self.cfg.render_menu(area, buf, ctx);
    }

    fn hints(&self, focus: Option<WidgetId>, w: &World) -> Vec<Hint> {
        if self.saving {
            return vec![hint("", "Saving…")];
        }
        if self.cfg.menu_open() {
            return termrock::widgets::row_menu::hints();
        }
        let mut v = vec![];
        let fixed = w.surfaces.fixed.is_some();
        if focus == Some(TABS) {
            v.push(hint("←→", "Tab"));
            v.push(hint("1–5", "Jump"));
            v.push(hint("Enter", "Edit fields"));
        } else if let Some(b) = [&self.cancel, &self.save]
            .into_iter()
            .find(|b| Some(b.id) == focus)
        {
            v.push(hint("←→", "Choose"));
            v.push(b.hint());
        } else {
            match self.tab {
                StTab::General => {
                    if !fixed {
                        v.push(hint("Space", "Toggle"));
                    }
                    v.push(hint("↑↓", "Move"));
                }
                StTab::Trust => {
                    if !fixed {
                        v.push(hint("Space", "Toggle trust"));
                    }
                    v.push(Hint::new("o", w.surfaces.vocabulary.trust_hint.clone()));
                }
                StTab::Agents => {
                    if !fixed {
                        v.push(hint("Space", "Cycle mode"));
                        v.push(hint("d", "Remove override"));
                    }
                    v.push(hint("c", "Manage accounts"));
                }
                t => v.extend(self.cfg.hints(t.cfg().unwrap(), focus, w)),
            }
        }
        v.push(hint("[ ]", "Switch tab"));
        v.push(hint("Ctrl+S", "Save"));
        v.push(hint("Esc", "Back"));
        v
    }

    fn crumb(&self, _w: &World) -> String {
        format!("Settings › global › {}", TAB_NAMES[self.tabs.active.min(4)])
    }

    fn stakes(&mut self, _w: &World) -> termrock::ui::leave::Stakes {
        self.sync_pending();
        let n = self.change_count();
        if n > 0 {
            termrock::ui::leave::Stakes::Unsaved { changes: n }
        } else {
            termrock::ui::leave::Stakes::Clean
        }
    }

    fn on_esc_top(&mut self, _w: &mut World, cx: &mut Cx) -> Outcome {
        if cx.focus.is(TABS) {
            self.leave(cx);
        } else {
            cx.focus.focus(TABS);
        }
        Outcome::Changed
    }
}

impl Screen for SettingsScreen {
    fn layer_open(&self) -> bool {
        self.cfg.menu_hides_focus()
    }

    fn picker_items(&mut self, tag: &LayerTag, query: &str, _w: &World) -> Option<Vec<Item>> {
        if tag.kind == "cfg.role" {
            return Some(self.cfg.role_picker_items(query));
        }
        None
    }

    fn form_changed(&mut self, tag: &LayerTag, form: &mut super::modals::FormDialog, w: &World) {
        self.cfg.form_changed(tag, form, w);
    }

    fn strip_right(&self, w: &World) -> Vec<Segment> {
        let mut v = vec![];
        if self.saving {
            v.push(
                Segment::new(
                    format!("{} saving…", spinner_frame(w.now_ms() as u64 / 80)),
                    Tone::Secondary,
                )
                .priority(9),
            );
        }
        let n = self.change_count();
        if n > 0 {
            v.push(
                Segment::new(
                    format!("• {}", plural(n, "change", "changes")),
                    Tone::Warning,
                )
                .priority(8),
            );
        }
        if let Some(s) = &self.row_status {
            v.push(Segment::new(s.clone(), Tone::Muted).priority(3));
        }
        v
    }

    fn request_leave(&mut self, _w: &mut World, cx: &mut Cx) -> Outcome {
        self.leave(cx);
        Outcome::Changed
    }
}

impl SettingsScreen {
    fn click(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        let id = ev.id;
        if id == CANCEL {
            cx.focus.focus(CANCEL);
            if self.cancel.on_click() {
                self.leave(cx);
            }
            return Outcome::Changed;
        }
        if id == SAVE {
            cx.focus.focus(SAVE);
            if self.save.on_click() {
                self.open_preview(w, cx);
            }
            return Outcome::Changed;
        }
        match self.tab {
            StTab::General => {
                if id == COAUTHOR {
                    cx.focus.focus(COAUTHOR);
                    let o = self.coauthor.on_click();
                    self.sync_pending();
                    return o;
                }
                if id == DCO {
                    cx.focus.focus(DCO);
                    let o = self.dco.on_click();
                    self.sync_pending();
                    return o;
                }
                Outcome::Ignored
            }
            StTab::Agents => {
                for i in 0..Agent::ALL.len() {
                    if AGENTS.child(i) == id {
                        let same = self.agents_cursor == i;
                        self.agents_cursor = i;
                        cx.focus.focus(AGENTS);
                        if same {
                            let k = Key {
                                code: KeyCode::Char(' '),
                                mods: ratatui::crossterm::event::KeyModifiers::NONE,
                            };
                            return self.agents_key(&k, w, cx);
                        }
                        return Outcome::Changed;
                    }
                }
                Outcome::Ignored
            }
            StTab::Trust => {
                for i in 0..self.pending.trust.len() {
                    if TRUST.child(i) == id {
                        let same = self.trust_cursor == i;
                        self.trust_cursor = i;
                        cx.focus.focus(TRUST);
                        if same {
                            let k = Key {
                                code: KeyCode::Char(' '),
                                mods: ratatui::crossterm::event::KeyModifiers::NONE,
                            };
                            return self.trust_key(&k, w, cx);
                        }
                        return Outcome::Changed;
                    }
                }
                Outcome::Ignored
            }
            t => {
                let o = self.cfg.pointer(t.cfg().unwrap(), ev, w, cx);
                if o.consumed() {
                    self.sync_pending();
                }
                o
            }
        }
    }
}

const AGENT_NOTES: [&str; 2] = [
    "sync mirrors the host login · api key and oauth token take material from the registry account",
    "ignore starts the agent without credentials and removes it from session pickers",
];

fn agent_notes(w: &World) -> [&str; 2] {
    match &w.surfaces.fixed {
        Some(f) => [&f.agent_notes[0], &f.agent_notes[1]],
        None => AGENT_NOTES,
    }
}
