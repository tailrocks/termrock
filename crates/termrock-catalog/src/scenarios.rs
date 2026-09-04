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
    /// TablePro: document loaded into the active query tab before the first draw.
    pub seed_sql: Option<&'static str>,
    /// TablePro: optional in-flight run length for source running-state shots.
    pub run_ticks_left: Option<u32>,
    /// Source-retained terminal cursor for captures where no app cursor is visible.
    pub capture_cursor: Option<(u16, u16)>,
    /// TablePro: final table view reconstructed from source artifact evidence.
    pub table_state: Option<TableStateSeed>,
    /// TablePro: active table identity when source navigation evidence is stale.
    pub table_name: Option<&'static str>,
    /// TablePro: active table mode (0=data, 1=structure).
    pub table_mode: Option<u8>,
    /// TablePro: retained host focus for source footer parity.
    pub table_focus: Option<TableFocusSeed>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableStateSeed {
    pub filter_column: &'static str,
    pub filter_value: &'static str,
    pub sort_column: &'static str,
    pub sort_ascending: bool,
    pub hscroll: u16,
    pub cursor_row: usize,
    pub cursor_col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableFocusSeed {
    Explorer,
    TabStrip,
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
        seed_sql: None,
        run_ticks_left: None,
        capture_cursor: None,
        table_state: None,
        table_name: None,
        table_mode: None,
        table_focus: None,
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
        seed_sql: None,
        run_ticks_left: None,
        capture_cursor: None,
        table_state: None,
        table_name: None,
        table_mode: None,
        table_focus: None,
    }
}

const fn tp_sql(
    id: &'static str,
    cols: u16,
    rows: u16,
    connect: Option<&'static str>,
    sql: &'static str,
    steps: &'static [Step],
) -> Scenario {
    Scenario {
        id,
        cols,
        rows,
        host: Host::TablePro { connect },
        steps,
        seed_sql: Some(sql),
        run_ticks_left: None,
        capture_cursor: None,
        table_state: None,
        table_name: None,
        table_mode: None,
        table_focus: None,
    }
}

const fn tp_sql_running(
    id: &'static str,
    cols: u16,
    rows: u16,
    connect: Option<&'static str>,
    sql: &'static str,
    run_ticks_left: u32,
    steps: &'static [Step],
) -> Scenario {
    let mut scenario = tp_sql(id, cols, rows, connect, sql, steps);
    scenario.run_ticks_left = Some(run_ticks_left);
    scenario.capture_cursor = Some((47, 17));
    scenario
}

const fn tp_table_state(
    id: &'static str,
    cols: u16,
    rows: u16,
    connect: Option<&'static str>,
    state: TableStateSeed,
    steps: &'static [Step],
) -> Scenario {
    let mut scenario = tp(id, cols, rows, connect, steps);
    scenario.table_state = Some(state);
    scenario
}

const fn tp_table(
    id: &'static str,
    cols: u16,
    rows: u16,
    connect: Option<&'static str>,
    name: &'static str,
    mode: u8,
    focus: TableFocusSeed,
    steps: &'static [Step],
) -> Scenario {
    let mut scenario = tp(id, cols, rows, connect, steps);
    scenario.table_name = Some(name);
    scenario.table_mode = Some(mode);
    scenario.table_focus = Some(focus);
    scenario
}

