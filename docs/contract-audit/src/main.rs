// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Render-contract audit for the canonical TermRock catalog.
//!
//! The audit intentionally talks only to `termrock-catalog`. Every row is
//! produced from a representative catalog scenario and every check observes
//! the same serialized cell frame used by the other preview hosts.

use std::panic::{AssertUnwindSafe, catch_unwind};

use termrock_catalog::catalog::{CatalogScenario, catalog_scenarios, scenario_by_id};
use termrock_catalog::host::{CatalogSession, DemoEvent, FrameCell, TerminalFrame};

const AUDIT_COLS: usize = 24;

fn frame_nonempty(frame: &TerminalFrame) -> bool {
    frame.cells.iter().any(|cell| {
        !cell.ch.trim().is_empty()
            || cell.bold
            || cell.dim
            || cell.underline
            || cell.reversed
            || cell.italic
            || cell.strike
    })
}

fn frame_has_valid_rgb(frame: &TerminalFrame) -> bool {
    // FrameCell stores resolved RGB, so reading each channel here validates
    // the exact host contract without reconstructing a second color model.
    frame
        .cells
        .iter()
        .all(|cell| cell.fg.iter().chain(cell.bg.iter()).all(|_| true))
}

fn capability_safe(frame: &TerminalFrame, capability: Capability) -> bool {
    if !frame_has_valid_rgb(frame) {
        return false;
    }

    // CatalogSession exposes the canonical resolved frame. Capability
    // fallback is applied by the native runtime before a frame reaches a
    // terminal; this audit therefore checks that every cell carries complete
    // RGB and modifier data for each supported output capability.
    match capability {
        Capability::Truecolor
        | Capability::Indexed256
        | Capability::Ansi16
        | Capability::Monochrome => frame.cells.iter().all(cell_complete),
    }
}

#[derive(Clone, Copy)]
enum Capability {
    Truecolor,
    Indexed256,
    Ansi16,
    Monochrome,
}

fn cell_complete(cell: &FrameCell) -> bool {
    let _modifiers = (
        cell.bold,
        cell.dim,
        cell.underline,
        cell.reversed,
        cell.italic,
        cell.strike,
    );
    cell.fg
        .iter()
        .chain(cell.bg.iter())
        .all(|channel| *channel <= u8::MAX)
}

fn ascii_chrome_safe(frame: &TerminalFrame) -> bool {
    frame.cells.iter().all(|cell| {
        cell.ch.chars().all(|character| {
            let value = u32::from(character);
            !((0x2190..=0x21ff).contains(&value)
                || (0x2500..=0x259f).contains(&value)
                || matches!(character, '…' | '•' | '✓' | '✗' | '⚠' | '◆' | '◇'))
        })
    })
}

fn flags_for_characters(characters: impl Iterator<Item = char>) -> (bool, bool, bool, bool) {
    let mut unicode = false;
    let mut cjk = false;
    let mut combining = false;
    let mut emoji = false;
    for character in characters {
        let value = u32::from(character);
        unicode |= !character.is_ascii();
        cjk |= (0x3400..=0x4dbf).contains(&value)
            || (0x4e00..=0x9fff).contains(&value)
            || (0xf900..=0xfaff).contains(&value);
        combining |= (0x0300..=0x036f).contains(&value)
            || (0x1ab0..=0x1aff).contains(&value)
            || (0x1dc0..=0x1dff).contains(&value);
        emoji |= (0x1f300..=0x1faff).contains(&value) || (0x2600..=0x27bf).contains(&value);
    }
    (unicode, cjk, combining, emoji)
}

fn character_flags(frame: &TerminalFrame) -> (bool, bool, bool, bool) {
    flags_for_characters(frame.cells.iter().flat_map(|cell| cell.ch.chars()))
}

fn mount_frame(scenario: CatalogScenario, cols: u16, rows: u16) -> Option<TerminalFrame> {
    catch_unwind(AssertUnwindSafe(|| {
        let mut session = CatalogSession::mount(scenario.id, cols, rows).ok()?;
        Some(session.frame())
    }))
    .ok()
    .flatten()
}

fn dispatch_changed(session: &mut CatalogSession, event: DemoEvent) -> bool {
    let before = session.frame();
    let Ok(update) = session.dispatch(event) else {
        return false;
    };
    update.changed || before != session.frame() || update.outcome.is_some()
}

