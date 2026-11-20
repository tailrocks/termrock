use std::cell::RefCell;
use std::rc::Rc;

use crate::common::key;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};
use termrock::core::focus::{Focus, FocusRing};
use termrock::core::hit::HitRegistry;
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::pointer::PointerEvent;
use termrock::theme::{ColorLevel, Theme};
use termrock::ui::ctx::{Interaction, RenderCtx};
use termrock::widgets::file_browser::{FileBrowser, FileEntry, FileSource, FsSource, MemorySource};

const ID: WidgetId = WidgetId::of("t.files");
const SCREEN: Rect = Rect::new(0, 0, 100, 30);

/// Accepts only the exact directory strings in `known` and records every request.
struct Strict {
    known: Vec<(&'static str, Vec<FileEntry>)>,
    calls: Rc<RefCell<Vec<String>>>,
}

impl Strict {
    fn new(known: Vec<(&'static str, Vec<FileEntry>)>) -> (Self, Rc<RefCell<Vec<String>>>) {
        let calls = Rc::new(RefCell::new(Vec::new()));
        (
            Self {
                known,
                calls: Rc::clone(&calls),
            },
            calls,
        )
    }
}

impl FileSource for Strict {
    fn list(&self, dir: &str) -> Result<Vec<FileEntry>, String> {
        self.calls.borrow_mut().push(dir.to_owned());
        self.known
            .iter()
            .find(|(path, _)| *path == dir)
            .map(|(_, entries)| entries.clone())
            .ok_or_else(|| format!("no such directory {dir:?}"))
    }

    fn is_dir(&self, _: &str) -> bool {
        true
    }
}

/// Uses only the trait's default `parent`, so the slash-path contract is what runs.
struct Plain;

impl FileSource for Plain {
    fn list(&self, _: &str) -> Result<Vec<FileEntry>, String> {
        Ok(vec![])
    }

    fn is_dir(&self, _: &str) -> bool {
        true
    }
}

fn render(browser: &mut FileBrowser) -> HitRegistry {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(SCREEN);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    browser.render(SCREEN, &mut buf, &mut ctx);
    hits
}

fn click_at(browser: &mut FileBrowser, hits: &HitRegistry, at: Position) {
    let id = hits.hit(at).expect("a region under the pointer");
    let ev = PointerEvent::new(hits, Phase::Click, id, at, 1, KeyModifiers::NONE);
    let mut focus = Focus::default();
    browser.pointer(&ev, &mut focus);
}

fn entry_paths(browser: &FileBrowser) -> Vec<String> {
    browser.entries.iter().map(|e| e.path.clone()).collect()
}

#[test]
fn file_browser_root_is_not_empty_source_path() {
    let (source, calls) = Strict::new(vec![("/", vec![FileEntry::dir("/a")])]);

    let browser = FileBrowser::new(ID, "Files", source, "/");

    assert_eq!(*calls.borrow(), ["/"]);
    assert_eq!(browser.error, None);
    assert_eq!(entry_paths(&browser), ["/a"]);
}

#[test]
fn file_browser_parent_from_first_level_reaches_root() {
    let source = MemorySource::new(vec![FileEntry::dir("/a"), FileEntry::file("/a/x")]);
    let mut browser = FileBrowser::new(ID, "Files", source, "/a");
    assert_eq!(entry_paths(&browser), ["/a/x"]);
    let mut focus = Focus::default();
    focus.focus(browser.list.id);
    let ring = FocusRing::default();

    browser.on_key(&key(KeyCode::Enter), &mut focus, &ring);

    assert_eq!(browser.cwd, "/");
    assert_eq!(browser.error, None);
    assert_eq!(entry_paths(&browser), ["/a"]);
}

#[test]
fn file_browser_root_breadcrumb_keeps_identity() {
    let (source, calls) = Strict::new(vec![
        ("/", vec![FileEntry::dir("/a")]),
        ("/a", vec![FileEntry::dir("/a/b")]),
        ("/a/b", vec![]),
    ]);
    let mut browser = FileBrowser::new(ID, "Files", source, "/a/b").path_bar(true);
    let hits = render(&mut browser);
    assert_eq!(
        browser.breadcrumb.as_ref().expect("path bar").segments,
        ["/", "a", "b"]
    );
    calls.borrow_mut().clear();

    let root = browser.breadcrumb.as_ref().expect("path bar").areas[0];
    click_at(&mut browser, &hits, Position::new(root.x, root.y));

    assert_eq!(*calls.borrow(), ["/"]);
    assert_eq!(browser.cwd, "/");
    assert_eq!(browser.error, None);
}

#[test]
fn file_browser_preserves_remote_trailing_separator() {
    let (source, calls) = Strict::new(vec![(
        "remote:/bucket/",
        vec![FileEntry::file("remote:/bucket/k")],
    )]);

    let browser = FileBrowser::new(ID, "Remote", source, "remote:/bucket/");

    assert_eq!(*calls.borrow(), ["remote:/bucket/"]);
    assert_eq!(browser.error, None);
    assert_eq!(entry_paths(&browser), ["remote:/bucket/k"]);
}

#[test]
fn file_source_default_parent_root_matrix() {
    let cases: [(&str, Option<&str>); 9] = [
        ("", None),
        ("/", None),
        ("//", None),
        ("/a", Some("/")),
        ("/a/", Some("/")),
        ("/a/b", Some("/a")),
        ("/a/b/", Some("/a")),
        ("a/b", Some("a")),
        ("a", None),
    ];
    for (path, expected) in cases {
        assert_eq!(
            Plain.parent(path).as_deref(),
            expected,
            "parent of {path:?}"
        );
    }
    assert_eq!(Plain.parent("/é/ß").as_deref(), Some("/é"));
    assert_eq!(Plain.parent("/ü").as_deref(), Some("/"));
}

#[cfg(unix)]
#[test]
fn file_browser_fs_source_opens_the_filesystem_root() {
    let browser = FileBrowser::new(ID, "Disk", FsSource::new(), "/");

    assert_eq!(browser.cwd, "/");
    assert_eq!(browser.error, None);
}
