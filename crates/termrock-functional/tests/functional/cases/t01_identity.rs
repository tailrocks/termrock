//! #85: two saved connections that share a display name stay distinct
//! when a pending connect completes. State and text assertions only, so the
//! journey adds no visual baseline.

use crate::support::{App, TABLEPRO, find_all, text};
use tui_test::TerminalSnapshot;

const NAME: &str = "Local PostgreSQL";

fn listed(screen: &TerminalSnapshot) -> usize {
    find_all(screen, NAME)
        .into_iter()
        .filter(|hit| hit.x <= 35)
        .count()
}

#[test]
fn tablepro_same_name_connection_connects_to_its_own_settings() {
    let app = App::new(
        "t01-identity-same-name",
        TABLEPRO,
        &["--motion", "reduced"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Filter connections");
    app.key("alt+d");
    app.wait_text("Duplicated");
    app.key("down");
    app.key("down");
    app.wait_text("Local PostgreSQL (Copy)");

    app.key("e");
    app.wait_text("Edit connection");
    app.type_text(NAME);
    app.key("enter");
    app.click_text("Read-Only");
    app.key("ctrl+s");
    let saved = app.wait_text("Connection saved");
    assert_eq!(
        listed(&saved),
        2,
        "two rows share one name\n{}",
        text(&saved)
    );
    assert!(!text(&saved).contains("(Copy)"), "{}", text(&saved));
    assert!(
        text(&saved).contains("Safe Mode    Read-Only"),
        "the selected row is the edited second connection\n{}",
        text(&saved)
    );

    app.key("enter");
    let opened = app.wait_text("Connected to Local PostgreSQL");
    let header = text(&opened).lines().next().unwrap_or_default().to_owned();
    assert!(
        header.contains("read-only") && !header.contains("silent"),
        "the second connection's settings opened, not the first's: {header}"
    );
    app.finish();
}
