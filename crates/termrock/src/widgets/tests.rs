use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{StatefulWidget, Widget},
};

use super::*;
use crate::input::{KeyCode, KeyEvent, KeyModifiers};
use crate::style::{DesignSystem, Role, RolePalette};

/// Shared press/repeat/release key factory for widget tests.
pub(crate) fn key_with_kind(
    code: KeyCode,
    modifiers: KeyModifiers,
    kind: crate::input::KeyEventKind,
) -> KeyEvent {
    let mut key = KeyEvent::new(code, modifiers);
    key.kind = kind;
    key
}

#[cfg(feature = "serde")]
#[test]
fn persistable_states_implement_serde_contracts() {
    fn assert_serde<T: serde::Serialize + serde::de::DeserializeOwned>() {}

    // DiffViewState is session-local (scroll/search/folds); not serde-stable.
    assert_serde::<SplitRatio>();
    assert_serde::<TextInputState>();
}

fn areas() -> [Rect; 5] {
    [
        Rect::new(0, 0, 0, 0),
        Rect::new(0, 0, 1, 1),
        Rect::new(3, 2, 8, 3),
        Rect::new(0, 0, 40, 8),
        Rect::new(7, 4, 80, 12),
    ]
}

#[test]
fn leaf_widgets_render_at_tiny_and_off_origin_areas() {
    let panel_tokens = DesignSystem::new(RolePalette::default());
    let system = panel_tokens.clone();
    let panel = Panel::new(&panel_tokens)
        .title("Title")
        .emphasis(PanelChrome::Focused);
    let hints = [Hint {
        chord: "Enter",
        label: "choose",
        priority: 1,
        visible: true,
    }];
    let hint_bar = HintBar::new(&hints, &system).separator(" · ");
    let toast = Toast::new(&system, "Updated", Severity::Success).anchor(Anchor::TopRight);
    let backdrop = Backdrop::new(&system);
    for area in areas() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 100, 30));
        panel.paint(area, &mut buffer, None);
        (&hint_bar).render(area, &mut buffer);
        (&toast).render(area, &mut buffer);
        (&backdrop).render(area, &mut buffer);
    }
}

#[test]
fn focused_quiet_panel_remains_borderless() {
    let panel_tokens = DesignSystem::new(RolePalette::default());
    let area = Rect::new(0, 0, 10, 3);
    let mut buffer = Buffer::empty(area);
    let panel = Panel::new(&panel_tokens).emphasis(PanelChrome::Focused);
    panel.paint(area, &mut buffer, None);
    assert_quiet_panel_has_no_box(&buffer, area);
}

#[test]
fn inactive_quiet_panel_remains_borderless() {
    let panel_tokens = DesignSystem::new(RolePalette::default());
    let area = Rect::new(0, 0, 10, 3);
    let mut buffer = Buffer::empty(area);
    Panel::new(&panel_tokens).paint(area, &mut buffer, None);
    assert_quiet_panel_has_no_box(&buffer, area);
}

fn assert_quiet_panel_has_no_box(buffer: &Buffer, area: Rect) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            assert_eq!(buffer[(x, y)].symbol(), " ");
        }
    }
}

#[test]
fn stable_ids_survive_reordering() {
    let _tokens = DesignSystem::default();
    let first = [
        ListRow {
            id: "a",
            label: Line::from("Alpha"),
            leading: None,
            secondary: None,
            status: None,
            badge: None,
            shortcut: None,
            actions: None,
            custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: "b",
            label: Line::from("Beta"),
            leading: None,
            secondary: None,
            status: None,
            badge: None,
            shortcut: None,
            actions: None,
            custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
    ];
    let second = [first[1].clone(), first[0].clone()];
    let mut state = ListState::new(Some("b"));
    let area = Rect::new(0, 0, 20, 2);
    let mut buffer = Buffer::empty(area);
    let _theme = RolePalette::default();
    StatefulWidget::render(
        &List::new(&first, &DesignSystem::default()),
        area,
        &mut buffer,
        &mut state,
    );
    StatefulWidget::render(
        &List::new(&second, &DesignSystem::default()),
        area,
        &mut buffer,
        &mut state,
    );
    assert_eq!(state.selected(), Some(&"b"));
    assert_eq!(
        state
            .regions()
            .iter()
            .find(|region| region.id == "b")
            .unwrap()
            .area
            .y,
        0
    );
}

#[test]
fn disabled_and_separator_rows_have_no_hit_regions() {
    let _tokens = DesignSystem::default();
    let rows = [
        ListRow {
            id: 1,
            label: Line::from("Disabled"),
            leading: None,
            secondary: None,
            status: None,
            badge: None,
            shortcut: None,
            actions: None,
            custom: None,
            role: RowRole::Item,
            enabled: false,
            loading: false,
        },
        ListRow {
            id: 2,
            label: Line::from("Section"),
            leading: None,
            secondary: None,
            status: None,
            badge: None,
            shortcut: None,
            actions: None,
            custom: None,
            role: RowRole::Separator,
            enabled: true,
            loading: false,
        },
        ListRow {
            id: 3,
            label: Line::from("Enabled"),
            leading: None,
            secondary: None,
            status: None,
            badge: None,
            shortcut: None,
            actions: None,
            custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        },
    ];
    let mut state = ListState::default();
    let area = Rect::new(4, 3, 20, 3);
    let mut buffer = Buffer::empty(Rect::new(0, 0, 30, 10));
    let _theme = RolePalette::default();
    StatefulWidget::render(
        &List::new(&rows, &DesignSystem::default()),
        area,
        &mut buffer,
        &mut state,
    );
    assert_eq!(state.regions().len(), 1);
    assert_eq!(state.regions()[0].id, 3);
    assert_eq!(state.regions()[0].area, Rect::new(4, 5, 20, 1));
}

#[test]
fn text_input_edits_extended_graphemes_atomically() {
    for value in ["e\u{301}", "👩‍💻", "👍🏽", "🌐", "🧪", "\u{200b}"] {
        let mut state = TextInputState::new(value);
        state.apply(EditAction::move_left());
        assert_eq!(state.cursor_byte(), 0, "{value:?}");
        state.apply(EditAction::move_right());
        assert_eq!(state.cursor_byte(), value.len(), "{value:?}");
        state.apply(EditAction::Backspace);
        assert_eq!(state.value(), "", "{value:?}");
    }
}

#[test]
fn action_and_status_regions_match_painted_geometry() {
    let theme = RolePalette::default();
    let system = crate::style::DesignSystem::new(theme.clone());
    let actions = [
        Action {
            id: "save",
            label: "Save",
            enabled: true,
            variant: ActionVariant::Secondary,
        },
        Action {
            id: "cancel",
            label: "Cancel",
            enabled: true,
            variant: ActionVariant::Secondary,
        },
    ];
    let mut action_state = ActionBarState::default();
    let area = Rect::new(5, 2, 30, 1);
    let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 5));
    StatefulWidget::render(
        &ActionBar::new(&actions, &system).gap(" "),
        area,
        &mut buffer,
        &mut action_state,
    );
    assert_eq!(action_state.regions[0].id, "save");
    assert_eq!(action_state.regions[0].area.x, area.x);

    let left = [StatusSlot::new("left", "Ready").priority(1)];
    let right = [StatusSlot::new("right", "42%")
        .priority(1)
        .region(crate::widgets::StatusRegion::Right)];
    let status = StatusBar::new(&left, &right, &system).alpha(1.0);
    let regions = status.regions(area);
    assert_eq!(regions[1].area.right(), area.right());
    (&status).render(area, &mut buffer, &mut StatusBarState::default());
}

