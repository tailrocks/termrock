//! Lookbook-owned model, rendering, and interaction routing.

use std::ops::ControlFlow;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use termrock::{
    Theme,
    input::{Event, KeyCode, KeyEvent, KeyEventKind, MouseEvent, MouseEventKind},
    keymap::KeyChord,
    scroll::{self, ScrollSpan},
    style::{PHOSPHOR_DARK, PHOSPHOR_GREEN, PREVIEW_CARD, Role},
};

use crate::{
    Focus, PREVIEW_KEYMAP, PreviewAction, SIDEBAR_KEYMAP, SidebarAction,
    interactors::StoryInteraction, stories::stories,
};

pub(crate) struct Lookbook {
    selected: usize,
    preview_scroll: u16,
    sidebar_scroll: u16,
    focus: Focus,
    interactor: Box<dyn StoryInteraction>,
    component_area: Rect,
    preview_panel_area: Rect,
    sidebar_area: Rect,
    sidebar_inner_area: Rect,
    sidebar_viewport_items: usize,
    preview_viewport_rows: usize,
    theme: Theme,
}

impl Lookbook {
    pub(crate) fn new() -> Self {
        Self {
            selected: 0,
            preview_scroll: 0,
            sidebar_scroll: 0,
            focus: Focus::Sidebar,
            interactor: stories()[0].make_interactor(),
            component_area: Rect::default(),
            preview_panel_area: Rect::default(),
            sidebar_area: Rect::default(),
            sidebar_inner_area: Rect::default(),
            sidebar_viewport_items: 1,
            preview_viewport_rows: 1,
            theme: Theme::default(),
        }
    }

    pub(crate) fn render(&mut self, frame: &mut Frame<'_>) {
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

        frame.render_widget(
            Paragraph::new("TermRock  lookbook").style(self.theme.style(Role::Text)),
            brand_area,
        );
        self.render_sidebar(frame, sidebar_area);
        self.render_description(frame, description_area);
        self.render_preview(frame, preview_area);
        self.render_hints(frame, hint_area);
    }

    fn render_sidebar(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let catalog = stories();
        let border_style = if self.focus == Focus::Sidebar {
            Style::new().fg(PHOSPHOR_GREEN)
        } else {
            self.theme.style(Role::Border)
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Stories ")
            .border_style(border_style);
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
        let mut state = ListState::default()
            .with_offset(offset)
            .with_selected(Some(self.selected));
        frame.render_stateful_widget(
            List::new(items)
                .highlight_style(
                    Style::default()
                        .bg(PHOSPHOR_GREEN)
                        .fg(PHOSPHOR_DARK)
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
        let story = stories()[self.selected];
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" About ")
            .border_style(self.theme.style(Role::Border));
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
                        .fg(PHOSPHOR_GREEN)
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
        let story = stories()[self.selected];
        let border_style = if self.focus == Focus::Preview {
            Style::new().fg(PHOSPHOR_GREEN)
        } else {
            self.theme.style(Role::Border)
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Preview ")
            .border_style(border_style);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(
            Block::default().style(Style::default().bg(PREVIEW_CARD)),
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

    fn render_hints(&self, frame: &mut Frame<'_>, area: Rect) {
        let spans = match self.focus {
            Focus::Preview => PREVIEW_KEYMAP.hint_spans(),
            Focus::Sidebar => SIDEBAR_KEYMAP.hint_spans(),
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
            .join(" ");
        frame.render_widget(Paragraph::new(text), area);
    }

    pub(crate) fn update(&mut self, event: Event) -> ControlFlow<()> {
        match event {
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            Event::Key(key) if key.kind == KeyEventKind::Press => return self.handle_key(key),
            Event::Resize { .. } | Event::FocusGained | Event::FocusLost => {}
            Event::Key(_) | Event::Paste | Event::Unknown => {}
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
                        .min(stories().len().saturating_sub(1));
                    self.select(index);
                    self.focus = Focus::Sidebar;
                }
                if self.preview_panel_area.contains(mouse.position) {
                    self.focus = Focus::Preview;
                }
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
                    stories().len(),
                    self.sidebar_viewport_items,
                    delta,
                );
                if self.selected != before {
                    self.preview_scroll = 0;
                    self.interactor = stories()[self.selected].make_interactor();
                }
            }
            MouseEventKind::ScrollUp
            | MouseEventKind::ScrollDown
            | MouseEventKind::ScrollLeft
            | MouseEventKind::ScrollRight
                if self.focus == Focus::Preview =>
            {
                let mut ignored_x = 0;
                scroll::apply_mouse_scroll_u16(
                    mouse.kind,
                    mouse.modifiers,
                    scroll::ScrollAxes {
                        vertical: scroll::is_scrollable(
                            usize::from(stories()[self.selected].height),
                            self.preview_viewport_rows,
                        ),
                        horizontal: false,
                    },
                    ScrollSpan::new(0, 0),
                    ScrollSpan::new(
                        usize::from(stories()[self.selected].height),
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

    fn handle_key(&mut self, key: KeyEvent) -> ControlFlow<()> {
        let chord = KeyChord::from(key);
        match self.focus {
            Focus::Preview => self.handle_preview_key(key, chord),
            Focus::Sidebar => return self.handle_sidebar_key(chord),
        }
        ControlFlow::Continue(())
    }

    fn handle_preview_key(&mut self, key: KeyEvent, chord: KeyChord) {
        let content = usize::from(stories()[self.selected].height);
        match PREVIEW_KEYMAP
            .dispatch(chord)
            .unwrap_or(PreviewAction::Forward)
        {
            PreviewAction::BackToList => self.focus = Focus::Sidebar,
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

    fn handle_sidebar_key(&mut self, chord: KeyChord) -> ControlFlow<()> {
        match SIDEBAR_KEYMAP.dispatch(chord) {
            Some(SidebarAction::Quit) => return ControlFlow::Break(()),
            Some(SidebarAction::FocusPreview) => self.focus = Focus::Preview,
            Some(SidebarAction::Navigate) => {
                let down = matches!(chord.key, KeyCode::Down | KeyCode::Char('j'));
                let target = if down {
                    (self.selected + 1).min(stories().len().saturating_sub(1))
                } else {
                    self.selected.saturating_sub(1)
                };
                self.select(target);
            }
            Some(SidebarAction::GoToEdge) => {
                let target = if chord.key == KeyCode::Home {
                    0
                } else {
                    stories().len().saturating_sub(1)
                };
                self.select(target);
            }
            None => {}
        }
        ControlFlow::Continue(())
    }

    fn select(&mut self, selected: usize) {
        if selected != self.selected {
            self.interactor = stories()[selected].make_interactor();
            self.preview_scroll = 0;
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
}
