mod common;

#[allow(dead_code)]
#[path = "../src/bin/showcase/coverage.rs"]
mod coverage;

use common::*;
use junie_tui::core::focus::{Focus, FocusRing};
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::{DismissReason, Phase};
use junie_tui::core::pointer::PointerEvent;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::layer::{Layer, LayerCx, LayerOut};
use junie_tui::widgets::diff::{DiffFile, DiffHunk, DiffLine, DiffStatus, UnreadFile};
use junie_tui::widgets::inspect_changes::{ChangeSet, InspectChanges, InspectMode, InspectOutcome};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};

const ID: WidgetId = WidgetId::of("t.inspect");
const SCREEN: Rect = Rect::new(0, 0, 120, 36);

#[derive(Debug, PartialEq, Eq)]
struct Out(InspectOutcome);

impl From<InspectOutcome> for Out {
    fn from(o: InspectOutcome) -> Self {
        Self(o)
    }
}

fn file(path: &str) -> DiffFile {
    DiffFile {
        path: path.into(),
        status: DiffStatus::Modified,
        hunks: vec![DiffHunk {
            old_start: 1,
            new_start: 1,
            lines: vec![DiffLine::context("same"), DiffLine::add("new line")],
        }],
    }
}

fn unread(path: &str) -> UnreadFile {
    UnreadFile {
        path: path.into(),
        status: DiffStatus::Modified,
    }
}

fn changes(files: Vec<DiffFile>, unread: Vec<UnreadFile>) -> ChangeSet {
    ChangeSet {
        files,
        unpushed: 0,
        omitted: 0,
        unread,
    }
}

fn render(ic: &mut InspectChanges) -> Buffer {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(SCREEN);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    Layer::<Out, ()>::render(ic, SCREEN, &mut buf, &mut ctx, &());
    buf
}

fn screen_text(buf: &Buffer) -> String {
    (0..buf.area.height)
        .map(|y| row(buf, y))
        .collect::<Vec<_>>()
        .join("\n")
}

fn closed_with(ic: &mut InspectChanges) -> Option<InspectOutcome> {
    match Layer::<Out, ()>::take_result(ic)? {
        LayerOut::Close(Out(o)) | LayerOut::Event(Out(o)) => Some(o),
    }
}

#[test]
fn the_summary_counts_unread_files_that_the_list_shows() {
    let set = changes(vec![], vec![unread("a.bin"), unread("b.bin")]);

    assert_eq!(set.summary(), "2 files · +0 −0");
    assert_eq!(
        changes(vec![file("a.rs")], vec![unread("b.bin")]).summary(),
        "2 files · +1 −0"
    );
}

#[test]
fn the_advanced_header_names_the_key_that_switches_the_diff() {
    let mut ic = InspectChanges::new(
        ID,
        "Changes",
        changes(vec![file("a.rs")], vec![]),
        InspectMode::Advanced,
    );
    let text = screen_text(&render(&mut ic));
    assert!(text.contains("unified · v switches"), "{text}");
    assert!(!text.contains("d switches"));

    let mut focus = Focus::default();
    let ring = FocusRing::default();
    let mut cx = LayerCx {
        focus: &mut focus,
        ring: &ring,
        app: &(),
    };
    Layer::<Out, ()>::on_key(&mut ic, &key(KeyCode::Char('v')), &mut cx);
    let text = screen_text(&render(&mut ic));
    assert!(text.contains("review · v switches"), "{text}");
}

fn opened(returns_to_exit: bool) -> InspectChanges {
    let mut ic = InspectChanges::new(
        ID,
        "Changes",
        changes(vec![file("a.rs")], vec![]),
        InspectMode::Compact,
    )
    .returns_to_exit(returns_to_exit);
    render(&mut ic);
    ic
}

#[test]
fn a_dismissal_resolves_like_leaving_with_escape() {
    for (returns_to_exit, expected) in
        [(true, InspectOutcome::Back), (false, InspectOutcome::Close)]
    {
        let mut dismissed = opened(returns_to_exit);
        Layer::<Out, ()>::on_dismiss(&mut dismissed, DismissReason::OutsideClick);

        let mut escaped = opened(returns_to_exit);
        let mut focus = Focus::default();
        let ring = FocusRing::default();
        let mut cx = LayerCx {
            focus: &mut focus,
            ring: &ring,
            app: &(),
        };
        Layer::<Out, ()>::on_key(&mut escaped, &key(KeyCode::Esc), &mut cx);

        assert_eq!(closed_with(&mut dismissed), Some(expected));
        assert_eq!(closed_with(&mut escaped), Some(expected));
    }
}

