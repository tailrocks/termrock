// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Mechanical design gates over the painted surface.
//!
//! These scan `src/widgets` and `src/patterns` for rules that a reviewer would
//! otherwise have to catch by eye. Only the *painted* half of each file is
//! scanned: everything from the first `#[cfg(test)]` on is fixture text, and
//! comment lines are documentation prose, which the law exempts.
//!
//! Rules enforced here come from `docs/design/web-premium-tui-law.md` §4.1
//! ("One voice"). Add new scans next to these rather than inventing a second
//! mechanism.

use std::fs;
use std::path::{Path, PathBuf};

/// A painted source file, already trimmed of its test module.
struct PaintedSource {
    path: PathBuf,
    /// `(1-based line number, line)` for the painted half only.
    lines: Vec<(usize, String)>,
    /// Indices into `lines` that sit inside an `example_*` payload function.
    ///
    /// Those functions carry *simulated third-party output* — git porcelain,
    /// cargo test lines — which the copy rules do not govern. A literal `...`
    /// there is data, not microcopy.
    payload: Vec<bool>,
}

fn crate_src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Every painted source under `widgets/` and `patterns/`.
fn painted_sources() -> Vec<PaintedSource> {
    let src = crate_src();
    let mut out = Vec::new();
    for dir in ["widgets", "patterns"] {
        for path in rust_files(&src.join(dir)) {
            let body = fs::read_to_string(&path).expect("read source");
            let lines: Vec<(usize, String)> = body
                .lines()
                .enumerate()
                .map(|(i, l)| (i + 1, l.to_string()))
                .take_while(|(_, l)| !l.trim_start().starts_with("#[cfg(test)]"))
                .filter(|(_, l)| !l.trim_start().starts_with("//"))
                .collect();
            let payload = payload_mask(&lines);
            out.push(PaintedSource {
                path,
                lines,
                payload,
            });
        }
    }
    assert!(out.len() > 100, "painted source scan found too few files");
    out
}

/// Marks the lines that belong to an `example_*` payload function.
///
/// Top-level `fn`/`pub fn` items start at column zero, so a run ends at the
/// next such item — good enough for a lint, and it never spans a file.
fn payload_mask(lines: &[(usize, String)]) -> Vec<bool> {
    let mut out = Vec::with_capacity(lines.len());
    let mut inside = false;
    for (_, line) in lines {
        let starts_item = line.starts_with("fn ") || line.starts_with("pub fn ");
        if starts_item {
            inside = line.contains(" example_");
        }
        out.push(inside);
    }
    out
}

/// String literals on one line, quotes included.
///
/// Deliberately simple: it walks the line and respects `\"` escapes. Raw
/// strings and multi-line literals are not painted copy in this codebase.
fn string_literals(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = line.char_indices();
    while let Some((start, ch)) = chars.next() {
        if ch != '"' {
            continue;
        }
        let mut escaped = false;
        for (end, c) in chars.by_ref() {
            if escaped {
                escaped = false;
                continue;
            }
            match c {
                '\\' => escaped = true,
                '"' => {
                    out.push(line[start..=end].to_string());
                    break;
                }
                _ => {}
            }
        }
    }
    out
}

/// Whether the literal's ASCII form is explicitly gated nearby.
///
/// The law allows an ASCII twin of a Unicode string; it forbids a bare `...`
/// that no profile switch selects. Gating shows up as an `ascii` flag test or
/// an `_ASCII` constant within the surrounding few lines.
fn ascii_gated(lines: &[(usize, String)], index: usize) -> bool {
    let start = index.saturating_sub(4);
    lines[start..=index]
        .iter()
        .any(|(_, l)| l.to_ascii_lowercase().contains("ascii"))
}

