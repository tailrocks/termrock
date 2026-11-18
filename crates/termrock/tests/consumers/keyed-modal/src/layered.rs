//! An independent consumer of `termrock` that drives a keyed list under a
//! modal layer, with every feature off.
//!
//! The list borrows its rows from a [`KeyedVec`] this crate owns. Selection
//! and cursor are held by key in the library's `KeyedState`; this crate keeps
//! no index mirror. A modal layer opens and closes while the provider is
//! reordered, and the library restores focus.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use termrock::core::event::{Key, Outcome};
use termrock::core::focus::{Focus, FocusRing};
use termrock::core::hit::HitRegistry;
use termrock::core::id::{ItemKey, WidgetId};
use termrock::core::keyed::{KeyedSourceError, KeyedVec};
use termrock::theme::{ColorLevel, Theme};
use termrock::ui::ctx::{Interaction, RenderCtx};
use termrock::ui::layer::{
    DismissReason, Dismissed, Layer, LayerCx, LayerOut, LayerSpec, LayerTag, ShellLayers,
};
use termrock::ui::popup::Dismiss;
use termrock::widgets::list::{ListAction, ListBox, ListItem, SelectMode};

pub const LIST: WidgetId = WidgetId::of("keyed-modal.list");
pub const MODAL: WidgetId = WidgetId::of("keyed-modal.dialog");

/// The smallest modal: it is a layer and nothing else.
struct Confirm;

impl Layer<(), ()> for Confirm {
    fn spec(&self) -> LayerSpec {
        LayerSpec::modal(Dismiss::LIGHT)
            .initial_focus(MODAL)
            .restore_focus(true)
    }

    fn render(&mut self, _: Rect, _: &mut Buffer, _: &mut RenderCtx, _: &()) {}

    fn on_key(&mut self, _: &Key, _: &mut LayerCx<'_, ()>) -> Outcome {
        Outcome::Ignored
    }

    fn on_dismiss(&mut self, _: DismissReason) {}

    fn take_result(&mut self) -> Option<LayerOut<()>> {
        None
    }
}

pub struct Screen {
    pub store: KeyedVec<ListItem>,
    pub list: ListBox,
    pub focus: Focus,
    layers: ShellLayers<(), ()>,
}

impl Screen {
    pub fn new(rows: &[(u64, &str)]) -> Self {
        let store = KeyedVec::try_from_rows(
            rows.iter()
                .map(|(key, label)| (ItemKey::num(*key), ListItem::new(label))),
        )
        .expect("unique keys");
        let mut focus = Focus::default();
        focus.set(Some(LIST));
        focus.take_changes();
        Self {
            store,
            list: ListBox::new(LIST, Vec::new(), SelectMode::Single),
            focus,
            layers: ShellLayers::new(),
        }
    }

    /// Paints the list over the current provider, which reconciles the keyed
    /// state with whatever order and membership the provider has now.
    pub fn render(&mut self, area: Rect) -> Result<Buffer, KeyedSourceError> {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        self.list
            .render_source(area, &self.store, &mut buf, &mut ctx, theme.canvas)?;
        Ok(buf)
    }

    /// Keys the list and returns the actions it emitted, in order.
    pub fn press(&mut self, key: &Key) -> Result<Vec<ListAction>, KeyedSourceError> {
        let response = self.list.on_key_source(key, &self.store)?;
        Ok(response.actions().copied().collect())
    }

    pub fn open_modal(&mut self) {
        self.layers.open(
            Box::new(Confirm),
            LayerTag::new("confirm"),
            (),
            &mut self.focus,
        );
    }

    /// Dismisses the top layer the way an Esc does; `true` when one closed.
    pub fn dismiss_modal(&mut self) -> bool {
        matches!(
            self.layers.dismiss(DismissReason::Esc, &mut self.focus),
            Dismissed::Closed(..)
        )
    }

    pub fn modal_open(&self) -> bool {
        !self.layers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    use super::*;

    const AREA: Rect = Rect::new(0, 0, 30, 6);

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
    fn consumer_keyed_modal_preserves_selection() {
        let mut screen = Screen::new(&[(10, "alpha"), (20, "bravo"), (30, "charlie")]);
        screen.render(AREA).expect("valid provider");

        screen.press(&key(KeyCode::Down)).expect("valid provider");
        let actions = screen.press(&key(KeyCode::Enter)).expect("valid provider");
        assert_eq!(
            actions,
            [
                ListAction::Activated(ItemKey::num(20)),
                ListAction::SelectionChanged
            ]
        );

        screen.open_modal();
        assert!(screen.modal_open());
        assert_eq!(screen.focus.current(), Some(MODAL));

        // The provider changes while the modal is open: the chosen record
        // moves to the end and a new record goes in front.
        screen.store.move_to(ItemKey::num(20), 2);
        screen
            .store
            .insert(0, ItemKey::num(5), ListItem::new("zulu"))
            .expect("new key");

        assert!(screen.dismiss_modal());
        assert!(!screen.modal_open());
        assert_eq!(
            screen.focus.current(),
            Some(LIST),
            "focus returns to the list"
        );

        let shown = text(&screen.render(AREA).expect("valid provider"));
        let order: Vec<usize> = ["zulu", "alpha", "charlie", "bravo"]
            .iter()
            .map(|label| {
                shown
                    .find(label)
                    .unwrap_or_else(|| panic!("{label}:\n{shown}"))
            })
            .collect();
        assert!(
            order.windows(2).all(|w| w[0] < w[1]),
            "order follows the provider:\n{shown}"
        );
        assert_eq!(
            screen.list.keyed().selected().collect::<Vec<_>>(),
            [ItemKey::num(20)],
            "the selected key survives the reorder"
        );
        assert_eq!(screen.list.keyed().cursor(), Some(ItemKey::num(20)));
        assert_eq!(screen.list.core.cursor, 3, "the view index follows the key");
        assert_eq!(
            screen.list.chosen,
            Some(3),
            "the chosen row follows the key"
        );
    }
}