/// All 63 inventoried `shots/` stems, inventory order.
pub static ALL: &[Scenario] = &[
    cat(
        "f_80x24_taskrunner",
        PageId::TASK_RUNNER,
        80,
        24,
        &[Step::Char('r'), Step::Ticks(19)],
    ),
    cat(
        "f_buttons_hover",
        PageId::BUTTONS,
        120,
        40,
        &[Step::Tab, Step::Tab, Step::Move(49, 7)],
    ),
    cat(
        "f_dialog_delete",
        PageId::DIALOGS,
        120,
        40,
        &[Step::Tab, Step::Tab, Step::Tab, Step::Tab, Step::Enter],
    ),
    cat(
        "f_forms",
        PageId::FORMS,
        120,
        40,
        &[Step::Tab, Step::Tab, Step::Tab, Step::Tab],
    ),
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
        &[Step::Tab, Step::Down, Step::Down, Step::Move(86, 9)],
    ),
    cat("f_overview", PageId::OVERVIEW, 120, 40, &[]),
    cat("f_panels", PageId::PANELS, 120, 40, &[Step::Tab, Step::Tab]),
    cat("f_progress", PageId::PROGRESS, 120, 40, &[Step::Ticks(26)]),
    cat(
        "f_scrolling",
        PageId::SCROLLING,
        120,
        40,
        &[Step::Tab, Step::Tab, Step::Tab, Step::Ticks(17)],
    ),
    cat(
        "f_settings_members",
        PageId::SETTINGS,
        120,
        40,
        &[
            Step::Tab,
            Step::Right,
            Step::Tab,
            Step::Down,
            Step::Right,
            Step::Right,
            Step::Enter,
        ],
    ),
    cat("f_sidebars", PageId::SIDEBARS, 120, 40, &[Step::Tab]),
    cat(
        "f_tables_hover",
        PageId::TABLES,
        120,
        40,
        &[Step::Tab, Step::Move(117, 9)],
    ),
    cat(
        "f_taskrunner_running",
        PageId::TASK_RUNNER,
        120,
        40,
        &[Step::Char('r'), Step::Ticks(35)],
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
    tp_sql(
        "t_100",
        100,
        30,
        Some("Production"),
        "SELECT * FROM customers LIMIT 20",
        // The source capture leaves the narrow explorer drawer before running
        // the seeded query; this is the same Tab transition covered by the
        // source's `narrow_terminals_turn_the_explorer_into_a_drawer` test.
        &[Step::Tab, Step::Ctrl('r'), Step::Ticks(4)],
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
    tp_sql(
        "t_160",
        160,
        50,
        Some("Production"),
        "SELECT * FROM orders WHERE status = 'pending' LIMIT 30",
        &[Step::Alt('x'), Step::Ticks(8)],
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
            Step::Ticks(3),
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Char('f'),
            Step::BackTab,
            Step::BackTab,
            Step::Enter,
            Step::Ctrl('l'),
            Step::Type("pending"),
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
    tp_sql(
        "t_80_query",
        80,
        24,
        Some("Production"),
        "SELECT * FROM orders LIMIT 20",
        &[Step::Tab, Step::Ctrl('r'), Step::Ticks(8)],
    ),
    tp(
        "t_complete",
        120,
        40,
        Some("Production"),
        &[
            Step::Tab,
            Step::Char('i'),
            Step::Type("SELECT * FROM ord"),
            Step::Ctrl(' '),
        ],
    ),
    tp_sql(
        "t_complete2",
        120,
        40,
        Some("Production"),
        "SELECT o. FROM orders o",
        &[
            Step::Tab,
            Step::Char('i'),
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
            Step::Ctrl(' '),
            // The source frame was captured after the transient connection
            // status expired; preserve that timing in the deterministic replay.
            Step::Ticks(51),
        ],
    ),
    tp("t_conn_dup", 120, 40, None, &[Step::Ctrl('d')]),
    tp("t_conn_form", 120, 40, None, &[Step::Char('n')]),
    tp(
        "t_conn_form_adv",
        120,
        40,
        None,
        &[
            Step::Tab,
            Step::Tab,
            Step::Enter,
            Step::Click(116, 17),
            Step::Move(117, 17),
        ],
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
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Enter,
            Step::Ctrl('l'),
            Step::Type("EUR"),
            Step::Enter,
            Step::Down,
            Step::Space,
            Step::Char('-'),
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
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Right,
            Step::Enter,
            Step::Ctrl('l'),
            Step::Type("EUR"),
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
            // The checked-in source capture is 120 columns and preserves the
            // rightmost table window from the operator's prior navigation.
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
            Step::Right,
        ],
    ),
    tp_sql(
        "t_result",
        120,
        40,
        Some("Production"),
        "SELECT * FROM orders WHERE status = 'pending' ORDER BY total_amount DESC LIMIT 40",
        &[Step::Tab, Step::Ctrl('r'), Step::Ticks(10)],
    ),
    tp_sql_running(
        "t_running",
        120,
        40,
        Some("Production"),
        "SELECT * FROM orders WHERE status = 'pending' ORDER BY total_amount DESC LIMIT 40",
        6,
        &[Step::Tab, Step::Ctrl('r'), Step::Ticks(4)],
    ),
    tp(
        "t_safemode",
        120,
        40,
        Some("Production"),
        &[Step::Ctrl('l')],
    ),
    tp_table_state(
        "t_sorted_filtered",
        120,
        40,
        Some("Production"),
        TableStateSeed {
            filter_column: "status",
            filter_value: "pending",
            sort_column: "status",
            sort_ascending: true,
            hscroll: 4,
            cursor_row: 0,
            cursor_col: 4,
        },
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
    tp_table(
        "t_structure",
        120,
        40,
        Some("Production"),
        "orders",
        1,
        TableFocusSeed::Explorer,
        &[Step::Down, Step::Down, Step::Enter, Step::Ctrl('d')],
    ),
    tp_table(
        "t_structure_fk",
        120,
        40,
        Some("Production"),
        "orders",
        1,
        TableFocusSeed::TabStrip,
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
            Step::Type("UPDATE orders SET status = 'paid' WHERE id = 'x'"),
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
