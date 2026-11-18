//! Direct calls to widget APIs whose Showcase demo is still deferred (#14).
//! The catalog gate names these tests, so each must call the API on its own
//! type.

use ratatui::layout::Constraint;
use termrock::core::id::WidgetId;
use termrock::widgets::table::{Cell, Column, DataTable};
use termrock::widgets::tabs::{TabItem, Tabs};

#[test]
fn tabs_push_appends_the_item_after_the_existing_ones() {
    let mut tabs = Tabs::new(WidgetId::of("t.push"), &["one"]);
    tabs.push(TabItem::new("two").closable());
    tabs.push(TabItem::new("three"));
    let labels: Vec<&str> = tabs.items().iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, ["one", "two", "three"]);
    assert!(tabs.items()[1].closable, "the pushed item keeps its flags");
}

#[test]
fn data_table_columns_returns_the_columns_in_construction_order() {
    let columns = vec![
        Column::new("name", Constraint::Length(10)).editable(),
        Column::new("owner", Constraint::Length(8)),
    ];
    let table = DataTable::try_new(
        WidgetId::of("t.columns"),
        columns.clone(),
        vec![vec![Cell::new("a"), Cell::new("b")]],
    )
    .expect("rectangular fixture");
    assert_eq!(table.columns(), columns.as_slice());
    assert_eq!(table.columns()[0].title, "name");
    assert!(table.columns()[0].editable && !table.columns()[1].editable);
}
