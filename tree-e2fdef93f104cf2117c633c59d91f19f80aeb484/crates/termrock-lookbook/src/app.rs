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
    Theme,
    input::{
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
        MouseEventKind,
    },
    interaction::{ModalStack, Outcome, render_backdrop},
    keymap::KeyChord,
    layout::centered_rect,
    runtime::FrameTick,
    scroll::{self, ScrollSpan},
    style::Role,
    widgets::{
        Action, ChoiceDialog, ChoiceDialogState, Dialog, List as ComponentList, ListRow,
        ListState as ComponentListState, Panel, PanelEmphasis, Progress, ProgressKind, RowRole,
        Severity, Toast, ToastLifetime, ToastState,
    },
};

use crate::{
    PREVIEW_KEYMAP, PreviewAction, SIDEBAR_KEYMAP, SidebarAction,
    focus::{FocusId, FocusRing, FocusScope},
    interactors::StoryInteraction,
    stories::gallery_stories,
};

const PROTOTYPE_TOAST_TTL: Duration = Duration::from_secs(2);

#[derive(Debug)]
struct PrototypeModal {
    state: ChoiceDialogState<FocusId>,
}

impl PrototypeModal {
    fn new() -> Self {
        Self {
            state: ChoiceDialogState::new(Some(FocusId::ModalContinue)),
        }
    }
}

fn prototype_modal_actions() -> [Action<'static, FocusId>; 3] {
    [
        Action {
            id: FocusId::ModalContinue,
            label: "Continue",
            enabled: true,
            style: None,
        },
        Action {
            id: FocusId::ModalDisabled,
            label: "Unavailable",
            enabled: false,
            style: None,
        },
        Action {
            id: FocusId::ModalCancel,
            label: "Cancel",
            enabled: true,
            style: None,
        },
    ]
}

pub(crate) struct Lookbook {
    selected: usize,
    preview_scroll: u16,
    sidebar_scroll: u16,
    focus: FocusRing,
    interactor: Box<dyn StoryInteraction>,
    component_area: Rect,
    preview_panel_area: Rect,
    sidebar_area: Rect,
    sidebar_inner_area: Rect,
    sidebar_viewport_items: usize,
    preview_viewport_rows: usize,
    theme: Theme,
    knob_selected: usize,
    prototype_toast: ToastState,
    modals: ModalStack<PrototypeModal>,
}

impl Lookbook {
    pub(crate) fn new() -> Self {
        let theme = Theme::default();
        let mut interactor = gallery_stories()[0].make_interactor();
        interactor.set_theme(theme.clone());
        Self {
            selected: 0,
            preview_scroll: 0,
            sidebar_scroll: 0,
            focus: FocusRing::new(Some(FocusId::Sidebar)),
            interactor,
            component_area: Rect::default(),
            preview_panel_area: Rect::default(),
            sidebar_area: Rect::default(),
            sidebar_inner_area: Rect::default(),
            sidebar_viewport_items: 1,
            preview_viewport_rows: 1,
            theme,
            knob_selected: 0,
            prototype_toast: ToastState::new(ToastLifetime::ExpiresAfter(PROTOTYPE_TOAST_TTL)),
            modals: ModalStack::new(),
        }
    }

    pub(crate) fn next_deadline(&self) -> Option<std::time::Instant> {
        self.prototype_toast.next_deadline()
    }

