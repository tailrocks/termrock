//! `junie_tui::` is `termrock::`: an old import path names the same item, so a value
//! of one is a value of the other, with no conversion and no second type (#452).
//! The facade's own `facade_basic_types_preserve_identity` does the same against the
//! owning crates.

use std::any::TypeId;

fn same<A: 'static, B: 'static>() -> bool {
    TypeId::of::<A>() == TypeId::of::<B>()
}

fn takes_shim_response(response: junie_tui::core::response::Response<u8>) -> u8 {
    response.actions().next().copied().unwrap_or_default()
}

fn shim_row_count<S: junie_tui::core::list::RowSource>(source: &S) -> usize {
    source.len()
}

struct Rows(usize);

impl termrock::core::list::RowSource for Rows {
    fn len(&self) -> usize {
        self.0
    }

    fn label(&self, _: usize) -> &str {
        "row"
    }
}

#[test]
fn shim_basic_types_preserve_identity() {
    assert_eq!(
        takes_shim_response(termrock::core::response::Response::emit(7)),
        7
    );
    assert_eq!(shim_row_count(&Rows(4)), 4);
    let key: junie_tui::core::id::ItemKey = termrock::core::id::ItemKey::num(3);
    assert_eq!(key, junie_tui::core::id::ItemKey::num(3));
    let id: junie_tui::core::id::WidgetId = termrock::core::id::WidgetId::of("shim");
    assert_eq!(id, junie_tui::core::id::WidgetId::of("shim"));
}

#[test]
fn shim_types_are_the_facade_types() {
    assert!(same::<
        junie_tui::core::response::Response<u8>,
        termrock::core::response::Response<u8>,
    >());
    assert!(same::<
        junie_tui::core::id::ItemKey,
        termrock::core::id::ItemKey,
    >());
    assert!(same::<
        junie_tui::core::id::WidgetId,
        termrock::core::id::WidgetId,
    >());
    assert!(same::<
        junie_tui::core::list::SourceStamp,
        termrock::prelude::SourceStamp,
    >());
    assert!(same::<junie_tui::theme::Theme, termrock::theme::Theme>());
    assert!(same::<
        junie_tui::ui::ctx::RenderCtx<'static>,
        termrock::ui::ctx::RenderCtx<'static>,
    >());
    assert!(same::<
        junie_tui::widgets::table::DataTable,
        termrock::widgets::table::DataTable,
    >());
    assert!(same::<junie_tui::shell::Shell, termrock::shell::Shell>());
}

struct Idle;

impl junie_tui::runtime::Application for Idle {
    fn handle(&mut self, _: junie_tui::core::event::Input) -> junie_tui::core::event::Outcome {
        junie_tui::core::event::Outcome::Ignored
    }

    fn render(&mut self, _: &mut ratatui::Frame) {}

    fn should_quit(&self) -> bool {
        true
    }

    fn tick_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(1)
    }
}

/// An `impl` written against the shim's trait satisfies a bound on the facade's.
fn requires_the_facade_application<A: termrock::runtime::Application>(_: &A) {}

#[test]
fn shim_reaches_the_run_loop_and_the_macros() {
    requires_the_facade_application(&Idle);
    let _: fn(&mut Idle) -> std::io::Result<()> = |app| junie_tui::runtime::run(app);
    // A macro the facade re-exports resolves through the glob, with or without `work-metrics`.
    junie_tui::record_work!(LabelsRead, 1);
    termrock::record_work!(LabelsRead, 1);
}

#[cfg(feature = "terminal")]
#[test]
fn shim_default_features_expose_the_terminal_module() {
    assert!(same::<
        junie_tui::terminal::TerminalState,
        termrock::terminal::TerminalState,
    >());
}

#[cfg(feature = "cli")]
#[test]
fn shim_default_features_expose_the_cli_module() {
    assert!(same::<junie_tui::cli::ColorArg, termrock::cli::ColorArg>());
}
