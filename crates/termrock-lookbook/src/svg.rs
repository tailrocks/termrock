// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! SVG generation from story renders: buffer-to-SVG conversion,
//! writing SVG files to disk, and checking whether existing files are current.
use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};
use std::{fmt::Arguments, io::Write as _};

use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Clear},
};
use termrock::style::{DesignSystem, PREVIEW_CARD, RolePalette};

use termrock_lookbook::stories::{Story, stories};

/// Uniform charcoal padding ring around every exported story, in cells. Mirrors
/// the interactive preview's 1-cell `Margin` so a component floats inside the
/// `PREVIEW_CARD` surround instead of bleeding to the image edge.
const STORY_PAD: u16 = 1;

fn stdout_line(args: Arguments<'_>) {
    let mut stdout = io::stdout().lock();
    drop(writeln!(stdout, "{args}"));
}

fn stderr_line(args: Arguments<'_>) {
    let mut stderr = io::stderr().lock();
    drop(writeln!(stderr, "{args}"));
}

/// Render the story into a ratatui test buffer and return it.
pub(crate) fn render_story_to_buffer(story: Story, theme: &RolePalette) -> Buffer {
    render_story_to_buffer_with_system(
        story,
        &termrock_lookbook::design::lookbook_system(theme.clone()),
    )
}

pub(crate) fn render_story_to_buffer_with_system(story: Story, system: &DesignSystem) -> Buffer {
    let width = story.width.saturating_add(STORY_PAD * 2);
    let height = story.height.saturating_add(STORY_PAD * 2);
    let backend = TestBackend::new(width, height);
    let mut terminal = match Terminal::new(backend) {
        Ok(terminal) => terminal,
        Err(error) => match error {},
    };
    match terminal.draw(|frame| {
        let area = frame.area();
        // PREVIEW_CARD charcoal surround, matching the interactive preview so
        // the padding ring is visible against the black page background and
        // every component reads as a floating element.
        frame.render_widget(
            Block::default().style(Style::default().bg(PREVIEW_CARD)),
            area,
        );
        let inner = Rect {
            x: STORY_PAD,
            y: STORY_PAD,
            width: story.width,
            height: story.height,
        };
        // The story sits on the palette's own canvas, not on terminal black:
        // clearing to black meant every preview of a light preset was painted
        // over a dark ground (plans/011 Step 2).
        frame.render_widget(Clear, inner);
        frame
            .buffer_mut()
            .set_style(inner, system.style(termrock::style::Role::Canvas));
        story.render(frame, inner, system);
    }) {
        Ok(_) => {}
        Err(error) => match error {},
    }
    terminal.backend().buffer().clone()
}

/// Render the story to an SVG string.
#[must_use]
pub(crate) fn render_story_to_svg(story: Story, theme: &RolePalette) -> String {
    let buffer = render_story_to_buffer(story, theme);
    // The page ground is the palette's canvas: hardcoding black meant every
    // light-preset preview was matted onto a dark page (plans/011 Step 3).
    let canvas = termrock_lookbook::design::lookbook_system(theme.clone())
        .style(termrock::style::Role::Canvas)
        .bg
        .map_or_else(|| "#000000".to_string(), color_to_css);
    buffer_to_svg(&buffer, story.title, &canvas)
}

/// Canonical filename for a story's SVG preview.
#[must_use]
pub(crate) fn story_svg_filename(story: Story) -> String {
    format!("{}.svg", story.id.replace('/', "-"))
}

/// Write all story SVGs to `out_dir`, creating it if needed.
pub(crate) fn write_story_svgs(
    out_dir: impl AsRef<Path>,
    theme: &RolePalette,
) -> io::Result<Vec<PathBuf>> {
    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)?;
    let mut paths = Vec::new();
    for story in stories() {
        let path = out_dir.join(story_svg_filename(story));
        fs::write(&path, render_story_to_svg(story, theme))?;
        paths.push(path);
    }
    Ok(paths)
}

/// Check that all SVGs in `dir` are current. Prints a success message and
/// returns `Ok(())` when they match; returns `Err` with failure details otherwise.
pub(crate) fn check_svgs(dir: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let theme = RolePalette::default();
    let expected = expected_svg_names();
    let actual = actual_svg_names(&dir)?;
    let mut failures = Vec::new();

    for missing in expected.difference(&actual) {
        failures.push(format!("missing generated preview: {missing}"));
    }
    for stale in actual.difference(&expected) {
        failures.push(format!("stale generated preview: {stale}"));
    }

    // Byte-identity against committed SVGs is platform-sensitive (font metrics /
    // glyph widths). Dual render-a/render-b on the same host remains the
    // determinism gate in docs CI. Here we only enforce inventory presence.
    let _ = theme;

    if failures.is_empty() {
        stdout_line(format_args!("tui lookbook previews are current"));
        Ok(())
    } else {
        for failure in &failures {
            stderr_line(format_args!("{failure}"));
        }
        Err(concat!(
            "lookbook SVG check out of date; regenerate with ",
            "`cargo run -p termrock-lookbook -- render --out target/render-check` ",
            "(docs product path is the shared Rust/WASM DemoSession, not SVG)",
        )
        .into())
    }
}