fn drawn(ic: &mut InspectChanges) -> (Buffer, HitRegistry) {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(SCREEN);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    Layer::<Out, ()>::render(ic, SCREEN, &mut buf, &mut ctx, &());
    (buf, hits)
}

fn thumb(buf: &Buffer) -> (u16, u16, u16) {
    let thumb = junie_tui::widgets::scrollbar::THUMB;
    for x in 0..buf.area.width {
        let rows: Vec<u16> = (0..buf.area.height)
            .filter(|&y| buf[(x, y)].symbol() == thumb)
            .collect();
        if rows.len() >= 2 {
            return (x, rows[0], rows.len() as u16);
        }
    }
    panic!("no scrollbar thumb");
}

fn mouse(ic: &mut InspectChanges, phase: Phase, x: u16, y: u16) {
    let (_, hits) = drawn(ic);
    let pos = Position::new(x, y);
    let id = hits.hit(pos).expect("a region under the pointer");
    let ev = PointerEvent::new(&hits, phase, id, pos, 1, KeyModifiers::NONE);
    let mut focus = Focus::default();
    let ring = FocusRing::default();
    let mut cx = LayerCx {
        focus: &mut focus,
        ring: &ring,
        app: &(),
    };
    Layer::<Out, ()>::pointer(ic, &ev, &mut cx);
}

#[test]
fn a_press_off_centre_on_the_list_thumb_drags_it_cell_for_cell() {
    let files = (0..60).map(|n| file(&format!("f{n:02}.rs"))).collect();
    let mut ic = InspectChanges::new(ID, "Changes", changes(files, vec![]), InspectMode::Compact);
    let (x, start, len) = thumb(&drawn(&mut ic).0);
    assert!(len >= 4, "the thumb is long enough to grab off-centre");
    let grab = start + 1;

    mouse(&mut ic, Phase::Press, x, grab);
    assert_eq!(
        thumb(&drawn(&mut ic).0).1,
        start,
        "pressing the thumb does not move it"
    );

    mouse(&mut ic, Phase::Drag, x, grab + 1);
    assert_eq!(
        thumb(&drawn(&mut ic).0).1,
        start + 1,
        "one cell of drag moves the thumb one cell"
    );
}

fn many_files(n: usize) -> ChangeSet {
    changes(
        (0..n)
            .map(|i| file(&format!("src/a/very/long/path/file{i}.rs")))
            .collect(),
        vec![unread("zz.bin")],
    )
}

fn render_in(ic: &mut InspectChanges, outer: Rect, screen: Rect) -> Painted {
    paint_in(outer, Some(ID), |buf, ctx| {
        Layer::<Out, ()>::render(ic, screen, buf, ctx, &());
    })
}

#[test]
fn inspect_composite_tiny_render_has_no_off_parent_hits() {
    let outer = Rect::new(0, 0, 140, 40);
    for mode in [InspectMode::Advanced, InspectMode::Compact] {
        for w in [0u16, 1, 5, 12, 30, 60, 100] {
            for h in [0u16, 1, 2, 3, 4, 5, 6, 8, 12, 24] {
                let screen = Rect::new(5, 3, w, h);
                let mut ic = InspectChanges::new(ID, "Changes", many_files(3), mode);
                let p = render_in(&mut ic, outer, screen);
                let what = format!("{mode:?} screen {screen:?}");
                let frame = ic.area;
                for y in outer.y..outer.bottom() {
                    for x in outer.x..outer.right() {
                        if !frame.contains((x, y).into()) {
                            assert_eq!(
                                p.buf[(x, y)].symbol(),
                                "#",
                                "{what}: cell ({x},{y}) outside the frame {frame:?} was written"
                            );
                        }
                    }
                }
                for r in p.hits.regions() {
                    if r.owner == junie_tui::ui::modal::BACKDROP {
                        continue;
                    }
                    assert!(
                        contains(frame, r.area),
                        "{what}: hit {:?} escapes {frame:?}",
                        r.area
                    );
                }
            }
        }
    }
}

fn path_in(text: &str, name: &str) -> bool {
    text.contains(name)
}

