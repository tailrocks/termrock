use termrock::prelude::{
    Actions, Flow, Invalidate, ItemKey, KeyedRowSource, Response, RowSource, SourceStamp, WidgetId,
};

fn takes_impl_response(response: termrock_core::response::Response<u8>) -> u8 {
    response.actions().next().copied().unwrap_or_default()
}

fn takes_impl_key(key: termrock_core::id::ItemKey) -> termrock_core::id::ItemKey {
    key
}

fn impl_row_count<S: termrock_core::list::RowSource>(source: &S) -> usize {
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
        termrock_core::id::ItemKey::num(3)
    );
    assert_eq!(impl_row_count(&Rows(4)), 4);

    let id: termrock_core::id::WidgetId = WidgetId::of("facade");
    assert_eq!(id, termrock_core::id::WidgetId::of("facade"));

    let ignored: Response = Response::ignored();
    assert_eq!(ignored.flow(), Flow::Ignored);
    assert_eq!(ignored.invalidate(), Invalidate::None);
}

#[test]
fn facade_module_paths_name_the_implementation_types() {
    let motion: termrock_core::motion::Motion = termrock::motion::Motion::default();
    assert_eq!(motion, termrock_core::motion::Motion::default());
}

#[cfg(feature = "terminal")]
#[test]
fn facade_terminal_feature_exposes_the_viewport() {
    let state: termrock_terminal::TerminalState = termrock::terminal::TerminalState::new(5, 20);
    let viewport: termrock_terminal::TerminalViewport =
        termrock::terminal::TerminalViewport::new(WidgetId::of("terminal"), state);
    assert_eq!(viewport.id, WidgetId::of("terminal"));
}

#[cfg(feature = "cli")]
#[test]
fn facade_cli_feature_exposes_the_arguments() {
    let level: termrock_theme::ColorLevel = termrock::cli::ColorArg::Mono.into();
    assert_eq!(level, termrock::theme::ColorLevel::Mono);
}

struct Keyed;

impl RowSource for Keyed {
    fn len(&self) -> usize {
        1
    }

    fn label(&self, _: usize) -> &str {
        "only"
    }

    fn key(&self, _: usize) -> ItemKey {
        ItemKey::num(9)
    }
}

impl KeyedRowSource for Keyed {
    fn stamp(&self) -> SourceStamp {
        SourceStamp {
            instance: ItemKey::num(1),
            data: 0,
            view: 0,
        }
    }

    fn source_len(&self) -> usize {
        1
    }

    fn source_key(&self, _: usize) -> ItemKey {
        ItemKey::num(9)
    }
}

fn impl_stamp<S: termrock_core::list::KeyedRowSource>(
    source: &S,
) -> termrock_core::list::SourceStamp {
    source.stamp()
}

#[test]
fn facade_keyed_source_contract_preserves_identity() {
    let stamp: SourceStamp = impl_stamp(&Keyed);
    assert_eq!(stamp.instance, ItemKey::num(1));
}

#[test]
fn facade_prelude_actions_preserve_identity() {
    fn takes_actions(actions: termrock_core::response::Actions<u8>) -> usize {
        actions.len()
    }
    assert_eq!(takes_actions(Actions::One(9u8)), 1);
}
