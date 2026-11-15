//! Deterministic work counts for result projections and paint.
//!
//! Counters are work units from `core::work` (unit tests only), never timings.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::{QueryRequest, ResultRowKind, SearchAction, SearchConfig, SearchDetail, SearchSource};
use crate::core::focus::FocusRing;
use crate::core::hit::HitRegistry;
use crate::core::id::{ItemKey, WidgetId};
use crate::core::list::{KeyedRowSource, RowSource, SourceStamp};
use crate::core::work::Work;
use crate::theme::{ColorLevel, Theme};
use crate::ui::ctx::{Interaction, RenderCtx};
use crate::ui::layout::Split;
use crate::widgets::input::TextInput;
use crate::widgets::list_detail::SourceFrame;

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
    let requests = |reply: crate::core::response::Response<SearchAction>| -> Vec<QueryRequest> {
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
    let (accepted, built) = Work::measure(|| search.submit(first[0].token, &source));
    accepted.expect("accepted").into_actions();
    assert_eq!(built.projection_rebuilds, 1, "{built:?}");
    assert!(
        built.source_rows >= N as u64,
        "the real build is counted: {built:?}"
    );

    let visible = paint(&mut search, &source);
    assert!(
        visible > 0 && visible < 40,
        "only the viewport paints: {visible}"
    );

    let mut painted = 0;
    let ((), idle) = Work::measure(|| {
        for _ in 0..50 {
            assert!(requests(search.refresh_source(stamp)).is_empty());
            assert!(requests(search.set_query("")).is_empty());
            painted += paint(&mut search, &source);
        }
    });
    assert_eq!(idle.projection_rebuilds, 0, "{idle:?}");
    assert_eq!(idle.source_rows, 0, "{idle:?}");
    assert_eq!(idle.index_builds, 0, "{idle:?}");
    assert_eq!(idle.join_probes, 0, "{idle:?}");
    assert_eq!(idle.width_rows, 0, "{idle:?}");
    assert_eq!(painted, 50 * visible);
    assert_eq!(
        idle.visible_rows_painted, painted as u64,
        "paint work is the visible rows only: {idle:?}"
    );

    let changed = requests(search.set_query("a"));
    assert_eq!(changed.len(), 1, "a real change requests once");
}