#[test]
fn inspect_composite_shrink_grow_retains_selection() {
    let outer = Rect::new(0, 0, 140, 40);
    let mut ic = InspectChanges::new(ID, "Changes", many_files(3), InspectMode::Advanced);
    render_in(&mut ic, outer, SCREEN);
    let mut focus = Focus::default();
    let ring = FocusRing::default();
    {
        let mut cx = LayerCx {
            focus: &mut focus,
            ring: &ring,
            app: &(),
        };
        for _ in 0..2 {
            Layer::<Out, ()>::on_key(&mut ic, &key(KeyCode::Down), &mut cx);
        }
    }
    let before = screen_text(&render_in(&mut ic, outer, SCREEN).buf);
    assert!(path_in(&before, "file2.rs"), "{before}");

    let tiny = render_in(&mut ic, outer, Rect::new(5, 3, 40, 3));
    for r in tiny.hits.regions() {
        assert!(
            r.owner == junie_tui::ui::modal::BACKDROP || contains(ic.area, r.area),
            "a hit survives outside the shrunk frame: {:?}",
            r.area
        );
    }
    assert_eq!(
        ic.mode,
        InspectMode::Advanced,
        "the mode survives the shrink"
    );

    let after = screen_text(&render_in(&mut ic, outer, SCREEN).buf);
    assert!(
        path_in(&after, "file2.rs"),
        "the selected file survives grow: {after}"
    );
}

#[test]
fn inspect_composite_stacked_render_stays_in_frame() {
    let outer = Rect::new(0, 0, 140, 40);
    for h in 0..=14u16 {
        let screen = Rect::new(2, 2, 60, h);
        let mut ic = InspectChanges::new(ID, "Changes", many_files(3), InspectMode::Advanced);
        let p = render_in(&mut ic, outer, screen);
        let frame = ic.area;
        assert!(
            contains(screen, frame),
            "h={h}: frame {frame:?} in {screen:?}"
        );
        for y in outer.y..outer.bottom() {
            for x in outer.x..outer.right() {
                if !frame.contains((x, y).into()) {
                    assert_eq!(p.buf[(x, y)].symbol(), "#", "h={h}: ({x},{y})");
                }
            }
        }
    }
}

#[test]
fn inspect_composite_stacked_body_of_one_row_stays_in_the_frame() {
    let outer = Rect::new(0, 0, 140, 40);
    for height in [5u16, 6, 7, 8, 9] {
        let screen = Rect::new(5, 3, 60, height);
        let mut ic = InspectChanges::new(ID, "Changes", many_files(3), InspectMode::Advanced);
        let p = render_in(&mut ic, outer, screen);
        let frame = ic.area;
        for y in outer.y..outer.bottom() {
            for x in outer.x..outer.right() {
                if !frame.contains((x, y).into()) {
                    assert_eq!(
                        p.buf[(x, y)].symbol(),
                        "#",
                        "height {height}: cell ({x},{y}) outside {frame:?} was written"
                    );
                }
            }
        }
        for r in p.hits.regions() {
            assert!(
                r.owner == junie_tui::ui::modal::BACKDROP || contains(frame, r.area),
                "height {height}: hit {:?} escapes {frame:?}",
                r.area
            );
        }
    }
}

mod showcase_coverage {
    use std::collections::{BTreeMap, BTreeSet};

    use syn::{ImplItem, Item, Type, Visibility};

    use super::coverage;

    const MODULE: &str = "widgets::inspect_changes";

    fn exported_api(source: &str) -> BTreeSet<String> {
        let file = syn::parse_file(source).expect("inspect_changes source parses");
        let public = |vis: &Visibility| matches!(vis, Visibility::Public(_));
        let mut ids = BTreeSet::new();
        let mut types = BTreeSet::new();
        for item in &file.items {
            match item {
                Item::Enum(item) if public(&item.vis) => {
                    let root = format!("{MODULE}::{}", item.ident);
                    ids.extend(
                        item.variants
                            .iter()
                            .map(|variant| format!("{root}::{}", variant.ident)),
                    );
                    ids.insert(root);
                    types.insert(item.ident.to_string());
                }
                Item::Struct(item) if public(&item.vis) => {
                    ids.insert(format!("{MODULE}::{}", item.ident));
                    types.insert(item.ident.to_string());
                }
                Item::Fn(item) if public(&item.vis) => {
                    ids.insert(format!("{MODULE}::{}", item.sig.ident));
                }
                _ => {}
            }
        }
        for item in &file.items {
            let Item::Impl(item) = item else { continue };
            let Type::Path(path) = &*item.self_ty else {
                continue;
            };
            let Some(ty) = path.path.segments.last().map(|s| s.ident.to_string()) else {
                continue;
            };
            if item.trait_.is_some() || !types.contains(&ty) {
                continue;
            }
            for member in &item.items {
                if let ImplItem::Fn(method) = member
                    && public(&method.vis)
                {
                    ids.insert(format!("{MODULE}::{ty}::{}", method.sig.ident));
                }
            }
        }
        ids
    }

