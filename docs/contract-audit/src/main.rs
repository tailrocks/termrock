// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

use std::panic::{AssertUnwindSafe, catch_unwind};

use ratatui_core::{buffer::Buffer, style::Color};
use termrock::style::{ColorCapability, DesignSystem, GlyphSet};
use termrock_lookbook::{
    demo::{DemoEvent, DemoSession},
    frame::{STORY_PAD, paint_story_buffer, story_by_id},
    stories::{Story, stories},
};

fn inner_cells(buffer: &Buffer) -> impl Iterator<Item = &ratatui_core::buffer::Cell> {
    let area = buffer.area;
    (STORY_PAD..area.height.saturating_sub(STORY_PAD)).flat_map(move |y| {
        (STORY_PAD..area.width.saturating_sub(STORY_PAD)).map(move |x| &buffer[(x, y)])
    })
}

fn nonempty(buffer: &Buffer) -> bool {
    inner_cells(buffer).any(|cell| {
        !cell.symbol().trim().is_empty()
            || cell.fg != Color::Reset
            || cell.bg != Color::Reset
            || !cell.modifier.is_empty()
    })
}

fn capability_color(color: Color, capability: ColorCapability) -> bool {
    match capability {
        ColorCapability::Truecolor => true,
        ColorCapability::Indexed256 => !matches!(color, Color::Rgb(_, _, _)),
        ColorCapability::Ansi16 => match color {
            Color::Rgb(_, _, _) => false,
            Color::Indexed(index) => index <= 15,
            _ => true,
        },
        ColorCapability::Monochrome => matches!(color, Color::Reset),
        _ => false,
    }
}

fn capability_safe(buffer: &Buffer, capability: ColorCapability) -> bool {
    inner_cells(buffer).all(|cell| {
        capability_color(cell.fg, capability) && capability_color(cell.bg, capability)
    })
}

