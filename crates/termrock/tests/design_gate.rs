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

// ── Motion gates (docs/design/tui-motion-system.md §3, §7) ─────────────────

use std::time::Duration;

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use termrock::runtime::{FrameTick, Instant};
use termrock::style::SPINNER_DOT_PULSE_FRAMES;
use termrock::style::{DesignSystem, MotionPolicy};
use termrock::widgets::{
    Progress, ProgressKind, SPINNER_ASCII_FRAMES, SPINNER_BRAILLE_FRAMES,
    SPINNER_RECONNECT_UNICODE, SPINNER_STREAM_ASCII, SPINNER_STREAM_UNICODE, SPINNER_WAITING_ASCII,
    SPINNER_WAITING_UNICODE, Skeleton, SkeletonState, Spinner, SpinnerState,
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

    for motion in [MotionPolicy::Off, MotionPolicy::Basic] {
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
        spinner.paint(area, b, &state, first, MotionPolicy::Full)
    });
    let c = painted(area, |b| {
        spinner.paint(area, b, &state, second, MotionPolicy::Full);
    });
    assert_ne!(a, c, "Spinner is static even under MotionPolicy::Full");
}

#[test]
fn spinner_frames_one_column() {
    // Layout-stable animation (§7 anti-pattern 3): a frame that changes width
    // shoves its neighbours every tick.
    let sets: [(&str, &[&str]); 8] = [
        ("braille", SPINNER_BRAILLE_FRAMES),
        ("dot-pulse", SPINNER_DOT_PULSE_FRAMES),
        ("ascii", SPINNER_ASCII_FRAMES),
        ("waiting-unicode", SPINNER_WAITING_UNICODE),
        ("waiting-ascii", SPINNER_WAITING_ASCII),
        ("reconnect", SPINNER_RECONNECT_UNICODE),
        ("stream-unicode", SPINNER_STREAM_UNICODE),
        ("stream-ascii", SPINNER_STREAM_ASCII),
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

// ── One field chrome (plan 008) ──────────────────────────────────────────────

/// A focused field looks focused, and every field in the family agrees how.
///
/// Before `input_recipe` had consumers, a focused `TextInput` was
/// pixel-identical to an unfocused one apart from the caret — the label
/// underline had been carrying the entire focus signal, on the wrong element,
/// and plan 005 removed it. The well plus the prompt cell carry it now.
#[test]
fn a_focused_field_says_so() {
    use termrock::{
        style::{DesignSystem, Role},
        widgets::{TextInput, TextInputState},
    };

    let system = DesignSystem::phosphor();
    let area = Rect::new(0, 0, 20, 2);

    let mut resting = Buffer::empty(area);
    let mut resting_state = TextInputState::new("value");
    TextInput::new("Name", &system).paint(area, &mut resting, &mut resting_state);

    let mut focused = Buffer::empty(area);
    let mut focused_state = TextInputState::new("value");
    focused_state.set_focused(true);
    TextInput::new("Name", &system).paint(area, &mut focused, &mut focused_state);

    assert_ne!(
        resting, focused,
        "a focused field must differ from a resting one by more than its caret"
    );

    let well = system
        .style(Role::Sunken)
        .bg
        .expect("the field well carries a background");
    let resting_wells = resting.content().iter().filter(|c| c.bg == well).count();
    assert!(
        resting_wells > 0,
        "the well is painted in every state, not only when focused"
    );

    let cue = system
        .style(Role::BorderFocused)
        .fg
        .expect("the focus role carries a foreground");
    assert!(
        focused.content().iter().any(|c| c.fg == cue),
        "the focused field paints its prompt cue"
    );
    assert!(
        !resting.content().iter().any(|c| c.fg == cue),
        "a resting field does not"
    );
}

// ── One selection language (plan 006) ────────────────────────────────────────

/// Every collection marks its selected row with the same glyph.
///
/// Five widgets used to invent their own "current row" marker — `›`, `>`, `•`,
/// `▸`, `*` — so moving between a list, a table and a rail meant relearning
/// what selection looks like. The catalog's `selection_gutter()` is the one
/// answer, and this renders the families side by side to prove it.
#[test]
fn collections_share_one_gutter_glyph() {
    use ratatui_core::widgets::Widget;
    use termrock::{
        style::DesignSystem,
        widgets::{
            Column, ColumnWidth, Table, TableRow, TableState, Timeline, TimelineEvent, Tree,
            TreeNode, TreeState,
        },
    };

    let system = DesignSystem::phosphor();
    let gutter = system.glyphs.selection_gutter();
    let area = Rect::new(0, 0, 28, 4);

    // List
    let rows = rows();
    let mut list_state = ListState::new(Some("beta"));
    let mut list_buffer = Buffer::empty(area);
    (&List::new(&rows, &system)).render(area, &mut list_buffer, &mut list_state);
    let list_row = list_state
        .regions()
        .iter()
        .find(|r| r.id == "beta")
        .expect("the selected row was painted")
        .area;
    assert_eq!(
        list_buffer[(list_row.x, list_row.y)].symbol(),
        gutter,
        "List"
    );

    // Table
    let columns = [Column::new("name", "Name", ColumnWidth::Fixed(10))];
    let alpha = [Line::from("alpha")];
    let beta = [Line::from("beta")];
    let table_rows = [TableRow::new(0u8, &alpha), TableRow::new(1u8, &beta)];
    let mut table_state = TableState::new(Some(1u8));
    let mut table_buffer = Buffer::empty(area);
    (&Table::new(&columns, &table_rows, &system)).render(area, &mut table_buffer, &mut table_state);
    let table_row = table_state
        .row_regions
        .iter()
        .find(|r| r.id == 1u8)
        .expect("the selected row was painted")
        .area;
    assert_eq!(
        table_buffer[(table_row.x, table_row.y)].symbol(),
        gutter,
        "Table"
    );

    // Tree
    let nodes = vec![
        TreeNode::new("root", Line::from("Workspace"), 0),
        TreeNode::new("leaf", Line::from("File"), 1),
    ];
    let mut tree_state = TreeState::new(Some("leaf"));
    let mut tree_buffer = Buffer::empty(area);
    Tree::new(&nodes, &system).render(area, &mut tree_buffer, &mut tree_state);
    let tree_row = tree_state
        .regions()
        .iter()
        .find(|r| r.id == "leaf")
        .expect("the selected row was painted")
        .area;
    assert_eq!(
        tree_buffer[(tree_row.x, tree_row.y)].symbol(),
        gutter,
        "Tree"
    );

    // Timeline
    let events = [
        TimelineEvent::new("12:01", "Started"),
        TimelineEvent::new("12:02", "Running"),
    ];
    let mut timeline_buffer = Buffer::empty(area);
    Widget::render(&Timeline::new(&events, &system), area, &mut timeline_buffer);
    assert_eq!(
        timeline_buffer[(area.x, area.y)].symbol(),
        gutter,
        "Timeline"
    );
}

// ── Underline-free interaction grammar (plan 005) ────────────────────────────

/// Underline means "link", and nothing else.
///
/// It used to mean focus, selection, hover, current item, sort, severity,
/// match, syntax class and button affordance — everything except the one thing
/// a reader expects. The binding grammar is
/// `docs/design/termrock-design-language.md` §5.9: content passthrough,
/// monochrome links, and an explicit cursor fallback are the only survivors.
/// Every file below is on the whitelist for one of those reasons.
///
/// The scan covers `TextSpan::underline(true)` as well as the modifier itself:
/// a builder that sets the modifier without naming it is the same cue wearing
/// a different spelling, and that is exactly how one survivor slipped through.
#[test]
fn interaction_underline_is_dead() {
    /// file -> why its underline is legitimate
    const WHITELIST: &[(&str, &str)] = &[
        ("link.rs", "the link affordance itself (LinkStyle policy)"),
        ("citation.rs", "a citation is a link"),
        ("key_value_list.rs", "href values are links"),
        ("primitives.rs", "ButtonVariant::Link renders as a link"),
        ("code_block.rs", "diagnostic spans: squiggle substitute"),
        (
            "text.rs",
            "TextSpan::underline is author-set content, not a state",
        ),
        ("markdown.rs", "a markdown link is a link"),
    ];

    let widgets = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/widgets");
    let mut offenders = Vec::new();
    for path in rust_files(&widgets) {
        let name = path
            .file_name()
            .expect("file has a name")
            .to_string_lossy()
            .into_owned();
        if WHITELIST.iter().any(|(f, _)| *f == name) {
            continue;
        }
        let body = fs::read_to_string(&path).expect("widget source is readable");
        let painted = body
            .split("#[cfg(test)]")
            .next()
            .expect("split always yields a head");
        // `TextSpan::underline(true)` reaches the same modifier without ever
        // naming it — that is how the focused search match stayed underlined
        // through plan 005's sweep (plans/009).
        if painted.contains("Modifier::UNDERLINED")
            || painted.contains(".underlined()")
            || painted.contains(".underline(true)")
        {
            offenders.push(name);
        }
    }
    offenders.sort();
    offenders.dedup();
    assert!(
        offenders.is_empty(),
        "underline is not an interaction cue (design-language §5.9) — found in: {offenders:?}"
    );
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
/// Active as of plan 009: every collection paints the row grammar (gutter +
/// strong label + optional wash), so the neon fill is opt-in chrome a host
/// asks for, never what a list does on its own.
#[test]
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

/// Chords a footer hint literal may advertise.
///
/// Sixteen chords in one row is a keymap dump wearing a hint row's clothes:
/// the eye reads none of them. The rest belong in the keyboard-help overlay.
const HINT_COPY_BUDGET: usize = 5;

/// Whether a ` · `-joined literal reads as a row of chords.
///
/// A hint segment is a short chord token followed by its verb — `x stop`,
/// `C-r run`, `esc cancel`. Content rows join facts the same way (`8 items ·
/// 2 failed`), so a literal only counts when most of its segments have that
/// chord shape.
fn hint_segments(literal: &str) -> Option<usize> {
    let body = literal.trim_matches('"');
    let segments: Vec<&str> = body.split(" · ").map(str::trim).collect();
    if segments.len() < 2 {
        return None;
    }
    let chord_like = segments
        .iter()
        .filter(|segment| {
            let mut parts = segment.split_whitespace();
            let (Some(chord), Some(verb)) = (parts.next(), parts.next()) else {
                return false;
            };
            chord.len() <= 4
                && !chord.contains('{')
                && verb.chars().next().is_some_and(char::is_alphabetic)
        })
        .count();
    (chord_like * 2 >= segments.len()).then_some(segments.len())
}

/// Whether this line sits inside a `StatusSlot::shortcut(...)` call.
///
/// The call wraps across lines once its literal is long, so the marker can be
/// a couple of lines above the string it introduces.
fn slot_shortcut_context(lines: &[(usize, String)], index: usize) -> bool {
    let start = index.saturating_sub(2);
    lines[start..=index]
        .iter()
        .any(|(_, line)| line.contains("StatusSlot::shortcut"))
}

#[test]
fn pattern_hint_copy_budget() {
    let mut over: Vec<String> = Vec::new();
    for source in painted_sources() {
        if !source.path.to_string_lossy().contains("patterns") {
            continue;
        }
        for (index, (number, line)) in source.lines.iter().enumerate() {
            // `StatusSlot::shortcut` is the status bar's shortcut channel, not
            // a footer row: it is contracted by priority and it carries the
            // keyboard-path parity contract for pointer actions, which
            // outranks the hint budget (law §4.2).
            if slot_shortcut_context(&source.lines, index) {
                continue;
            }
            for literal in string_literals(line) {
                if let Some(count) = hint_segments(&literal)
                    && count > HINT_COPY_BUDGET
                {
                    let name = source
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    over.push(format!("{name}:{number}: {count} chords in {literal}"));
                }
            }
        }
    }
    assert!(
        over.is_empty(),
        "footer hint rows over the {HINT_COPY_BUDGET}-chord budget:\n  {}",
        over.join("\n  ")
    );
}

// ── Information-budget gates (docs/design/web-premium-tui-law.md §4.2) ──────

use termrock::patterns::{
    AgentStatusHeader, AgentStatusHeaderState, AgentStatusPresentation, ConnectionManager,
    ConnectionManagerState, IntegrationStatus, IntegrationStatusPresentation,
    IntegrationStatusState, PlanReview, PlanReviewState, SessionPicker, SessionPickerState,
    example_agent_status, example_connections, example_integrations, example_plan_document,
    example_sessions,
};

/// Foreground colors that paint *content* in `buffer`, not single glyphs.
///
/// Hue count is the "too much information" proxy: a frame that speaks in nine
/// colors asks the eye to rank nine things at once. A color carrying a status
/// glyph is not that — one cell of meaning next to neutral text is exactly the
/// shape the design language asks for — so a hue has to cover a word before it
/// counts against the budget.
const GLYPH_CELL_ALLOWANCE: usize = 3;

fn content_foregrounds(buffer: &Buffer) -> Vec<ratatui_core::style::Color> {
    let mut counts: Vec<(ratatui_core::style::Color, usize)> = Vec::new();
    for cell in buffer.content() {
        if cell.symbol().trim().is_empty() {
            continue;
        }
        match counts.iter_mut().find(|(color, _)| *color == cell.fg) {
            Some((_, seen)) => *seen += 1,
            None => counts.push((cell.fg, 1)),
        }
    }
    counts
        .into_iter()
        .filter(|(_, seen)| *seen > GLYPH_CELL_ALLOWANCE)
        .map(|(color, _)| color)
        .collect()
}

/// Footer hint rows in a frame, and how many chords each advertises.
///
/// A hint row is recognisable by its joins — two or more meta separators that
/// are not a scrollbar track — and by where it sits: the footer band. Meta
/// separators higher up belong to content (a row's `project · time`), and two
/// panes side by side put two of those on one buffer row.
fn hint_rows(buffer: &Buffer, system: &DesignSystem) -> Vec<usize> {
    let separator = system.glyphs.meta_separator();
    let track_fg = system
        .style(Role::ScrollTrack)
        .fg
        .expect("scroll track carries a color");
    let footer_band = buffer.area.height.saturating_sub(3);
    (footer_band..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .filter(|x| {
                    let cell = &buffer[(buffer.area.x + x, buffer.area.y + y)];
                    cell.symbol() == separator && cell.fg != track_fg
                })
                .count()
        })
        .filter(|joins| *joins >= 2)
        .map(|joins| joins + 1)
        .collect()
}

/// The default frame of each priority pattern, painted with its own fixture.
fn priority_pattern_frames(system: &DesignSystem) -> Vec<(&'static str, Buffer)> {
    let area = Rect::new(0, 0, 72, 18);

    let mut agent = AgentStatusHeaderState::new();
    agent.snapshot = example_agent_status();
    agent.presentation = AgentStatusPresentation::Header;
    let agent_area = Rect::new(0, 0, 72, 3);

    let mut sessions = SessionPickerState::new();
    sessions.set_sessions(example_sessions());

    let mut connections = ConnectionManagerState::new();
    connections.set_connections(example_connections());

    let mut plan = PlanReviewState::new();
    plan.open(example_plan_document());

    let mut integrations = IntegrationStatusState::new();
    integrations.set_entries(example_integrations());
    integrations.presentation = IntegrationStatusPresentation::Panel;

    vec![
        (
            "agent_status_header",
            painted(agent_area, |buffer| {
                AgentStatusHeader::new(system).paint(agent_area, buffer, &mut agent);
            }),
        ),
        (
            "session_picker",
            painted(area, |buffer| {
                SessionPicker::new(system).paint(area, buffer, &mut sessions);
            }),
        ),
        (
            "connection_manager",
            painted(area, |buffer| {
                ConnectionManager::new(system).paint(area, buffer, &mut connections);
            }),
        ),
        (
            "plan_review",
            painted(area, |buffer| {
                PlanReview::new(system).paint(area, buffer, &mut plan);
            }),
        ),
        (
            "integration_status",
            painted(area, |buffer| {
                IntegrationStatus::new(system).paint(area, buffer, &mut integrations);
            }),
        ),
    ]
}

/// Hues a default frame may speak in before it is shouting.
const STYLE_DIVERSITY_BUDGET: usize = 8;

#[test]
fn pattern_style_diversity() {
    let system = DesignSystem::default();
    let mut over: Vec<String> = Vec::new();
    for (name, buffer) in priority_pattern_frames(&system) {
        let hues = content_foregrounds(&buffer);
        if hues.len() > STYLE_DIVERSITY_BUDGET {
            over.push(format!("{name}: {} hues {hues:?}", hues.len()));
        }
    }
    assert!(
        over.is_empty(),
        "default frames over the {STYLE_DIVERSITY_BUDGET}-hue budget:\n  {}",
        over.join("\n  ")
    );
}

/// Chords one footer row may advertise before it becomes a keymap dump.
const HINT_BUDGET: usize = 5;

#[test]
fn pattern_hint_budget() {
    let system = DesignSystem::default();
    let mut over: Vec<String> = Vec::new();
    for (name, buffer) in priority_pattern_frames(&system) {
        let rows = hint_rows(&buffer, &system);
        if rows.len() > 1 {
            over.push(format!("{name}: {} hint rows", rows.len()));
        }
        for hints in &rows {
            if *hints > HINT_BUDGET {
                over.push(format!("{name}: {hints} hints on one row"));
            }
        }
    }
    assert!(
        over.is_empty(),
        "footer hint budget exceeded:\n  {}",
        over.join("\n  ")
    );
}

// ── Geometry gates (plans/022 Step 2) ───────────────────────────────────────

/// Overlay widgets whose chrome must never let text touch a border glyph.
const BORDERED_OVERLAYS: &[&str] = &[
    "drawer.rs",
    "dropdown_menu.rs",
    "notification_center.rs",
    "preview_card.rs",
    "popover.rs",
    "menu_bar.rs",
    "fullscreen_viewer.rs",
    "image_surface.rs",
    "callout.rs",
];

#[test]
fn text_never_touches_borders() {
    use termrock::widgets::{Surface, SurfaceRecipe};

    // The contract holds at every width, including the narrow ones where
    // density padding used to collapse to zero.
    let system = DesignSystem::default();
    for recipe in [
        SurfaceRecipe::Overlay,
        SurfaceRecipe::OverlayFocused,
        SurfaceRecipe::Raised,
        SurfaceRecipe::Interactive,
    ] {
        for width in 3..40u16 {
            let area = Rect::new(0, 0, width, 5);
            let content = Surface::new(&system)
                .recipe(recipe)
                .bordered(true)
                .content_inset()
                .layout(area)
                .content;
            if content.width == 0 {
                continue;
            }
            assert!(
                content.x >= area.x + 2,
                "{recipe:?} at width {width}: content starts at {} — border at {} plus one",
                content.x,
                area.x
            );
            assert!(
                content.right() + 2 <= area.right(),
                "{recipe:?} at width {width}: content ends at {} against border {}",
                content.right(),
                area.right()
            );
        }
    }
}

#[test]
fn bordered_overlays_reserve_their_gutters() {
    // A `padding(0, 0)` on this family is how text ended up flush against the
    // border glyph; `content_inset()` is the sanctioned form.
    let mut flush: Vec<String> = Vec::new();
    for source in painted_sources() {
        let name = source
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if !BORDERED_OVERLAYS.contains(&name.as_str()) {
            continue;
        }
        for (number, line) in &source.lines {
            if line.contains("padding(0, 0)") {
                flush.push(format!("{name}:{number}"));
            }
        }
    }
    assert!(
        flush.is_empty(),
        "bordered overlays painting flush against their border: {flush:?}"
    );
}

// ── Scroll and truncation gates (plans/022 Step 6) ──────────────────────────

#[test]
fn one_scrollbar_language() {
    // Thumb and track roles belong to `scroll::render`. A widget that resolves
    // them itself is painting a second scrollbar language.
    let mut local: Vec<String> = Vec::new();
    for source in painted_sources() {
        let name = source
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        for (number, line) in &source.lines {
            if line.contains("Role::ScrollThumb") || line.contains("Role::ScrollTrack") {
                local.push(format!("{name}:{number}"));
            }
        }
    }
    assert!(
        local.is_empty(),
        "widgets resolving scrollbar roles instead of calling scroll::render: {local:?}"
    );
}

#[test]
fn truncation_has_ellipsis() {
    use ratatui_core::widgets::Widget;
    use termrock::widgets::{Panel, PanelVariant};

    let system = DesignSystem::default();
    let ellipsis = system.glyphs.ellipsis();
    // A title far wider than its chrome, in every panel variant.
    for variant in [
        PanelVariant::Bordered,
        PanelVariant::Quiet,
        PanelVariant::DividerOnly,
    ] {
        let area = Rect::new(0, 0, 24, 4);
        let buffer = painted(area, |buffer| {
            Widget::render(
                &Panel::new(&system)
                    .variant(variant)
                    .title("a title far wider than the chrome it was given"),
                area,
                buffer,
            );
        });
        let painted_text: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(
            painted_text.contains(ellipsis),
            "{variant:?} clipped its title with no ellipsis: {painted_text:?}"
        );
    }
}

/// Deterministic pseudo-random sizes: a fuzz that reproduces.
fn lcg(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    *seed >> 33
}

#[test]
fn flagship_widgets_survive_tiny_and_random_geometry() {
    use ratatui_core::widgets::{StatefulWidget, Widget};
    use termrock::widgets::{
        Panel, StatusBar, StatusBarState, StatusSlot, TextInput, TextInputState,
    };

    let system = DesignSystem::default();
    let mut seed = 0x5eed_1234_u64;
    for round in 0..200 {
        let (width, height) = if round < 4 {
            // The documented tiny terminal, and the degenerate cases around it.
            [(20u16, 5u16), (1, 1), (0, 4), (3, 2)][round]
        } else {
            (
                u16::try_from(lcg(&mut seed) % 60).unwrap_or(0),
                u16::try_from(lcg(&mut seed) % 20).unwrap_or(0),
            )
        };
        let area = Rect::new(0, 0, width, height);
        let _ = painted(area, |buffer| {
            Widget::render(
                &Panel::new(&system).title("panel").footer("esc close"),
                area,
                buffer,
            );
        });
        let _ = painted(area, |buffer| {
            let mut state = TextInputState::new("a value long enough to need contraction");
            let _ = TextInput::new("Label", &system).paint(area, buffer, &mut state);
        });
        let _ = painted(area, |buffer| {
            let slots = [
                StatusSlot::new("mode", "edit"),
                StatusSlot::new("branch", "main"),
            ];
            let mut state = StatusBarState::<&str>::new();
            StatefulWidget::render(
                &StatusBar::new(&slots, &[], &system),
                area,
                buffer,
                &mut state,
            );
        });
    }
}

/// Modal placement yields to the terminal instead of asserting a minimum.
///
/// The widget fuzz above never opens an overlay, which is how three patterns
/// shipped a `clamp(min, max)` that panics whenever the terminal is narrower
/// than the modal's own minimum: a 20-column pane took the whole host
/// application down the moment an agent asked for permission. The geometry now
/// has one authority ([`termrock::layout::modal_rect`]) and this gate walks it
/// across every degenerate size the law names (plans/019, migrations/0323).
#[test]
fn modal_geometry_never_escapes_its_terminal() {
    use termrock::layout::{ModalSpec, modal_rect};
    use termrock::patterns::{dialog_modal_rect, diff_modal_rect, permission_modal_rect};

    let specs = [
        ModalSpec::new(3, 4, 16).height(1, 3, 6),
        ModalSpec::new(3, 5, 28).height(1, 2, 8),
        ModalSpec::new(5, 6, 24).height(1, 3, 10),
        // A modal that wants more than the terminal has on both axes.
        ModalSpec::new(9, 1, 400).height(9, 1, 400),
    ];
    for width in 0..=64u16 {
        for height in 0..=24u16 {
            let area = Rect::new(3, 2, width, height);
            let mut rects = vec![
                permission_modal_rect(area),
                dialog_modal_rect(area),
                diff_modal_rect(area),
            ];
            rects.extend(specs.iter().map(|spec| modal_rect(area, *spec)));
            for rect in rects {
                assert!(
                    rect.x >= area.x
                        && rect.y >= area.y
                        && rect.right() <= area.right()
                        && rect.bottom() <= area.bottom(),
                    "modal {rect:?} escaped {area:?}"
                );
            }
        }
    }
}

/// Composed patterns keep painting with an overlay open at any size.
///
/// Frames, not helpers: the geometry gate above proves the rectangles are
/// sane, this one proves the surfaces that place children inside them survive
/// the same sizes.
#[test]
fn workbench_overlays_survive_tiny_and_random_geometry() {
    use termrock::patterns::{
        AgentWorkbenchState, WorkbenchSurfaces, default_modes, render_agent_workbench,
    };
    use termrock::widgets::{
        ListRow, PermissionPrompt, PermissionPromptState, PermissionRequest, PromptComposer,
        PromptComposerState, StatusBarState, StatusSlot, Transcript, TranscriptState,
    };

    let system = DesignSystem::default();
    let mut seed = 0xfeed_9876_u64;
    for round in 0..80 {
        let (width, height) = if round < 5 {
            [(20u16, 5u16), (1, 1), (0, 4), (3, 2), (44, 9)][round]
        } else {
            (
                u16::try_from(lcg(&mut seed) % 90).unwrap_or(0),
                u16::try_from(lcg(&mut seed) % 30).unwrap_or(0),
            )
        };
        let area = Rect::new(0, 0, width, height);

        let mut workbench = AgentWorkbenchState::new();
        let mut permission_state = PermissionPromptState::new();
        let _ = permission_state.enqueue(
            PermissionRequest::new("req-1", "shell", "repository")
                .command("rm -rf build")
                .expected("nothing runs until you decide"),
        );
        let permission = PermissionPrompt::new(&system);
        let mut prompt_state = PromptComposerState::new();
        let prompt = PromptComposer::new(&system);
        let mut transcript_state = TranscriptState::<&str>::new();
        let blocks = [];
        let transcript = Transcript::new(&blocks, &system);
        let mut status_state = StatusBarState::<&str>::new();
        let slots = [StatusSlot::mode("mode", "busy")];
        let modes = default_modes("build");
        let tasks: [ListRow<'_, &'static str>; 0] = [];

        let _ = painted(area, |buffer| {
            render_agent_workbench(
                buffer,
                area,
                WorkbenchSurfaces {
                    system: &system,
                    state: &mut workbench,
                    task_models: None,
                    tasks: &tasks,
                    modes: &modes,
                    transcript: &transcript,
                    transcript_state: &mut transcript_state,
                    activities: None,
                    prompt: &prompt,
                    prompt_state: &mut prompt_state,
                    status_slots: &slots,
                    status_state: &mut status_state,
                    permission: Some((&permission, &mut permission_state)),
                    question: None,
                    plan: None,
                    diff: None,
                    session: None,
                    working: None,
                },
            );
        });
    }
}

// ── Accent budget (plans/007 Step 7) ────────────────────────────────────────

/// Cells one flagship frame may paint in the reserved accent before the
/// screen stops having a single subject.
///
/// Measured after the plans/007 sweep: a 60×12 list with a selected row spends
/// one accent cell (its gutter mark) and a three-slot status bar spends none.
/// Eight leaves room for a focused control's chip without permitting a flood.
/// The number is the regression guard for design-language law 1.1: raising it
/// is a design decision, not a test fix.
const ACCENT_CELL_BUDGET: usize = 8;

/// Cells painted in the palette's reserved accent, fg or bg.
fn accent_cells(buffer: &Buffer, system: &DesignSystem) -> usize {
    let accent = system.style(Role::Accent).fg;
    buffer
        .content()
        .iter()
        .filter(|cell| {
            !cell.symbol().trim().is_empty() && (Some(cell.fg) == accent || Some(cell.bg) == accent)
        })
        .count()
}

#[test]
fn accent_budget() {
    use ratatui_core::text::Line;
    use ratatui_core::widgets::StatefulWidget;
    use termrock::widgets::{
        List, ListRow, ListState, RowRole, StatusBar, StatusBarState, StatusSlot,
    };

    let system = DesignSystem::default();
    let area = Rect::new(0, 0, 60, 12);
    let mut over: Vec<String> = Vec::new();

    let rows: Vec<ListRow<'static, usize>> = (0..8)
        .map(|id| ListRow {
            id,
            label: Line::from("a list row that says something"),
            leading: None,
            secondary: Some(Line::from("meta")),
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
        .collect();
    let mut list_state = ListState::new(Some(2));
    let list = painted(area, |buffer| {
        StatefulWidget::render(&List::new(&rows, &system), area, buffer, &mut list_state);
    });

    let slots = [
        StatusSlot::new("mode", "edit"),
        StatusSlot::new("branch", "main"),
        StatusSlot::new("sel", "3 selected"),
    ];
    let bar_area = Rect::new(0, 0, 60, 1);
    let mut bar_state = StatusBarState::<&str>::new();
    let bar = painted(bar_area, |buffer| {
        StatefulWidget::render(
            &StatusBar::new(&slots, &[], &system),
            bar_area,
            buffer,
            &mut bar_state,
        );
    });

    for (name, buffer) in [("list", list), ("status_bar", bar)] {
        let cells = accent_cells(&buffer, &system);
        if cells > ACCENT_CELL_BUDGET {
            over.push(format!("{name}: {cells} accent cells"));
        }
    }
    assert!(
        over.is_empty(),
        "frames over the {ACCENT_CELL_BUDGET}-cell accent budget:\n  {}",
        over.join("\n  ")
    );
}

/// Every field in the input family wears the same chrome.
///
/// TextInput is the substrate for six delegating widgets. Before
/// `input_recipe` had consumers, each of them resolved its own well, its own
/// value tone and its own focus cue — so "focused" looked different in six
/// places (plans/008 Step 7).
#[test]
fn inputs_share_field_chrome() {
    use termrock::{
        style::{DesignSystem, Role},
        widgets::{
            NumberInput, NumberInputState, PasswordInput, PasswordInputState, SearchInput,
            SearchInputState, TextInput, TextInputState, TokenField, TokenFieldState,
        },
    };

    let system = DesignSystem::phosphor();
    let area = Rect::new(0, 0, 24, 2);
    let well = system
        .style(Role::Sunken)
        .bg
        .expect("the field well carries a background");
    let cue = system
        .style(Role::BorderFocused)
        .fg
        .expect("the focus role carries a foreground");

    let mut frames: Vec<(&str, Buffer)> = Vec::new();

    let mut text = TextInputState::new("value");
    text.set_focused(true);
    frames.push((
        "TextInput",
        painted(area, |buffer| {
            TextInput::new("Name", &system).paint(area, buffer, &mut text);
        }),
    ));

    let mut number = NumberInputState::new();
    number.set_focused(true);
    frames.push((
        "NumberInput",
        painted(area, |buffer| {
            NumberInput::new("Count", &system).paint(area, buffer, &mut number);
        }),
    ));

    let mut search = SearchInputState::new();
    search.set_focused(true);
    frames.push((
        "SearchInput",
        painted(area, |buffer| {
            SearchInput::new(&system).paint(area, buffer, &mut search);
        }),
    ));

    let mut password = PasswordInputState::new();
    password.set_focused(true);
    frames.push((
        "PasswordInput",
        painted(area, |buffer| {
            PasswordInput::new("Secret", &system).paint(area, buffer, &mut password);
        }),
    ));

    let mut tokens = TokenFieldState::new();
    tokens.set_focused(true);
    frames.push((
        "TokenField",
        painted(area, |buffer| {
            TokenField::new(&system).paint(area, buffer, &mut tokens);
        }),
    ));

    for (name, buffer) in &frames {
        assert!(
            buffer.content().iter().any(|cell| cell.bg == well),
            "{name} does not paint the shared field well"
        );
        assert!(
            buffer.content().iter().any(|cell| cell.fg == cue),
            "{name} does not paint the shared focus cue"
        );
    }
}

/// Data rows read as tiers, not as one tone.
///
/// The ten flat data widgets each built a row as one `format!` and painted it
/// with one style, which makes the text ladder — primary, muted secondary,
/// faint meta — structurally unreachable. A row that states five facts must
/// paint them in more than one voice (plans/012).
#[test]
fn data_rows_have_ladder() {
    use termrock::widgets::{
        EventSeverity, EventStream, EventStreamState, LogLevel, LogLine, LogLineRecipe, LogStream,
        LogStreamState, StreamEvent, TraceSpan, TraceWaterfall, TraceWaterfallState,
    };

    let system = DesignSystem::default();
    let area = Rect::new(0, 0, 96, 12);
    let mut frames: Vec<(&'static str, Buffer)> = Vec::new();

    let log_lines = vec![
        LogLine::new("1", LogLevel::Info, "boot complete")
            .timestamp("12:00:00")
            .source("main"),
        LogLine::new("2", LogLevel::Error, "connection refused")
            .timestamp("12:00:01")
            .source("net"),
    ];
    let mut log_state = LogStreamState::new();
    log_state.set_following(false);
    log_state.recipe = LogLineRecipe::Detailed;
    frames.push((
        "LogStream",
        painted(area, |buffer| {
            LogStream::new(&log_lines, &system)
                .focused(true)
                .render(area, buffer, &mut log_state);
        }),
    ));

    let events: Vec<StreamEvent<'_, ()>> = vec![
        StreamEvent::with_id((), "Normal", "12:01:00", "Scheduled pod")
            .severity(EventSeverity::Info)
            .source("scheduler")
            .fields("pod=api-7 node=n1"),
        StreamEvent::with_id((), "Failed", "12:01:04", "Back-off restarting")
            .severity(EventSeverity::Error)
            .source("kubelet")
            .fields("pod=api-7"),
    ];
    let mut event_state = EventStreamState::new();
    event_state.set_following(false);
    frames.push((
        "EventStream",
        painted(area, |buffer| {
            EventStream::new(&events, &system)
                .focused(true)
                .render(area, buffer, &mut event_state);
        }),
    ));

    let spans = vec![
        TraceSpan::new("root", "HTTP GET /api", 0, 420)
            .service("gateway")
            .branch()
            .expanded(),
        TraceSpan::new("db", "SELECT users", 50, 180)
            .parent("root")
            .service("postgres")
            .depth(1),
    ];
    let mut trace_state = TraceWaterfallState::new();
    frames.push((
        "TraceWaterfall",
        painted(area, |buffer| {
            TraceWaterfall::new(&spans, &system).focused(true).render(
                area,
                buffer,
                &mut trace_state,
            );
        }),
    ));

    for (name, buffer) in &frames {
        let rows = data_row_tones(buffer);
        assert!(
            !rows.is_empty(),
            "{name} painted no data row for the gate to judge"
        );
        for (y, tones) in rows {
            assert!(
                tones >= 2,
                "{name} paints row {y} in {tones} tone(s); a row of several \
                 facts must not arrive as several equals"
            );
        }
    }
}

/// Distinct foregrounds for every buffer row carrying a row of data.
///
/// A data row is one with enough content to state more than one fact; a
/// header, a rule or a footer chip is not judged here.
fn data_row_tones(buffer: &Buffer) -> Vec<(u16, usize)> {
    (0..buffer.area.height)
        .filter_map(|y| {
            let mut seen: Vec<ratatui_core::style::Color> = Vec::new();
            let mut content = 0usize;
            for x in 0..buffer.area.width {
                let cell = &buffer[(buffer.area.x + x, buffer.area.y + y)];
                if cell.symbol().trim().is_empty() {
                    continue;
                }
                content += 1;
                if !seen.contains(&cell.fg) {
                    seen.push(cell.fg);
                }
            }
            (content >= 16).then_some((y, seen.len()))
        })
        .collect()
}

/// Examples compose; they do not paint.
///
/// An example that hand-rolls chrome teaches the wrong thing and leaves the
/// design language without an enforcement point. Single rows go through
/// `DesignSystem::paint_row`, which contracts honestly; everything else goes
/// through a widget (plans/016).
#[test]
fn patterns_only_compose() {
    let dir = crate_src().join("patterns");
    let mut offenders = Vec::new();
    for path in rust_files(&dir) {
        let body = fs::read_to_string(&path).expect("read pattern");
        let source: String = body
            .lines()
            .take_while(|l| !l.trim_start().starts_with("#[cfg(test)]"))
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for needle in ["set_stringn(", "cell_mut("] {
            if source.contains(needle) {
                offenders.push(format!("{}: {needle}", path.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "raw buffer paint in an example — compose a widget, or report the missing widget:\n{}",
        offenders.join("\n")
    );
}

/// Every example says what it teaches.
#[test]
fn patterns_have_charter_docs() {
    let dir = crate_src().join("patterns");
    let mut offenders = Vec::new();
    for path in rust_files(&dir) {
        if path.ends_with("mod.rs") {
            continue;
        }
        let body = fs::read_to_string(&path).expect("read pattern");
        if !body.contains("//! Teaches:") {
            offenders.push(path.display().to_string());
        }
    }
    assert!(
        offenders.is_empty(),
        "example without a `//! Teaches:` header — say what assembly it teaches:\n{}",
        offenders.join("\n")
    );
}

/// Building blocks never depend on examples (boundary law §6).
#[test]
fn widgets_never_import_patterns() {
    let dir = crate_src().join("widgets");
    let mut offenders = Vec::new();
    for path in rust_files(&dir) {
        let body = fs::read_to_string(&path).expect("read widget");
        for (i, line) in body.lines().enumerate() {
            let trimmed = line.trim_start();
            // Doc links to an example are fine; code depending on one is not.
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.contains("crate::patterns") || trimmed.contains("super::patterns") {
                offenders.push(format!("{}:{}: {trimmed}", path.display(), i + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a widget depends on an example; the dependency runs the other way:\n{}",
        offenders.join("\n")
    );
}

/// Examples do not invent selection chrome or bypass the hint vocabulary.
///
/// Full-row reversed slabs and `Role::Selection` washes were how patterns used
/// to say "this row is selected"; the row recipe says it now, and the reversed
/// form survives only as the colorless fallback (plans/010).
#[test]
fn patterns_compose_chrome() {
    let dir = crate_src().join("patterns");
    let mut offenders = Vec::new();
    for path in rust_files(&dir) {
        let body = fs::read_to_string(&path).expect("read pattern");
        let lines: Vec<(usize, &str)> = body
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l))
            .take_while(|(_, l)| !l.trim_start().starts_with("#[cfg(test)]"))
            .filter(|(_, l)| !l.trim_start().starts_with("//"))
            .collect();
        for (i, (line_no, line)) in lines.iter().enumerate() {
            if line.contains("Role::Selection") {
                offenders.push(format!(
                    "{}:{line_no}: Role::Selection — the row recipe owns selection fill",
                    path.display()
                ));
            }
            if !line.contains("Modifier::REVERSED") {
                continue;
            }
            // Colorless terminals keep the reversed cue: it is the only cue
            // they have. Look for the guard within the enclosing few lines.
            let window = lines
                .iter()
                .skip(i.saturating_sub(8))
                .take(9)
                .map(|(_, l)| *l)
                .collect::<Vec<_>>()
                .join("\n");
            if window.contains("colorless") {
                continue;
            }
            offenders.push(format!(
                "{}:{line_no}: reversed slab outside a colorless branch",
                path.display()
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "an example invented its own selection chrome:\n{}",
        offenders.join("\n")
    );
}

/// One chip family: no second bracket-paint body.
///
/// Tag, Chip, keycaps and token-field entries are the same token with
/// different brackets. When each grows its own painter they drift — different
/// bracket faintness, different focus cue, different remove affordance
/// (plans/015 Step 2).
#[test]
fn one_chip_recipe() {
    let src = crate_src();
    let mut offenders = Vec::new();
    for (file, allowance) in [("tag_chip.rs", 1usize), ("kbd.rs", 1usize)] {
        let body = fs::read_to_string(src.join("widgets").join(file)).expect("read widget");
        let paint_only: String = body
            .lines()
            .take_while(|l| !l.trim_start().starts_with("#[cfg(test)]"))
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        // A bracket-composing paint body is recognisable by the literal it
        // wraps its label in.
        let bodies = paint_only.matches("String::from(\"[\"").count()
            + paint_only.matches("format!(\"[{").count();
        if bodies > allowance {
            offenders.push(format!(
                "{file}: {bodies} bracket-composing bodies (allowed {allowance})"
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "a second chip/keycap painter appeared; route it through the shared one:\n{}",
        offenders.join("\n")
    );
}

/// At most one bold run per row.
///
/// The four-level text ladder collapses when several parts of a row shout at
/// once: the label may carry weight, its metadata never does (law P4).
#[test]
fn bold_budget_per_row() {
    use ratatui_core::style::Modifier;
    use termrock::style::{Density, DesignSystem, ListRowVisualState, RolePalette};

    for density in [Density::Compact, Density::Comfortable, Density::Dashboard] {
        let system = DesignSystem::new(RolePalette::default(), density);
        for selected in [false, true] {
            for focused in [false, true] {
                for hovered in [false, true] {
                    let recipe = system.resolve_list_row(ListRowVisualState {
                        selected,
                        focused,
                        hovered,
                        enabled: true,
                        loading: false,
                        checked: false,
                    });
                    let meta = [
                        ("secondary", recipe.secondary),
                        ("trailing", recipe.trailing),
                        ("shortcut", recipe.shortcut),
                    ];
                    for (name, style) in meta {
                        assert!(
                            !style.add_modifier.contains(Modifier::BOLD),
                            "{density:?} row (selected={selected}, focused={focused}, \
                             hovered={hovered}) paints {name} bold; weight belongs to the label"
                        );
                    }
                }
            }
        }
    }
}

/// Every state a control claims to have must look different.
///
/// The state-coverage audit found pressed painting nowhere and hover missing
/// from most of the library — which happened because nothing rendered the
/// states side by side and compared them. This does (plans/021 Step 5).
#[test]
fn state_matrix_distinct() {
    use ratatui_core::layout::Position;
    use termrock::input::{KeyReleaseReporting, MouseButton, MouseEvent, MouseEventKind};
    use termrock::style::{ControlState, DesignSystem, ListRowVisualState};
    use termrock::widgets::{Button, ButtonState, ButtonVariant};

    let system = DesignSystem::default();

    // Button: idle / hovered / pressed / disabled must be four frames.
    let area = Rect::new(0, 0, 16, 1);
    let mut frames: Vec<(&'static str, Buffer)> = Vec::new();

    let mut idle = ButtonState::new();
    idle.activation.set_accepts_input(true);
    frames.push((
        "idle",
        painted(area, |buffer| {
            Button::new("Run", &system)
                .variant(ButtonVariant::Primary)
                .paint(area, buffer, &mut idle);
        }),
    ));

    let mut hovered = ButtonState::new();
    hovered.activation.set_accepts_input(true);
    hovered.hovered = true;
    frames.push((
        "hovered",
        painted(area, |buffer| {
            Button::new("Run", &system)
                .variant(ButtonVariant::Primary)
                .paint(area, buffer, &mut hovered);
        }),
    ));

    let mut pressed = ButtonState::new();
    pressed.activation.set_accepts_input(true);
    pressed
        .activation
        .set_release_reporting(KeyReleaseReporting::Reported);
    // A press needs a painted hit region to land in.
    let _ = painted(area, |buffer| {
        Button::new("Run", &system)
            .variant(ButtonVariant::Primary)
            .paint(area, buffer, &mut pressed);
    });
    let armed = pressed.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        position: Position::new(area.x, area.y),
        modifiers: termrock::input::KeyModifiers::NONE,
    });
    assert!(
        pressed.activation.is_armed(),
        "the press must arm the button: {armed:?}"
    );
    frames.push((
        "pressed",
        painted(area, |buffer| {
            Button::new("Run", &system)
                .variant(ButtonVariant::Primary)
                .paint(area, buffer, &mut pressed);
        }),
    ));

    let mut disabled = ButtonState::new();
    disabled.activation.set_accepts_input(true);
    disabled.activation.set_enabled(false);
    frames.push((
        "disabled",
        painted(area, |buffer| {
            Button::new("Run", &system)
                .variant(ButtonVariant::Primary)
                .paint(area, buffer, &mut disabled);
        }),
    ));

    for (i, (name, frame)) in frames.iter().enumerate() {
        for (other_name, other) in frames.iter().skip(i + 1) {
            assert_ne!(
                frame.content(),
                other.content(),
                "Button paints {name} and {other_name} identically"
            );
        }
    }

    // Row recipe: idle / hovered / selected / disabled must differ too.
    let states = [
        (
            "idle",
            ListRowVisualState {
                selected: false,
                focused: false,
                hovered: false,
                enabled: true,
                loading: false,
                checked: false,
            },
        ),
        (
            "hovered",
            ListRowVisualState {
                selected: false,
                focused: false,
                hovered: true,
                enabled: true,
                loading: false,
                checked: false,
            },
        ),
        (
            "selected",
            ListRowVisualState {
                selected: true,
                focused: true,
                hovered: false,
                enabled: true,
                loading: false,
                checked: false,
            },
        ),
        (
            "disabled",
            ListRowVisualState {
                selected: false,
                focused: false,
                hovered: false,
                enabled: false,
                loading: false,
                checked: false,
            },
        ),
    ];
    let recipes: Vec<(&str, _)> = states
        .iter()
        .map(|(name, state)| (*name, system.resolve_list_row(*state)))
        .collect();
    for (i, (name, recipe)) in recipes.iter().enumerate() {
        for (other_name, other) in recipes.iter().skip(i + 1) {
            assert_ne!(
                (
                    recipe.label,
                    recipe.tint,
                    recipe.hover_wash,
                    recipe.show_actions
                ),
                (
                    other.label,
                    other.tint,
                    other.hover_wash,
                    other.show_actions
                ),
                "the row recipe resolves {name} and {other_name} identically"
            );
        }
    }

    // A control state that claims to be pressed must not resolve as hovered.
    let button_states = [
        ControlState::Default,
        ControlState::Hovered,
        ControlState::Pressed,
        ControlState::Focused,
        ControlState::Disabled,
    ];
    let resolved: Vec<_> = button_states
        .iter()
        .map(|state| system.button_recipe(Default::default(), *state))
        .collect();
    for (i, recipe) in resolved.iter().enumerate() {
        for (j, other) in resolved.iter().enumerate().skip(i + 1) {
            assert_ne!(
                (recipe.label, recipe.fill),
                (other.label, other.fill),
                "button recipe resolves {:?} and {:?} identically",
                button_states[i],
                button_states[j]
            );
        }
    }
}

/// Examples do not leave "(no rows)" where an empty state belongs.
///
/// A parenthetical string is a placeholder, not an empty state: it names
/// nothing, offers nothing, and reads as debug output. Every one of them is an
/// `EmptyState` now (plans/013 Step 4).
#[test]
fn patterns_have_real_empty_states() {
    let mut offenders = Vec::new();
    for dir in ["patterns", "widgets"] {
        for path in rust_files(&crate_src().join(dir)) {
            let body = fs::read_to_string(&path).expect("read source");
            for (i, line) in body
                .lines()
                .take_while(|l| !l.trim_start().starts_with("#[cfg(test)]"))
                .enumerate()
            {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") {
                    continue;
                }
                for literal in string_literals(line) {
                    if literal.starts_with("(no ") || literal.starts_with("(select ") {
                        offenders.push(format!("{}:{}: {literal}", path.display(), i + 1));
                    }
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "placeholder copy where an EmptyState belongs:\n{}",
        offenders.join("\n")
    );
}

/// Wide emoji never sit in a one-column slot.
///
/// A two-column glyph in a one-column gutter shifts every column to its right
/// by one cell, on every row that has it (plans/013 Step 2).
#[test]
fn no_wide_emoji_in_chrome() {
    // Emoji-presentation characters — the block that terminals render two
    // columns wide by default. `⚙` and friends below U+1F000 default to text
    // presentation and stay one column, so they are not the problem here.
    fn is_emoji_presentation(c: char) -> bool {
        matches!(c as u32, 0x1F000..=0x1FAFF)
    }

    let mut offenders = Vec::new();
    for source in painted_sources() {
        if source.path.ends_with("tests.rs") {
            continue;
        }
        for (i, (line_no, line)) in source.lines.iter().enumerate() {
            if source.payload[i] {
                continue;
            }
            for literal in string_literals(line) {
                if literal.chars().any(is_emoji_presentation) {
                    offenders.push(format!("{}:{line_no}: {literal}", source.path.display()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "wide emoji in painted chrome — use a one-column catalog glyph:\n{}",
        offenders.join("\n")
    );
}
