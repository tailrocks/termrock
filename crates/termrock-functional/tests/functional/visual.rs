use super::bins::SHOWCASE;
use super::support::{App, find, text};

#[test]
fn showcase_held_button_matches_frozen_tuiscotti_baseline() {
    let app = App::new(
        "showcase-visual-held",
        SHOWCASE,
        &["--page", "buttons", "--motion", "reduced", "--frame", "40"],
        120,
        40,
        "truecolor",
    );
    let initial = app.wait_text("Run task");
    let run = find(&initial, "Run task");
    let hit = super::support::Point {
        x: run.x + 2,
        ..run
    };
    app.move_to(hit);
    app.down(hit);
    app.wait_screen("held state before frozen capture", |s| {
        !text(s).contains("Run task ✓")
    });
    app.visual("showcase/buttons/held");
    app.up(hit);
    app.finish();
}