fn ascii_chrome_safe(buffer: &Buffer) -> bool {
    inner_cells(buffer).all(|cell| {
        cell.symbol().chars().all(|character| {
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

fn character_flags(buffer: &Buffer) -> (bool, bool, bool, bool) {
    flags_for_characters(inner_cells(buffer).flat_map(|cell| cell.symbol().chars()))
}

fn injected_character_flags(story: Story, sample: &str) -> (bool, bool, bool, bool) {
    let Ok(mut session) = DemoSession::mount(story.id, None, None) else {
        return (false, false, false, false);
    };
    if !session.captures_text_input() {
        return (false, false, false, false);
    }
    if session
        .dispatch(DemoEvent::Paste { text: sample.to_owned() })
        .is_err()
    {
        return (false, false, false, false);
    }
    let frame = session.frame();
    flags_for_characters(frame.cells.iter().flat_map(|cell| cell.ch.chars()))
}

fn character_evidence(story: Story) -> (String, String, String, String) {
    let system = DesignSystem::phosphor();
    let mut unicode = Vec::new();
    let mut cjk = Vec::new();
    let mut combining = Vec::new();
    let mut emoji = Vec::new();
    for sibling in stories()
        .into_iter()
        .filter(|candidate| candidate.identity() == story.identity())
    {
        let Some(buffer) = render(sibling, &system, sibling.width, sibling.height) else {
            continue;
        };
        let flags = character_flags(&buffer);
        if flags.0 {
            unicode.push(sibling.id);
        }
        if flags.1 {
            cjk.push(sibling.id);
        }
        if flags.2 {
            combining.push(sibling.id);
        }
        if flags.3 {
            emoji.push(sibling.id);
        }
    }
    for (sample, target) in [
        ("漢字", &mut cjk),
        ("e\u{301}", &mut combining),
        ("🧪", &mut emoji),
    ] {
        let flags = injected_character_flags(story, sample);
        if (flags.0 && !unicode.contains(&story.id)) {
            unicode.push(story.id);
        }
        let matched = match sample {
            "漢字" => flags.1,
            "e\u{301}" => flags.2,
            "🧪" => flags.3,
            _ => false,
        };
        if matched && !target.contains(&story.id) {
            target.push(story.id);
        }
    }
    (
        unicode.join(","),
        cjk.join(","),
        combining.join(","),
        emoji.join(","),
    )
}

fn sibling_story_evidence(
    story: Story,
    predicate: impl Fn(&str) -> bool,
) -> String {
    let system = DesignSystem::phosphor();
    stories()
        .into_iter()
        .filter(|candidate| candidate.identity() == story.identity())
        .filter(|candidate| {
            let metadata = format!("{} {} {}", candidate.id, candidate.title, candidate.description)
                .to_ascii_lowercase();
            predicate(&metadata)
                && render(*candidate, &system, candidate.width, candidate.height)
                    .is_some_and(|buffer| nonempty(&buffer))
        })
        .map(|candidate| candidate.id)
        .collect::<Vec<_>>()
        .join(",")
}

fn state_evidence(story: Story) -> [String; 7] {
    [
        sibling_story_evidence(story, |text| text.contains("disabled")),
        sibling_story_evidence(story, |text| {
            ["loading", "skeleton", "busy", "waiting"].iter().any(|token| text.contains(token))
        }),
        sibling_story_evidence(story, |text| text.contains("empty")),
        sibling_story_evidence(story, |text| {
            ["error", "failure", "failed", "invalid", "offline"].iter().any(|token| text.contains(token))
        }),
        sibling_story_evidence(story, |text| {
            ["streaming", "stream", "live output"].iter().any(|token| text.contains(token))
        }),
        sibling_story_evidence(story, |text| {
            ["million", "large data", "large dataset", "virtualized", "virtualization"]
                .iter()
                .any(|token| text.contains(token))
        }),
        sibling_story_evidence(story, |text| {
            ["overlay", "dialog", "popover", "drawer", "tooltip", "menu", "palette"]
                .iter()
                .any(|token| text.contains(token))
        }),
    ]
}

fn render(story: Story, system: &DesignSystem, cols: u16, rows: u16) -> Option<Buffer> {
    catch_unwind(AssertUnwindSafe(|| {
        paint_story_buffer(story, system, Some(cols), Some(rows))
    }))
    .ok()
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

fn keyboard_capability(story: Story) -> bool {
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
        let Ok(mut session) = DemoSession::mount(story.id, None, None) else {
            return false;
        };
        session
            .dispatch(key_event(key))
            .is_ok_and(|update| update.changed || update.outcome.is_some())
    })
}

fn escape_capability(story: Story) -> bool {
    let Ok(mut session) = DemoSession::mount(story.id, None, None) else {
        return false;
    };
    session
        .dispatch(key_event("Escape"))
        .is_ok_and(|update| update.changed || update.outcome.is_some())
}

fn mouse_capability(story: Story) -> bool {
    let points = [
        (2, 2),
        (story.width / 4 + 1, story.height / 4 + 1),
        (story.width / 2 + 1, story.height / 2 + 1),
        (story.width.saturating_sub(1), story.height.saturating_sub(1)),
    ];
    points.into_iter().any(|(x, y)| {
        let Ok(mut session) = DemoSession::mount(story.id, None, None) else {
            return false;
        };
        session
            .dispatch(DemoEvent::Pointer {
                kind: "down".to_owned(),
                x,
                y,
                button: "left".to_owned(),
            })
            .is_ok_and(|update| update.changed || update.outcome.is_some())
    }) || [1, -1].into_iter().any(|delta_y| {
        let Ok(mut session) = DemoSession::mount(story.id, None, None) else {
            return false;
        };
        session
            .dispatch(DemoEvent::Wheel {
                delta_x: 0,
                delta_y,
                x: story.width / 2 + 1,
                y: story.height / 2 + 1,
            })
            .is_ok_and(|update| update.changed || update.outcome.is_some())
    })
}

fn focus_capability(story: Story) -> bool {
    let Ok(mut session) = DemoSession::mount(story.id, None, None) else {
        return false;
    };
    let before = session.frame();
    let Ok(update) = session.dispatch(DemoEvent::Focus { focused: true }) else {
        return false;
    };
    let after = session.frame();
    update.changed && before.cells != after.cells
}

fn audit(story: Story) -> Option<String> {
    let canonical_system = DesignSystem::phosphor();
    let canonical = render(story, &canonical_system, story.width, story.height)?;
    let canonical_repeat = render(story, &canonical_system, story.width, story.height);
    let canonical_nonempty = nonempty(&canonical);
    let narrow = render(
        story,
        &canonical_system,
        story.width.saturating_div(2).max(8),
        story.height.saturating_div(2).max(4),
    );
    let tiny = render(story, &canonical_system, 8, 4);
    let large = render(
        story,
        &canonical_system,
        story.width.saturating_add(17),
        story.height.saturating_add(7),
    );
    let responsive = narrow.as_ref().is_some_and(nonempty);
    let tiny_safe = tiny.is_some();
    let resize_safe = large.as_ref().is_some_and(nonempty);

    let capabilities = [
        ColorCapability::Truecolor,
        ColorCapability::Indexed256,
        ColorCapability::Ansi16,
        ColorCapability::Monochrome,
    ];
    let color_ladder_safe = capabilities.into_iter().all(|capability| {
        let system = DesignSystem::phosphor().quantize(capability);
        render(story, &system, story.width, story.height)
            .is_some_and(|buffer| nonempty(&buffer) && capability_safe(&buffer, capability))
    });
    let no_color = DesignSystem::phosphor().no_color().glyphs(GlyphSet::Ascii);
    let no_color_buffer = render(story, &no_color, story.width, story.height);
    let no_color_safe = no_color_buffer
        .as_ref()
        .is_some_and(|buffer| nonempty(buffer) && capability_safe(buffer, ColorCapability::Monochrome));
    let ascii_safe = no_color_buffer.as_ref().is_some_and(ascii_chrome_safe);
    let (unicode, cjk, combining, emoji) = character_evidence(story);
    let states = state_evidence(story);
    let panic_safe = canonical_nonempty
        && canonical_repeat.as_ref() == Some(&canonical)
        && narrow.is_some()
        && tiny_safe
        && large.is_some()
        && no_color_buffer.is_some();
    let keyboard = story.is_interactive() && keyboard_capability(story);
    let mouse = story.is_interactive() && mouse_capability(story);
    let focus = story.is_interactive() && focus_capability(story);
    let escape = story.is_interactive() && escape_capability(story);
    Some(format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        story.id,
        canonical_nonempty,
        responsive,
        tiny_safe,
        resize_safe,
        panic_safe,
        color_ladder_safe,
        no_color_safe,
        ascii_safe,
        if unicode.is_empty() { "-" } else { &unicode },
        if cjk.is_empty() { "-" } else { &cjk },
        if combining.is_empty() { "-" } else { &combining },
        if emoji.is_empty() { "-" } else { &emoji },
        keyboard,
        mouse,
        focus,
        escape,
        if states[0].is_empty() { "-" } else { &states[0] },
        if states[1].is_empty() { "-" } else { &states[1] },
        if states[2].is_empty() { "-" } else { &states[2] },
        if states[3].is_empty() { "-" } else { &states[3] },
        if states[4].is_empty() { "-" } else { &states[4] },
        if states[5].is_empty() { "-" } else { &states[5] },
        if states[6].is_empty() { "-" } else { &states[6] },
    ))
}

fn main() {
    let mut failed = Vec::new();
    for id in std::env::args().skip(1) {
        let Some(story) = story_by_id(&id) else {
            failed.push(format!("{id}: missing story"));
            continue;
        };
        match audit(story) {
            Some(report) => println!("{report}"),
            None => failed.push(format!("{id}: canonical render failed")),
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
