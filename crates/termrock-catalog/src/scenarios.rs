// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Event sequences reconstructed from junie-tui shots/, pages, and app_tests
// (MIT), https://github.com/donbeave/terminal-components-claude

//! Inventoried source `shots/` scenarios: host, size, and replay steps.

use crate::catalog::PageId;

/// Which binary produced the shot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    /// Showcase / catalog page.
    Catalog(PageId),
    /// Standalone TablePro. `connect` is `--connect NAME`.
    TablePro {
        /// Saved connection name, if the shot used `--connect`.
        connect: Option<&'static str>,
    },
}

/// One input, resize, or clock step. Replay in order after the first draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Tab,
    BackTab,
    Enter,
    Esc,
    Space,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Backspace,
    Char(char),
    Ctrl(char),
    Alt(char),
    /// Type each character as a key press.
    Type(&'static str),
    Move(u16, u16),
    Click(u16, u16),
    WheelDown(u16, u16),
    Resize(u16, u16),
    /// Dispatch `on_tick` this many times (source progress ticks at 80 ms).
    Ticks(u16),
}

/// One inventoried source shot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scenario {
    pub id: &'static str,
    pub cols: u16,
    pub rows: u16,
    pub host: Host,
    pub steps: &'static [Step],
}

const fn cat(
    id: &'static str,
    page: PageId,
    cols: u16,
    rows: u16,
    steps: &'static [Step],
) -> Scenario {
    Scenario {
        id,
        cols,
        rows,
        host: Host::Catalog(page),
        steps,
    }
}

const fn tp(
    id: &'static str,
    cols: u16,
    rows: u16,
    connect: Option<&'static str>,
    steps: &'static [Step],
) -> Scenario {
    Scenario {
        id,
        cols,
        rows,
        host: Host::TablePro { connect },
        steps,
    }
}

