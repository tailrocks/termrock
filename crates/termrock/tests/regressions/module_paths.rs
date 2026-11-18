//! Every module path the single `junie-tui` package exposed before the split still
//! resolves through the facade (#452). The lists come from `src/{core,ui,widgets}/mod.rs`,
//! `src/lib.rs` and the `pub mod` lines of the other modules at `ecffc8e21`, the commit the
//! crate split was planned against, and are committed rather than derived from the current
//! tree: a derived list could not notice a module that went missing.
//!
//! The check is compile-time: each `use` names a path, so a path that stops resolving
//! fails the build of this target. The test only records that it ran.
#![allow(unused_imports)]

use termrock::catalog as _;
#[cfg(feature = "cli")]
use termrock::cli as _;
use termrock::clock as _;
use termrock::core as _;
use termrock::docs as _;
use termrock::metrics as _;
use termrock::motion as _;
use termrock::runtime as _;
use termrock::shell as _;
#[cfg(feature = "terminal")]
use termrock::terminal as _;
use termrock::theme as _;
use termrock::ui as _;
use termrock::widgets as _;
// The two exported macros.
use termrock::reach_tests as _;
use termrock::record_work as _;

// core: 21 modules
use termrock::core::data as _;
use termrock::core::edit as _;
use termrock::core::event as _;
use termrock::core::focus as _;
use termrock::core::hints as _;
use termrock::core::hit as _;
use termrock::core::id as _;
use termrock::core::intent as _;
use termrock::core::keyboard as _;
use termrock::core::keyed as _;
use termrock::core::keymap as _;
use termrock::core::list as _;
use termrock::core::nav as _;
use termrock::core::pointer as _;
use termrock::core::refusal as _;
use termrock::core::response as _;
use termrock::core::scroll as _;
use termrock::core::selection as _;
use termrock::core::shape as _;
use termrock::core::text as _;
use termrock::core::typeahead as _;

// ui: 22 modules
use termrock::ui::columns as _;
use termrock::ui::content as _;
use termrock::ui::context as _;
use termrock::ui::ctx as _;
use termrock::ui::edit_menu as _;
use termrock::ui::fade as _;
use termrock::ui::layer as _;
use termrock::ui::layers as _;
use termrock::ui::layout as _;
use termrock::ui::leave as _;
use termrock::ui::list_core as _;
use termrock::ui::master_detail as _;
use termrock::ui::measure as _;
use termrock::ui::modal as _;
use termrock::ui::pane_tree as _;
use termrock::ui::popup as _;
use termrock::ui::reveal as _;
use termrock::ui::row as _;
use termrock::ui::scroll_view as _;
use termrock::ui::specimen as _;
use termrock::ui::text as _;
use termrock::ui::tooltip as _;

// widgets: 58 modules
use termrock::widgets::action_row as _;
use termrock::widgets::brand as _;
use termrock::widgets::breadcrumb as _;
use termrock::widgets::button as _;
use termrock::widgets::chips as _;
use termrock::widgets::choice as _;
use termrock::widgets::choice_dialog as _;
use termrock::widgets::code as _;
use termrock::widgets::completion as _;
use termrock::widgets::detail_card as _;
use termrock::widgets::dialog as _;
use termrock::widgets::diff as _;
use termrock::widgets::document_workspace as _;
use termrock::widgets::edit as _;
use termrock::widgets::empty as _;
use termrock::widgets::field_common as _;
use termrock::widgets::file_browser as _;
use termrock::widgets::form_dialog as _;
use termrock::widgets::grid as _;
use termrock::widgets::help as _;
use termrock::widgets::hintbar as _;
use termrock::widgets::icon_button as _;
use termrock::widgets::info_dialog as _;
use termrock::widgets::input as _;
use termrock::widgets::inspect_changes as _;
use termrock::widgets::keyhint as _;
use termrock::widgets::keyref as _;
use termrock::widgets::list as _;
use termrock::widgets::list_detail as _;
use termrock::widgets::menu as _;
use termrock::widgets::meter_list as _;
use termrock::widgets::nav_list as _;
use termrock::widgets::notice as _;
use termrock::widgets::option_editor as _;
use termrock::widgets::option_list as _;
use termrock::widgets::page_header as _;
use termrock::widgets::panel as _;
use termrock::widgets::picker as _;
use termrock::widgets::progress as _;
use termrock::widgets::props as _;
use termrock::widgets::rail as _;
use termrock::widgets::row_menu as _;
use termrock::widgets::scrollbar as _;
use termrock::widgets::search_detail as _;
use termrock::widgets::segments as _;
use termrock::widgets::select as _;
use termrock::widgets::splitter as _;
use termrock::widgets::statusbar as _;
use termrock::widgets::steps as _;
use termrock::widgets::table as _;
use termrock::widgets::tabs as _;
use termrock::widgets::textarea as _;
use termrock::widgets::too_small as _;
use termrock::widgets::toolbar as _;
use termrock::widgets::tree as _;
use termrock::widgets::viewport as _;
use termrock::widgets::viewport_ops as _;
use termrock::widgets::widget_hints as _;

// Modules of the other facade modules.
use termrock::docs::widget as _;
use termrock::metrics::measurement as _;
#[cfg(feature = "work-metrics")]
use termrock::metrics::{harness as _, report as _};
#[cfg(unix)]
use termrock::runtime::job_control as _;
use termrock::shell::{clipboard as _, reach as _};

#[test]
fn facade_module_paths_resolve() {
    // Nothing to run: this target compiles only while every path above resolves.
}
