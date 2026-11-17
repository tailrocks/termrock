//! A modal layer: keys go to the topmost layer first, the page behind it
//! leaves the focus ring, and closing the modal returns focus to the page.
//!
//! `cargo run -p termrock --example modal_focus`

mod common;

use common::{frame, key};
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use termrock::core::event::Outcome;
use termrock::core::focus::Focus;
use termrock::core::id::WidgetId;
use termrock::ui::modal::ModalStack;
use termrock::widgets::list::{ListBox, ListItem, SelectMode};

const SCREEN: Rect = Rect::new(0, 0, 32, 10);
const PAGE: WidgetId = WidgetId::of("example.page");
const CHOICES: WidgetId = WidgetId::of("example.choices");

fn list(id: WidgetId, names: &[&str]) -> ListBox {
    let items = names.iter().map(|n| ListItem::new(n)).collect();
    ListBox::new(id, items, SelectMode::Single)
}

struct App {
    page: ListBox,
    modal: ModalStack<ListBox>,
    focus: Focus,
}

impl App {
    fn new() -> Self {
        let mut focus = Focus::default();
        focus.focus(PAGE);
        Self {
            page: list(PAGE, &["personal", "work"]),
            modal: ModalStack::new(),
            focus,
        }
    }

    fn open(&mut self) {
        let choices = list(CHOICES, &["reader", "writer", "owner"]);
        self.modal.push(choices, &mut self.focus, Some(CHOICES));
    }

    /// The topmost layer gets the key; the page sees it only with no modal.
    /// Escape closes the modal, and the stack restores the focus it saved.
    fn press(&mut self, code: KeyCode) -> Outcome {
        let key = key(code);
        match self.modal.top_mut() {
            Some(_) if code == KeyCode::Esc => {
                self.modal.pop(&mut self.focus);
                Outcome::Changed
            }
            Some(modal) => modal.on_key(&key),
            None => self.page.on_key(&key),
        }
    }

    /// Returns the screen and the widgets keyboard focus can reach.
    fn screen(&mut self) -> (String, Vec<WidgetId>) {
        let (page, modal) = (&mut self.page, &mut self.modal);
        frame(SCREEN.width, SCREEN.height, |buf, ctx| {
            page.render(SCREEN, buf, ctx, ctx.theme.canvas);
            if let Some(choices) = modal.top_mut() {
                ctx.modal_backdrop(buf, SCREEN);
                let area = Rect::new(4, 2, 16, 5);
                choices.render(area, buf, ctx, ctx.theme.canvas);
            }
            ctx.ring.reachable().to_vec()
        })
    }
}

fn main() {
    let mut app = App::new();
    app.press(KeyCode::Down);
    app.open();
    app.press(KeyCode::Down);
    let (screen, reachable) = app.screen();
    println!("-- modal open, reachable {reachable:?} --\n{screen}");
    app.press(KeyCode::Esc);
    let (screen, reachable) = app.screen();
    println!(
        "-- closed, focus {:?}, reachable {reachable:?} --\n{screen}",
        app.focus.current()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_modal_gets_keys_first_and_closing_it_restores_focus() {
        let mut app = App::new();
        app.screen();
        app.press(KeyCode::Down);
        assert_eq!(app.page.core.cursor, 1);

        app.open();
        let (screen, reachable) = app.screen();
        assert_eq!(app.focus.current(), Some(CHOICES), "the modal took focus");
        assert_eq!(reachable, [CHOICES], "the page left the focus ring");
        assert!(screen.contains("writer"), "{screen}");

        app.press(KeyCode::Down);
        assert_eq!(app.modal.top().map(|m| m.core.cursor), Some(1));
        assert_eq!(app.page.core.cursor, 1, "the page behind did not move");

        app.press(KeyCode::Esc);
        let (_, reachable) = app.screen();
        assert!(app.modal.is_empty());
        assert_eq!(app.focus.current(), Some(PAGE), "focus came back");
        assert_eq!(reachable, [PAGE]);
    }
}