#[test]
fn viewport_clamps_scroll_and_paints_an_overflow_thumb() {
    let lines = [
        Line::from("zero"),
        Line::from("one"),
        Line::from("two"),
        Line::from("three"),
    ];
    let theme = RolePalette::default();
    let system = crate::style::DesignSystem::new(theme.clone());
    let viewport = Viewport::new(&lines, &system).title(" Log ");
    let area = Rect::new(0, 0, 12, 4);
    let mut buffer = Buffer::empty(area);
    let mut state = crate::widgets::ViewportState::default();
    state.scroll = crate::scroll::DialogScroll {
        scroll_x: 0,
        scroll_y: 1,
        ..crate::scroll::DialogScroll::default()
    };

    StatefulWidget::render(&viewport, area, &mut buffer, &mut state);

    assert_eq!(state.scroll.scroll_y, 1);
    assert_eq!(buffer[(1, 1)].symbol(), "o");
    let (start, len) = crate::scroll::overflow_thumb(4, 2, 2, 1).expect("4 lines overflow 2");
    assert_eq!((start, len), (1, 1));
    assert_eq!(buffer[(11, 1)].symbol(), "│");
    assert_eq!(buffer[(11, 2)].symbol(), "┃");
    assert_eq!(buffer[(0, 0)].fg, theme.style(Role::Border).fg.unwrap());
}

#[test]
fn viewport_emphasis_focused_uses_border_focused_role() {
    let lines = [Line::from("row")];
    let theme = RolePalette::default();
    let system = crate::style::DesignSystem::new(theme.clone());
    let viewport = Viewport::new(&lines, &system)
        .title("Active")
        .emphasis(PanelChrome::Focused);
    let area = Rect::new(0, 0, 16, 4);
    let mut buffer = Buffer::empty(area);
    let mut state = crate::widgets::ViewportState::default();

    StatefulWidget::render(&viewport, area, &mut buffer, &mut state);

    assert_eq!(
        buffer[(0, 0)].fg,
        theme.style(Role::BorderFocused).fg.unwrap()
    );
    assert!(
        buffer[(2, 0)]
            .modifier
            .contains(ratatui_core::style::Modifier::BOLD)
    );
}

#[test]
fn theme_override_reaches_active_tab_cells() {
    use ratatui_core::style::Color;

    let theme = RolePalette::default().with_role(Role::TabActive, Style::new().bg(Color::Blue));
    let system = crate::style::DesignSystem::new(theme.clone());
    let tabs = [Tab {
        id: "active",
        label: "Active",
        glyph: None,
        badge: None,
        status: TabStatus::None,
        enabled: true,
        closable: false,
    }];
    let widget = Tabs::new(&tabs, &system).gap(1);
    let area = Rect::new(0, 0, 12, 2);
    let mut buffer = Buffer::empty(area);
    let mut state = TabsState::default();

    (&widget).render(area, &mut buffer, &mut state);

    assert_eq!(
        buffer[(1, 1)].fg,
        system.junie_theme().accent,
        "active document tab states itself with the accent rule, not a fill"
    );
}

#[test]
fn owned_panel_render_matches_borrowed_render() {
    let panel_tokens = DesignSystem::new(RolePalette::default());
    let area = Rect::new(0, 0, 12, 3);
    let mut owned = Buffer::empty(area);
    let mut borrowed = Buffer::empty(area);

    Panel::new(&panel_tokens)
        .title("Panel")
        .paint(area, &mut owned, None);
    let panel = Panel::new(&panel_tokens).title("Panel");
    panel.paint(area, &mut borrowed, None);

    assert_eq!(owned, borrowed);
}
