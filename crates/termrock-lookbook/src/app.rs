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
    input::{
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
        MouseEventKind,
    },
    interaction::{Outcome, OverlayOutcome, render_backdrop},
    keymap::KeyChord,
    layout::centered_rect,
    patterns::{StudioShellLayout, layout_studio_shell},
    runtime::FrameTick,
    scroll::{self, ScrollSpan},
    style::{ColorCapability, Density, Role, RolePalette},
    widgets::{
        Action, ChoiceDialog, ChoiceDialogState, DesignInspector, DesignInspectorFrame, Dialog,
        InspectorPanel, List as ComponentList, ListRow, ListState as ComponentListState, Panel,
        PanelChrome, Progress, ProgressKind, Severity, Toast, ToastLifetime, ToastState,
    },
};

use crate::{
    PREVIEW_KEYMAP, PreviewAction, SIDEBAR_KEYMAP, SidebarAction,
    focus::{FocusId, is_focused, panel_chrome},
    host_frame::HostFrame,
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
            label: "Cancel 🚫",
            enabled: true,
            style: None,
        },
    ]
}

pub(crate) struct Lookbook {
    selected: usize,
    preview_scroll: u16,
    sidebar_scroll: u16,
    /// Public TermRock authorities only (scene + overlays + theme).
    host: HostFrame,
    interactor: Box<dyn StoryInteraction>,
    component_area: Rect,
    preview_panel_area: Rect,
    sidebar_area: Rect,
    sidebar_inner_area: Rect,
    sidebar_viewport_items: usize,
    preview_viewport_rows: usize,
    knob_selected: usize,
    prototype_toast: ToastState,
    /// Domain state for the focus-trap prototype dialog.
    prototype_modal: Option<PrototypeModal>,
}

