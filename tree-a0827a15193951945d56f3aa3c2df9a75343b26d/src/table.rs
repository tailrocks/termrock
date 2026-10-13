//! Lookbook-local column-layout prototype for Plan 033.

use std::num::NonZeroU16;

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};
use termrock::{
    Theme,
    style::Role,
    text::{display_cols, display_cols_slice},
};

use crate::stories::Story;

const COLUMN_GAP: u16 = 2;
const ROW_MARKER_WIDTH: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColumnWidth {
    Fixed(u16),
    Min(u16),
    Fill(NonZeroU16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CellAlignment {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy)]
struct PrototypeColumn {
    title: &'static str,
    width: ColumnWidth,
    alignment: CellAlignment,
    sort: Option<SortDirection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedLayout {
    widths: Vec<u16>,
    visible: Vec<usize>,
}

/// Resolves the cell budget only; callers subtract inter-column gaps first.
pub(crate) fn resolve_widths(columns: &[ColumnWidth], available: u16) -> Vec<u16> {
    let mut widths = columns
        .iter()
        .map(|column| match column {
            ColumnWidth::Fixed(width) | ColumnWidth::Min(width) => *width,
            ColumnWidth::Fill(_) => 0,
        })
        .collect::<Vec<_>>();
    let mandatory = widths
        .iter()
        .fold(0_u64, |total, width| total + u64::from(*width));

    if mandatory > u64::from(available) {
        let mut deficit = mandatory - u64::from(available);
        shrink_right_to_left(columns, &mut widths, &mut deficit, false);
        shrink_right_to_left(columns, &mut widths, &mut deficit, true);
        return widths;
    }

    let remainder = u64::from(available) - mandatory;
    let total_weight = columns.iter().fold(0_u64, |total, column| {
        total
            + match column {
                ColumnWidth::Fill(weight) => u64::from(weight.get()),
                ColumnWidth::Fixed(_) | ColumnWidth::Min(_) => 0,
            }
    });
    if remainder == 0 || total_weight == 0 {
        return widths;
    }

    let mut distributed = 0_u64;
    for (index, column) in columns.iter().enumerate() {
        if let ColumnWidth::Fill(weight) = column {
            let share = remainder * u64::from(weight.get()) / total_weight;
            widths[index] = u16::try_from(share).unwrap_or(u16::MAX);
            distributed += share;
        }
    }
    let mut leftover = remainder - distributed;
    for (index, column) in columns.iter().enumerate() {
        if leftover == 0 {
            break;
        }
        if matches!(column, ColumnWidth::Fill(_)) {
            widths[index] = widths[index].saturating_add(1);
            leftover -= 1;
        }
    }
    widths
}

fn resolve_layout(columns: &[ColumnWidth], available: u16, gap: u16) -> ResolvedLayout {
    let mut visible = columns
        .iter()
        .enumerate()
        .filter_map(|(index, width)| can_paint(*width).then_some(index))
        .collect::<Vec<_>>();
    let mut widths = vec![0; columns.len()];
    if visible.is_empty() || available == 0 {
        visible.clear();
        return ResolvedLayout { widths, visible };
    }

    let initial = resolve_visible(columns, &visible, available, gap);
    let survivors = visible
        .iter()
        .zip(initial)
        .filter_map(|(index, width)| (width > 0).then_some(*index))
        .collect::<Vec<_>>();
    if survivors.is_empty() {
        visible.truncate(1);
    } else {
        visible = survivors;
    }
    let resolved = resolve_visible(columns, &visible, available, gap);
    debug_assert!(resolved.iter().all(|width| *width > 0));
    for (index, width) in visible.iter().zip(resolved) {
        widths[*index] = width;
    }
    ResolvedLayout { widths, visible }
}

fn resolve_visible(
    columns: &[ColumnWidth],
    visible: &[usize],
    available: u16,
    gap: u16,
) -> Vec<u16> {
    let gap_count = u16::try_from(visible.len().saturating_sub(1)).unwrap_or(u16::MAX);
    let gap_budget = gap.saturating_mul(gap_count);
    let policies = visible
        .iter()
        .map(|index| columns[*index])
        .collect::<Vec<_>>();
    resolve_widths(&policies, available.saturating_sub(gap_budget))
}

const fn can_paint(width: ColumnWidth) -> bool {
    match width {
        ColumnWidth::Fixed(width) | ColumnWidth::Min(width) => width > 0,
        ColumnWidth::Fill(_) => true,
    }
}

fn shrink_right_to_left(
    columns: &[ColumnWidth],
    widths: &mut [u16],
    deficit: &mut u64,
    fixed: bool,
) {
    for (index, column) in columns.iter().enumerate().rev() {
        let eligible = matches!(column, ColumnWidth::Fixed(_) if fixed)
            || matches!(column, ColumnWidth::Min(_) if !fixed);
        if !eligible || *deficit == 0 {
            continue;
        }
        let reduction = u64::from(widths[index]).min(*deficit);
        widths[index] -= u16::try_from(reduction).unwrap_or(widths[index]);
        *deficit -= reduction;
    }
}

pub(crate) fn story() -> Story {
    Story::new(
        "prototype/table-columns",
        "Table column prototype",
        "Table (spike)",
        "App-only width solver sketch; excluded from the component catalog.",
        74,
        13,
        render_story,
    )
}

fn render_story(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    debug_assert_eq!(sort_glyph(SortDirection::Ascending), "▲");
    let columns = [
        PrototypeColumn {
            title: "PID",
            width: ColumnWidth::Fixed(7),
            alignment: CellAlignment::Right,
            sort: None,
        },
        PrototypeColumn {
            title: "Process",
            width: ColumnWidth::Fill(weight(2)),
            alignment: CellAlignment::Left,
            sort: None,
        },
        PrototypeColumn {
            title: "Region",
            width: ColumnWidth::Min(10),
            alignment: CellAlignment::Left,
            sort: None,
        },
        PrototypeColumn {
            title: "CPU",
            width: ColumnWidth::Fixed(8),
            alignment: CellAlignment::Right,
            sort: Some(SortDirection::Descending),
        },
        PrototypeColumn {
            title: "State",
            width: ColumnWidth::Fill(weight(1)),
            alignment: CellAlignment::Center,
            sort: None,
        },
    ];
    let rows = [
        ["101", "termrock-lookbook", "東京🧪alpha", "82.4%", "run"],
        ["208", "cargo-nextest", "eu-west", "31.0%", "run"],
        ["317", "rust-analyzer", "local", "17.8%", "idle"],
        ["422", "bun-docs", "us-east", "9.2%", "run"],
        ["509", "shell", "東京", "4.4%", "wait"],
        ["612", "indexer", "ap-south", "2.7%", "idle"],
        ["734", "preview-worker", "eu-north", "1.8%", "run"],
        ["801", "test-runner", "local", "1.1%", "done"],
        ["922", "docs-search", "us-west", "0.8%", "idle"],
        ["1004", "asset-check", "local", "0.5%", "done"],
        ["1118", "lint", "eu-west", "0.2%", "done"],
        ["1201", "release-check", "local", "0.1%", "wait"],
    ];
    let layout = resolve_layout(
        &columns
            .iter()
            .map(|column| column.width)
            .collect::<Vec<_>>(),
        area.width.saturating_sub(ROW_MARKER_WIDTH),
        COLUMN_GAP,
    );

    let header = layout
        .visible
        .iter()
        .map(|index| {
            let column = columns[*index];
            let title = column.sort.map_or_else(
                || column.title.to_owned(),
                |sort| format!("{} {}", column.title, sort_glyph(sort)),
            );
            format_cell(&title, layout.widths[*index], column.alignment)
        })
        .collect::<Vec<_>>()
        .join("  ");
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::raw("  "), Span::raw(header)]))
            .style(theme.style(Role::TextStrong)),
        Rect::new(area.x, area.y, area.width, 1),
    );

    for (row_index, row) in rows
        .iter()
        .enumerate()
        .take(area.height.saturating_sub(1).into())
    {
        let selected = row_index == 3;
        let cells = layout
            .visible
            .iter()
            .map(|index| {
                format_cell(
                    row[*index],
                    layout.widths[*index],
                    columns[*index].alignment,
                )
            })
            .collect::<Vec<_>>()
            .join("  ");
        let line = format!("{}{}", if selected { "▸ " } else { "  " }, cells);
        frame.render_widget(
            Paragraph::new(line).style(theme.style(if selected {
                Role::Selection
            } else {
                Role::Text
            })),
            Rect::new(
                area.x,
                area.y.saturating_add(1 + row_index as u16),
                area.width,
                1,
            ),
        );
    }
}