/// All 63 inventoried `shots/` stems, inventory order.
pub static ALL: &[Scenario] = &[
    cat(
        "f_80x24_taskrunner",
        PageId::TASK_RUNNER,
        80,
        24,
        &[Step::Char('r'), Step::Ticks(18)],
    ),
    cat(
        "f_buttons_hover",
        PageId::BUTTONS,
        120,
        40,
        &[Step::Move(49, 7)],
    ),
    cat(
        "f_dialog_delete",
        PageId::DIALOGS,
        120,
        40,
        &[Step::Tab, Step::Tab, Step::Tab, Step::Tab, Step::Enter],
    ),
    cat("f_forms", PageId::FORMS, 120, 40, &[]),
    cat(
        "f_inputs_edit",
        PageId::INPUTS,
        120,
        40,
        &[Step::Tab, Step::Enter],
    ),
    cat(
        "f_lists_hover",
        PageId::LISTS,
        120,
        40,
        &[Step::Move(86, 9)],
    ),
    cat("f_overview", PageId::OVERVIEW, 120, 40, &[]),
    cat("f_panels", PageId::PANELS, 120, 40, &[]),
    cat("f_progress", PageId::PROGRESS, 120, 40, &[Step::Ticks(25)]),
    cat(
        "f_scrolling",
        PageId::SCROLLING,
        120,
        40,
        &[Step::Resize(120, 40), Step::Ticks(20)],
    ),
    cat(
        "f_settings_members",
        PageId::SETTINGS,
        120,
        40,
        &[Step::Tab, Step::Right, Step::Tab, Step::Down],
    ),
    cat("f_sidebars", PageId::SIDEBARS, 120, 40, &[]),
    cat(
        "f_tables_hover",
        PageId::TABLES,
        120,
        40,
        &[Step::Move(117, 9)],
    ),
    cat(
        "f_taskrunner_running",
        PageId::TASK_RUNNER,
        120,
        40,
        &[Step::Char('r'), Step::Ticks(28)],
    ),
    cat("s_chips", PageId::CHIPS, 120, 40, &[Step::Tab, Step::Right]),
    cat("s_chips_80", PageId::CHIPS, 80, 24, &[Step::Tab, Step::Tab]),
    cat(
        "s_chips_select",
        PageId::CHIPS,
        120,
        40,
        &[Step::Tab, Step::Tab, Step::Enter],
    ),
    cat("s_editor", PageId::EDITOR, 120, 40, &[Step::Tab]),
    cat("s_editor_80", PageId::EDITOR, 80, 24, &[Step::Tab]),
    cat(
        "s_editor_complete",
        PageId::EDITOR,
        120,
        40,
        // Golden: `    cl` between the comment and `pub async` in the same
        // block. Enter at the start of `pub async` leaves a blank line above
        // the cursor; Up then type fills that line.
        &[
            Step::Tab,
            Step::Enter,
            Step::Down,
            Step::Enter,
            Step::Up,
            Step::Type("    cl"),
        ],
    ),
    cat(
        "s_editor_diag",
        PageId::EDITOR,
        120,
        40,
        &[Step::Tab, Step::Char('}'), Step::Ctrl('r'), Step::Ticks(10)],
    ),
    cat(
        "s_editor_running",
        PageId::EDITOR,
        120,
        40,
        &[Step::Tab, Step::Char('}'), Step::Ctrl('r'), Step::Ticks(4)],
    ),
    cat("s_grid", PageId::GRID, 120, 40, &[Step::Tab]),
    cat("s_grid_80", PageId::GRID, 80, 24, &[Step::Tab]),
    cat(
        "s_grid_failed",
        PageId::GRID,
        120,
        40,
        // Two seat edits, then save: row 1 600 is over the plan limit.
        &[
            Step::Tab,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Enter,
            Step::Backspace,
            Step::Type("600"),
            Step::Enter,
            Step::Down,
            Step::Enter,
            Step::Backspace,
            Step::Backspace,
            Step::Type("12"),
            Step::Enter,
            Step::Ctrl('s'),
            Step::Ticks(4),
        ],
    ),
    cat(
        "s_grid_pending",
        PageId::GRID,
        120,
        40,
        &[
            Step::Tab,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Enter,
            Step::Backspace,
            Step::Type("600"),
            Step::Enter,
            Step::Down,
            Step::Enter,
            Step::Backspace,
            Step::Backspace,
            Step::Type("12"),
            Step::Enter,
            Step::Right,
        ],
    ),
    cat(
        "s_grid_preview",
        PageId::GRID,
        120,
        40,
        &[
            Step::Tab,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Enter,
            Step::Backspace,
            Step::Type("600"),
            Step::Enter,
            Step::Down,
            Step::Enter,
            Step::Backspace,
            Step::Backspace,
            Step::Type("12"),
            Step::Enter,
            Step::Right,
            Step::Char('p'),
        ],
    ),
    cat(
        "s_pickers",
        PageId::PICKERS,
        120,
        40,
        &[Step::Tab, Step::Enter, Step::Type("bil")],
    ),
    cat(
        "s_pickers_80",
        PageId::PICKERS,
        80,
        24,
        &[Step::Tab, Step::Enter],
    ),
    cat(
        "s_pickers_level",
        PageId::PICKERS,
        120,
        40,
        &[Step::Tab, Step::Tab, Step::Tab, Step::Enter],
    ),
    cat(
        "s_pickers_tabs",
        PageId::PICKERS,
        120,
        40,
        &[Step::Tab, Step::Tab, Step::Enter],
    ),
    tp(
        "t_100",
        100,
        30,
        Some("Production"),
        &[
            Step::Tab,
            Step::Char('i'),
            Step::Type("SELECT * FROM customers LIMIT 20"),
            Step::Esc,
            Step::Ctrl('r'),
            Step::Ticks(4),
        ],
    ),
    tp(
        "t_100_table",
        100,
        30,
        Some("Production"),
        &[
            Step::Char('0'),
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Enter,
            Step::Ticks(3),
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Char('s'),
        ],
    ),
    tp(
        "t_160",
        160,
        50,
        Some("Production"),
        &[
            Step::Tab,
            Step::Char('i'),
            Step::Type("SELECT * FROM orders WHERE status = 'pending' LIMIT 30"),
            Step::Esc,
            Step::Alt('x'),
            Step::Ticks(8),
        ],
    ),
    tp(
        "t_160_table",
        160,
        50,
        Some("Production"),
        &[
            Step::Char('0'),
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Enter,
        ],
    ),
    tp(
        "t_80",
        80,
        24,
        Some("Production"),
        &[
            Step::Char('0'),
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Enter,
            // The source capture session outlived the status message's
            // five-second TTL before this shot: the golden footer carries
            // hints only. The replay outlives the TTL with ticks (80 ms
            // each, source cadence).
            Step::Ticks(64),
        ],
    ),
    tp(
        "t_80_drawer",
        80,
        24,
        Some("Production"),
        &[
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Ctrl('b'),
            Step::Ctrl('b'),
        ],
    ),
    tp(
        "t_80_query",
        80,
        24,
        Some("Production"),
        &[
            Step::Tab,
            Step::Char('i'),
            Step::Type("SELECT * FROM orders LIMIT 20"),
            Step::Esc,
            Step::Ctrl('r'),
            Step::Ticks(8),
        ],
    ),
    tp(
        "t_complete",
        120,
        40,
        Some("Production"),
        &[
            Step::Tab,
            Step::Char('i'),
            // The recorded capture types only `SEL` before requesting
            // completion; the golden popup lists keyword matches for that
            // prefix.
            Step::Type("SEL"),
            Step::Ctrl(' '),
        ],
    ),
    tp(
        "t_complete2",
        120,
        40,
        Some("Production"),
        &[
            Step::Tab,
            Step::Char('i'),
            Step::Type("SELECT * FROM o"),
            Step::Ctrl(' '),
        ],
    ),
    tp("t_conn_dup", 120, 40, None, &[Step::Ctrl('d')]),
    tp("t_conn_form", 120, 40, None, &[Step::Char('n')]),
    tp(
        "t_conn_form_adv",
        120,
        40,
        None,
        &[Step::Char('n'), Step::Tab],
    ),
    tp(
        "t_conn_prod",
        120,
        40,
        None,
        &[
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
        ],
    ),
    tp("t_connections", 120, 40, None, &[]),
    tp(
        "t_danger",
        120,
        40,
        Some("Production"),
        &[
            Step::Tab,
            Step::Char('i'),
            Step::Type("DELETE FROM orders"),
            Step::Esc,
            Step::Ctrl('r'),
        ],
    ),
    tp(
        "t_dirty",
        120,
        40,
        Some("Production"),
        &[
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Enter,
            Step::Enter,
            Step::Type("x"),
        ],
    ),
    tp(
        "t_editing_cell",
        120,
        40,
        Some("Production"),
        &[
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Enter,
            Step::Enter,
        ],
    ),
    tp(
        "t_error",
        120,
        40,
        Some("Production"),
        &[
            Step::Tab,
            Step::Char('i'),
            Step::Type("SELECT nope FROM orders"),
            Step::Esc,
            Step::Esc,
            Step::Ctrl('r'),
            Step::Ticks(10),
        ],
    ),
    tp(
        "t_explain",
        120,
        40,
        Some("Production"),
        &[
            Step::Tab,
            Step::Char('i'),
            Step::Type(
                "SELECT * FROM orders WHERE notes LIKE '%gift%' ORDER BY created_at LIMIT 10",
            ),
            Step::Esc,
            Step::Alt('x'),
            Step::Ticks(10),
        ],
    ),
    tp("t_history", 120, 40, Some("Production"), &[Step::Ctrl('y')]),
    tp(
        "t_orders",
        120,
        40,
        Some("Production"),
        &[
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Enter,
        ],
    ),
    tp(
        "t_orders_wide",
        120,
        40,
        Some("Production"),
        &[
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Enter,
        ],
    ),
    tp(
        "t_result",
        120,
        40,
        Some("Production"),
        &[
            Step::Tab,
            Step::Char('i'),
            Step::Type("SELECT * FROM orders LIMIT 25"),
            Step::Esc,
            Step::Ctrl('r'),
            Step::Ticks(10),
        ],
    ),
    tp(
        "t_running",
        120,
        40,
        Some("Production"),
        &[
            Step::Tab,
            Step::Char('i'),
            Step::Type("SELECT * FROM events"),
            Step::Esc,
            Step::Ctrl('r'),
        ],
    ),
    tp(
        "t_safemode",
        120,
        40,
        Some("Production"),
        &[Step::Ctrl('l')],
    ),
    tp(
        "t_sorted_filtered",
        120,
        40,
        Some("Production"),
        &[
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Down,
            Step::Enter,
            Step::Home,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Char('s'),
            Step::Char('f'),
        ],
    ),
    tp(
        "t_structure",
        120,
        40,
        Some("Production"),
        &[Step::Down, Step::Down, Step::Enter, Step::Ctrl('d')],
    ),
    tp(
        "t_structure_fk",
        120,
        40,
        Some("Production"),
        &[
            Step::Down,
            Step::Down,
            Step::Enter,
            Step::Ctrl('d'),
            Step::Right,
            Step::Right,
        ],
    ),
    tp(
        "t_switcher",
        120,
        40,
        Some("Production"),
        &[Step::Ctrl('o')],
    ),
    tp("t_tablist", 120, 40, Some("Production"), &[Step::Ctrl('g')]),
    tp("t_workbench", 120, 40, Some("Production"), &[]),
    tp(
        "t_write",
        120,
        40,
        Some("Production"),
        &[
            Step::Tab,
            Step::Char('i'),
            Step::Type("UPDATE orders SET notes = 'x' WHERE id = 1"),
            Step::Esc,
            Step::Ctrl('r'),
        ],
    ),
];

