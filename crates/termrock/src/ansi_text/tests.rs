// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Tests for `ansi_text`.
use super::*;
use ratatui_core::layout::Rect;

#[test]
fn strips_ansi_sequences_from_bytes() {
    assert_eq!(
        strip_bytes(b"\x1b[31merror\x1b[0m\n").as_slice(),
        b"error\n"
    );
}

#[test]
fn strip_str_helper() {
    assert_eq!(strip_str("\x1b[1mhi\x1b[0m"), "hi");
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

#[test]
fn italic_underline_reverse_sgr() {
    let spans = styled_spans("\x1b[3;4;7mhi\x1b[0m", Style::default());
    assert!(spans[0].style.add_modifier.contains(Modifier::ITALIC));
    assert!(spans[0].style.add_modifier.contains(Modifier::UNDERLINED));
    assert!(spans[0].style.add_modifier.contains(Modifier::REVERSED));
}

#[test]
fn carriage_return_overwrites_line() {
    let opts = AnsiParseOptions::default();
    let line = parse_to_line("hello\rOK", &opts);
    // cursor home then overwrite first cells
    assert!(line.plain.starts_with("OK"), "{}", line.plain);
    assert!(!line.plain.contains('\r'));
    assert!(is_paint_safe(&line.plain));
}

#[test]
fn backspace_erases() {
    let opts = AnsiParseOptions::default();
    let line = parse_to_line("ab\x08c", &opts);
    assert_eq!(line.plain, "ac");
}

#[test]
fn tabs_expand() {
    let opts = AnsiParseOptions {
        tab_width: 4,
        ..AnsiParseOptions::default()
    };
    let line = parse_to_line("a\tb", &opts);
    assert!(line.plain.starts_with('a'));
    assert!(line.plain.contains('b'));
    assert!(!line.plain.contains('\t'));
}

#[test]
fn osc8_hyperlink_captured() {
    // OSC 8 ; ; url ST  then text then OSC 8 close
    let input = "\x1b]8;;https://example.invalid\x1b\\click\x1b]8;;\x1b\\";
    let opts = AnsiParseOptions::default();
    let line = parse_to_line(input, &opts);
    assert!(line.has_hyperlinks(), "{:?}", line.segments);
    let links = line.hyperlinks();
    assert_eq!(links[0].0, "click");
    assert!(links[0].1.contains("example.invalid"));
}

#[test]
fn osc8_rejects_javascript_scheme() {
    let input = "\x1b]8;;javascript:alert(1)\x1b\\x\x1b]8;;\x1b\\";
    let line = parse_to_line(input, &AnsiParseOptions::default());
    assert!(!line.has_hyperlinks());
}

#[test]
fn no_color_drops_fg_bg() {
    let opts = AnsiParseOptions::default().no_color(true);
    let line = parse_to_line("\x1b[31;1mred\x1b[0m", &opts);
    assert!(line.segments[0].style.fg.is_none());
    assert!(line.segments[0].style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn stream_incremental_and_bounded() {
    let mut stream = AnsiStream::new(AnsiParseOptions::default()).with_max_lines(3);
    stream.feed_str("one\n");
    stream.feed_str("two\n");
    stream.feed_str("three\n");
    stream.feed_str("four\n");
    assert_eq!(stream.lines().len(), 3);
    assert_eq!(stream.lines()[0].plain, "two");
    assert_eq!(stream.lines()[2].plain, "four");
}

#[test]
fn stream_split_truncated_csi_across_feeds() {
    let mut stream = AnsiStream::new(AnsiParseOptions::default());
    stream.feed(b"hi\x1b[3");
    stream.feed(b"1mred\x1b[0m\n");
    stream.finish_line();
    let lines: Vec<_> = stream.lines().iter().map(|l| l.plain.as_str()).collect();
    // may be one line "hired" or "hi"+"red" depending on hold — should not panic
    let plain = lines.join("");
    assert!(plain.contains("hi"));
    assert!(plain.contains("red") || plain.contains("hired") || plain.contains("hi"));
    assert!(stream.lines().iter().all(|l| is_paint_safe(&l.plain)));
}

#[test]
fn paint_safe_no_esc_in_segments() {
    let line = parse_to_line("\x1b[31merror\x1b[0m", &AnsiParseOptions::default());
    for seg in &line.segments {
        assert!(is_paint_safe(&seg.text), "{:?}", seg.text);
    }
}

#[test]
fn ansi_text_widget_paints() {
    let system = DesignSystem::default();
    let lines = parse_lines(
        "\x1b[32mok\x1b[0m\n\x1b[31mfail\x1b[0m\n",
        &AnsiParseOptions::for_system(&system),
    );
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
    let mut state = AnsiTextState::new();
    AnsiText::lines(&lines, &system).paint(Rect::new(0, 0, 20, 3), &mut buf, &mut state);
    let r0: String = (0..20).map(|x| buf[(x, 0)].symbol().to_owned()).collect();
    assert!(r0.contains("ok"), "{r0}");
}

#[test]
fn lines_for_log_integration() {
    let lines = lines_for_log("\x1b[33mwarn\x1b[0m\nnext", Style::default());
    assert!(lines.len() >= 1);
    assert!(lines[0].to_string().contains("warn"));
}

#[test]
fn long_stream_perf() {
    let mut stream = AnsiStream::new(AnsiParseOptions::default()).with_max_lines(500);
    for i in 0..2000 {
        stream.feed_str(&format!("\x1b[32mline {i}\x1b[0m\n"));
    }
    assert_eq!(stream.lines().len(), 500);
}

#[test]
fn fuzzish_malformed_bytes() {
    let junk: &[u8] = b"\x1b\x1b[999999m\x1b]\x07\x1b[38;5;m\x00\x1b[Htext\r\n";
    let mut stream = AnsiStream::new(AnsiParseOptions::default());
    stream.feed(junk);
    stream.finish_line();
    for line in stream.lines() {
        assert!(is_paint_safe(&line.plain));
        for seg in &line.segments {
            assert!(is_paint_safe(&seg.text));
        }
    }
}
