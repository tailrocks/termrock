//! The pointer over a terminal pane in the Capsule: its scrollbar and the
//! Shift-click that extends a selection.

mod capsule_support;
mod pane_support;
mod product_support;

use capsule_support::*;
use jackin_console::domain::instance::{InstanceEntry, SandboxPhase, UnavailableInstance};
use jackin_console::world::SaveWarning;
use ratatui::crossterm::event::KeyModifiers;
use termrock::core::event::MouseKind;

fn history() -> Vec<u8> {
    (1..=100)
        .flat_map(|i| format!("line {i:03}\r\n").into_bytes())
        .collect()
}

#[test]
fn a_press_off_centre_on_the_pane_thumb_drags_it_cell_for_cell() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, &history());
    rig.advance(80);
    let (x, start, len) = rig.thumb();
    assert!(len >= 4, "the thumb is long enough to grab off-centre");
    let grab = start + 1;

    rig.mouse(MouseKind::Down, (x, grab));
    assert_eq!(rig.thumb().1, start, "pressing the thumb does not move it");

    rig.mouse(MouseKind::Drag, (x, grab - 1));
    assert_eq!(
        rig.thumb().1,
        start - 1,
        "one cell of drag moves the thumb one cell"
    );

    rig.mouse(MouseKind::Up, (x, grab - 1));
    assert_eq!(rig.thumb().1, start - 1, "letting go leaves it there");
    assert!(
        !rig.daemon().panes[0].body.viewport().has_selection(),
        "the track is not text to select"
    );
}

#[test]
fn a_shift_click_extends_the_selection_from_the_last_click() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, b"line one\r\nline two");
    rig.advance(80);
    let at = rig.locate("line one");
    let from = (at.0 + 2, at.1);
    let to = (at.0 + 6, at.1);

    rig.mouse(MouseKind::Down, from);
    rig.mouse(MouseKind::Up, from);
    rig.mouse_with(MouseKind::Down, to, KeyModifiers::SHIFT);
    rig.mouse_with(MouseKind::Up, to, KeyModifiers::SHIFT);

    assert_eq!(
        rig.daemon().panes[0].body.viewport().selected_text(),
        Some("ne o".to_owned())
    );
}

#[test]
fn the_clear_pane_menu_item_drops_the_history_and_says_so() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, &history());
    rig.advance(80);
    let content = |rig: &Rig| rig.daemon().panes[0].body.viewport().scroll().content_len;
    let before = content(&rig);
    assert!(before > 100, "the pane keeps its history: {before}");

    let edit = rig.locate("Edit");
    rig.mouse(MouseKind::Down, (edit.0 + 1, edit.1));
    rig.mouse(MouseKind::Up, (edit.0 + 1, edit.1));
    let clear = rig.locate("Clear pane");
    rig.mouse(MouseKind::Down, (clear.0 + 1, clear.1));
    rig.mouse(MouseKind::Up, (clear.0 + 1, clear.1));

    rig.screen();
    assert!(content(&rig) < before, "the history is gone");
    assert_eq!(rig.status(), "Scrollback cleared");
}

fn full_rows(width: u16) -> Vec<u8> {
    (1..=100)
        .flat_map(|_| format!("{}\r\n", "x".repeat(usize::from(width))).into_bytes())
        .collect()
}

#[test]
fn a_double_click_on_the_scroll_track_selects_and_copies_nothing() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, &history());
    rig.advance(80);
    let (track, _, _) = rig.thumb();
    rig.bench.output(0, &full_rows(track));
    rig.advance(80);
    let (x, start, len) = rig.thumb();
    let on_thumb = (x, start + len / 2);

    for _ in 0..2 {
        rig.mouse(MouseKind::Down, on_thumb);
        rig.mouse(MouseKind::Up, on_thumb);
    }

    assert_eq!(rig.daemon().panes[0].body.viewport().selected_text(), None);
    assert_eq!(*rig.copied.0.lock().expect("sink"), Vec::<String>::new());
}

#[test]
fn pointer_motion_keeps_the_durability_warning_and_a_click_dismisses_it() {
    let mut rig = Rig::new();
    rig.app
        .world
        .note_committed_save_warning(SaveWarning::DirectorySync);
    rig.mouse(MouseKind::Move, (3, 3));
    assert_eq!(
        rig.app.world.committed_save_warning,
        Some(SaveWarning::DirectorySync),
        "moving the pointer over the window does not dismiss the warning"
    );
    rig.mouse(MouseKind::Down, (3, 3));
    assert_eq!(
        rig.app.world.committed_save_warning, None,
        "a deliberate click dismisses the warning"
    );
}

#[test]
fn a_degraded_but_ready_sandbox_keeps_ticking_its_pane() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, &history());
    let id = rig
        .app
        .world
        .daemons
        .keys()
        .next()
        .cloned()
        .expect("the daemon");
    rig.app.world.instances = vec![InstanceEntry::Unavailable(UnavailableInstance::new(
        id,
        None,
        None,
        Some("the-architect".into()),
        SandboxPhase::Ready,
        None,
        "the agent metadata could not be verified",
    ))];
    rig.advance(80);
    let shown = rig.screen();
    assert!(
        shown.contains("line 0"),
        "the retained daemon keeps ticking a sandbox that is still ready: {shown}"
    );
}