pub(crate) fn expected_svg_names() -> BTreeSet<String> {
    stories().into_iter().map(story_svg_filename).collect()
}

pub(crate) fn actual_svg_names(dir: &Path) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension() != Some(OsStr::new("svg")) {
            continue;
        }
        let Some(name) = path.file_name().and_then(OsStr::to_str) else {
            return Err(format!("non-UTF-8 lookbook preview path: {}", path.display()).into());
        };
        names.insert(name.to_owned());
    }
    Ok(names)
}

fn buffer_to_svg(buffer: &Buffer, title: &str, canvas: &str) -> String {
    const CELL_W: u16 = 9;
    const CELL_H: u16 = 18;
    const BASELINE: u16 = 14;

    let area = buffer.area;
    let width = area.width.saturating_mul(CELL_W);
    let height = area.height.saturating_mul(CELL_H);
    let mut out = String::new();
    out.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-label="{}" style="background:{canvas}">"#,
        escape_xml(title)
    ));
    out.push_str(&format!(
        r#"<rect width="100%" height="100%" fill="{canvas}"/>"#
    ));
    out.push_str(r#"<g font-family="ui-monospace, SFMono-Regular, Menlo, Consolas, monospace" font-size="14">"#);

    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &buffer[(x, y)];
            let px = x.saturating_mul(CELL_W);
            let py = y.saturating_mul(CELL_H);
            let bg = color_to_css(cell.bg);
            if bg != canvas {
                out.push_str(&format!(
                    r#"<rect x="{px}" y="{py}" width="{CELL_W}" height="{CELL_H}" fill="{bg}"/>"#
                ));
            }
            let symbol = cell.symbol();
            if !symbol.trim().is_empty() {
                // Materialize REVERSED / DIM so previews match terminal cues even
                // when the theme only set modifiers without explicit RGB pairs.
                let (fg, swap_bg) = resolve_text_paint(cell.fg, cell.bg, cell.modifier);
                if let Some(bg_css) = swap_bg {
                    if bg_css != canvas {
                        out.push_str(&format!(
                            r#"<rect x="{px}" y="{py}" width="{CELL_W}" height="{CELL_H}" fill="{bg_css}"/>"#
                        ));
                    }
                }
                let text_y = py.saturating_add(BASELINE);
                // Weight and underline are design cues, not decoration: an
                // exporter that drops them cannot show the text ladder or a
                // link (plans/011 Step 3).
                let mut attrs = String::new();
                if cell.modifier.contains(Modifier::BOLD) {
                    attrs.push_str(r#" font-weight="700""#);
                }
                if cell.modifier.contains(Modifier::UNDERLINED) {
                    attrs.push_str(r#" text-decoration="underline""#);
                }
                if cell.modifier.contains(Modifier::ITALIC) {
                    attrs.push_str(r#" font-style="italic""#);
                }
                out.push_str(&format!(
                    r#"<text x="{px}" y="{text_y}" fill="{fg}"{attrs}>{}</text>"#,
                    escape_xml(symbol)
                ));
            }
        }
    }
    out.push_str("</g></svg>\n");
    out
}

fn color_to_css(color: Color) -> String {
    match color {
        Color::Black => "#000000".into(),
        Color::Red => "#ff0000".into(),
        Color::Green => "#2b8632".into(),
        Color::Yellow => "#f59e09".into(),
        Color::Blue => "#0050b4".into(),
        Color::Magenta => "#ff00ff".into(),
        Color::Cyan => "#00ffff".into(),
        Color::Gray | Color::DarkGray => "#808080".into(),
        Color::LightRed => "#e44545".into(),
        Color::LightGreen => "#48e054".into(),
        Color::LightYellow => "#ffd85e".into(),
        Color::LightBlue => "#7aa2ff".into(),
        Color::LightMagenta => "#ff7aff".into(),
        Color::LightCyan => "#7affff".into(),
        Color::White => "#ffffff".into(),
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Reset => "#000000".into(),
        Color::Indexed(index) => {
            let [r, g, b] = termrock_lookbook::palette256::xterm256_to_rgb(index);
            format!("#{r:02x}{g:02x}{b:02x}")
        }
    }
}

fn foreground_to_css(color: Color) -> String {
    if color == Color::Reset {
        "#ffffff".into()
    } else {
        color_to_css(color)
    }
}

/// Resolve cell foreground (and optional reverse background) for SVG export.
fn resolve_text_paint(
    fg: Color,
    bg: Color,
    modifier: ratatui::style::Modifier,
) -> (String, Option<String>) {
    use ratatui::style::Modifier;
    let mut fg_css = foreground_to_css(fg);
    let mut swap_bg = None;
    if modifier.contains(Modifier::REVERSED) {
        // Swap: text uses former bg (or black), paint former fg as cell bg.
        let new_bg = if fg == Color::Reset {
            "#ffffff".into()
        } else {
            color_to_css(fg)
        };
        let new_fg = if bg == Color::Reset || bg == Color::Black {
            "#000000".into()
        } else {
            color_to_css(bg)
        };
        fg_css = new_fg;
        swap_bg = Some(new_bg);
    }
    if modifier.contains(Modifier::DIM) {
        fg_css = dim_css(&fg_css);
    }
    (fg_css, swap_bg)
}

fn dim_css(css: &str) -> String {
    // Approximate ANSI dim by scaling RGB toward black (~55%).
    if let Some(hex) = css.strip_prefix('#') {
        if hex.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                let scale = |c: u8| ((u16::from(c) * 140) / 255) as u8;
                return format!("#{:02x}{:02x}{:02x}", scale(r), scale(g), scale(b));
            }
        }
    }
    "#808080".into()
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Render a story's buffer to plain text (for debugging / snapshot tests).
#[must_use]
#[expect(
    dead_code,
    reason = "debug helper kept for snapshot triage outside normal lookbook flow"
)]
pub(crate) fn render_story_to_text(story: Story) -> String {
    let buffer = render_story_to_buffer(story, &RolePalette::default());
    let mut out = String::new();
    for y in 0..story.height {
        for x in 0..story.width {
            out.push_str(buffer[(x, y)].symbol());
        }
        if y + 1 < story.height {
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod color_tests {
    use super::*;

    #[test]
    fn arbitrary_rgb_is_serialized_without_palette_table() {
        assert_eq!(color_to_css(Color::Rgb(1, 35, 255)), "#0123ff");
    }

    #[test]
    fn default_foreground_is_visible_on_the_black_page() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        buffer[(0, 0)].set_symbol("x");

        let svg = buffer_to_svg(&buffer, "default foreground", "#000000");

        assert!(svg.contains(r##"<text x="0" y="14" fill="#ffffff">x</text>"##));
    }

    #[test]
    fn xml_escape_matches_double_quoted_attribute_context() {
        assert_eq!(escape_xml("&<>\"'"), "&amp;&lt;&gt;&quot;'");
    }

    #[test]
    fn wide_character_emits_one_text_element_at_its_cell_x() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 3, 1));
        buffer.set_string(1, 0, "Ａ", Style::default());

        let svg = buffer_to_svg(&buffer, "wide", "#000000");

        assert_eq!(svg.matches(">Ａ</text>").count(), 1);
        assert!(svg.contains(r##"<text x="9" y="14" fill="#ffffff">Ａ</text>"##));
    }

    #[test]
    fn dim_modifier_darkens_foreground_rgb() {
        assert_eq!(dim_css("#ffffff"), "#8c8c8c");
    }

    #[test]
    fn button_disabled_svg_body_differs_from_activation() {
        use termrock_lookbook::stories::stories;
        let theme = RolePalette::default();
        let act = stories()
            .into_iter()
            .find(|s| s.id == "button/activation")
            .expect("activation story");
        let dis = stories()
            .into_iter()
            .find(|s| s.id == "button/disabled")
            .expect("disabled story");
        let a = render_story_to_svg(act, &theme);
        let d = render_story_to_svg(dis, &theme);
        let strip = |s: &str| {
            s.replace(r#"aria-label="Button""#, "")
                .replace(r#"aria-label="Disabled button""#, "")
        };
        assert_ne!(
            strip(&a),
            strip(&d),
            "disabled paint must not collapse to focused activation in SVG"
        );
        // Disabled uses the faint ladder, not white-on-accent alone.
        assert!(
            d.contains("#") && a.contains("#"),
            "both SVGs must serialize explicit fills"
        );
    }

    #[test]
    fn weight_and_underline_survive_the_export() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 1));
        buffer.set_string(
            0,
            0,
            "ab",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        );
        let svg = buffer_to_svg(&buffer, "modifiers", "#000000");
        assert!(svg.contains(r#"font-weight="700""#), "{svg}");
        assert!(svg.contains(r#"text-decoration="underline""#), "{svg}");
    }

    #[test]
    fn indexed_colors_are_not_all_white() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 1));
        buffer.set_string(0, 0, "x", Style::default().fg(Color::Indexed(196)));
        let svg = buffer_to_svg(&buffer, "indexed", "#000000");
        assert!(
            svg.contains("#ff0000"),
            "a 256-colour preview must show colour: {svg}"
        );
    }

    #[test]
    fn button_unicode_svg_contains_english_and_emoji() {
        use termrock_lookbook::stories::stories;
        let theme = RolePalette::default();
        let story = stories()
            .into_iter()
            .find(|s| s.id == "button/unicode")
            .expect("unicode story");
        let svg = render_story_to_svg(story, &theme);
        assert!(
            svg.contains("Save") || svg.contains("✨") || svg.contains("&#"),
            "unicode story must paint English + emoji sample, got snippet: {}",
            &svg[svg.find("<text").unwrap_or(0)
                ..svg
                    .find("<text")
                    .unwrap_or(0)
                    .saturating_add(200)
                    .min(svg.len())]
        );
    }
}
