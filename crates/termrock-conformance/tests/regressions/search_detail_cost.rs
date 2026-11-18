//! Deterministic work counts for result projections and paint.
//!
//! Counters are the #56 work metrics (`work-metrics` feature), never timings.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::keyed_cost::nz;
use termrock_collections::list_detail::SourceFrame;
use termrock_collections::search_detail::{
    QueryRequest, ResultRowKind, SearchAction, SearchConfig, SearchDetail, SearchSource,
};
use termrock_core::focus::FocusRing;
use termrock_core::hit::HitRegistry;
use termrock_core::id::{ItemKey, WidgetId};
use termrock_core::list::{KeyedRowSource, RowSource, SourceStamp};
use termrock_fields::input::TextInput;
use termrock_layout::layout::Split;
use termrock_metrics::{Metric, measure};
use termrock_runtime::ctx::{Interaction, RenderCtx};
use termrock_theme::{ColorLevel, Theme};

const AREA: Rect = Rect::new(0, 0, 120, 24);

struct Catalog {
    stamp: SourceStamp,
    len: usize,
}

impl RowSource for Catalog {
    fn len(&self) -> usize {
        self.len
    }
    fn enabled(&self, _: usize) -> bool {
        true
    }
    fn label(&self, _: usize) -> &str {
        "result"
    }
    fn key(&self, i: usize) -> ItemKey {
        ItemKey::num(i as u64 + 1)
    }
}

impl KeyedRowSource for Catalog {
    fn stamp(&self) -> SourceStamp {
        self.stamp
    }
    fn source_len(&self) -> usize {
        self.len
    }
    fn source_key(&self, i: usize) -> ItemKey {
        ItemKey::num(i as u64 + 1)
    }
}

impl SearchSource for Catalog {
    fn row_kind(&self, _: usize) -> ResultRowKind {
        ResultRowKind::Item
    }
    fn cell_count(&self, _: usize) -> usize {
        1
    }
    fn cell_key(&self, row: usize, _: usize) -> ItemKey {
        self.key(row)
    }
    fn cell_label(&self, _: usize, _: usize) -> &str {
        "result"
    }
    fn cell_enabled(&self, _: usize, _: usize) -> bool {
        true
    }
}

fn paint(search: &mut SearchDetail, source: &Catalog) -> usize {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(AREA);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    let frame = SourceFrame {
        title: "Results",
        meta: "",
        head: &[],
        under: None,
        detail_title: "Preview",
        detail_meta: "",
        lines: &[],
        actions: &[],
    };
    let mut painted = 0;
    search
        .render_source_with(
            AREA,
            source,
            &frame,
            &mut buf,
            &mut ctx,
            |row, buf, _| {
                painted += 1;
                buf.set_string(
                    row.rect.x,
                    row.rect.y,
                    "result",
                    ratatui::style::Style::new(),
                );
            },
            |_, _, _, _| {},
        )
        .expect("accepted source");
    painted
}

#[test]
fn search_detail_noop_source_does_not_rebuild() {
    const N: usize = 10_000;
    let stamp = SourceStamp {
        instance: ItemKey::num(900),
        data: 1,
        view: 0,
    };
    let mut search = SearchDetail::new(
        WidgetId::of("cost"),
        TextInput::new(WidgetId::of("cost.q"), "Search"),
        Split::new(50, 36, 40),
    )
    .config(SearchConfig::default());
    let requests = |reply: termrock_core::response::Response<SearchAction>| -> Vec<QueryRequest> {
        reply
            .into_actions()
            .into_iter()
            .filter_map(|a| match a {
                SearchAction::QueryRequested(r) => Some(r),
                _ => None,
            })
            .collect()
    };

    let first = requests(search.refresh_source(stamp));
    assert_eq!(first.len(), 1);
    let source = Catalog {
        stamp: first[0].token.result_stamp(),
        len: N,
    };
    let (accepted, built) = measure(|| search.submit(first[0].token, &source));
    accepted.expect("accepted").into_actions();
    assert_eq!(built.get(Metric::ProjectionRebuilds), 1, "{:?}", nz(&built));
    assert!(
        built.get(Metric::SourceRowsEnumerated) >= N as u64,
        "the real build is counted: {:?}",
        nz(&built)
    );

    let visible = paint(&mut search, &source);
    assert!(
        visible > 0 && visible < 40,
        "only the viewport paints: {visible}"
    );

    let mut painted = 0;
    let ((), idle) = measure(|| {
        for _ in 0..50 {
            assert!(requests(search.refresh_source(stamp)).is_empty());
            assert!(requests(search.set_query("")).is_empty());
            painted += paint(&mut search, &source);
        }
    });
    assert_eq!(idle.get(Metric::ProjectionRebuilds), 0, "{:?}", nz(&idle));
    assert_eq!(idle.get(Metric::SourceRowsEnumerated), 0, "{:?}", nz(&idle));
    assert_eq!(idle.get(Metric::KeyIndexInserts), 0, "{:?}", nz(&idle));
    assert_eq!(idle.get(Metric::KeyLookups), 0, "{:?}", nz(&idle));
    assert_eq!(idle.get(Metric::WidthRowsMeasured), 0, "{:?}", nz(&idle));
    assert_eq!(painted, 50 * visible);
    assert_eq!(
        idle.get(Metric::VisibleRowsPainted),
        painted as u64,
        "paint work is the visible rows only: {:?}",
        nz(&idle)
    );

    let changed = requests(search.set_query("a"));
    assert_eq!(changed.len(), 1, "a real change requests once");
}
