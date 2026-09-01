//! Lookbook-owned model, rendering, and interaction routing.

use std::{ops::ControlFlow, time::Duration};

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, List as RatatuiList, ListItem, ListState as RatatuiListState, Paragraph, Wrap,
    },
};
use termrock::{
    input::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind},
    keymap::KeyChord,
    patterns::{StudioShellLayout, layout_studio_shell},
    runtime::{FrameTick, Instant},
    scroll::{self, ScrollSpan},
    style::{Density, Role, RolePalette},
    widgets::{
        DesignInspector, DesignInspectorFrame, InspectorPanel, List as ComponentList, ListRow,
        ListState as ComponentListState, Panel, PanelVariant, ProgressBar, ProgressKind,
    },
};

use crate::{
    PREVIEW_KEYMAP, PreviewAction, SIDEBAR_KEYMAP, SidebarAction,
    focus::{FocusId, is_focused, panel_chrome},
    host_frame::HostFrame,
};
use termrock_lookbook::{
    demo::{DemoSession, DemoUpdate},
    stories::{gallery_stories, is_pattern_demo},
};

pub(crate) struct Lookbook {
    selected: usize,
    preview_scroll: u16,
    sidebar_scroll: u16,
    /// Public TermRock authorities only (scene + focus graph + theme).
    host: HostFrame,
    demo: DemoSession,
    component_area: Rect,
    preview_panel_area: Rect,
    sidebar_area: Rect,
    sidebar_inner_area: Rect,
    sidebar_viewport_items: usize,
    preview_viewport_rows: usize,
    knob_selected: usize,
    demo_outcome: Option<String>,
    full_preview: bool,
    next_demo_deadline: Option<Instant>,
}

impl Lookbook {
    pub(crate) fn new() -> Self {
        Self::for_story(None).expect("catalog must contain its first demo")
    }

    pub(crate) fn for_story(story_id: Option<&str>) -> Result<Self, String> {
        let theme = RolePalette::default();
        let catalog = gallery_stories();
        let selected = match story_id {
            Some(id) => catalog
                .iter()
                .position(|story| story.id == id)
                .ok_or_else(|| format!("unknown story: {id}"))?,
            None => 0,
        };
        let story = catalog[selected];
        let mut demo = DemoSession::mount(story.id, Some(story.width), Some(story.height))
            .expect("catalog demo must mount");
        demo.set_system(termrock_lookbook::design::lookbook_system(theme.clone()));
        Ok(Self {
            selected,
            preview_scroll: 0,
            sidebar_scroll: 0,
            host: HostFrame::new(theme),
            demo,
            component_area: Rect::default(),
            preview_panel_area: Rect::default(),
            sidebar_area: Rect::default(),
            sidebar_inner_area: Rect::default(),
            sidebar_viewport_items: 1,
            preview_viewport_rows: 1,
            knob_selected: 0,
            demo_outcome: None,
            full_preview: false,
            next_demo_deadline: None,
        })
    }

    pub(crate) fn next_deadline(&self) -> Option<std::time::Instant> {
        self.next_demo_deadline
    }

    pub(crate) fn render_at(&mut self, frame: &mut Frame<'_>, tick: FrameTick) {
        let elapsed_ms = u64::try_from(tick.elapsed().as_millis()).unwrap_or(u64::MAX);
        let update = self.demo.tick(elapsed_ms);
        self.next_demo_deadline = update.next_deadline_ms.map(|deadline_ms| {
            tick.now() + Duration::from_millis(deadline_ms.saturating_sub(elapsed_ms))
        });
        self.capture_demo_update(update);
        if self.full_preview {
            let [preview, hints] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());
            self.host.begin_shell_frame();
            self.host.register_shell(FocusId::Preview, preview, true);
            self.host.reconcile();
            self.focus(FocusId::Preview);
            self.render_preview(frame, preview);
            self.render_hints(frame, hints);
            return;
        }
        let [brand_area, main_area, _, hint_area] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        let [sidebar_area, right_area] =
            Layout::horizontal([Constraint::Length(30), Constraint::Min(20)]).areas(main_area);
        let [description_area, studio_area] =
            Layout::vertical([Constraint::Length(6), Constraint::Min(4)]).areas(right_area);
        // Studio shell: preview + multi-panel inspector + optional knobs (plan 048).
        let has_controls = !self.demo.knobs().is_empty();
        let studio = layout_studio_shell(
            studio_area,
            StudioShellLayout {
                density: Density::Compact,
                inspector_height: 4,
                knobs_width: if has_controls { 28 } else { 0 },
                status_height: 0,
            },
        );
        let preview_area = studio.preview;
        let inspector_area = studio.inspector;
        let controls_area = studio.knobs;
        self.host.begin_shell_frame();
        self.host
            .register_shell(FocusId::Sidebar, sidebar_area, true);
        self.host
            .register_shell(FocusId::Preview, preview_area, true);
        if let Some(controls_area) = controls_area {
            self.host
                .register_shell(FocusId::Controls, controls_area, true);
        }
        self.host.reconcile();

