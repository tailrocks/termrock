#![cfg(feature = "terminal")]

//! What untrusted program output may cost the host, and what it may leave in
//! a cell. Everything goes through `TerminalState::feed`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use junie_tui::terminal::{TermEvent, TerminalState, TerminalView};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(by: usize) {
    let live = LIVE.fetch_add(by, Ordering::SeqCst) + by;
    PEAK.fetch_max(live, Ordering::SeqCst);
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        grow(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
        grow(new);
        unsafe { System.realloc(ptr, layout, new) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn row(state: &TerminalState, row: u16) -> String {
    let view = state.view(0);
    (0..view.cols())
        .filter_map(|col| view.cell(row, col))
        .filter(|cell| !cell.is_wide_continuation())
        .map(|cell| cell.text().to_owned())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

fn cell_text(state: &TerminalState, col: u16) -> String {
    state
        .view(0)
        .cell(0, col)
        .map(|c| c.text().to_owned())
        .unwrap_or_default()
}

fn text_at(state: &TerminalState, row: u16, col: u16) -> String {
    state
        .view(0)
        .cell(row, col)
        .map(|c| c.text().to_owned())
        .unwrap_or_default()
}

fn osc52(payload: &str) -> Vec<u8> {
    format!("\x1b]52;c;{payload}\x07").into_bytes()
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                ABC[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

#[test]
fn a_mebibyte_of_combining_marks_feeds_in_well_under_a_second() {
    let mut bytes = b"a".to_vec();
    for _ in 0..(1 << 20) / 2 {
        bytes.extend_from_slice("\u{301}".as_bytes());
    }
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut state = TerminalState::new(24, 80);
        let started = std::time::Instant::now();
        for chunk in bytes.chunks(65_536) {
            state.feed(chunk);
        }
        let _ = tx.send((started.elapsed(), cell_text(&state, 0)));
    });
    let (took, cell) = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("1 MiB of combining marks must not freeze the feed");
    assert!(took < Duration::from_secs(1), "{took:?}");
    assert_eq!(
        cell.chars().count(),
        1 + 16,
        "the cell keeps a bounded cluster"
    );
}

#[test]
fn ordinary_clusters_and_text_are_unchanged() {
    let mut state = TerminalState::new(4, 40);
    state.feed("e\u{301}x \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} ok".as_bytes());
    assert_eq!(
        row(&state, 0),
        "e\u{301}x \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} ok"
    );

    let mut split = TerminalState::new(4, 40);
    let text = "e\u{301}x \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} ok".as_bytes();
    for byte in text {
        split.feed(std::slice::from_ref(byte));
    }
    assert_eq!(
        row(&split, 0),
        row(&state, 0),
        "one byte per feed gives the same screen"
    );
}

#[test]
fn marks_are_counted_across_feeds_and_through_escape_sequences() {
    let mut state = TerminalState::new(4, 40);
    state.feed(b"a");
    for _ in 0..40 {
        state.feed("\u{301}".as_bytes());
        state.feed(b"\x1b[0m");
    }
    assert_eq!(cell_text(&state, 0).chars().count(), 1 + 16);

    state.feed(b"b");
    state.feed("\u{301}".as_bytes());
    assert_eq!(
        cell_text(&state, 1).chars().count(),
        2,
        "a new base starts again"
    );
}

#[test]
fn an_unterminated_string_cannot_grow_the_parser_without_bound() {
    for introducer in [
        &b"\x1b]52;c;"[..],
        b"\x1b]0;",
        b"\x1bP",
        b"\x1b_G",
        b"\x1b^",
        b"\x1bX",
    ] {
        let mut state = TerminalState::new(24, 80);
        let chunk = vec![b'A'; 1 << 16];
        state.feed(introducer);
        let base = LIVE.load(Ordering::SeqCst);
        PEAK.store(base, Ordering::SeqCst);
        for _ in 0..(32 << 20) / chunk.len() {
            state.feed(&chunk);
        }
        let grown = PEAK.load(Ordering::SeqCst).saturating_sub(base);
        assert!(
            grown < 2 << 20,
            "{:?}: 32 MiB of an open string grew the heap by {grown} bytes",
            String::from_utf8_lossy(introducer)
        );
        state.feed(b"\x1b\\ok");
        assert_eq!(
            row(&state, 0),
            "ok",
            "the string ended and text prints again"
        );
    }
}

#[test]
fn a_dropped_string_does_not_dispatch_its_prefix() {
    let mut state = TerminalState::new(24, 80);
    let mut events = state.feed(&osc52("SGVsbG8="));
    events.extend(state.feed(b"\x1b]52;c;"));
    for _ in 0..40 {
        events.extend(state.feed(&vec![b'A'; 1 << 16]));
    }
    events.extend(state.feed(b"\x07after"));
    assert_eq!(
        events,
        vec![TermEvent::ClipboardWrite("c;SGVsbG8=".to_owned())],
        "only the short sequence dispatches"
    );
    assert_eq!(row(&state, 0), "after");
}

#[test]
fn normal_strings_still_work_up_to_the_clipboard_limit() {
    let mut state = TerminalState::new(24, 80);
    let events = state.feed(b"\x1b]0;a title\x07\x1b]0;second\x1b\\x");
    assert_eq!(
        events,
        vec![
            TermEvent::Title("a title".to_owned()),
            TermEvent::Title("second".to_owned())
        ]
    );
    assert_eq!(row(&state, 0), "x");

    let payload = base64(&vec![b'z'; 100_000]);
    let events = state.feed(&osc52(&payload));
    assert_eq!(
        events,
        vec![TermEvent::ClipboardWrite(format!("c;{payload}"))],
        "a 100,000 byte clipboard write fits"
    );

    let split = state.feed(b"\x1b]0;in two");
    assert!(split.is_empty());
    assert_eq!(
        state.feed(b" parts\x07"),
        vec![TermEvent::Title("in two parts".to_owned())]
    );
}

fn draw(state: &TerminalState) -> Buffer {
    let (rows, cols) = state.size();
    let area = Rect::new(0, 0, cols, rows);
    let mut buf = Buffer::empty(area);
    TerminalView::new(state).render(area, &mut buf);
    buf
}

#[test]
fn a_delete_byte_never_reaches_a_cell() {
    let mut state = TerminalState::new(2, 10);
    state.feed(b"A\x7fB");
    assert_eq!(row(&state, 0), "AB");
    let buf = draw(&state);
    assert_eq!(buf[(0, 0)].symbol(), "A");
    assert_eq!(buf[(1, 0)].symbol(), "B");
}

#[test]
fn bidi_controls_never_reach_a_cell() {
    let mut state = TerminalState::new(2, 20);
    let mut text = String::from("A");
    for c in ('\u{202A}'..='\u{202E}').chain('\u{2066}'..='\u{2069}') {
        text.push(c);
    }
    text.push('B');
    state.feed(text.as_bytes());
    assert_eq!(row(&state, 0), "AB");
}

#[test]
fn escape_heavy_output_gives_the_screen_of_the_bare_parser_however_it_is_split() {
    let stream = "\x1b[31mred\x1b[0m \x1b(B\x1b]0;t\x07\x1b]2;u\x1b\\ \x1bP1$r\x1b\\e\u{301}\u{e9}\
                  \x1b[2;1Hnext \x1b[1;5Hmid\x1b\x1b[3;1Hlast\x1b]8;;http://x\x07link\x1b]8;;\x07 \u{fc}\
                  \x1b[4;1H\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\x1b^pm\x1b\\\x1b_apc\x07\x18z"
        .as_bytes();
    let mut bare = termpane::DamageGrid::new(5, 30, 10);
    bare.process(stream);
    let bare_rows: Vec<String> = (0..5)
        .map(|r| {
            (0..30)
                .filter_map(|c| bare.cell(r, c))
                .filter(|cell| !cell.is_wide_continuation)
                .map(|cell| cell.contents().to_owned())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect();
    assert!(bare_rows[0].starts_with("red mid"), "{bare_rows:?}");

    for size in [stream.len(), 1, 2, 3, 5, 7] {
        let mut state = TerminalState::new(5, 30);
        for chunk in stream.chunks(size) {
            state.feed(chunk);
        }
        let rows: Vec<String> = (0..5).map(|r| row(&state, r)).collect();
        assert_eq!(rows, bare_rows, "chunks of {size}");
    }
}

const MARKS: usize = 16;

fn marks() -> String {
    "\u{301}".repeat(MARKS)
}

fn cell_marks(state: &TerminalState, col: u16) -> usize {
    cell_text(state, col).chars().count().saturating_sub(1)
}

#[test]
fn marks_cannot_pile_onto_one_cell_by_moving_the_cursor_back() {
    let cases: [(&str, &[u8], &[u8]); 8] = [
        ("cub", b"", b"b\x1b[D"),
        ("bs", b"", b"b\x08"),
        ("cup", b"", b"b\x1b[1;2H"),
        ("cr_cuf", b"", b"b\r\x1b[C"),
        ("cha", b"", b"b\x1b[2G"),
        ("restore", b"\x1b7", b"b\x1b8"),
        ("bs_inside_sgr", b"", b"b\x1b[\x08m"),
        ("bs_inside_esc", b"", b"b\x1b\x08\x1b[m"),
    ];
    let mut broken = Vec::new();
    for (name, save, back) in cases {
        let mut state = TerminalState::new(4, 40);
        state.feed(b"a");
        state.feed(marks().as_bytes());
        state.feed(save);
        for _ in 0..50 {
            state.feed(back);
            state.feed(marks().as_bytes());
        }
        if cell_marks(&state, 0) != MARKS {
            broken.push((name, cell_marks(&state, 0)));
        }
    }
    assert_eq!(broken, vec![], "cell 0 must keep exactly {MARKS} marks");
}

#[test]
fn marks_cannot_pile_onto_one_cell_through_a_wide_base_that_does_not_print() {
    let mut state = TerminalState::new(2, 10);
    state.feed(b"\x1b[?7l\x1b[1;10Ha");
    state.feed(marks().as_bytes());
    for _ in 0..50 {
        state.feed("\u{4e2d}".as_bytes());
        state.feed(marks().as_bytes());
    }
    assert_eq!(cell_marks(&state, 9), MARKS);
}

#[test]
fn a_wide_base_at_the_margin_starts_a_new_run_in_the_cell_the_grid_prints_it_in() {
    let resets: [(&str, &[u8]); 3] = [
        ("decset_7h", b"\x1b[?7h"),
        ("ris", b"\x1bc"),
        ("decstr", b"\x1b[!p"),
    ];
    let mut broken = Vec::new();
    for (name, reset) in resets {
        let mut state = TerminalState::new(2, 10);
        state.feed(b"\x1b[?7l\x1b[1;10Ha");
        state.feed(marks().as_bytes());
        state.feed(reset);
        state.feed("\u{4e2d}".as_bytes());
        state.feed(marks().as_bytes());
        let wide = format!("\u{4e2d}{}", marks());
        let holders = (0..2)
            .flat_map(|row| (0..10).map(move |col| (row, col)))
            .filter(|&(row, col)| text_at(&state, row, col) == wide)
            .count();
        if holders != 1 {
            broken.push((name, holders));
        }
    }
    assert_eq!(
        broken,
        vec![],
        "the grid prints the wide base and keeps all {MARKS} marks on it"
    );
}

#[test]
fn characters_the_parser_swallows_do_not_count_as_a_base() {
    let swallowed: [(&str, &[u8]); 2] = [
        ("charset_designation", b"\x1b(B"),
        ("c1_control", "\u{85}".as_bytes()),
    ];
    let mut broken = Vec::new();
    for (name, bytes) in swallowed {
        let mut state = TerminalState::new(4, 40);
        state.feed(b"a");
        state.feed(marks().as_bytes());
        state.feed(bytes);
        state.feed(marks().as_bytes());
        if cell_marks(&state, 0) != MARKS {
            broken.push((name, cell_marks(&state, 0)));
        }
    }
    assert_eq!(broken, vec![], "cell 0 must keep exactly {MARKS} marks");
}

#[test]
fn combining_text_after_a_cursor_move_still_renders() {
    let mut state = TerminalState::new(4, 40);
    state.feed(
        "\x1b[2;5He\u{301}\x1b[1mx\x1b[0m\u{301}\x1b[3;1H\u{1F468}\u{200D}\u{1F469}".as_bytes(),
    );
    let view = state.view(0);
    let text = |row, col| view.cell(row, col).map(|c| c.text().to_owned());
    assert_eq!(text(1, 4), Some("e\u{301}".to_owned()));
    assert_eq!(text(1, 5), Some("x\u{301}".to_owned()));
    assert_eq!(text(2, 0), Some("\u{1F468}\u{200D}\u{1F469}".to_owned()));
}

#[test]
fn a_cluster_keeps_its_marks_up_to_the_cap_on_every_line() {
    let mut state = TerminalState::new(4, 40);
    for line in 0..3 {
        state.feed(format!("\x1b[{};1Hz", line + 1).as_bytes());
        state.feed("\u{301}".repeat(MARKS + 5).as_bytes());
    }
    for line in 0..3u16 {
        assert_eq!(
            state
                .view(0)
                .cell(line, 0)
                .map(|c| c.text().chars().count()),
            Some(1 + MARKS)
        );
    }
}

const CELL_CHARS: usize = 1 + MARKS;

fn longest_cell(state: &TerminalState) -> usize {
    let view = state.view(0);
    (0..view.rows())
        .flat_map(|row| (0..view.cols()).map(move |col| (row, col)))
        .filter_map(|(row, col)| view.cell(row, col))
        .map(|cell| cell.text().chars().count())
        .max()
        .unwrap_or(0)
}

fn any_cell_holds(state: &TerminalState, wanted: char) -> bool {
    let view = state.view(0);
    (0..view.rows())
        .flat_map(|row| (0..view.cols()).map(move |col| (row, col)))
        .filter_map(|(row, col)| view.cell(row, col))
        .any(|cell| cell.text().contains(wanted))
}

#[test]
fn a_dcs_ended_by_a_raw_st_byte_cannot_release_text_unfiltered() {
    let mut state = TerminalState::new(4, 40);
    let mut bytes = b"\x1bPq\x9cA\xe2\x80\xaeB".to_vec();
    bytes.extend_from_slice("\u{301}".repeat(1_000).as_bytes());
    bytes.extend_from_slice(b"\x18visible\xe2\x80\xaex");
    for chunk in bytes.chunks(97) {
        state.feed(chunk);
    }
    assert!(
        !any_cell_holds(&state, '\u{202e}'),
        "a bidi control reached a cell"
    );
    assert!(
        longest_cell(&state) <= CELL_CHARS,
        "{}",
        longest_cell(&state)
    );
    assert_eq!(
        row(&state, 0),
        "visiblex",
        "text after the string still prints"
    );
}

#[test]
fn a_printable_after_a_joiner_cannot_grow_a_cell_without_bound() {
    let mut state = TerminalState::new(4, 40);
    state.feed(b"a");
    for _ in 0..2_000 {
        state.feed("\u{200d}b".as_bytes());
    }
    assert!(
        longest_cell(&state) <= CELL_CHARS,
        "{}",
        longest_cell(&state)
    );
    assert_eq!(cell_marks(&state, 0), MARKS);
}

#[test]
fn marks_after_a_soft_reset_at_the_margin_cannot_grow_the_last_cell() {
    let mut state = TerminalState::new(2, 10);
    state.feed(b"\x1b[1;10Ha\x1b[!p");
    for _ in 0..200 {
        state.feed(b"b");
        state.feed("\u{301}".repeat(20).as_bytes());
    }
    assert!(
        longest_cell(&state) <= CELL_CHARS,
        "{}",
        longest_cell(&state)
    );
}

#[test]
fn marks_after_a_wide_glyph_suppressed_by_no_wrap_cannot_grow_the_last_cell() {
    let mut state = TerminalState::new(2, 10);
    state.feed("\x1b[?7l\x1b[1;10Ha\u{4e2d}\x1b[?7h".as_bytes());
    for _ in 0..200 {
        state.feed(b"b");
        state.feed("\u{301}".repeat(20).as_bytes());
    }
    assert!(
        longest_cell(&state) <= CELL_CHARS,
        "{}",
        longest_cell(&state)
    );
}

const SPOOFING: [(char, char); 5] = [
    ('\u{061c}', '\u{061c}'),
    ('\u{200e}', '\u{200f}'),
    ('\u{202a}', '\u{202e}'),
    ('\u{2066}', '\u{2069}'),
    ('\u{e0000}', '\u{e007f}'),
];

fn spoofing_chars() -> Vec<char> {
    SPOOFING
        .iter()
        .flat_map(|&(first, last)| first..=last)
        .collect()
}

fn leaked(state: &TerminalState) -> Vec<char> {
    let view = state.view(0);
    let mut found: Vec<char> = (0..view.rows())
        .flat_map(|row| (0..view.cols()).map(move |col| (row, col)))
        .filter_map(|(row, col)| view.cell(row, col))
        .flat_map(|cell| cell.text().chars().collect::<Vec<_>>())
        .filter(|c| spoofing_chars().contains(c))
        .collect();
    found.sort_unstable();
    found.dedup();
    found
}

fn fed_in_chunks(bytes: &[u8], chunk: usize) -> TerminalState {
    let mut state = TerminalState::new(4, 60);
    for part in bytes.chunks(chunk) {
        state.feed(part);
    }
    state
}

fn broken_encodings(c: char) -> Vec<Vec<u8>> {
    let mut utf8 = [0_u8; 4];
    let bytes = c.encode_utf8(&mut utf8).as_bytes().to_vec();
    let (lead, rest) = bytes.split_first().expect("a character has bytes");
    let mut deleted = vec![*lead, 0x7f];
    deleted.extend_from_slice(rest);
    let mut doubled = vec![*lead];
    doubled.extend_from_slice(&bytes);
    doubled.extend_from_slice(rest);
    vec![deleted, doubled]
}

#[test]
fn a_lead_byte_cut_short_cannot_leave_a_bidi_control_in_a_cell() {
    for c in spoofing_chars() {
        for broken in broken_encodings(c) {
            let mut bytes = b"ab".to_vec();
            bytes.extend_from_slice(&broken);
            bytes.extend_from_slice(b"cd");
            for chunk in [bytes.len(), 1] {
                let state = fed_in_chunks(&bytes, chunk);
                assert!(
                    leaked(&state).is_empty(),
                    "U+{:04X} leaked from {broken:02x?} in chunks of {chunk}: {:?}",
                    c as u32,
                    leaked(&state)
                );
                assert!(row(&state, 0).starts_with("ab"), "{broken:02x?}");
                assert!(row(&state, 0).ends_with("cd"), "{broken:02x?}");
            }
        }
    }
}

#[test]
fn the_reviewed_byte_sequences_leave_no_bidi_in_any_cell() {
    let cases: [&[u8]; 6] = [
        b"ab\xe2\x7f\x80\xae",
        b"ab\xe2\xe2\x80\xae\x80\xae",
        b"ab\xe2\x7f\x81\xa6",
        b"ab\xe2\x7f\x81\xa9",
        b"ab\xe2\xe2\x81\xa6\x81\xa6",
        b"ab\xe2\xe2\x81\xa9\x81\xa9",
    ];
    for bytes in cases {
        for chunk in [bytes.len(), 1] {
            let state = fed_in_chunks(bytes, chunk);
            assert!(
                leaked(&state).is_empty(),
                "{bytes:02x?} in chunks of {chunk}: {:?}",
                leaked(&state)
            );
            assert!(row(&state, 0).starts_with("ab"), "{bytes:02x?}");
        }
    }
}

#[test]
fn broken_utf8_prints_as_replacement_characters_and_text_continues() {
    let mut state = TerminalState::new(2, 20);
    state.feed(b"a\xe2");
    state.feed(b"b\x80c\xff\xc3(d");
    assert_eq!(row(&state, 0), "a\u{fffd}b\u{fffd}c\u{fffd}\u{fffd}(d");
}

#[test]
fn invisible_format_characters_are_dropped_from_every_cell() {
    for c in spoofing_chars() {
        let mut state = TerminalState::new(2, 20);
        state.feed(format!("a{c}b").as_bytes());
        assert_eq!(row(&state, 0), "ab", "U+{:04X}", c as u32);
    }
}

#[test]
fn legitimate_text_is_unchanged() {
    let text = "h\u{e9}llo w\u{f6}rld \u{65e5}\u{672c}\u{8a9e} \u{1f389} e\u{301} \u{1f468}\u{200d}\u{1f469} \u{5e9}\u{5dc}\u{5d5}\u{5dd} \u{645}\u{631}\u{62d}\u{628}\u{627}";
    for chunk in [text.len(), 1] {
        let mut state = TerminalState::new(2, 60);
        for part in text.as_bytes().chunks(chunk) {
            state.feed(part);
        }
        assert_eq!(row(&state, 0), text, "chunks of {chunk}");
    }
}
