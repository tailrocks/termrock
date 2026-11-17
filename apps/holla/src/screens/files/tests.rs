use super::*;
use crate::alloc_probe::allocated_by;
use crate::domain::fixtures::world_for;
use crate::scenario::{Motion, Scenario};
use junie_tui::core::focus::{Focus, FocusRing};
use junie_tui::core::hit::HitRegistry;
use junie_tui::theme::Theme;
use junie_tui::ui::ctx::Interaction;

const AREA: Rect = Rect::new(0, 0, 160, 40);
const MIB: usize = 1024 * 1024;
const CAP: usize = crate::sim::fs::PREVIEW_MAX_BYTES;

fn large_text(total: usize) -> String {
    let unit = "the same visible prefix of a large file\n";
    let mut text = unit.repeat(total / unit.len() + 1);
    text.truncate(total);
    text
}

fn screen_text(buf: &Buffer) -> String {
    buf.content().iter().map(|c| c.symbol()).collect()
}

fn page_on(w: &mut World, dir: &str, file: &str) -> FilesPage {
    let mut page = FilesPage::new(dir, None, w);
    let mut focus = Focus::default();
    let ring = FocusRing::default();
    let mut cx = Cx::new(&mut focus, &ring);
    page.enter(w, &mut cx);
    page.cursor = page
        .visible()
        .iter()
        .position(|e| e.path == file)
        .expect("row is listed");
    page
}

/// Draws the page into `buf` and returns the bytes the allocator served for it.
fn render_measured(page: &mut FilesPage, w: &World, buf: &mut Buffer) -> u64 {
    let theme = Theme::junie();
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    let ((), requested) = allocated_by(|| page.render(AREA, buf, &mut ctx, w));
    requested
}

#[test]
fn files_preview_large_selection_uses_bounded_prefix() {
    let mut w = world_for(Scenario::FirstUse, Motion::Full);
    let dir = w.location.cwd.clone();
    assert!(w.fs.is_dir(&dir), "the scenario has its working directory");

    let big = Fs::join(&dir, "huge.log");
    w.fs.text(&big, &large_text(16 * MIB), 0);
    let medium = Fs::join(&dir, "medium.log");
    w.fs.text(&medium, &large_text(MIB), 0);

    let mut big_page = page_on(&mut w, &dir, &big);
    let mut buf = Buffer::empty(AREA);
    let big_requested = render_measured(&mut big_page, &w, &mut buf);
    let text = screen_text(&buf);
    assert!(text.contains("first 256 KiB"), "truncation is shown");
    assert!(
        text.contains("the same visible prefix of a large file"),
        "the first lines are shown"
    );

    let mut medium_page = page_on(&mut w, &dir, &medium);
    let mut medium_buf = Buffer::empty(AREA);
    let medium_requested = render_measured(&mut medium_page, &w, &mut medium_buf);

    assert!(
        big_requested.abs_diff(medium_requested) < 1024,
        "rendering a 16 MiB selection requests about the same bytes as a 1 MiB one \
         (16 MiB {big_requested} B, 1 MiB {medium_requested} B; only the metadata text differs)"
    );

    big_page.preview_for = None;
    let ((), prepare_requested) = allocated_by(|| big_page.load_preview(&big, &w));
    assert!(
        prepare_requested < 4 * CAP as u64,
        "{prepare_requested} B requested to prepare a 16 MiB selection, expected under 4 x the cap"
    );

    let tiny = Fs::join(&dir, "tiny.txt");
    w.fs.text(&tiny, "tiny\n", 0);
    let mut tiny_page = page_on(&mut w, &dir, &tiny);
    let mut tiny_buf = Buffer::empty(AREA);
    render_measured(&mut tiny_page, &w, &mut tiny_buf);
    let tiny_text = screen_text(&tiny_buf);
    assert!(tiny_text.contains("tiny"), "the small row is previewed");
    assert!(
        !tiny_text.contains("first 256 KiB"),
        "a small row shows no truncation notice"
    );
}
