//! A keyed list borrowed from the host's own provider. The list stores keys,
//! so reordering or inserting rows never moves the cursor off its record.
//!
//! `cargo run -p termrock --example keyed_list`

mod common;

use common::{frame, key};
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use termrock::core::id::ItemKey;
use termrock::core::id::WidgetId;
use termrock::core::keyed::KeyedVec;
use termrock::widgets::list::{ListAction, ListBox, ListItem, SelectMode};

const AREA: Rect = Rect::new(0, 0, 24, 5);

struct Tasks {
    rows: KeyedVec<ListItem>,
    list: ListBox,
}

impl Tasks {
    fn new() -> Self {
        let rows = KeyedVec::try_from_rows(
            [(1, "write"), (2, "review"), (3, "ship")]
                .map(|(n, name)| (ItemKey::num(n), ListItem::new(name))),
        )
        .expect("unique keys");
        Self {
            rows,
            list: ListBox::new(WidgetId::of("example.tasks"), vec![], SelectMode::Single),
        }
    }

    /// The list borrows `rows` for this one call and keeps nothing of it.
    fn screen(&mut self) -> String {
        let (rows, list) = (&self.rows, &mut self.list);
        let (screen, result) = frame(AREA.width, AREA.height, |buf, ctx| {
            list.render_source(AREA, rows, buf, ctx, ctx.theme.canvas)
        });
        result.expect("the provider keeps the keyed contract");
        screen
    }

    fn press(&mut self, code: KeyCode) -> Vec<ListAction> {
        let response = self
            .list
            .on_key_source(&key(code), &self.rows)
            .expect("the provider keeps the keyed contract");
        response.actions().copied().collect()
    }
}

fn main() {
    let mut tasks = Tasks::new();
    tasks.screen();
    tasks.press(KeyCode::Down);
    println!("-- cursor on review --\n{}", tasks.screen());
    tasks
        .rows
        .insert(0, ItemKey::num(9), ListItem::new("plan"))
        .expect("new key");
    tasks.rows.move_to(ItemKey::num(3), 1);
    println!("-- rows reordered --\n{}", tasks.screen());
    println!("Enter: {:?}", tasks.press(KeyCode::Enter));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_stays_on_its_record_when_the_rows_are_reordered() {
        let mut tasks = Tasks::new();
        tasks.screen();
        tasks.press(KeyCode::Down);
        assert_eq!(tasks.list.keyed().cursor(), Some(ItemKey::num(2)));

        tasks
            .rows
            .insert(0, ItemKey::num(9), ListItem::new("plan"))
            .expect("new key");
        tasks.rows.move_to(ItemKey::num(3), 1);
        let screen = tasks.screen();

        assert_eq!(
            tasks.rows.position(ItemKey::num(2)),
            Some(3),
            "its index moved"
        );
        assert_eq!(
            tasks.list.keyed().cursor(),
            Some(ItemKey::num(2)),
            "its key did not"
        );
        assert!(
            screen.contains("plan") && screen.contains("review"),
            "{screen}"
        );
        assert_eq!(
            tasks.press(KeyCode::Enter).first(),
            Some(&ListAction::Activated(ItemKey::num(2))),
            "the activation names the record, not a row number"
        );
    }

    #[test]
    fn a_duplicate_key_is_rejected_and_the_rows_stay_unchanged() {
        let mut tasks = Tasks::new();
        let before = tasks.rows.stamp();
        assert!(
            tasks
                .rows
                .insert(0, ItemKey::num(2), ListItem::new("dup"))
                .is_err()
        );
        assert_eq!(tasks.rows.stamp(), before);
        assert_eq!(tasks.rows.len(), 3);
    }
}