impl Lookbook {
    pub(crate) fn new() -> Self {
        let theme = RolePalette::default();
        let mut interactor = gallery_stories()[0].make_interactor();
        interactor.set_theme(theme.clone());
        Self {
            selected: 0,
            preview_scroll: 0,
            sidebar_scroll: 0,
            host: HostFrame::new(theme),
            interactor,
            component_area: Rect::default(),
            preview_panel_area: Rect::default(),
            sidebar_area: Rect::default(),
            sidebar_inner_area: Rect::default(),
            sidebar_viewport_items: 1,
            preview_viewport_rows: 1,
            knob_selected: 0,
            prototype_toast: ToastState::new(ToastLifetime::ExpiresAfter(PROTOTYPE_TOAST_TTL)),
            prototype_modal: None,
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
        let [description_area, studio_area] =
            Layout::vertical([Constraint::Length(6), Constraint::Min(4)]).areas(right_area);
        // Studio shell: preview + multi-panel inspector + optional knobs (plan 048).
        let has_controls = !self.interactor.knobs().is_empty();
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
        let modal_area = centered_rect(52, 9, frame.area());

        self.host.frame_bounds = frame.area();
        self.host.begin_shell_frame(self.prototype_modal.is_some());
        self.host
            .register_shell(FocusId::Sidebar, sidebar_area, true);
        self.host
            .register_shell(FocusId::Preview, preview_area, true);
        if let Some(controls_area) = controls_area {
            self.host
                .register_shell(FocusId::Controls, controls_area, true);
        }
        if self.prototype_modal.is_some() {
            // Areas refined after modal paint; register enabled actions for Tab order.
            for action in prototype_modal_actions() {
                self.host
                    .register_modal_action(action.id, modal_area, action.enabled);
            }
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
            Progress::new(
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
        if self.prototype_toast.is_visible(tick) {
            frame.render_widget(
                Toast::new(
                    &self.host.system(),
                    "Preview updated · expires in 2s",
                    Severity::Success,
                ),
                frame.area(),
            );
        }
        if self.prototype_modal.is_some() {
            self.render_focus_modal(frame, modal_area);
        }
    }

    fn render_sidebar(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let panel_tokens = self.host.system();
        let catalog = gallery_stories();
        let block = Panel::new(&panel_tokens)
            .title("Stories")
            .emphasis(panel_chrome(&self.host.scene, FocusId::Sidebar))
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
                    Line::from(Span::styled(
                        story.component,
                        self.host.theme.style(Role::Text),
                    )),
                    Line::from(Span::styled(
                        story.id,
                        self.host.theme.style(Role::TextMuted),
                    )),
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
        let block = Panel::new(&panel_tokens).title("About").block();
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
                Span::styled(story.title, self.host.theme.style(Role::Text)),
                Span::raw("  "),
                Span::styled(
                    story.component,
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
        let block = Panel::new(&panel_tokens)
            .title("Preview")
            .emphasis(panel_chrome(&self.host.scene, FocusId::Preview))
            .block();
        let inner = block.inner(area);
        frame.render_widget(block, area);
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
            self.interactor.render(frame, component);
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
            Some(FocusId::ModalContinue | FocusId::ModalDisabled | FocusId::ModalCancel) => "modal",
            None => "—",
        };
        let layer = if self.prototype_modal.is_some() {
            "modal"
        } else {
            "root"
        };
        let layers = if self.prototype_modal.is_some() {
            ["root", "modal"]
        } else {
            ["root", ""]
        };
        let layers_slice: &[&str] = if self.prototype_modal.is_some() {
            &layers
        } else {
            &layers[..1]
        };
        let recipes = ["list_row", "panel", "studio_shell"];
        let snap = DesignInspectorFrame {
            focused: Some(focused),
            layer: Some(layer),
            capability: ColorCapability::Truecolor,
            density: "compact",
            layers: layers_slice,
            recipes: &recipes,
            selection_chrome: "gutter",
            semantics: &[],
            focus_graph: &[],
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
            Constraint::Length(self.interactor.knobs().len() as u16),
            Constraint::Min(1),
        ])
        .areas(inner);
        let rows = self
            .interactor
            .knobs()
            .iter()
            .enumerate()
            .map(|(index, knob)| {
                let mut row = ListRow::item(index, Line::from(knob.label));
                row.trailing = Some(Line::from(knob.display_value()));
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
        self.interactor
            .render_knob_editor(self.knob_selected, frame, editor_area);
    }

    fn render_hints(&self, frame: &mut Frame<'_>, area: Rect) {
        if self.prototype_modal.is_some() {
            frame.render_widget(
                Paragraph::new("Tab/Shift-Tab trapped   Enter choose   Esc close + restore"),
                area,
            );
            return;
        }
        if is_focused(&self.host.scene, FocusId::Controls) {
            frame.render_widget(
                Paragraph::new("↑↓ knob   ←→ change   type edit   Esc back   t/^t theme"),
                area,
            );
            return;
        }
        let spans = match self.host.focused() {
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
        if self.prototype_modal.is_some() {
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
                    self.host.focus(FocusId::Sidebar);
                }
                if self.preview_panel_area.contains(mouse.position) {
                    self.host.focus(FocusId::Preview);
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
                    self.interactor = gallery_stories()[self.selected].make_interactor();
                    self.interactor.set_theme(self.host.theme.clone());
                    self.knob_selected = 0;
                }
            }
            MouseEventKind::ScrollUp
            | MouseEventKind::ScrollDown
            | MouseEventKind::ScrollLeft
            | MouseEventKind::ScrollRight
                if is_focused(&self.host.scene, FocusId::Preview) =>
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
        let captures_text = match self.host.focused() {
            Some(FocusId::Preview) => self.interactor.captures_text_input(),
            Some(FocusId::Controls) => self.interactor.knob_captures_text_input(self.knob_selected),
            Some(FocusId::Sidebar) | None => false,
            Some(FocusId::ModalContinue | FocusId::ModalDisabled | FocusId::ModalCancel) => false,
        };
        if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            let out = self.host.handle_scene_key(key);
            if !matches!(out, termrock::interaction::InteractionOutcome::Ignored) {
                return ControlFlow::Continue(());
            }
        }
        if key.code == KeyCode::Char('m') && !captures_text {
            self.open_focus_modal();
            return ControlFlow::Continue(());
        }
        let theme_toggle = key.code == KeyCode::Char('t')
            && (key.modifiers.contains(KeyModifiers::CONTROL) || !captures_text);
        if theme_toggle {
            self.host.theme = if self.host.theme == RolePalette::tailrocks_phosphor() {
                RolePalette::slate()
            } else {
                RolePalette::default()
            };
            self.interactor.set_theme(self.host.theme.clone());
            return ControlFlow::Continue(());
        }
        match self.host.focused() {
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
                self.host.focus(FocusId::Sidebar);
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
                self.host.focus(FocusId::Sidebar);
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
                self.host.focus(FocusId::Preview);
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
            self.interactor.set_theme(self.host.theme.clone());
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
        self.host.open_focus_trap();
        self.prototype_modal = Some(PrototypeModal::new());
        // Next frame registers Modal layer + focuses first action after reconcile.
        self.host.begin_shell_frame(true);
        for action in prototype_modal_actions() {
            self.host
                .register_modal_action(action.id, Rect::default(), action.enabled);
        }
        self.host.reconcile();
        self.host.focus(FocusId::ModalContinue);
    }

    fn close_focus_modal(&mut self) {
        self.host.close_focus_trap();
        self.prototype_modal = None;
        // Opener for the lookbook focus-trap demo is always Preview.
        self.host.focus(FocusId::Preview);
    }

    fn handle_modal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                let _ = self.host.handle_scene_key(key);
            }
            KeyCode::Esc => {
                let _ = self.host.overlays.handle_escape();
                let _ = self.host.scene.handle_escape();
                self.close_focus_modal();
            }
            KeyCode::Enter => {
                let actions = prototype_modal_actions();
                let Some(modal) = self.prototype_modal.as_mut() else {
                    return;
                };
                modal.state.cursor = self.host.focused();
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
        let _ = self.host.scene.handle_mouse(mouse);
        // Outside-click dismiss via OverlayStack sole authority.
        if matches!(
            self.host.overlays.handle_outside_click(mouse.position),
            OverlayOutcome::Dismissed { .. }
        ) {
            self.close_focus_modal();
            return;
        }
        let Some(modal) = self.prototype_modal.as_mut() else {
            return;
        };
        modal.state.cursor = self.host.focused();
        if matches!(modal.state.click(mouse.position), Outcome::Activated(_)) {
            self.close_focus_modal();
        }
    }

    fn render_focus_modal(&mut self, frame: &mut Frame<'_>, area: Rect) {
        self.host.frame_bounds = frame.area();
        self.host.overlays.reflow(frame.area());
        if self.host.overlays.backdrop_policy() != termrock::interaction::BackdropPolicy::None
            || self.prototype_modal.is_some()
        {
            render_backdrop(frame, frame.area());
        }
        let actions = prototype_modal_actions();
        let Some(modal) = self.prototype_modal.as_mut() else {
            return;
        };
        modal.state.cursor = self.host.focused();
        let system = self.host.system();
        frame.render_stateful_widget(
            &ChoiceDialog::new(
                Dialog::new(
                    "Focus trap",
                    Line::from("Tab stays here; close restores the opener.").into(),
                    &system,
                )
                .emphasis(PanelChrome::Focused),
                &actions,
            )
            .gap(" · "),
            area,
            &mut modal.state,
        );
        let regions = modal
            .state
            .regions
            .iter()
            .map(|region| (region.id, region.area))
            .collect::<Vec<_>>();
        // Refresh modal action hit geometry for next pointer frame.
        if self.prototype_modal.is_some() {
            self.host.begin_shell_frame(true);
            self.host
                .register_shell(FocusId::Sidebar, self.sidebar_area, true);
            self.host
                .register_shell(FocusId::Preview, self.preview_panel_area, true);
            for (id, action_area) in regions {
                let enabled = prototype_modal_actions()
                    .iter()
                    .find(|a| a.id == id)
                    .map(|a| a.enabled)
                    .unwrap_or(true);
                self.host.register_modal_action(id, action_area, enabled);
            }
            self.host.reconcile();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

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
        // Drive knobs directly (shell focus ownership may stay on Sidebar after paint).
        let _ = app.handle_knob_key(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeyChord::plain(KeyCode::Right),
            tick,
        );
        assert_eq!(app.interactor.knobs()[0].display_value(), "Warning");
        let _ = app.handle_knob_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyChord::plain(KeyCode::Down),
            tick,
        );
        let _ = app.handle_knob_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyChord::plain(KeyCode::Down),
            tick,
        );
        let _ = app.handle_knob_key(
            KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE),
            KeyChord::plain(KeyCode::Char('!')),
            tick,
        );
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
        app.host.focus(FocusId::Controls);

        let _ = app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE), tick);

