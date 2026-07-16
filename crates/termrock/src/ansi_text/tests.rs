// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Tests for `ansi_text`.
use super::*;

#[test]
fn strips_ansi_sequences_from_bytes() {
    assert_eq!(
        strip_bytes(b"\x1b[31merror\x1b[0m\n").as_slice(),
        b"error\n"
    );
}

#[test]
fn converts_sgr_to_styled_spans() {
    let spans = styled_spans(
        "plain \x1b[31mbad\x1b[0m ok",
        Style::default().fg(Color::Gray),
    );
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[1].content, "bad");
    assert_eq!(spans[1].style.fg, Some(Color::Red));
    assert_eq!(spans[2].style.fg, Some(Color::Gray));
}

#[test]
fn converts_ansi_to_an_owned_line_once_at_ingest() {
    let default = Style::default().fg(Color::Gray);
    let styled = line_from_ansi("plain \x1b[31mbad\x1b[0m", default);
    assert_eq!(styled.to_string(), "plain bad");
    assert_eq!(styled.spans[1].style.fg, Some(Color::Red));

    let plain = line_from_ansi("plain", default);
    assert_eq!(plain.spans.len(), 1);
    assert_eq!(plain.spans[0].content, "plain");
    assert_eq!(plain.spans[0].style, default);
}

#[test]
fn parses_indexed_truecolor_bright_and_background_colors() {
    for (input, foreground, background) in [
        ("\x1b[38;5;196mx", Some(Color::Indexed(196)), None),
        ("\x1b[38;5;300mx", Some(Color::Indexed(255)), None),
        ("\x1b[38;2;1;2;3mx", Some(Color::Rgb(1, 2, 3)), None),
        ("\x1b[48;5;27mx", None, Some(Color::Indexed(27))),
        ("\x1b[41mx", None, Some(Color::Red)),
        ("\x1b[104mx", None, Some(Color::LightBlue)),
        ("\x1b[92mx", Some(Color::LightGreen), None),
    ] {
        let spans = styled_spans(input, Style::default());
        assert_eq!(spans.len(), 1, "{input:?}");
        assert_eq!(spans[0].style.fg, foreground, "{input:?}");
        assert_eq!(spans[0].style.bg, background, "{input:?}");
    }
}

#[test]
fn modifiers_combine_and_code_22_clears_bold_and_dim() {
    let spans = styled_spans("\x1b[1;2mstrong\x1b[22mplain", Style::default());
    assert!(spans[0].style.add_modifier.contains(Modifier::BOLD));
    assert!(spans[0].style.add_modifier.contains(Modifier::DIM));
    assert!(!spans[1].style.add_modifier.contains(Modifier::BOLD));
    assert!(!spans[1].style.add_modifier.contains(Modifier::DIM));
}

#[test]
fn foreground_background_and_empty_reset_restore_defaults() {
    let default = Style::default().fg(Color::Gray).bg(Color::Blue);
    let spans = styled_spans(
        "\x1b[31;42mchanged\x1b[39;49mdefaults\x1b[1mstrong\x1b[mreset",
        default,
    );
    assert_eq!(spans[0].style.fg, Some(Color::Red));
    assert_eq!(spans[0].style.bg, Some(Color::Green));
    assert_eq!(spans[1].style, default);
    assert!(spans[2].style.add_modifier.contains(Modifier::BOLD));
    assert_eq!(spans[3].style, default);
}

#[test]
fn multi_code_sequence_applies_every_supported_attribute() {
    let spans = styled_spans("\x1b[1;31;44mx", Style::default());
    assert_eq!(spans[0].style.fg, Some(Color::Red));
    assert_eq!(spans[0].style.bg, Some(Color::Blue));
    assert!(spans[0].style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn malformed_and_truncated_sequences_do_not_panic() {
    let malformed = styled_spans("\x1b[38mx", Style::default().fg(Color::Gray));
    assert_eq!(malformed[0].content, "x");
    assert_eq!(malformed[0].style.fg, Some(Color::Gray));

    let truncated = styled_spans("text\x1b[", Style::default());
    assert_eq!(truncated.len(), 1);
    assert_eq!(truncated[0].content, "text");
}

#[test]
fn stripping_removes_escape_bytes_from_supported_and_malformed_sequences() {
    for input in [
        "\x1b[38;5;196mindexed\x1b[0m",
        "\x1b[38;2;1;2;3mtruecolor\x1b[0m",
        "\x1b[48;5;27mbackground\x1b[49m",
        "\x1b[1;31;44mmulti\x1b[m",
        "\x1b[38mmalformed",
        "truncated\x1b[",
    ] {
        assert!(
            !strip_bytes(input.as_bytes()).contains(&b'\x1b'),
            "{input:?}"
        );
    }
}
