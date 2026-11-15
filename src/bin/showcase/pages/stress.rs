//! Stress data for the overflow demos.
//!
//! Four pages (Toolbars, Breadcrumbs, Chrome's status bar, Chips) each have a
//! `Stress data` option. The option swaps the page's demo data for a large
//! item set or for one label 65,536 cells wide. The data is generated here,
//! and every page feeds it through the same library fitting code the real
//! apps use.

/// Option values shared by every `Stress data` row.
pub const VALUES: &[&str] = &["off", "20k items", "65k label"];

/// Items in the large set. Twenty thousand labels of twelve cells each total
/// more than `u16::MAX` cells, so the fitting code runs past the old `u16`
/// sums.
pub const COUNT: usize = 20_000;

/// Cells in the long label. 65,536 wraps to zero when narrowed to `u16`.
pub const LABEL_CELLS: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Off,
    Items,
    Label,
}

/// The mode for an option value from [`VALUES`].
pub fn mode(value: &str) -> Mode {
    match value {
        "20k items" => Mode::Items,
        "65k label" => Mode::Label,
        _ => Mode::Off,
    }
}

/// `COUNT` labels named `prefix 00001` onward.
pub fn numbered(prefix: &str) -> impl Iterator<Item = String> + '_ {
    (1..=COUNT).map(move |n| format!("{prefix} {n:05}"))
}

/// A measured width for a page layout. A saturated measure (`u16::MAX`) means
/// "at least this wide", so the page gives it at most its own width. Any other
/// measure is used as measured.
pub fn clamp_saturated(measured: u16, page: u16) -> u16 {
    if measured == u16::MAX { page } else { measured }
}

/// One ASCII label exactly `LABEL_CELLS` cells wide.
pub fn long_label() -> String {
    "long label "
        .repeat(LABEL_CELLS / 11 + 1)
        .chars()
        .take(LABEL_CELLS)
        .collect()
}
