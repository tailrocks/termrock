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
    style::{Color, Style},
    widgets::{Block, Clear},
};
use termrock::{Theme, style::PREVIEW_CARD};

use crate::stories::{Story, stories};

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
pub(crate) fn render_story_to_buffer(story: Story, theme: &Theme) -> Buffer {
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
        // Clear the component area to the terminal default (black) so the story
        // renders on the same surface as the real app, with PREVIEW_CARD only
        // as the surround — identical to the interactive preview.
        frame.render_widget(Clear, inner);
        story.render(frame, inner, theme);
    }) {
        Ok(_) => {}
        Err(error) => match error {},
    }
    terminal.backend().buffer().clone()
}

/// Render the story to an SVG string.
#[must_use]
pub(crate) fn render_story_to_svg(story: Story, theme: &Theme) -> String {
    let buffer = render_story_to_buffer(story, theme);
    buffer_to_svg(&buffer, story.title)
}

/// Canonical filename for a story's SVG preview.
#[must_use]
pub(crate) fn story_svg_filename(story: Story) -> String {
    format!("{}.svg", story.id.replace('/', "-"))
}

/// Write all story SVGs to `out_dir`, creating it if needed.
pub(crate) fn write_story_svgs(
    out_dir: impl AsRef<Path>,
    theme: &Theme,
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
    let theme = Theme::default();
    let expected = expected_svg_names();
    let actual = actual_svg_names(&dir)?;
    let mut failures = Vec::new();

    for missing in expected.difference(&actual) {
        failures.push(format!("missing generated preview: {missing}"));
    }
    for stale in actual.difference(&expected) {
        failures.push(format!("stale generated preview: {stale}"));
    }

    for story in stories() {
        let filename = story_svg_filename(story);
        let path = dir.join(&filename);
        if !path.exists() {
            continue;
        }
        let committed = fs::read_to_string(&path)?;
        let rendered = render_story_to_svg(story, &theme);
        if committed != rendered {
            failures.push(format!("generated preview is stale: {}", path.display()));
        }
    }

    if failures.is_empty() {
        stdout_line(format_args!("tui lookbook previews are current"));
        Ok(())
    } else {
        for failure in &failures {
            stderr_line(format_args!("{failure}"));
        }
        Err(concat!(
            "tui lookbook previews are out of date; regenerate with ",
            "`cargo run -p termrock-lookbook -- render --out docs/public/component-previews`",
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

fn buffer_to_svg(buffer: &Buffer, title: &str) -> String {
    const CELL_W: u16 = 9;
    const CELL_H: u16 = 18;
    const BASELINE: u16 = 14;

    let area = buffer.area;
    let width = area.width.saturating_mul(CELL_W);
    let height = area.height.saturating_mul(CELL_H);
    let mut out = String::new();
    out.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-label="{}" style="background:#000000">"#,
        escape_xml(title)
    ));
    out.push_str(r##"<rect width="100%" height="100%" fill="#000000"/>"##);
    out.push_str(r#"<g font-family="ui-monospace, SFMono-Regular, Menlo, Consolas, monospace" font-size="14">"#);

    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &buffer[(x, y)];
            let px = x.saturating_mul(CELL_W);
            let py = y.saturating_mul(CELL_H);
            let bg = color_to_css(cell.bg);
            if bg != "#000000" {
                out.push_str(&format!(
                    r#"<rect x="{px}" y="{py}" width="{CELL_W}" height="{CELL_H}" fill="{bg}"/>"#
                ));
            }
            let symbol = cell.symbol();
            if !symbol.trim().is_empty() {
                let fg = foreground_to_css(cell.fg);
                let text_y = py.saturating_add(BASELINE);
                out.push_str(&format!(
                    r#"<text x="{px}" y="{text_y}" fill="{fg}">{}</text>"#,
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
        Color::Green => "#00ff41".into(),
        Color::Yellow => "#ffd85e".into(),
        Color::Blue => "#0050b4".into(),
        Color::Magenta => "#ff00ff".into(),
        Color::Cyan => "#00ffff".into(),
        Color::Gray | Color::DarkGray => "#808080".into(),
        Color::LightRed => "#ff5e7a".into(),
        Color::LightGreen => "#00ff41".into(),
        Color::LightYellow => "#ffd85e".into(),
        Color::LightBlue => "#7aa2ff".into(),
        Color::LightMagenta => "#ff7aff".into(),
        Color::LightCyan => "#7affff".into(),
        Color::White => "#ffffff".into(),
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Reset => "#000000".into(),
        Color::Indexed(_) => "#ffffff".into(),
    }
}

fn foreground_to_css(color: Color) -> String {
    if color == Color::Reset {
        "#ffffff".into()
    } else {
        color_to_css(color)
    }
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
    let buffer = render_story_to_buffer(story, &Theme::default());
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

        let svg = buffer_to_svg(&buffer, "default foreground");

        assert!(svg.contains(r##"<text x="0" y="14" fill="#ffffff">x</text>"##));
    }

    #[test]
    fn xml_escape_matches_double_quoted_attribute_context() {
        assert_eq!(escape_xml("&<>\"'"), "&amp;&lt;&gt;&quot;'");
    }

    #[test]
    fn wide_character_emits_one_text_element_at_its_cell_x() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 3, 1));
        buffer.set_string(1, 0, "日", Style::default());

        let svg = buffer_to_svg(&buffer, "wide");

        assert_eq!(svg.matches(">日</text>").count(), 1);
        assert!(svg.contains(r##"<text x="9" y="14" fill="#ffffff">日</text>"##));
    }
}
