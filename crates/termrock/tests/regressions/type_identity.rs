use termrock::prelude::{Flow, Invalidate, ItemKey, Response, RowSource, WidgetId};

fn takes_impl_response(response: junie_tui::core::response::Response<u8>) -> u8 {
    response.actions().next().copied().unwrap_or_default()
}

fn takes_impl_key(key: junie_tui::core::id::ItemKey) -> junie_tui::core::id::ItemKey {
    key
}

fn impl_row_count<S: junie_tui::core::list::RowSource>(source: &S) -> usize {
    source.len()
}

struct Rows(usize);

impl RowSource for Rows {
    fn len(&self) -> usize {
        self.0
    }

    fn label(&self, i: usize) -> &str {
        ["a", "b", "c", "d"][i]
    }
}

#[test]
fn facade_basic_types_preserve_identity() {
    assert_eq!(takes_impl_response(Response::emit(7)), 7);
    assert_eq!(
        takes_impl_key(ItemKey::num(3)),
        junie_tui::core::id::ItemKey::num(3)
    );
    assert_eq!(impl_row_count(&Rows(4)), 4);

    let id: junie_tui::core::id::WidgetId = WidgetId::of("facade");
    assert_eq!(id, junie_tui::core::id::WidgetId::of("facade"));

    let ignored: Response = Response::ignored();
    assert_eq!(ignored.flow(), Flow::Ignored);
    assert_eq!(ignored.invalidate(), Invalidate::None);
}

#[test]
fn facade_module_paths_name_the_implementation_types() {
    let motion: junie_tui::motion::Motion = termrock::motion::Motion::default();
    assert_eq!(motion, junie_tui::motion::Motion::default());
}

#[cfg(feature = "terminal")]
#[test]
fn facade_terminal_feature_exposes_the_viewport() {
    let state: junie_tui::terminal::TerminalState = termrock::terminal::TerminalState::new(5, 20);
    let viewport: junie_tui::terminal::TerminalViewport =
        termrock::terminal::TerminalViewport::new(WidgetId::of("terminal"), state);
    assert_eq!(viewport.id, WidgetId::of("terminal"));
}

#[cfg(feature = "cli")]
#[test]
fn facade_cli_feature_exposes_the_arguments() {
    let level: junie_tui::theme::ColorLevel = termrock::cli::ColorArg::Mono.into();
    assert_eq!(level, termrock::theme::ColorLevel::Mono);
}