#[test]
fn no_bare_ellipsis_in_paint() {
    let mut offenders = Vec::new();
    for source in painted_sources() {
        for (i, (line_no, line)) in source.lines.iter().enumerate() {
            if source.payload[i] {
                continue;
            }
            for literal in string_literals(line) {
                if !literal.contains("...") {
                    continue;
                }
                if ascii_gated(&source.lines, i) {
                    continue;
                }
                offenders.push(format!("{}:{line_no}: {literal}", source.path.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "bare `...` in painted copy — resolve through GlyphSet::ellipsis() or pair it with an \
         ASCII-gated twin (law §4.1 rule 4):\n{}",
        offenders.join("\n")
    );
}

#[test]
fn one_chord_notation() {
    // `kbd.rs` owns the spelled and symbolic renderings; it is the formatter,
    // not a caller.
    const FORMATTER: &str = "kbd.rs";
    const SPELLED: [&str; 5] = ["Ctrl+", "Control+", "Cmd+", "Alt+", "Shift+"];
    /// Mac modifier symbols. These double as resource badges (`⌘` marks an SSH
    /// host, `⌥` a branch), so only a symbol *bound to a key* is chord notation.
    const SYMBOLS: [char; 4] = ['⌘', '⌥', '⇧', '⌃'];

    let mut offenders = Vec::new();
    for source in painted_sources() {
        if source.path.ends_with(FORMATTER) {
            continue;
        }
        for (line_no, line) in &source.lines {
            for literal in string_literals(line) {
                let spelled = SPELLED.iter().find(|f| literal.contains(**f)).copied();
                let symbolic = SYMBOLS.iter().copied().find(|symbol| {
                    literal
                        .split(*symbol)
                        .skip(1)
                        .any(|rest| rest.chars().next().is_some_and(char::is_alphanumeric))
                });
                let found = spelled
                    .map(str::to_string)
                    .or_else(|| symbolic.map(|c| c.to_string()));
                if let Some(found) = found {
                    offenders.push(format!(
                        "{}:{line_no}: {found} in {literal}",
                        source.path.display()
                    ));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "spelled chord in painted copy — use the `C-x` / `A-x` / `S-x` notation or \
         `widgets::kbd::format_chord` (law §4.1 rule 3):\n{}",
        offenders.join("\n")
    );
}

#[test]
fn gates_detect_their_own_violations() {
    // The scans are only worth their runtime if they actually fire.
    let bare = [(1usize, r#"    let msg = "loading...";"#.to_string())];
    assert!(!ascii_gated(&bare, 0));
    assert_eq!(string_literals(&bare[0].1), vec!["\"loading...\""]);

    let gated = [
        (1usize, "    let msg = if self.ascii {".to_string()),
        (2, r#"        "loading...""#.to_string()),
    ];
    assert!(ascii_gated(&gated, 1));

    let escaped = r#"println!("a \"b\" c", "d");"#;
    assert_eq!(
        string_literals(escaped),
        vec![r#""a \"b\" c""#.to_string(), "\"d\"".to_string()]
    );

    let lines = [
        (
            1usize,
            "pub fn example_terminal_lines() -> Vec<Line> {".to_string(),
        ),
        (2, r#"    line("test x ... ok")"#.to_string()),
        (3, "}".to_string()),
        (4, "pub fn paint() {".to_string()),
        (5, r#"    let msg = "loading...";"#.to_string()),
    ];
    assert_eq!(payload_mask(&lines), vec![true, true, true, false, false]);
}

// ── Motion gates (docs/design/tui-motion-system.md §3, §7) ──────────────────

use std::time::Duration;

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use termrock::runtime::{FrameTick, Instant};
use termrock::style::SPINNER_DOT_PULSE_FRAMES;
use termrock::style::{DesignSystem, Motion};
use termrock::widgets::{
    Progress, ProgressKind, SPINNER_ASCII_FRAMES, SPINNER_BRAILLE_FRAMES,
    SPINNER_RECONNECT_UNICODE, SPINNER_WAITING_ASCII, SPINNER_WAITING_UNICODE, Skeleton,
    SkeletonState, Spinner, SpinnerState,
};

/// Two ticks far enough apart that any animation would have advanced.
fn two_ticks() -> (FrameTick, FrameTick) {
    let start = Instant::now();
    (
        FrameTick::manual(start, Duration::ZERO, Duration::ZERO),
        FrameTick::manual(
            start + Duration::from_millis(950),
            Duration::from_millis(950),
            Duration::from_millis(16),
        ),
    )
}

/// Renders one painter into a fresh buffer.
fn painted(area: Rect, paint: impl FnOnce(&mut Buffer)) -> Buffer {
    let mut buffer = Buffer::empty(area);
    paint(&mut buffer);
    buffer
}

#[test]
fn motion_policy_off_is_static() {
    let system = DesignSystem::default();
    let area = Rect::new(0, 0, 24, 3);
    let (first, second) = two_ticks();

    for motion in [Motion::Off, Motion::Reduced] {
        let spinner = Spinner::labeled("working", &system);
        let state = SpinnerState::new();
        let a = painted(area, |b| spinner.paint(area, b, &state, first, motion));
        let c = painted(area, |b| spinner.paint(area, b, &state, second, motion));
        assert_eq!(a, c, "Spinner animated under {motion:?}");

        let skeleton = Skeleton::new(2, &system);
        let skeleton_state = SkeletonState::new();
        let a = painted(area, |b| {
            skeleton.paint_with_state(area, b, &skeleton_state, first, motion);
        });
        let c = painted(area, |b| {
            skeleton.paint_with_state(area, b, &skeleton_state, second, motion);
        });
        assert_eq!(a, c, "Skeleton animated under {motion:?}");

        let a = painted(area, |b| {
            Progress::new(ProgressKind::indeterminate_from(first, motion), &system).paint(area, b);
        });
        let c = painted(area, |b| {
            Progress::new(ProgressKind::indeterminate_from(second, motion), &system).paint(area, b);
        });
        assert_eq!(a, c, "Progress animated under {motion:?}");
    }
}

#[test]
fn motion_policy_full_actually_animates() {
    // The Off gate is only meaningful if Full moves; otherwise it would pass
    // on a widget that never animates at all.
    let system = DesignSystem::default();
    let area = Rect::new(0, 0, 24, 3);
    let (first, second) = two_ticks();

    let spinner = Spinner::labeled("working", &system);
    let state = SpinnerState::new();
    let a = painted(area, |b| {
        spinner.paint(area, b, &state, first, Motion::Full)
    });
    let c = painted(area, |b| {
        spinner.paint(area, b, &state, second, Motion::Full);
    });
    assert_ne!(a, c, "Spinner is static even under Motion::Full");
}

#[test]
fn spinner_frames_one_column() {
    // Layout-stable animation (§7 anti-pattern 3): a frame that changes width
    // shoves its neighbours every tick.
    let sets: [(&str, &[&str]); 6] = [
        ("braille", SPINNER_BRAILLE_FRAMES),
        ("dot-pulse", SPINNER_DOT_PULSE_FRAMES),
        ("ascii", SPINNER_ASCII_FRAMES),
        ("waiting-unicode", SPINNER_WAITING_UNICODE),
        ("waiting-ascii", SPINNER_WAITING_ASCII),
        ("reconnect", SPINNER_RECONNECT_UNICODE),
    ];
    for (name, frames) in sets {
        assert!(!frames.is_empty(), "{name} frame set is empty");
        for frame in frames {
            assert_eq!(
                termrock::text::display_cols(frame),
                1,
                "{name} frame {frame:?} is not one column",
            );
        }
    }
}

// ── Selection / focus paint authority (plan 004) ─────────────────────────────

use ratatui_core::{text::Line, widgets::StatefulWidget};
use termrock::{
    style::Role,
    widgets::{List, ListRow, ListState, RowRole},
};

fn rows() -> [ListRow<'static, &'static str>; 3] {
    ["alpha", "beta", "gamma"].map(|id| ListRow {
        id,
        label: Line::from(id),
        leading: None,
        secondary: None,
        status: None,
        badge: None,
        shortcut: None,
        actions: None,
        trailing: None,
        custom: None,
        role: RowRole::Item,
        enabled: true,
        loading: false,
    })
}

/// Selection authority: a widget resolves selection chrome from the theme.
///
/// Ten collections used to clone the `DesignSystem` and force
/// `SelectionChrome::Tint` on it, so a consumer theme asking for a gutter got
/// a tint anyway — and one asking for a tint got it twice over. The theme is
/// the only authority; a widget that needs a different chrome is a bug report,
/// not a local override.
#[test]
fn selection_chrome_is_not_overridden_in_widget_paint() {
    let widgets = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/widgets");
    let mut offenders = Vec::new();
    for entry in fs::read_dir(&widgets).expect("widgets directory is readable") {
        let path = entry.expect("directory entry is readable").path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("widget source is readable");
        // Test modules legitimately build systems with an explicit chrome to
        // prove both branches paint; production paint must not.
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("split always yields a head");
        // `.selection()` is also a state accessor; only the chrome setter counts.
        if production.contains(".selection(SelectionChrome")
            || production.contains(".selection(crate::style::SelectionChrome")
        {
            offenders.push(
                path.file_name()
                    .expect("file has a name")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    offenders.sort();
    assert!(
        offenders.is_empty(),
        "these widgets override the theme's selection chrome in paint: {offenders:?}"
    );
}

/// The selection fill is opt-in: no widget paints `Role::Selection` by default.
///
/// Enabled by plan 009 once every collection has moved onto the row grammar
/// (gutter + strong label + optional wash). Until then the gate documents the
/// target and the remaining raw `Role::Selection` readers keep it red.
#[test]
#[ignore = "enable after plans 005-009 migrate the raw Role::Selection readers"]
fn no_widget_paints_selection_fill_by_default() {
    let system = DesignSystem::phosphor();
    let fill = system
        .style(Role::Selection)
        .bg
        .expect("the selection role carries a fill");
    let rows = rows();
    let area = Rect::new(0, 0, 24, 4);
    let mut buffer = Buffer::empty(area);
    let mut state = ListState::new(Some("beta"));
    (&List::new(&rows, &system)).render(area, &mut buffer, &mut state);
    for cell in buffer.content() {
        assert_ne!(
            cell.bg, fill,
            "selection fill must be opt-in, not the default row paint"
        );
    }
}