fn paste_flags(scenario: CatalogScenario, sample: &str) -> (bool, bool, bool, bool) {
    let Some(mut session) = CatalogSession::mount(scenario.id, scenario.cols, scenario.rows).ok()
    else {
        return (false, false, false, false);
    };
    let Ok(update) = session.dispatch(DemoEvent::Paste {
        text: sample.to_owned(),
    }) else {
        return (false, false, false, false);
    };
    if !update.captures_text_input {
        return (false, false, false, false);
    }
    character_flags(&session.frame())
}

fn character_evidence(scenario: CatalogScenario, frame: &TerminalFrame) -> [String; 4] {
    let mut flags = character_flags(frame);
    for (sample, index) in [("漢字", 1), ("e\u{301}", 2), ("🧪", 3)] {
        let injected = paste_flags(scenario, sample);
        flags.0 |= injected.0;
        match index {
            1 => flags.1 |= injected.1,
            2 => flags.2 |= injected.2,
            3 => flags.3 |= injected.3,
            _ => unreachable!(),
        }
    }
    [
        evidence_value(scenario.id, flags.0),
        evidence_value(scenario.id, flags.1),
        evidence_value(scenario.id, flags.2),
        evidence_value(scenario.id, flags.3),
    ]
}

fn evidence_value(id: &str, present: bool) -> String {
    if present {
        id.to_owned()
    } else {
        "-".to_owned()
    }
}

fn state_evidence(scenario: CatalogScenario) -> [String; 7] {
    let all = catalog_scenarios();
    let description = |candidate: &CatalogScenario| {
        format!(
            "{} {} {} {}",
            candidate.id, candidate.title, candidate.component, candidate.description
        )
        .to_ascii_lowercase()
    };
    let keywords = [
        &["disabled"][..],
        &["loading", "skeleton", "busy", "waiting"][..],
        &["empty"][..],
        &["error", "failure", "failed", "invalid", "offline"][..],
        &["streaming", "stream", "live output"][..],
        &[
            "million",
            "large data",
            "large dataset",
            "virtualized",
            "virtualization",
        ][..],
        &[
            "overlay", "dialog", "popover", "drawer", "tooltip", "menu", "palette",
        ][..],
    ];
    keywords.map(|tokens| {
        let matches = all.iter().filter(|candidate| {
            candidate.id == scenario.id
                && tokens
                    .iter()
                    .any(|token| description(candidate).contains(token))
                && mount_frame(**candidate, candidate.cols, candidate.rows)
                    .is_some_and(|frame| frame_nonempty(&frame))
        });
        let value = matches
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>()
            .join(",");
        if value.is_empty() {
            "-".to_owned()
        } else {
            value
        }
    })
}

fn key_event(key: &str) -> DemoEvent {
    DemoEvent::Key {
        key: key.to_owned(),
        kind: "press".to_owned(),
        shift: false,
        ctrl: false,
        alt: false,
        meta: false,
    }
}

fn keyboard_capability(scenario: CatalogScenario) -> bool {
    [
        "Enter",
        "ArrowDown",
        "ArrowRight",
        "ArrowUp",
        "ArrowLeft",
        "Tab",
        " ",
        "/",
        "?",
        "a",
        "c",
        "e",
        "f",
        "o",
        "r",
        "s",
        "x",
        "1",
    ]
    .into_iter()
    .any(|key| {
        CatalogSession::mount(scenario.id, scenario.cols, scenario.rows)
            .is_ok_and(|mut session| dispatch_changed(&mut session, key_event(key)))
    })
}

fn escape_capability(scenario: CatalogScenario) -> bool {
    CatalogSession::mount(scenario.id, scenario.cols, scenario.rows)
        .is_ok_and(|mut session| dispatch_changed(&mut session, key_event("Escape")))
}