/// TablePro application states used by the presentation verification harness.
/// These are separate from [`ALL`], which is the exact 63-file source `shots/`
/// inventory, because the source repository's TablePro captures were operator
/// evidence rather than checked-in `shots/` fixtures.
pub static TABLEPRO: &[Scenario] = &[
    tp("tablepro_default_120x40", 120, 40, None, &[]),
    tp("tablepro_default_80x24", 80, 24, None, &[]),
    tp(
        "tablepro_local_120x40",
        120,
        40,
        Some("Local PostgreSQL"),
        &[],
    ),
    tp(
        "tablepro_production_120x40",
        120,
        40,
        Some("Production"),
        &[],
    ),
    tp("tablepro_help_120x40", 120, 40, None, &[Step::Char('?')]),
];

/// Every deterministic presentation capture, including application fixtures.
#[must_use]
pub fn capture_scenarios() -> impl Iterator<Item = &'static Scenario> {
    ALL.iter().chain(TABLEPRO.iter())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sixty_three_unique_ids() {
        assert_eq!(ALL.len(), 63);
        let mut ids: Vec<_> = ALL.iter().map(|s| s.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 63);
    }

    #[test]
    fn capture_scenarios_have_unique_ids() {
        let mut ids: Vec<_> = capture_scenarios().map(|s| s.id).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
        assert_eq!(count, 68);
    }
}
