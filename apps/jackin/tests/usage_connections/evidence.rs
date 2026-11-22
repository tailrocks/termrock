use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use std::path::Path;
use tui_test::profile::{Colors, Rgb};
use tui_test::terminal::cell::Attrs;
use tui_test::{
    SnapshotCell, SnapshotColor, SnapshotCursor, SnapshotCursorShape, SnapshotMetadata,
    SnapshotParts, SnapshotScope, SnapshotUnderline, TerminalSnapshot,
};

fn color(color: Color) -> SnapshotColor {
    match color {
        Color::Reset => SnapshotColor::Default,
        Color::Rgb(r, g, b) => SnapshotColor::Rgb(r, g, b),
        Color::Indexed(index) => SnapshotColor::Indexed(index),
        Color::Black => SnapshotColor::Indexed(0),
        Color::Red => SnapshotColor::Indexed(1),
        Color::Green => SnapshotColor::Indexed(2),
        Color::Yellow => SnapshotColor::Indexed(3),
        Color::Blue => SnapshotColor::Indexed(4),
        Color::Magenta => SnapshotColor::Indexed(5),
        Color::Cyan => SnapshotColor::Indexed(6),
        Color::Gray => SnapshotColor::Indexed(7),
        Color::DarkGray => SnapshotColor::Indexed(8),
        Color::LightRed => SnapshotColor::Indexed(9),
        Color::LightGreen => SnapshotColor::Indexed(10),
        Color::LightYellow => SnapshotColor::Indexed(11),
        Color::LightBlue => SnapshotColor::Indexed(12),
        Color::LightMagenta => SnapshotColor::Indexed(13),
        Color::LightCyan => SnapshotColor::Indexed(14),
        Color::White => SnapshotColor::Indexed(15),
    }
}

pub(super) fn save(buffer: &Buffer, path: &Path, name: &str) {
    let mut cells = vec![];
    let mut continuation = 0;
    for cell in buffer.content() {
        let mut attributes = Attrs::empty();
        for (modifier, attribute) in [
            (Modifier::BOLD, Attrs::BOLD),
            (Modifier::DIM, Attrs::DIM),
            (Modifier::ITALIC, Attrs::ITALIC),
            (Modifier::REVERSED, Attrs::INVERSE),
            (Modifier::HIDDEN, Attrs::INVISIBLE),
            (Modifier::CROSSED_OUT, Attrs::STRIKE),
        ] {
            if cell.modifier.contains(modifier) {
                attributes.insert(attribute);
            }
        }
        let (text, width) = if continuation > 0 {
            continuation -= 1;
            (String::new(), 0)
        } else {
            let width = junie_tui::ui::text::width(cell.symbol()) as u16;
            continuation = width.saturating_sub(1);
            (cell.symbol().into(), width)
        };
        cells.push(
            SnapshotCell::new(
                text,
                width,
                color(cell.fg),
                color(cell.bg),
                attributes,
                if cell.modifier.contains(Modifier::UNDERLINED) {
                    SnapshotUnderline::Single
                } else {
                    SnapshotUnderline::None
                },
                color(cell.underline_color),
                None,
                None,
            )
            .unwrap(),
        );
    }
    let colors = Colors::default();
    let snapshot = TerminalSnapshot::try_new(SnapshotParts {
        cols: buffer.area.width.into(),
        rows: buffer.area.height.into(),
        scope: SnapshotScope::ActiveScreen,
        viewport_origin: 0,
        cells,
        palette: std::array::from_fn(|index| colors.rgb(index as u8)),
        default_foreground: colors.foreground,
        default_background: colors.background,
        default_cursor: colors.cursor,
        cursor: SnapshotCursor::new(
            0,
            0,
            false,
            false,
            SnapshotCursorShape::Block,
            Rgb::new(255, 255, 255),
            None,
        ),
        title: None,
        metadata: SnapshotMetadata::default(),
    })
    .unwrap();
    let sample = tuiscotti_latest::sample::Sample::state(
        &format!("usage_flow/{name}"),
        snapshot,
        tuiscotti_latest::sample::Observation::empty(),
    )
    .unwrap()
    .to_bytes()
    .unwrap();
    std::fs::write(path.with_extension("tui.pb"), &sample).unwrap();
    tuiscotti_latest::render::render_sample(&sample, path).unwrap();
}