fn format_cell(value: &str, width: u16, alignment: CellAlignment) -> String {
    let width = usize::from(width);
    let clipped = display_cols_slice(value, 0, width);
    let padding = width.saturating_sub(display_cols(&clipped));
    let (left, right) = match alignment {
        CellAlignment::Left => (0, padding),
        CellAlignment::Center => (padding / 2, padding - padding / 2),
        CellAlignment::Right => (padding, 0),
    };
    format!("{}{}{}", " ".repeat(left), clipped, " ".repeat(right))
}

const fn weight(value: u16) -> NonZeroU16 {
    match NonZeroU16::new(value) {
        Some(value) => value,
        None => panic!("table fill weight must be non-zero"),
    }
}

const fn sort_glyph(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Ascending => "▲",
        SortDirection::Descending => "▼",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solver_contract_table_is_deterministic() {
        let cases: &[(&[ColumnWidth], u16, &[u16])] = &[
            (&[], 10, &[]),
            (&[ColumnWidth::Fill(weight(1))], 0, &[0]),
            (&[ColumnWidth::Fill(weight(1))], 7, &[7]),
            (
                &[ColumnWidth::Fill(weight(1)), ColumnWidth::Fill(weight(1))],
                5,
                &[3, 2],
            ),
            (
                &[ColumnWidth::Fill(weight(1)), ColumnWidth::Fill(weight(2))],
                9,
                &[3, 6],
            ),
            (&[ColumnWidth::Fixed(4), ColumnWidth::Min(3)], 7, &[4, 3]),
            (
                &[
                    ColumnWidth::Fixed(4),
                    ColumnWidth::Fill(weight(1)),
                    ColumnWidth::Min(3),
                ],
                12,
                &[4, 5, 3],
            ),
            (&[ColumnWidth::Fixed(4), ColumnWidth::Min(3)], 6, &[4, 2]),
            (&[ColumnWidth::Fixed(4), ColumnWidth::Min(3)], 3, &[3, 0]),
            (&[ColumnWidth::Min(100)], 9, &[9]),
            (
                &[
                    ColumnWidth::Min(2),
                    ColumnWidth::Min(3),
                    ColumnWidth::Min(4),
                ],
                6,
                &[2, 3, 1],
            ),
            (
                &[ColumnWidth::Fixed(0), ColumnWidth::Fill(weight(2))],
                7,
                &[0, 7],
            ),
            (
                &[
                    ColumnWidth::Fixed(2),
                    ColumnWidth::Fixed(3),
                    ColumnWidth::Fixed(4),
                ],
                4,
                &[2, 2, 0],
            ),
        ];

        for (columns, available, expected) in cases {
            assert_eq!(resolve_widths(columns, *available), *expected);
        }
    }

    #[test]
    fn unicode_cell_clipping_never_paints_half_a_wide_character() {
        assert_eq!(format_cell("東京🧪alpha", 5, CellAlignment::Left), "東京 ");
        assert_eq!(
            display_cols(&format_cell("東京🧪alpha", 5, CellAlignment::Left)),
            5
        );
    }

    #[test]
    fn maximum_fill_weights_do_not_overflow_distribution() {
        assert_eq!(
            resolve_widths(
                &[
                    ColumnWidth::Fill(weight(u16::MAX)),
                    ColumnWidth::Fill(weight(u16::MAX)),
                    ColumnWidth::Fill(weight(u16::MAX)),
                ],
                u16::MAX,
            ),
            [21_845, 21_845, 21_845]
        );
    }

    #[test]
    fn joint_layout_removes_phantom_gaps_without_reviving_hidden_columns() {
        assert_eq!(
            resolve_layout(&[ColumnWidth::Fixed(4), ColumnWidth::Min(3)], 5, 2),
            ResolvedLayout {
                widths: vec![4, 0],
                visible: vec![0],
            }
        );
        assert_eq!(
            resolve_layout(
                &[ColumnWidth::Fill(weight(1)), ColumnWidth::Fill(weight(1)),],
                2,
                2,
            ),
            ResolvedLayout {
                widths: vec![2, 0],
                visible: vec![0],
            }
        );
    }

    #[test]
    fn joint_layout_handles_zero_and_inherently_hidden_columns() {
        assert_eq!(
            resolve_layout(
                &[
                    ColumnWidth::Fixed(0),
                    ColumnWidth::Fill(weight(1)),
                    ColumnWidth::Min(2),
                ],
                1,
                2,
            ),
            ResolvedLayout {
                widths: vec![0, 1, 0],
                visible: vec![1],
            }
        );
        assert_eq!(
            resolve_layout(&[ColumnWidth::Fill(weight(1))], 0, 2),
            ResolvedLayout {
                widths: vec![0],
                visible: vec![],
            }
        );
    }

    #[test]
    fn joint_layout_saturates_gap_count_for_more_than_u16_columns() {
        let columns = vec![ColumnWidth::Fill(weight(1)); usize::from(u16::MAX) + 2];
        let layout = resolve_layout(&columns, 1, 2);
        assert_eq!(layout.visible, [0]);
        assert_eq!(layout.widths[0], 1);
        assert!(layout.widths[1..].iter().all(|width| *width == 0));
    }

    #[test]
    fn typed_sort_directions_map_to_canonical_indicators() {
        assert_eq!(sort_glyph(SortDirection::Ascending), "▲");
        assert_eq!(sort_glyph(SortDirection::Descending), "▼");
    }

    #[test]
    fn app_only_story_renders_header_sort_selection_and_unicode() {
        let buffer = crate::svg::render_story_to_buffer(story(), &Theme::default());
        let text = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("CPU ▼"));
        assert!(text.contains("▸"));
        assert!(text.contains("東 京 🧪"));
        assert!(text.contains("release-check"));
    }

    #[test]
    fn prototype_story_has_stable_dimensions() {
        assert_eq!(story().width, 74);
        assert_eq!(story().height, 13);
    }
}