    pub(crate) fn render_at(&mut self, frame: &mut Frame<'_>, tick: FrameTick) {
        let [brand_area, main_area, _, hint_area] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        let [sidebar_area, right_area] =
            Layout::horizontal([Constraint::Length(30), Constraint::Min(20)]).areas(main_area);
        let [description_area, preview_area] =
            Layout::vertical([Constraint::Length(6), Constraint::Min(4)]).areas(right_area);
        let has_controls = !self.interactor.knobs().is_empty();
        let (preview_area, controls_area) = if has_controls {
            let [preview_area, controls_area] =
                Layout::horizontal([Constraint::Min(20), Constraint::Length(32)])
                    .areas(preview_area);
            (preview_area, Some(controls_area))
        } else {
            (preview_area, None)
        };
        let modal_area = centered_rect(52, 9, frame.area());

        self.focus.begin_frame();
        self.focus
            .register(FocusScope::Screen, FocusId::Sidebar, sidebar_area, true);
        self.focus
            .register(FocusScope::Screen, FocusId::Preview, preview_area, true);
        if let Some(controls_area) = controls_area {
            self.focus
                .register(FocusScope::Screen, FocusId::Controls, controls_area, true);
        }
        if self.modals.is_open() {
            for action in prototype_modal_actions() {
                self.focus.register(
                    FocusScope::Modal,
                    action.id,
                    Rect::default(),
                    action.enabled,
                );
            }
        }
        self.focus.reconcile();

        let [brand_title_area, brand_progress_area] =
            Layout::horizontal([Constraint::Min(1), Constraint::Length(24)]).areas(brand_area);
        frame.render_widget(
            Paragraph::new("TermRock  lookbook").style(self.theme.style(Role::Text)),
            brand_title_area,
        );
        let spinner_tick = u64::try_from(tick.elapsed().as_millis() / 100).unwrap_or(u64::MAX);
        let live_label = format!("live · {}ms", tick.delta().as_millis());
        frame.render_widget(
            Progress::new(
                ProgressKind::Indeterminate { tick: spinner_tick },
                &self.theme,
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
        if let Some(controls_area) = controls_area {
            self.render_knobs(frame, controls_area);
        }
        self.render_hints(frame, hint_area);
        if self.prototype_toast.is_visible(tick) {
            frame.render_widget(
                Toast::new(
                    &self.theme,
                    "Preview updated · expires in 2s",
                    Severity::Success,
                ),
                frame.area(),
            );
        }
        if self.modals.is_open() {
            self.render_focus_modal(frame, modal_area);
        }
    }

    fn render_sidebar(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let catalog = gallery_stories();
        let block = Panel::new(&self.theme)
            .title("Stories")
            .emphasis(self.focus.panel_emphasis_for(FocusId::Sidebar))
            .block();
        let inner = block.inner(area);
        frame.render_widget(block, area);

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
                ListItem::new(vec![
                    Line::from(Span::styled(story.component, self.theme.style(Role::Text))),
                    Line::from(Span::styled(story.id, self.theme.style(Role::TextMuted))),
                ])
            })
            .collect::<Vec<_>>();
        let mut state = RatatuiListState::default()
            .with_offset(offset)
            .with_selected(Some(self.selected));
        frame.render_stateful_widget(
            RatatuiList::new(items)
                .highlight_style(
                    self.theme
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
        let story = gallery_stories()[self.selected];
        let block = Panel::new(&self.theme).title("About").block();
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let [title, _, description] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .areas(inner);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(story.title, self.theme.style(Role::Text)),
                Span::raw("  "),
                Span::styled(
                    story.component,
                    Style::default()
                        .patch(self.theme.style(Role::Accent))
                        .add_modifier(Modifier::DIM),
                ),
                Span::raw("  "),
                Span::styled(story.id, self.theme.style(Role::TextMuted)),
            ])),
            title,
        );
        frame.render_widget(
            Paragraph::new(story.description)
                .style(self.theme.style(Role::Border))
                .wrap(Wrap { trim: false }),
            description,
        );
    }

    fn render_preview(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let story = gallery_stories()[self.selected];
        let block = Panel::new(&self.theme)
            .title("Preview")
            .emphasis(self.focus.panel_emphasis_for(FocusId::Preview))
            .block();
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(
            Block::default().style(self.theme.style(Role::Surface)),
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
            self.interactor.render(frame, component);
        }
        self.component_area = component;
        self.preview_panel_area = area;
    }

    fn render_knobs(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let panel = Panel::new(&self.theme)
            .title("Controls")
            .emphasis(self.focus.panel_emphasis_for(FocusId::Controls));
        let inner = panel.inner(area);
        frame.render_widget(panel, area);
        let [list_area, editor_area] = Layout::vertical([
            Constraint::Length(self.interactor.knobs().len() as u16),
            Constraint::Min(1),
        ])
        .areas(inner);
        let rows = self
            .interactor
            .knobs()
            .iter()
            .enumerate()
            .map(|(index, knob)| ListRow {
                id: index,
                label: Line::from(knob.label),
                trailing: Some(Line::from(knob.display_value())),
                role: RowRole::Item,
                enabled: true,
            })
            .collect::<Vec<_>>();
        let mut state = ComponentListState::new(Some(self.knob_selected));
        state.set_focused(self.focus.is_focused(FocusId::Controls));
        frame.render_stateful_widget(
            &ComponentList::new(&rows, &self.theme),
            list_area,
            &mut state,
        );
        self.interactor
            .render_knob_editor(self.knob_selected, frame, editor_area);
    }

    fn render_hints(&self, frame: &mut Frame<'_>, area: Rect) {
        if self.modals.is_open() {
            frame.render_widget(
                Paragraph::new("Tab/Shift-Tab trapped   Enter choose   Esc close + restore"),
                area,
            );
            return;
        }
        if self.focus.is_focused(FocusId::Controls) {
            frame.render_widget(
                Paragraph::new("↑↓ knob   ←→ change   type edit   Esc back   t/^t theme"),
                area,
            );
            return;
        }
        let spans = match self.focus.focused() {
            Some(FocusId::Preview) => PREVIEW_KEYMAP.hint_spans(),
            Some(FocusId::Sidebar) | None => SIDEBAR_KEYMAP.hint_spans(),
            Some(FocusId::Controls) => unreachable!(),
            Some(FocusId::ModalContinue | FocusId::ModalDisabled | FocusId::ModalCancel) => {
                unreachable!()
            }
        };
        let text = spans
            .iter()
            .map(|span| match span {
                termrock::widgets::HintSpan::Key(value)
                | termrock::widgets::HintSpan::Text(value) => (*value).to_owned(),
                termrock::widgets::HintSpan::DynKey(value)
                | termrock::widgets::HintSpan::Dyn(value) => value.clone(),
                termrock::widgets::HintSpan::Sep => " · ".to_owned(),
                termrock::widgets::HintSpan::GroupSep => "   ".to_owned(),
            })
            .collect::<Vec<_>>()
            .join(" ")
            + "   t/^t theme";
        frame.render_widget(Paragraph::new(text), area);
    }

    pub(crate) fn update_at(&mut self, event: Event, tick: FrameTick) -> ControlFlow<()> {
        if self.modals.is_open() {
            match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => self.handle_modal_key(key),
                Event::Mouse(mouse) => self.handle_modal_mouse(mouse),
                _ => {}
            }
            return ControlFlow::Continue(());
        }
        match event {
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                return self.handle_key(key, tick);
            }
            Event::Resize { .. } | Event::FocusGained | Event::FocusLost => {}
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
                    self.focus.request_focus(FocusId::Sidebar);
                }
                if self.preview_panel_area.contains(mouse.position) {
                    self.focus.request_focus(FocusId::Preview);
                }
                let _ = self.focus.focus_at(mouse.position);
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
                    self.interactor = gallery_stories()[self.selected].make_interactor();
                    self.interactor.set_theme(self.theme.clone());
                    self.knob_selected = 0;
                }
            }
            MouseEventKind::ScrollUp
            | MouseEventKind::ScrollDown
            | MouseEventKind::ScrollLeft
            | MouseEventKind::ScrollRight
                if self.focus.is_focused(FocusId::Preview) =>
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
            _ => {}
        }
        if self.component_area.contains(mouse.position) {
            self.interactor.handle_mouse(mouse, self.component_area);
        }
    }

    fn handle_key(&mut self, key: KeyEvent, tick: FrameTick) -> ControlFlow<()> {
        let chord = KeyChord::from(key);
        let captures_text = match self.focus.focused() {
            Some(FocusId::Preview) => self.interactor.captures_text_input(),
            Some(FocusId::Controls) => self.interactor.knob_captures_text_input(self.knob_selected),
            Some(FocusId::Sidebar) | None => false,
            Some(FocusId::ModalContinue | FocusId::ModalDisabled | FocusId::ModalCancel) => false,
        };
        if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            let _ = self.focus.handle_key(key.code);
            return ControlFlow::Continue(());
        }
        if key.code == KeyCode::Char('m') && !captures_text {
            self.open_focus_modal();
            return ControlFlow::Continue(());
        }
        let theme_toggle = key.code == KeyCode::Char('t')
            && (key.modifiers.contains(KeyModifiers::CONTROL) || !captures_text);
        if theme_toggle {
            self.theme = if self.theme == Theme::tailrocks_phosphor() {
                Theme::slate()
            } else {
                Theme::default()
            };
            self.interactor.set_theme(self.theme.clone());
            return ControlFlow::Continue(());
        }
        match self.focus.focused() {
            Some(FocusId::Preview) => self.handle_preview_key(key, chord),
            Some(FocusId::Sidebar) | None => return self.handle_sidebar_key(chord),
            Some(FocusId::Controls) => self.handle_knob_key(key, chord, tick),
            Some(FocusId::ModalContinue | FocusId::ModalDisabled | FocusId::ModalCancel) => {
                unreachable!()
            }
        }
        ControlFlow::Continue(())
    }

    fn handle_preview_key(&mut self, key: KeyEvent, chord: KeyChord) {
        if chord.key == KeyCode::Esc && self.interactor.handle_preview_escape(key) {
            return;
        }
        let content = usize::from(gallery_stories()[self.selected].height);
        match PREVIEW_KEYMAP
            .dispatch(chord)
            .unwrap_or(PreviewAction::Forward)
        {
            PreviewAction::BackToList => {
                self.focus.request_focus(FocusId::Sidebar);
            }
            PreviewAction::MovePreviewDown => self.scroll_preview(content, 1),
            PreviewAction::MovePreviewUp => self.scroll_preview(content, -1),
            PreviewAction::PageDown => {
                self.scroll_preview(content, self.preview_viewport_rows as isize)
            }
            PreviewAction::PageUp => {
                self.scroll_preview(content, -(self.preview_viewport_rows as isize))
            }
            PreviewAction::Forward => {
                self.interactor.handle_key(key);
            }
        }
    }

    fn handle_knob_key(&mut self, key: KeyEvent, chord: KeyChord, tick: FrameTick) {
        match chord.key {
            KeyCode::Esc => {
                let _ = self.focus.request_focus(FocusId::Sidebar);
            }
            KeyCode::Up => self.knob_selected = self.knob_selected.saturating_sub(1),
            KeyCode::Down => {
                self.knob_selected =
                    (self.knob_selected + 1).min(self.interactor.knobs().len().saturating_sub(1));
            }
            _ => {
                let changed = self.interactor.handle_knob_key(self.knob_selected, key);
                if changed && gallery_stories()[self.selected].component == "Toast" {
                    self.prototype_toast.show(tick);
                }
            }
        }
    }

    fn handle_sidebar_key(&mut self, chord: KeyChord) -> ControlFlow<()> {
        match SIDEBAR_KEYMAP.dispatch(chord) {
            Some(SidebarAction::Quit) => return ControlFlow::Break(()),
            Some(SidebarAction::FocusPreview) => {
                let _ = self.focus.request_focus(FocusId::Preview);
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
            self.interactor = gallery_stories()[selected].make_interactor();
            self.interactor.set_theme(self.theme.clone());
            self.preview_scroll = 0;
            self.knob_selected = 0;
            self.selected = selected;
        }
    }

    fn scroll_preview(&mut self, content: usize, delta: isize) {
        scroll::apply_delta_u16(
            content,
            self.preview_viewport_rows,
            &mut self.preview_scroll,
            delta,
        );
    }

    fn open_focus_modal(&mut self) {
        self.modals.open(PrototypeModal::new());
        self.focus.push_scope(FocusScope::Modal);
    }

    fn close_focus_modal(&mut self) {
        self.modals.pop();
        self.focus.pop_scope();
    }

    fn handle_modal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                let _ = self.focus.handle_key(key.code);
            }
            KeyCode::Esc => self.close_focus_modal(),
            KeyCode::Enter => {
                let actions = prototype_modal_actions();
                let Some(modal) = self.modals.current_mut() else {
                    return;
                };
                modal.state.focused = self.focus.focused();
                if matches!(
                    modal.state.activate_selected(&actions),
                    Outcome::Activated(_)
                ) {
                    self.close_focus_modal();
                }
            }
            _ => {}
        }
    }

    fn handle_modal_mouse(&mut self, mouse: MouseEvent) {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }
        let _ = self.focus.focus_at(mouse.position);
        let Some(modal) = self.modals.current_mut() else {
            return;
        };
        modal.state.focused = self.focus.focused();
        if matches!(modal.state.click(mouse.position), Outcome::Activated(_)) {
            self.close_focus_modal();
        }
    }

    fn render_focus_modal(&mut self, frame: &mut Frame<'_>, area: Rect) {
        render_backdrop(frame, frame.area());
        let actions = prototype_modal_actions();
        let Some(modal) = self.modals.current_mut() else {
            return;
        };
        modal.state.focused = self.focus.focused();
        frame.render_stateful_widget(
            &ChoiceDialog::new(
                Dialog::new(
                    "Focus trap",
                    Line::from("Tab stays here; close restores the opener.").into(),
                    &self.theme,
                )
                .emphasis(PanelEmphasis::Focused),
                &actions,
            ),
            area,
            &mut modal.state,
        );
        let regions = modal
            .state
            .regions
            .iter()
            .map(|region| (region.id, region.area))
            .collect::<Vec<_>>();
        for (id, action_area) in regions {
            let _ = self.focus.attach_area(FocusScope::Modal, id, action_area);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{ops::ControlFlow, time::Instant};

    use ratatui::{Terminal, backend::TestBackend, layout::Position};
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
    fn toast_controls_route_focus_and_update_live_values() {
        let mut app = Lookbook::new();
        let tick = tick_at(Instant::now(), 0);
        let toast = gallery_stories()
            .iter()
            .position(|story| story.id == "toast/success")
            .unwrap();
        app.select(toast);
        render_app(&mut app, tick);
        assert!(app.focus.request_focus(FocusId::Preview));

        assert_eq!(
            app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), tick),
            ControlFlow::Continue(())
        );
        assert_eq!(app.focus.focused(), Some(FocusId::Controls));
        let _ = app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), tick);
        assert_eq!(app.interactor.knobs()[0].display_value(), "Warning");
        let _ = app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), tick);
        let _ = app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), tick);
        let _ = app.handle_key(KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE), tick);
        assert_eq!(app.interactor.knobs()[2].display_value(), "Updated!");
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
        assert!(app.focus.request_focus(FocusId::Controls));

        let _ = app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE), tick);

        assert_eq!(app.theme, Theme::slate());
    }

    #[test]
    fn text_story_keeps_plain_t_and_uses_control_t_for_theme() {
        let mut app = Lookbook::new();
        let tick = tick_at(Instant::now(), 0);
        let picker = gallery_stories()
            .iter()
            .position(|story| story.id == "text-input/filter")
            .unwrap();
        app.select(picker);
        render_app(&mut app, tick);
        assert!(app.focus.request_focus(FocusId::Preview));

        let _ = app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE), tick);
        assert_eq!(app.theme, Theme::default());
        let _ = app.handle_key(
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
            tick,
        );
        assert_eq!(app.theme, Theme::slate());
    }

    #[test]
    fn toast_interactor_action_starts_and_expires_local_ttl() {
        let mut app = Lookbook::new();
        let toast = gallery_stories()
            .iter()
            .position(|story| story.id == "toast/success")
            .unwrap();
        app.select(toast);
        let start = Instant::now();
        let action_tick = tick_at(start, 100);
        render_app(&mut app, action_tick);
        assert!(app.focus.request_focus(FocusId::Controls));

        app.handle_knob_key(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeyChord::plain(KeyCode::Right),
            action_tick,
        );

        assert!(app.prototype_toast.is_visible(tick_at(start, 2_099)));
        assert!(!app.prototype_toast.is_visible(tick_at(start, 2_100)));
    }

    #[test]
    fn modal_traps_skips_disabled_and_restores_preview_focus() {
        let mut app = Lookbook::new();
        let tick = tick_at(Instant::now(), 0);
        render_app(&mut app, tick);
        assert!(app.focus.request_focus(FocusId::Preview));

        app.open_focus_modal();
        render_app(&mut app, tick);
        assert_eq!(app.focus.focused(), Some(FocusId::ModalContinue));

        app.handle_modal_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.focus.focused(), Some(FocusId::ModalCancel));
        app.handle_modal_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(!app.modals.is_open());
        assert_eq!(app.focus.focused(), Some(FocusId::Preview));
    }

    #[test]
    fn modal_pointer_activation_uses_action_regions_and_never_leaks_to_background() {
        let mut app = Lookbook::new();
        let tick = tick_at(Instant::now(), 0);
        render_app(&mut app, tick);
        assert!(app.focus.request_focus(FocusId::Preview));
        app.open_focus_modal();
        render_app(&mut app, tick);

        let _ = app.update_at(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position::new(0, 0),
                modifiers: KeyModifiers::NONE,
            }),
            tick,
        );
        assert!(app.modals.is_open());
        assert_eq!(app.focus.focused(), Some(FocusId::ModalContinue));

        let cancel = app
            .modals
            .current()
            .unwrap()
            .state
            .regions
            .iter()
            .find(|region| region.id == FocusId::ModalCancel)
            .unwrap()
            .area;
        let _ = app.update_at(
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position::new(cancel.x, cancel.y),
                modifiers: KeyModifiers::NONE,
            }),
            tick,
        );

        assert!(!app.modals.is_open());
        assert_eq!(app.focus.focused(), Some(FocusId::Preview));
    }
}
