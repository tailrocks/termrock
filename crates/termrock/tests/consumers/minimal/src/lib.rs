//! An independent consumer of `termrock` with every feature off.
//!
//! This package has its own `[workspace]` and lockfile, so Cargo resolves it
//! from the facade's declared features alone. Nothing here can lean on the
//! feature unification of the repository workspace.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use termrock::core::focus::FocusRing;
use termrock::core::hit::HitRegistry;
use termrock::core::id::WidgetId;
use termrock::theme::{ColorLevel, Theme};
use termrock::ui::ctx::{Interaction, RenderCtx};
use termrock::widgets::input::TextInput;
use termrock::widgets::list::{ListBox, ListItem, SelectMode};

/// A name field above a short list, the smallest form a consumer builds.
pub struct Form {
    pub name: TextInput,
    pub colors: ListBox,
}

impl Form {
    pub fn new() -> Self {
        Self {
            name: TextInput::new(WidgetId::of("minimal.name"), "Name").value("alpha"),
            colors: ListBox::new(
                WidgetId::of("minimal.colors"),
                ["Amber", "Green", "Blue"]
                    .into_iter()
                    .map(ListItem::new)
                    .collect(),
                SelectMode::Single,
            ),
        }
    }

    /// Lays the field in the first `TextInput::HEIGHT` rows and the list in the rest of `area`.
    pub fn render(&mut self, area: Rect) -> Buffer {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        let [field, list] =
            Layout::vertical([Constraint::Length(TextInput::HEIGHT), Constraint::Min(1)])
                .areas(area);
        self.name.render(field, &mut buf, &mut ctx, theme.canvas);
        self.colors.render(list, &mut buf, &mut ctx, theme.canvas);
        buf
    }
}