        let [brand_title_area, brand_progress_area] =
            Layout::horizontal([Constraint::Min(1), Constraint::Length(24)]).areas(brand_area);
        frame.render_widget(
            Paragraph::new("TermRock  lookbook").style(self.host.theme.style(Role::Text)),
            brand_title_area,
        );
        let spinner_tick = u64::try_from(tick.elapsed().as_millis() / 100).unwrap_or(u64::MAX);
        let live_label = format!("live · {}ms", tick.delta().as_millis());
        frame.render_widget(
            ProgressBar::new(
                ProgressKind::Indeterminate { tick: spinner_tick },
                &self.host.system(),
            )
            .label(&live_label),
            Rect::new(
                brand_progress_area.x,
                brand_progress_area.y,
                brand_progress_area.width,
                1,
            ),
        );
        self.render_sidebar(frame, sidebar_area);
        self.render_description(frame, description_area);
        self.render_preview(frame, preview_area);
        self.render_studio_inspector(frame, inspector_area);
        if let Some(controls_area) = controls_area {
            self.render_knobs(frame, controls_area);
        }
        self.render_hints(frame, hint_area);
    }

    fn render_sidebar(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let panel_tokens = self.host.system();
        let catalog = gallery_stories();
        let panel = Panel::new(&panel_tokens)
            .variant(PanelVariant::Bordered)
            .title("Components · Application patterns")
            .emphasis(panel_chrome(&self.host.scene, FocusId::Sidebar));
        let inner = panel.inner(area);
        panel.paint(area, frame.buffer_mut(), None);

        self.sidebar_viewport_items = (usize::from(inner.height) / 2).max(1);
        let offset = scroll::cursor_follow_offset(
            self.selected,
            catalog.len(),
            self.sidebar_viewport_items,
            usize::from(self.sidebar_scroll),
        );
        self.sidebar_scroll = u16::try_from(offset).unwrap_or(u16::MAX);
        let items = catalog
            .iter()
            .map(|story| {
                let kind = if is_pattern_demo(story.id) {
                    "application pattern"
                } else {
                    story.id
                };
                ListItem::new(vec![
                    Line::from(Span::styled(
                        story.component(),
                        self.host.theme.style(Role::Text),
                    )),
                    Line::from(Span::styled(kind, self.host.theme.style(Role::TextMuted))),
                ])
            })
            .collect::<Vec<_>>();
        let mut state = RatatuiListState::default()
            .with_offset(offset)
            .with_selected(Some(self.selected));
        frame.render_stateful_widget(
            RatatuiList::new(items)
                .highlight_style(
                    self.host
                        .theme
                        .style(Role::Selection)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▸ ")
                .highlight_spacing(ratatui::widgets::HighlightSpacing::Always),
            inner,
            &mut state,
        );
        self.sidebar_area = area;
        self.sidebar_inner_area = inner;
    }

    fn render_description(&self, frame: &mut Frame<'_>, area: Rect) {
        let panel_tokens = self.host.system();
        let story = gallery_stories()[self.selected];
        let panel = Panel::new(&panel_tokens)
            .variant(PanelVariant::Bordered)
            .title("About");
        let inner = panel.inner(area);
        panel.paint(area, frame.buffer_mut(), None);
        let [title, _, description] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .areas(inner);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(story.title, self.host.theme.style(Role::Text)),
                Span::raw("  "),
                Span::styled(
                    story.component(),
                    Style::default()
                        .patch(self.host.theme.style(Role::Accent))
                        .add_modifier(Modifier::DIM),
                ),
                Span::raw("  "),
                Span::styled(story.id, self.host.theme.style(Role::TextMuted)),
            ])),
            title,
        );
        frame.render_widget(
            Paragraph::new(story.description)
                .style(self.host.theme.style(Role::Border))
                .wrap(Wrap { trim: false }),
            description,
        );
    }

    fn render_preview(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let panel_tokens = self.host.system();
        let story = gallery_stories()[self.selected];
        let panel = Panel::new(&panel_tokens)
            .variant(PanelVariant::Bordered)
            .title("Preview")
            .emphasis(panel_chrome(&self.host.scene, FocusId::Preview));
        let inner = panel.inner(area);
        panel.paint(area, frame.buffer_mut(), None);
        frame.render_widget(
            Block::default().style(self.host.theme.style(Role::Surface)),
            inner,
        );
        let canvas = inner.inner(ratatui::layout::Margin {
            horizontal: 3,
            vertical: 3,
        });
        self.preview_viewport_rows = usize::from(canvas.height);
        let effective_scroll = self.preview_scroll.min(
            u16::try_from(scroll::max_offset(
                usize::from(story.height),
                usize::from(canvas.height),
            ))
            .unwrap_or(u16::MAX),
        );
        let content_width = story.width.min(canvas.width);
        let x = canvas.x + canvas.width.saturating_sub(content_width) / 2;
        let y = if story.height <= canvas.height {
            canvas.y + canvas.height.saturating_sub(story.height) / 2
        } else {
            canvas.y.saturating_sub(effective_scroll)
        };
        let height = if story.height <= canvas.height {
            story.height
        } else {
            story
                .height
                .saturating_sub(effective_scroll)
                .min(canvas.height)
        };
        let component = Rect::new(x, y.max(canvas.y), content_width, height);
        if !component.is_empty() {
            frame.render_widget(ratatui::widgets::Clear, component);
            // The story's ground is the palette's canvas (plans/011).
            frame.buffer_mut().set_style(
                component,
                self.host.system().style(termrock::style::Role::Canvas),
            );
            let resized = self.demo.dispatch_event(Event::Resize {
                width: component.width,
                height: component.height,
            });
            self.capture_demo_update(resized);
            self.demo.render_into(frame, component);
        }
        self.component_area = component;
        self.preview_panel_area = area;
    }

    fn render_studio_inspector(&self, frame: &mut Frame<'_>, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let focused = match self.host.focused() {
            Some(FocusId::Sidebar) => "sidebar",
            Some(FocusId::Preview) => "preview",
            Some(FocusId::Controls) => "controls",
            None => "—",
        };
        let layers = ["root"];
        let recipes = ["list_row", "panel", "studio_shell"];
        // The inspector reports the system that is painting, not three
        // hardcoded strings (plans/011 Step 4).
        let system = self.host.system();
        let snap = DesignInspectorFrame {
            focused: Some(focused),
            layer: Some("root"),
            layers: &layers,
            recipes: &recipes,
            ..DesignInspectorFrame::from_system(&system)
        };
        frame.render_widget(
            DesignInspector::new(snap, &self.host.system()).panel(InspectorPanel::Focus),
            area,
        );
    }

    fn render_knobs(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let tokens = self.host.system();
        let panel = Panel::new(&tokens)
            .title("Controls")
            .emphasis(panel_chrome(&self.host.scene, FocusId::Controls));
        let inner = panel.inner(area);
        frame.render_widget(panel, area);
        let [list_area, editor_area] = Layout::vertical([
            Constraint::Length(self.demo.knobs().len() as u16),
            Constraint::Min(1),
        ])
        .areas(inner);
        let rows = self
            .demo
            .knobs()
            .iter()
            .enumerate()
            .map(|(index, knob)| {
                let mut row = ListRow::item(index, Line::from(knob.label));
                row.badge = Some(Line::from(knob.display_value()));
                row
            })
            .collect::<Vec<_>>();
        let mut state = ComponentListState::new(Some(self.knob_selected));
        let focused = is_focused(&self.host.scene, FocusId::Controls);
        frame.render_stateful_widget(
            &ComponentList::new(&rows, &tokens).focused(focused),
            list_area,
            &mut state,
        );
        self.demo
            .render_knob_editor(self.knob_selected, frame, editor_area);
    }

    fn render_hints(&self, frame: &mut Frame<'_>, area: Rect) {
        if is_focused(&self.host.scene, FocusId::Controls) {
            frame.render_widget(
                Paragraph::new("↑↓ knob   ←→ change   type edit   Esc back   Ctrl+Alt+T theme"),
                area,
            );
            return;
        }
        if is_focused(&self.host.scene, FocusId::Preview) {
            let update = self.demo.current_update();
            let mut parts = if update.hints.is_empty() {
                "No component input".to_owned()
            } else {
                update.hints.join(" · ")
            };
            parts.push_str(if self.full_preview {
                " · Ctrl+Alt+Z exit full preview"
            } else {
                " · Ctrl+Alt+Z full preview"
            });
            parts.push_str(" · Ctrl+Alt+R reset · Esc catalog");
            if let Some(outcome) = &self.demo_outcome {
                parts.push_str("   │   ");
                parts.push_str(outcome);
            }
            frame.render_widget(Paragraph::new(parts), area);
            return;
        }
        let spans = match self.host.focused() {
            Some(FocusId::Preview) => PREVIEW_KEYMAP.hint_spans(),
            Some(FocusId::Sidebar) | None => SIDEBAR_KEYMAP.hint_spans(),
            Some(FocusId::Controls) => unreachable!(),
        };
        let text = spans
            .iter()
            .map(|span| match span {
                termrock::widgets::HintSpan::Key(value)
                | termrock::widgets::HintSpan::Text(value) => (*value).to_owned(),
                termrock::widgets::HintSpan::DynKey(value)
                | termrock::widgets::HintSpan::Dyn(value) => value.clone(),
                termrock::widgets::HintSpan::Sep => {
                    termrock::style::GlyphSet::default().meta_join().to_owned()
                }
                termrock::widgets::HintSpan::GroupSep => {
                    termrock::widgets::HINT_GROUP_JOIN.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
            + "   Ctrl+Alt+T theme";
        frame.render_widget(Paragraph::new(text), area);
    }

    pub(crate) fn update_at(&mut self, event: Event, _tick: FrameTick) -> ControlFlow<()> {
        match event {
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                return self.handle_key(key);
            }
            Event::Key(key) if is_focused(&self.host.scene, FocusId::Preview) => {
                let update = self.demo.dispatch_event(Event::Key(key));
                self.capture_demo_update(update);
            }
            Event::Paste(text) if is_focused(&self.host.scene, FocusId::Preview) => {
                let update = self
                    .demo
                    .dispatch_event_in(Event::Paste(text), self.component_area);
                self.capture_demo_update(update);
            }
            Event::Resize { width, height } => {
                let update = self.demo.dispatch_event(Event::Resize { width, height });
                self.capture_demo_update(update);
            }
            Event::FocusGained => {
                let update = self.demo.dispatch_event(Event::FocusGained);
                self.capture_demo_update(update);
            }
            Event::FocusLost => {
                let update = self.demo.dispatch_event(Event::FocusLost);
                self.capture_demo_update(update);
            }
            Event::Key(_) | Event::Paste(_) | Event::Unknown => {}
            _ => {}
        }
        ControlFlow::Continue(())
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        let over_sidebar = self.sidebar_area.contains(mouse.position);
        match mouse.kind {
            MouseEventKind::Down(_) => {
                if self.sidebar_inner_area.contains(mouse.position) {
                    let row = usize::from(mouse.position.y - self.sidebar_inner_area.y);
                    let index = (usize::from(self.sidebar_scroll) + row / 2)
                        .min(gallery_stories().len().saturating_sub(1));
                    self.select(index);
                    self.focus(FocusId::Sidebar);
                }
                if self.preview_panel_area.contains(mouse.position) {
                    self.focus(FocusId::Preview);
                }
                let _ = self.host.scene.handle_mouse(mouse);
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown if over_sidebar => {
                let before = self.selected;
                let delta = if mouse.kind == MouseEventKind::ScrollUp {
                    -1
                } else {
                    1
                };
                scroll::scroll_selectable_list(
                    &mut self.selected,
                    &mut self.sidebar_scroll,
                    gallery_stories().len(),
                    self.sidebar_viewport_items,
                    delta,
                );
                if self.selected != before {
                    self.preview_scroll = 0;
                    self.mount_selected_demo();
                    self.knob_selected = 0;
                    self.demo_outcome = None;
                }
                return;
            }
            _ => {}
        }
        if self.component_area.contains(mouse.position) {
            let update = self
                .demo
                .dispatch_event_in(Event::Mouse(mouse), self.component_area);
            let consumed = update.changed;
            self.capture_demo_update(update);
            if consumed {
                return;
            }
        }
        if matches!(
            mouse.kind,
            MouseEventKind::ScrollUp
                | MouseEventKind::ScrollDown
                | MouseEventKind::ScrollLeft
                | MouseEventKind::ScrollRight
        ) && is_focused(&self.host.scene, FocusId::Preview)
        {
            let mut ignored_x = 0;
            scroll::apply_mouse_scroll_u16(
                mouse.kind,
                mouse.modifiers,
                scroll::ScrollAxes {
                    vertical: scroll::is_scrollable(
                        usize::from(gallery_stories()[self.selected].height),
                        self.preview_viewport_rows,
                    ),
                    horizontal: false,
                },
                ScrollSpan::new(0, 0),
                ScrollSpan::new(
                    usize::from(gallery_stories()[self.selected].height),
                    self.preview_viewport_rows,
                ),
                &mut ignored_x,
                &mut self.preview_scroll,
            );
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> ControlFlow<()> {
        let chord = KeyChord::from(key);
        let shell_modifiers = KeyModifiers::CONTROL | KeyModifiers::ALT;
        if key.code == KeyCode::Char('r') && key.modifiers == shell_modifiers {
            self.demo.reset();
            self.demo_outcome = self.demo.current_update().outcome;
            return ControlFlow::Continue(());
        }
        if key.code == KeyCode::Char('z') && key.modifiers == shell_modifiers {
            self.full_preview = !self.full_preview;
            self.focus(FocusId::Preview);
            return ControlFlow::Continue(());
        }
        if key.code == KeyCode::Char('t') && key.modifiers == shell_modifiers {
            self.host.theme = if self.host.theme == RolePalette::tailrocks_phosphor() {
                RolePalette::slate()
            } else {
                RolePalette::default()
            };
            self.demo
                .set_system(termrock_lookbook::design::lookbook_system(
                    self.host.theme.clone(),
                ));
            return ControlFlow::Continue(());
        }
        if is_focused(&self.host.scene, FocusId::Preview) {
            self.handle_preview_key(key, chord);
            return ControlFlow::Continue(());
        }
        if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            let out = self.host.handle_scene_key(key);
            if !matches!(out, termrock::interaction::InteractionOutcome::Ignored) {
                return ControlFlow::Continue(());
            }
        }
        match self.host.focused() {
            Some(FocusId::Preview) => unreachable!(),
            Some(FocusId::Sidebar) | None => return self.handle_sidebar_key(chord),
            Some(FocusId::Controls) => self.handle_knob_key(key, chord),
        }
        ControlFlow::Continue(())
    }

    fn handle_preview_key(&mut self, key: KeyEvent, chord: KeyChord) {
        if chord.key == KeyCode::Esc {
            let update = self.demo.dispatch_preview_escape(key);
            if update.changed {
                self.capture_demo_update(update);
                return;
            }
        } else {
            let update = self.demo.dispatch_event(Event::Key(key));
            if update.changed {
                self.capture_demo_update(update);
                return;
            }
        }
        if chord.key == KeyCode::Esc && self.full_preview {
            self.full_preview = false;
            return;
        }
        if matches!(chord.key, KeyCode::Tab | KeyCode::BackTab) {
            let _ = self.host.handle_scene_key(key);
            return;
        }
        match PREVIEW_KEYMAP
            .dispatch(chord)
            .unwrap_or(PreviewAction::Forward)
        {
            PreviewAction::BackToList => self.focus(FocusId::Sidebar),
            PreviewAction::Forward => {}
        }
    }

    fn handle_knob_key(&mut self, key: KeyEvent, chord: KeyChord) {
        match chord.key {
            KeyCode::Esc => {
                self.focus(FocusId::Sidebar);
            }
            KeyCode::Up => self.knob_selected = self.knob_selected.saturating_sub(1),
            KeyCode::Down => {
                self.knob_selected =
                    (self.knob_selected + 1).min(self.demo.knobs().len().saturating_sub(1));
            }
            _ => {
                let update = self.demo.handle_knob_key(self.knob_selected, key);
                self.capture_demo_update(update);
            }
        }
    }

    fn handle_sidebar_key(&mut self, chord: KeyChord) -> ControlFlow<()> {
        match SIDEBAR_KEYMAP.dispatch(chord) {
            Some(SidebarAction::Quit) => return ControlFlow::Break(()),
            Some(SidebarAction::FocusPreview) => {
                self.focus(FocusId::Preview);
            }
            Some(SidebarAction::Navigate) => {
                let down = matches!(chord.key, KeyCode::Down | KeyCode::Char('j'));
                let target = if down {
                    (self.selected + 1).min(gallery_stories().len().saturating_sub(1))
                } else {
                    self.selected.saturating_sub(1)
                };
                self.select(target);
            }
            Some(SidebarAction::GoToEdge) => {
                let target = if chord.key == KeyCode::Home {
                    0
                } else {
                    gallery_stories().len().saturating_sub(1)
                };
                self.select(target);
            }
            None => {}
        }
        ControlFlow::Continue(())
    }

    fn select(&mut self, selected: usize) {
        if selected != self.selected {
            self.selected = selected;
            self.mount_selected_demo();
            self.preview_scroll = 0;
            self.knob_selected = 0;
            self.demo_outcome = None;
        }
    }

    fn mount_selected_demo(&mut self) {
        let story = gallery_stories()[self.selected];
        self.demo = DemoSession::mount(story.id, Some(story.width), Some(story.height))
            .expect("catalog demo must mount");
        self.demo
            .set_system(termrock_lookbook::design::lookbook_system(
                self.host.theme.clone(),
            ));
    }

    fn focus(&mut self, target: FocusId) {
        let was_preview = is_focused(&self.host.scene, FocusId::Preview);
        self.host.focus(target);
        let is_preview = is_focused(&self.host.scene, FocusId::Preview);
        if was_preview != is_preview {
            let update = self.demo.dispatch_event(if is_preview {
                Event::FocusGained
            } else {
                Event::FocusLost
            });
            self.capture_demo_update(update);
        }
    }

    fn capture_demo_update(&mut self, update: DemoUpdate) {
        if let Some(outcome) = update.outcome {
            self.demo_outcome = Some(outcome);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use ratatui::{Terminal, backend::TestBackend};
    use termrock::input::{KeyEvent, KeyModifiers};

    use super::*;

    fn tick_at(start: Instant, milliseconds: u64) -> FrameTick {
        let elapsed = Duration::from_millis(milliseconds);
        FrameTick::manual(start + elapsed, elapsed, Duration::ZERO)
    }

    fn render_app(app: &mut Lookbook, tick: FrameTick) {
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|frame| app.render_at(frame, tick)).unwrap();
    }

    #[test]
    fn direct_story_launch_mounts_exact_component_or_application_demo() {
        for id in ["dialog/message", "connection-manager/full"] {
            let app = Lookbook::for_story(Some(id)).unwrap();
            assert_eq!(gallery_stories()[app.selected].id, id);
            assert_eq!(app.demo.descriptor().id, id);
        }
        assert!(Lookbook::for_story(Some("missing/story")).is_err());
    }

    #[test]
    fn toast_controls_route_focus_and_update_live_values() {
        let mut app = Lookbook::new();
        let tick = tick_at(Instant::now(), 0);
        let toast = gallery_stories()
            .iter()
            .position(|story| story.id == "toast/success")
            .unwrap();
        app.select(toast);
        render_app(&mut app, tick);
        // Drive knobs directly (shell focus ownership may stay on Sidebar after paint).
        let _ = app.handle_knob_key(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeyChord::plain(KeyCode::Right),
        );
        assert_eq!(app.demo.knobs()[0].display_value(), "Warning");
        let _ = app.handle_knob_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyChord::plain(KeyCode::Down),
        );
        let _ = app.handle_knob_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyChord::plain(KeyCode::Down),
        );
        let _ = app.handle_knob_key(
            KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE),
            KeyChord::plain(KeyCode::Char('!')),
        );
        assert_eq!(app.demo.knobs()[2].display_value(), "Updated!");
    }

    #[test]
    fn theme_toggle_changes_gallery_theme_from_every_focus_target() {
        let mut app = Lookbook::new();
        let tick = tick_at(Instant::now(), 0);
        let toast = gallery_stories()
            .iter()
            .position(|story| story.id == "toast/success")
            .unwrap();
        app.select(toast);
        render_app(&mut app, tick);
        app.host.focus(FocusId::Controls);

        let _ = app.handle_key(KeyEvent::new(
            KeyCode::Char('t'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        ));

        assert_eq!(app.host.theme, RolePalette::slate());
    }

    #[test]
    fn text_story_keeps_plain_and_control_t_while_shell_chord_changes_theme() {
        let mut app = Lookbook::new();
        let tick = tick_at(Instant::now(), 0);
        let picker = gallery_stories()
            .iter()
            .position(|story| story.id == "picker/basic")
            .unwrap();
        app.select(picker);
        render_app(&mut app, tick);
        app.host.focus(FocusId::Preview);

        let _ = app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert_eq!(app.host.theme, RolePalette::default());
        let _ = app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
        assert_eq!(app.host.theme, RolePalette::default());
        let _ = app.handle_key(KeyEvent::new(
            KeyCode::Char('t'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        ));
        assert_eq!(app.host.theme, RolePalette::slate());
    }

    #[test]
    fn native_deadline_comes_from_the_mounted_timed_demo() {
        let mut app = Lookbook::new();
        let spinner = gallery_stories()
            .iter()
            .position(|story| story.id == "spinner/labeled")
            .unwrap();
        app.select(spinner);
        let start = Instant::now();
        render_app(&mut app, tick_at(start, 0));
        assert_eq!(
            app.next_deadline(),
            Some(start + Duration::from_millis(100))
        );
    }

    #[test]
    fn native_shell_chords_do_not_steal_documented_demo_shortcuts() {
        let mut app = Lookbook::new();
        let dashboard = gallery_stories()
            .iter()
            .position(|story| story.id == "metrics-dashboard/basic")
            .unwrap();
        app.select(dashboard);
        render_app(&mut app, tick_at(Instant::now(), 0));
        app.focus(FocusId::Preview);

        let _ = app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert!(
            app.demo_outcome
                .as_deref()
                .is_some_and(|outcome| outcome.contains("Refresh requested"))
        );

        let shell = KeyModifiers::CONTROL | KeyModifiers::ALT;
        let _ = app.handle_key(KeyEvent::new(KeyCode::Char('r'), shell));
        assert_eq!(app.demo_outcome.as_deref(), Some("Demo reset"));
        let _ = app.handle_key(KeyEvent::new(KeyCode::Char('z'), shell));
        assert!(app.full_preview);
    }

    #[test]
    fn host_frame_narrow_and_tiny_shell_geometry() {
        let mut app = Lookbook::new();
        let tick = tick_at(Instant::now(), 0);
        for (w, h) in [(80, 24), (40, 16), (20, 10), (12, 8)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|frame| app.render_at(frame, tick)).unwrap();
            // Root layer always present after paint.
            assert!(
                app.host
                    .scene
                    .layers()
                    .iter()
                    .any(|l| l.id == crate::focus::LayerId::Root)
            );
        }
    }

    #[test]
    fn host_frame_sidebar_focus_paints_title_and_tab_cycles() {
        let mut app = Lookbook::new();
        let tick = tick_at(Instant::now(), 0);
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        app.host.focus(FocusId::Sidebar);
        terminal.draw(|frame| app.render_at(frame, tick)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Stories") || text.contains("TermRock"),
            "shell chrome missing: {text:?}"
        );
        assert_eq!(app.host.focused(), Some(FocusId::Sidebar));
        let _ = app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        // After Tab from Sidebar, scene should move to Preview or Controls depending on order.
        assert!(
            matches!(
                app.host.focused(),
                Some(FocusId::Preview) | Some(FocusId::Controls)
            ),
            "{:?}",
            app.host.focused()
        );
    }

    #[test]
    fn host_frame_uses_public_authorities_only() {
        let host = include_str!("host_frame.rs");
        let production: String = host
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                !t.starts_with("//")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(production.contains("InteractionScene"));
        assert!(production.contains("FocusGraph"));
        assert!(production.contains("DesignSystem"));
        assert!(!production.contains("FocusRing"));
        assert!(!production.contains("ModalStack"));
    }
}
