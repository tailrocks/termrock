//! Interactive TermRock on-ramp.
//!
//! The example deliberately owns only product state: selected IDs, current
//! theme, and whether activation feedback is visible. TermRock owns rendering,
//! hit geometry, navigation behavior, and terminal lifecycle cleanup.

use std::{io, time::Duration};

use crossterm::event;
use ratatui_core::{
    layout::Rect,
    style::Style,
    terminal::Terminal,
    text::{Line, Span},
};
use termrock::{
    Theme,
    crossterm::{CrosstermBackend, Session, SessionOptions},
    input::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind},
    interaction::Outcome,
    keymap::{KeyBinding, KeyChord, Keymap, Visibility},
    layout::bottom_rows,
    style::Role,
    widgets::{
        List, ListRow, ListState, Panel, PanelEmphasis, RowRole, Severity, StatusBar,
        StatusBarState, StatusSlot, Tab, Tabs, TabsState, Toast, render_hint_bar,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Up,
    Down,
    Activate,
    ToggleTheme,
    Quit,
}

static BINDINGS: &[KeyBinding<Action>] = &[
    KeyBinding::borrowed(
        &[
            KeyChord::plain(KeyCode::Up),
            KeyChord::plain(KeyCode::Char('k')),
        ],
        Action::Up,
        Some("navigate"),
        Visibility::Shown,
        Some("↑↓"),
    ),
    KeyBinding::borrowed(
        &[
            KeyChord::plain(KeyCode::Down),
            KeyChord::plain(KeyCode::Char('j')),
        ],
        Action::Down,
        None,
        Visibility::HiddenAlias,
        None,
    ),
    KeyBinding::borrowed(
        &[KeyChord::plain(KeyCode::Enter)],
        Action::Activate,
        Some("activate"),
        Visibility::Shown,
        None,
    ),
    KeyBinding::borrowed(
        &[KeyChord::plain(KeyCode::Char('t'))],
        Action::ToggleTheme,
        Some("theme"),
        Visibility::Shown,
        None,
    ),
    KeyBinding::borrowed(
        &[KeyChord::plain(KeyCode::Char('q'))],
        Action::Quit,
        Some("quit"),
        Visibility::Shown,
        None,
    ),
];

fn main() -> io::Result<()> {
    // Session arms each terminal mode as it succeeds and restores them in
    // reverse order on explicit restore, early return, or drop.
    let mut session = Session::enter(io::stdout(), SessionOptions::default())?;
    let backend = CrosstermBackend::new(session.writer_mut());
    let mut terminal = Terminal::new(backend)?;

    let rows = showcase_rows();
    let keymap = Keymap::from_static(BINDINGS);
    let mut list_state = ListState::new(Some("list"));
    let mut tabs_state = TabsState::default();
    let mut status_state = StatusBarState::default();
    let mut phosphor = true;
    let mut activated = false;

    let result = loop {
        let theme = if phosphor {
            Theme::default()
        } else {
            Theme::slate()
        };
        terminal.draw(|frame| {
            let area = frame.area();
            let tabs_area = Rect::new(area.x, area.y, area.width, area.height.min(2));
            let below_tabs = Rect::new(
                area.x,
                area.y.saturating_add(tabs_area.height),
                area.width,
                area.height.saturating_sub(tabs_area.height),
            );
            let (content, [hints_area, status_area]) = bottom_rows(below_tabs, [1, 1]);
            render_tabs(frame, tabs_area, &theme, &mut tabs_state);

            let panel = Panel::new(&theme)
                .title("Components")
                .emphasis(PanelEmphasis::Focused);
            let list_area = panel.inner(content);
            frame.render_widget(&panel, content);
            frame.render_stateful_widget(List::new(&rows, &theme), list_area, &mut list_state);

            render_hint_bar(frame, hints_area, &keymap.hint_spans(), &theme);
            render_status(frame, status_area, &theme, phosphor, &mut status_state);
            if activated {
                frame.render_widget(
                    Toast::new(&theme, "Activated selected component", Severity::Success),
                    area,
                );
            }
        })?;

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        activated = false;
        match Event::from(event::read()?) {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                let Some(action) = keymap.dispatch(key.into()) else {
                    continue;
                };
                match action {
                    Action::Quit => break Ok(()),
                    Action::ToggleTheme => phosphor = !phosphor,
                    Action::Up | Action::Down => {
                        let _ = list_state.handle_key(&rows, key);
                    }
                    Action::Activate => {
                        activated =
                            matches!(list_state.handle_key(&rows, key), Outcome::Activated(_));
                    }
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::Moved => {
                    list_state.hover(mouse.position);
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    activated = matches!(list_state.click(mouse.position), Outcome::Activated(_));
                }
                _ => {}
            },
            _ => {}
        }
    };

    // Drop Terminal first so it releases its mutable writer borrow, then make
    // restoration errors visible to the caller.
    drop(terminal);
    session.restore()?;
    result
}

fn showcase_rows() -> [ListRow<'static, &'static str>; 6] {
    ["list", "tree", "form", "tabs", "log-pane", "progress"].map(|id| ListRow {
        id,
        label: Line::from(id),
        trailing: Some(Line::from("TermRock")),
        role: RowRole::Item,
        enabled: true,
    })
}

fn render_tabs(
    frame: &mut ratatui_core::terminal::Frame<'_>,
    area: Rect,
    theme: &Theme,
    state: &mut TabsState<&'static str>,
) {
    let tabs = [
        Tab {
            id: "components",
            label: "Components",
            glyph: Some(Span::styled("●", theme.style(Role::Accent))),
            active: true,
            enabled: true,
        },
        Tab {
            id: "events",
            label: "Events",
            glyph: None,
            active: false,
            enabled: true,
        },
    ];
    frame.render_stateful_widget(Tabs::new(&tabs, theme), area, state);
}

fn render_status(
    frame: &mut ratatui_core::terminal::Frame<'_>,
    area: Rect,
    theme: &Theme,
    phosphor: bool,
    state: &mut StatusBarState<&'static str>,
) {
    let left = [StatusSlot {
        id: "state",
        content: " ready ",
        priority: 10,
        min_width: 0,
        enabled: true,
        style: theme.style(Role::Success),
        hover_style: Some(theme.style(Role::LinkHover)),
    }];
    let right = [StatusSlot {
        id: "theme",
        content: if phosphor { " phosphor " } else { " slate " },
        priority: 10,
        min_width: 0,
        enabled: true,
        style: Style::new(),
        hover_style: None,
    }];
    frame.render_stateful_widget(StatusBar::new(&left, &right, theme), area, state);
}