impl Default for Form {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};
    use termrock::core::event::{Key, Outcome};
    use termrock::core::response::Response;
    use termrock::widgets::grid::{
        CellKind, CellValue, ColumnSpec, DataGrid, GridAction, GridRows, RowTotal,
    };

    use super::*;

    fn key(code: KeyCode) -> Key {
        Key {
            code,
            mods: KeyModifiers::NONE,
        }
    }

    fn text(buf: &Buffer) -> String {
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn consumer_minimal_renders_input_and_list() {
        let mut form = Form::new();
        let shown = text(&form.render(Rect::new(0, 0, 40, 8)));
        for expected in ["Name", "alpha", "Amber", "Green", "Blue"] {
            assert!(shown.contains(expected), "{expected} missing:\n{shown}");
        }
    }

    #[test]
    fn consumer_minimal_keys_reach_the_state() {
        let mut form = Form::new();
        form.name.on_key(&key(KeyCode::End));
        form.name.on_key(&key(KeyCode::Char('!')));
        assert_eq!(form.name.text(), "alpha!");
        assert_eq!(form.colors.on_key(&key(KeyCode::Down)), Outcome::Changed);
        assert_eq!(form.colors.core.cursor, 1);
    }

    #[test]
    fn consumer_minimal_validated_input_reports_and_clears_errors() {
        let field = |value: &str| {
            TextInput::new(WidgetId::of("minimal.validated"), "Handle")
                .required(true)
                .required_message("Enter a handle")
                .validator(|text| text.contains(' ').then(|| "No spaces".to_owned()))
                .value(value)
        };

        let mut empty = field("");
        assert!(!empty.validate(), "a required field refuses an empty value");
        assert_eq!(empty.error(), Some("Enter a handle"));

        let mut spaced = field("a b");
        assert!(!spaced.validate(), "the validator refuses the value");
        assert_eq!(spaced.error(), Some("No spaces"));

        let mut valid = field("ab");
        assert!(valid.validate());
        assert_eq!(valid.error(), None);

        spaced.set_value("ab");
        assert_eq!(
            spaced.error(),
            None,
            "replacing the value clears the stale error"
        );
    }

    /// What a host does with the typed actions of a grid, in order.
    #[derive(Debug, PartialEq, Eq)]
    enum Effect {
        Save { row: usize, col: usize },
        FocusNext,
        FocusPrevious,
    }

    fn effects(response: Response<GridAction>) -> Vec<Effect> {
        response
            .actions()
            .filter_map(|action| match action {
                GridAction::CellChanged { row, col } => Some(Effect::Save {
                    row: *row,
                    col: *col,
                }),
                GridAction::LeaveForward => Some(Effect::FocusNext),
                GridAction::LeaveBackward => Some(Effect::FocusPrevious),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn consumer_minimal_typed_grid_actions_arrive_in_order() {
        let mut grid = DataGrid::new(
            WidgetId::of("minimal.grid"),
            vec![ColumnSpec::new("Name", CellKind::Text)],
        )
        .editable(true);
        grid.try_set_rows(GridRows {
            rows: vec![vec![CellValue::Text("a".into())]],
            total: RowTotal::Exact(1),
            more: false,
        })
        .expect("a rectangular row");

        let _ = grid.begin_edit();
        let _ = grid.on_key(&key(KeyCode::Char('z')));
        let leaving = grid.on_key(&key(KeyCode::Tab));
        assert_eq!(
            effects(leaving),
            [Effect::Save { row: 0, col: 0 }, Effect::FocusNext],
            "the edit is reported before the focus leaves the grid"
        );
    }

    #[test]
    fn consumer_minimal_nested_scroll_views_bubble_what_they_cannot_scroll() {
        use ratatui::layout::Size;
        use termrock::ui::scroll_view::{Axes, ScrollView};

        const AREA: Rect = Rect::new(0, 0, 20, 4);
        fn paint(view: &mut ScrollView, content: Size) {
            let theme = Theme::for_level(ColorLevel::TrueColor);
            let mut buf = Buffer::empty(AREA);
            let mut hits = HitRegistry::default();
            let mut ring = FocusRing::default();
            let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
            view.render(AREA, content, &mut buf, &mut ctx, |_, _, _| {});
        }
        // The inner view is routed first; a key it cannot use returns `None`
        // and the host offers it to the outer view.
        fn route(inner: &mut ScrollView, outer: &mut ScrollView, key: &Key) -> Option<Outcome> {
            inner.on_key(key).or_else(|| outer.on_key(key))
        }

        let mut outer = ScrollView::new(WidgetId::of("minimal.outer")).axes(Axes::VERTICAL);
        let mut inner = ScrollView::new(WidgetId::of("minimal.inner")).axes(Axes::VERTICAL);
        paint(&mut outer, Size::new(20, 40));
        paint(&mut inner, Size::new(20, 2));
        let page_down = key(KeyCode::PageDown);

        assert_eq!(
            route(&mut inner, &mut outer, &page_down),
            Some(Outcome::Changed)
        );
        assert_eq!(
            inner.offset(),
            (0, 0),
            "the inner content fits, so it stays put"
        );
        assert_ne!(outer.offset(), (0, 0), "the outer view took the key");

        paint(&mut inner, Size::new(20, 40));
        let outer_before = outer.offset();
        assert_eq!(
            route(&mut inner, &mut outer, &page_down),
            Some(Outcome::Changed)
        );
        assert_ne!(inner.offset(), (0, 0), "the inner view scrolls first");
        assert_eq!(outer.offset(), outer_before, "the outer view did not move");
    }

    #[test]
    fn consumer_minimal_bounded_external_messages_return_the_overflow() {
        use std::num::NonZeroUsize;
        use std::time::Duration;

        use termrock::core::event::External;
        use termrock::runtime::{ExternalPolicy, ExternalSendError, bounded_external_channel};

        let policy = ExternalPolicy {
            max_messages: 2,
            max_duration: Duration::from_millis(2),
        };
        let (sender, receiver) =
            bounded_external_channel(NonZeroUsize::new(2).expect("nonzero"), policy);
        assert_eq!(receiver.policy(), policy);

        sender.try_send(External::new(1u32)).expect("room for one");
        sender.try_send(External::new(2u32)).expect("room for two");
        let rejected = sender
            .try_send(External::new(3u32))
            .expect_err("the queue is at capacity");
        let ExternalSendError::Full(message) = rejected else {
            panic!("a full queue answers Full, not Closed");
        };
        assert_eq!(
            message.downcast_ref::<u32>(),
            Some(&3),
            "the payload comes back"
        );

        let first = receiver.try_recv().expect("oldest first");
        assert_eq!(first.downcast_ref::<u32>(), Some(&1));
        sender.try_send(message).expect("a slot is free again");
        let rest: Vec<u32> = std::iter::from_fn(|| receiver.try_recv())
            .filter_map(|m| m.downcast_ref::<u32>().copied())
            .collect();
        assert_eq!(rest, [2, 3], "order is kept and nothing was lost");
    }
}
