//! Every entry of the Capsule's menu bar, brand menu and command palette,
//! activated once: each does something the operator can see, or refuses and
//! leaves its menu open.

mod capsule_support;
mod pane_support;
mod product_support;

use capsule_support::*;
use ratatui::layout::Position;
use termrock::core::event::MouseKind;

#[derive(PartialEq)]
struct Seen {
    screen: String,
    status: String,
    commands: usize,
    effects: usize,
    left: usize,
    copied: usize,
    selected: bool,
}

fn seen(rig: &mut Rig) -> Seen {
    Seen {
        screen: rig.screen(),
        status: rig.status(),
        commands: rig.bench.commands(0).len(),
        effects: rig.backend.effects.lock().expect("effects").len(),
        left: rig.backend.left().len(),
        copied: rig.copied.0.lock().expect("sink").len(),
        selected: rig.daemon().panes[0].body.viewport().has_selection(),
    }
}

fn rig_with_a_selection() -> Rig {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, b"select me\r\n");
    rig.advance(80);
    let (x, y) = rig.locate("select");
    let pane = rig.daemon().panes[0].id;
    let area = rig
        .app
        .world
        .daemons
        .get_mut(&pane_support::instance_id())
        .expect("daemon");
    area.pane_mut(pane)
        .expect("pane")
        .body
        .viewport_mut()
        .select_word_at(Position::new(x, y));
    rig.screen();
    rig
}

enum Outcome {
    Done,
    Refused,
}

fn activate(open: impl Fn(&mut Rig), downs: usize, label: &str) -> Outcome {
    let mut rig = rig_with_a_selection();
    let before = seen(&mut rig);
    open(&mut rig);
    for _ in 0..downs {
        rig.press("down");
    }
    let open = rig.screen();
    rig.press("enter");
    let after = seen(&mut rig);
    assert!(
        !after.status.contains("No command is bound"),
        "{label}: {}",
        after.status
    );
    if after.screen == open {
        return Outcome::Refused;
    }
    assert!(after != before, "{label} did nothing");
    Outcome::Done
}

fn menubar(rig: &mut Rig, menu: usize) {
    rig.press("f10");
    for _ in 0..menu {
        rig.press("right");
    }
}

fn brand(rig: &mut Rig) {
    let at = rig.locate("jackin");
    rig.mouse(MouseKind::Down, at);
    rig.mouse(MouseKind::Up, at);
}

#[test]
fn every_menu_entry_does_something_or_refuses() {
    let words = Rig::new().app.world.surfaces.vocabulary.clone();
    let menus: [Vec<String>; 5] = [
        [
            "New tab",
            "Split right",
            "Split below",
            "Export selected file…",
            "Export selected file and reveal…",
            "Close pane",
        ]
        .map(String::from)
        .to_vec(),
        vec![
            "Copy selection".into(),
            words.paste_label.clone(),
            "Clear selection".into(),
            "Clear pane".into(),
        ],
        vec![
            "Zoom pane".into(),
            "Redraw".into(),
            "Usage".into(),
            words.info_label(),
            "GitHub context".into(),
            "Inspect changes".into(),
        ],
        [
            "New tab with account…",
            "Change tab title…",
            "Detach",
            "Close tab",
            "Quit",
        ]
        .map(String::from)
        .to_vec(),
        ["Key reference", "Command palette"]
            .map(String::from)
            .to_vec(),
    ];
    let refused = ["Export selected file and reveal…"];
    for (menu, labels) in menus.iter().enumerate() {
        for (item, label) in labels.iter().enumerate() {
            let outcome = activate(|rig| menubar(rig, menu), item, label);
            let expected_refusal = refused.contains(&label.as_str());
            assert_eq!(
                matches!(outcome, Outcome::Refused),
                expected_refusal,
                "{label}"
            );
        }
    }
}

#[test]
fn every_brand_menu_entry_does_something() {
    let words = Rig::new().app.world.surfaces.vocabulary.clone();
    for (item, label) in [words.about_label.as_str(), "Usage", "Workspace manager"]
        .into_iter()
        .enumerate()
    {
        let outcome = activate(brand, item + 1, label);
        assert!(matches!(outcome, Outcome::Done), "{label}");
    }
}

#[test]
fn every_palette_entry_does_something_or_refuses() {
    let entries: [(&str, bool); 20] = [
        ("New tab", false),
        ("Split pane", false),
        ("Zoom / unzoom pane", false),
        ("Export file", false),
        ("Export file and reveal", true),
        ("Export file and open", true),
        ("Export file under cursor", true),
        ("Export file under cursor and reveal", true),
        ("Export file under cursor and open", true),
        ("Export selected file", false),
        ("Export selected file and reveal", true),
        ("Export selected file and open", true),
        ("Stage image from clipboard path", true),
        ("Paste image from host clipboard", true),
        ("Stage image without pasting", true),
        ("Open link under cursor", true),
        ("Clear pane", false),
        ("Usage", false),
        ("Close tab", false),
        ("Quit", false),
    ];
    for (label, refuses) in entries {
        let mut rig = rig_with_a_selection();
        let before = seen(&mut rig);
        rig.press("ctrl+\\");
        rig.paste(label);
        let open = rig.screen();
        rig.press("enter");
        let after = seen(&mut rig);
        if refuses {
            assert!(after.screen == open, "{label} should refuse");
        } else {
            assert!(
                after.screen != open && after != before,
                "{label} did nothing"
            );
        }
    }
}