    fn catalog_ids(source: &str) -> BTreeSet<String> {
        let marker = format!("\"{MODULE}::");
        source
            .lines()
            .filter_map(|line| {
                let start = line.find(&marker)? + 1;
                let end = start + line[start..].find('"')?;
                Some(line[start..end].to_owned())
            })
            .collect()
    }

    fn words(source: &str) -> BTreeSet<&str> {
        source
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|word| !word.is_empty())
            .collect()
    }

    fn used_by_page(id: &str, page: &str, page_words: &BTreeSet<&str>) -> bool {
        let tail: Vec<&str> = id.rsplit("::").take(2).collect();
        match tail.as_slice() {
            [name, owner] if owner.chars().next().is_some_and(char::is_uppercase) => {
                page.contains(&format!("{owner}::{name}")) || page.contains(&format!(".{name}("))
            }
            [name, ..] => page_words.contains(name),
            [] => false,
        }
    }

    fn validate(widget: &str, catalog: &str, page: &str) -> Vec<String> {
        let exported = exported_api(widget);
        let cataloged = catalog_ids(catalog);
        let page_words = words(page);
        let claimed: BTreeSet<String> = coverage::INSPECT_CHANGES_COVERAGE
            .iter()
            .map(|(id, _)| (*id).to_owned())
            .collect();
        let allowed: BTreeMap<&str, &str> = coverage::INSPECT_CHANGES_ALLOWLIST
            .iter()
            .copied()
            .collect();
        let mut errors = Vec::new();
        for id in exported.difference(&cataloged) {
            errors.push(format!("exported API is missing from catalog: {id}"));
        }
        for id in cataloged.difference(&exported) {
            errors.push(format!("catalog names an API that is not exported: {id}"));
        }
        for id in &claimed {
            if !exported.contains(id) {
                errors.push(format!("coverage claims an API that is not exported: {id}"));
            } else if !used_by_page(id, page, &page_words) {
                errors.push(format!("coverage claim has no page usage: {id}"));
            }
            if allowed.contains_key(id.as_str()) {
                errors.push(format!("API is both showcased and allowlisted: {id}"));
            }
        }
        let close = format!("{MODULE}::InspectOutcome::Close");
        let always_back =
            page.contains(".returns_to_exit(true)") && !page.contains(".returns_to_exit(false)");
        if always_back && claimed.contains(&close) {
            errors.push(format!(
                "coverage claims {close}, but the page only builds returns_to_exit(true) inspectors"
            ));
        }
        for (id, reason) in &allowed {
            if reason.trim().is_empty() {
                errors.push(format!("allowlist entry has no reason: {id}"));
            }
            if !exported.contains(*id) {
                errors.push(format!("allowlist entry is not exported: {id}"));
            }
        }
        for id in &exported {
            if !claimed.contains(id) && !allowed.contains_key(id.as_str()) {
                errors.push(format!("exported API lacks coverage or a reason: {id}"));
            }
        }
        errors
    }

    fn sources() -> (String, String, String) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let read = |path: &str| {
            std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
        };
        (
            read("src/widgets/inspect_changes.rs"),
            read("src/catalog.rs"),
            read("src/bin/showcase/pages/diff.rs"),
        )
    }

    #[test]
    fn inspect_changes_api_is_cataloged_and_demonstrated_or_excused() {
        let (widget, catalog, page) = sources();
        let errors = validate(&widget, &catalog, &page);
        assert!(errors.is_empty(), "{}", errors.join("\n"));
    }

    #[test]
    fn inspect_changes_coverage_rejects_a_removed_page_demonstration() {
        let (widget, catalog, page) = sources();
        let broken = page.replace(".returns_to_exit(true)", "");
        assert_ne!(broken, page, "page mutation matched source");
        let errors = validate(&widget, &catalog, &broken);
        assert!(
            errors.iter().any(|e| e.contains(
                "no page usage: widgets::inspect_changes::InspectChanges::returns_to_exit"
            )),
            "removed demonstration unexpectedly passed: {errors:?}"
        );
    }

    #[test]
    fn inspect_changes_coverage_rejects_a_removed_catalog_line() {
        let (widget, catalog, page) = sources();
        let line = "    Capability::function(\"widgets::inspect_changes::InspectChanges::new\"),\n";
        let broken = catalog.replace(line, "");
        assert_ne!(broken, catalog, "catalog mutation matched source");
        let errors = validate(&widget, &broken, &page);
        assert!(
            errors.iter().any(|e| e
                .contains("missing from catalog: widgets::inspect_changes::InspectChanges::new")),
            "removed catalog line unexpectedly passed: {errors:?}"
        );
    }
}