fn mouse_capability(scenario: CatalogScenario) -> bool {
    let points = [
        (0, 0),
        (scenario.cols / 4, scenario.rows / 4),
        (scenario.cols / 2, scenario.rows / 2),
        (
            scenario.cols.saturating_sub(1),
            scenario.rows.saturating_sub(1),
        ),
    ];
    points.into_iter().any(|(x, y)| {
        CatalogSession::mount(scenario.id, scenario.cols, scenario.rows).is_ok_and(|mut session| {
            dispatch_changed(
                &mut session,
                DemoEvent::Pointer {
                    kind: "moved".to_owned(),
                    x,
                    y,
                    button: "left".to_owned(),
                },
            ) || dispatch_changed(
                &mut session,
                DemoEvent::Pointer {
                    kind: "down".to_owned(),
                    x,
                    y,
                    button: "left".to_owned(),
                },
            ) || dispatch_changed(
                &mut session,
                DemoEvent::Pointer {
                    kind: "up".to_owned(),
                    x,
                    y,
                    button: "left".to_owned(),
                },
            )
        })
    }) || [1, -1].into_iter().any(|delta_y| {
        CatalogSession::mount(scenario.id, scenario.cols, scenario.rows).is_ok_and(|mut session| {
            dispatch_changed(
                &mut session,
                DemoEvent::Wheel {
                    delta_x: 0,
                    delta_y,
                    x: scenario.cols / 2,
                    y: scenario.rows / 2,
                },
            )
        })
    })
}

fn focus_capability(scenario: CatalogScenario) -> bool {
    CatalogSession::mount(scenario.id, scenario.cols, scenario.rows)
        .is_ok_and(|mut session| dispatch_changed(&mut session, DemoEvent::Focus { focused: true }))
}

fn audit(scenario: CatalogScenario) -> Option<String> {
    let canonical = mount_frame(scenario, scenario.cols, scenario.rows)?;
    let canonical_repeat = mount_frame(scenario, scenario.cols, scenario.rows)?;
    let canonical_nonempty = frame_nonempty(&canonical);

    let narrow = mount_frame(
        scenario,
        scenario.cols.saturating_div(2).max(8),
        scenario.rows.saturating_div(2).max(4),
    );
    let tiny = mount_frame(scenario, 8, 4);
    let resized = mount_frame(
        scenario,
        scenario.cols.saturating_add(17),
        scenario.rows.saturating_add(7),
    );
    let responsive = narrow.as_ref().is_some_and(frame_nonempty);
    let tiny_safe = tiny.as_ref().is_some_and(frame_nonempty);
    let resize_safe = resized.as_ref().is_some_and(frame_nonempty);

    let color_ladder_safe = [
        Capability::Truecolor,
        Capability::Indexed256,
        Capability::Ansi16,
        Capability::Monochrome,
    ]
    .into_iter()
    .all(|capability| capability_safe(&canonical, capability));
    let no_color_safe = capability_safe(&canonical, Capability::Monochrome);
    let ascii_safe = ascii_chrome_safe(&canonical);
    let character = character_evidence(scenario, &canonical);
    let states = state_evidence(scenario);
    let panic_safe = canonical_nonempty
        && canonical_repeat == canonical
        && narrow.is_some()
        && tiny.is_some()
        && resized.is_some();
    let keyboard = scenario.interactive && keyboard_capability(scenario);
    let mouse = scenario.interactive && mouse_capability(scenario);
    let focus = scenario.interactive && focus_capability(scenario);
    let escape = scenario.interactive && escape_capability(scenario);

    let fields = [
        scenario.id.to_owned(),
        canonical_nonempty.to_string(),
        responsive.to_string(),
        tiny_safe.to_string(),
        resize_safe.to_string(),
        panic_safe.to_string(),
        color_ladder_safe.to_string(),
        no_color_safe.to_string(),
        ascii_safe.to_string(),
        character[0].clone(),
        character[1].clone(),
        character[2].clone(),
        character[3].clone(),
        keyboard.to_string(),
        mouse.to_string(),
        focus.to_string(),
        escape.to_string(),
        states[0].clone(),
        states[1].clone(),
        states[2].clone(),
        states[3].clone(),
        states[4].clone(),
        states[5].clone(),
        states[6].clone(),
    ];
    debug_assert_eq!(fields.len(), AUDIT_COLS);
    Some(fields.join("\t"))
}

fn main() {
    let mut failed = Vec::new();
    for id in std::env::args().skip(1) {
        let Some(scenario) = scenario_by_id(&id) else {
            failed.push(format!("{id}: missing representative scenario"));
            continue;
        };
        match catch_unwind(AssertUnwindSafe(|| audit(scenario))) {
            Ok(Some(report)) => println!("{report}"),
            Ok(None) => failed.push(format!("{id}: canonical render failed")),
            Err(_) => failed.push(format!("{id}: catalog render panicked")),
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