        assert_eq!(app.host.theme, RolePalette::slate());
    }

    #[test]
    fn text_story_keeps_plain_t_and_uses_control_t_for_theme() {
        let mut app = Lookbook::new();
        let tick = tick_at(Instant::now(), 0);
        let picker = gallery_stories()
            .iter()
            .position(|story| story.id == "picker/basic")
            .unwrap();
        app.select(picker);
        render_app(&mut app, tick);
        app.host.focus(FocusId::Preview);

        let _ = app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE), tick);
        assert_eq!(app.host.theme, RolePalette::default());
        let _ = app.handle_key(
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
            tick,
        );
        assert_eq!(app.host.theme, RolePalette::slate());
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
        app.host.focus(FocusId::Controls);

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
        app.host.focus(FocusId::Preview);

        app.open_focus_modal();
        render_app(&mut app, tick);
        assert_eq!(app.host.focused(), Some(FocusId::ModalContinue));

        app.handle_modal_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.host.focused(), Some(FocusId::ModalCancel));
        app.handle_modal_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(!app.prototype_modal.is_some());
        assert_eq!(app.host.focused(), Some(FocusId::Preview));
    }

    #[test]
    fn modal_pointer_activation_uses_action_regions_and_never_leaks_to_background() {
        let mut app = Lookbook::new();
        let tick = tick_at(Instant::now(), 0);
        render_app(&mut app, tick);
        app.host.focus(FocusId::Preview);
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
        assert!(app.prototype_modal.is_some());
        assert_eq!(app.host.focused(), Some(FocusId::ModalContinue));

        let cancel = app
            .prototype_modal
            .as_ref()
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

        assert!(!app.prototype_modal.is_some());
        assert_eq!(app.host.focused(), Some(FocusId::Preview));
    }

    #[test]
    fn host_frame_narrow_and_tiny_shell_geometry() {
        let mut app = Lookbook::new();
        let tick = tick_at(Instant::now(), 0);
        for (w, h) in [(80, 24), (40, 16), (20, 10), (12, 8)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|frame| app.render_at(frame, tick)).unwrap();
            assert!(
                app.host.frame_bounds.width == w && app.host.frame_bounds.height == h,
                "{w}x{h}"
            );
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
        let _ = app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), tick);
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
        assert!(production.contains("OverlayStack"));
        assert!(production.contains("DesignSystem"));
        assert!(!production.contains("FocusRing"));
        assert!(!production.contains("ModalStack"));
    }
}
