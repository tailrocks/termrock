use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};

use crate::app::PageId;
use crate::options::{PageValues, page_options};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent, Report};
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::keymap::{Actions, label_for};
use junie_tui::core::pointer::MouseTarget;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layer::LayerTag;
use junie_tui::ui::layers::LayerResult;
use junie_tui::ui::layout::StackChild;
use junie_tui::ui::popup::Dismiss;
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::ui::text::truncate;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::choice::Checkbox;
use junie_tui::widgets::file_browser::{
    Choose, FileBrowser, FileBrowserResult, FileCmd, FileEntry, FileSource, FsSource, MemorySource,
    UrlEntry,
};
use junie_tui::widgets::panel::Panel;

const ID: WidgetId = WidgetId::of("filebrowser");
const HOME: &str = "/Users/ada";

fn fixture() -> MemorySource {
    let d = |p: &str, meta: &str| FileEntry::dir(&format!("{HOME}{p}")).meta(meta);
    let f = |p: &str, meta: &str| FileEntry::file(&format!("{HOME}{p}")).meta(meta);
    MemorySource::new(vec![
        FileEntry::dir("/Users").meta("1 item"),
        FileEntry::dir(HOME),
        d("/src", "4 items"),
        d("/design", "2 items"),
        d("/notes", "3 items"),
        d("/.config", "settings"),
        d("/.ssh", "keys"),
        f("/.zshrc", "1.2 KB"),
        f("/todo.md", "2.0 KB"),
        d("/src/payments-platform", "git · main"),
        d("/src/payments-platform/crates", "6 items"),
        d("/src/payments-platform/docs", "adr"),
        f("/src/payments-platform/Cargo.toml", "1.1 KB"),
        f("/src/payments-platform/README.md", "4.3 KB"),
        f("/src/payments-platform/.env", "212 B"),
        d("/src/infra-control-plane", "git · main"),
        d("/src/infra-control-plane/modules", "terraform"),
        d("/src/customer-portal", "git · develop"),
        d("/src/customer-portal/web", "next.js"),
        d("/src/release-automation", "git · main"),
        d("/design/tokens", "json"),
        f("/design/brand.fig", "18 MB"),
        f("/notes/standup.md", "3 KB"),
        f("/notes/ideas.md", "1 KB"),
        f("/notes/oncall.md", "6 KB"),
    ])
    .home(HOME)
}

const INTRO: [&str; 2] = [
    "The folder mounted into the workspace. Browse… opens the modal over this page;",
    "Esc, the × or (with Dismiss light) a click outside cancels it.",
];
const NEW_META: &str = "FileBrowser on the layer stack · LayerResult::File";
const SOURCE_HEAD: &str =
    "FileEntry rows of ~ (folders first in the browser; hidden ones wait for the toggle):";
const RESULTS_META: &str = "FileBrowserResult: Chosen { path, checked } · Url · Cancelled";

const REPOS: [&str; 2] = ["acme/payments-platform", "acme/customer-portal"];

pub fn page() -> Box<dyn Page> {
    Box::new(FileBrowserPage::new())
}

pub struct FileBrowserPage {
    template: Template,
    opts: PageValues,
    browse: Button,

    pub(crate) chosen: String,

    pub(crate) log: Vec<String>,
    sections: crate::pages::layout::Sections,
}

impl FileBrowserPage {
    pub fn new() -> Self {
        Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::FileBrowser)),
            browse: Button::primary(ID.sub("browse"), "Browse…"),
            chosen: "none yet".into(),
            log: vec![],
            sections: crate::pages::layout::Sections::new(ID.sub("page")),
        }
    }

    fn choose_kind(&self) -> Choose {
        match self.opts.value("filebrowser.choose") {
            "files" => Choose::Files,
            "any" => Choose::Any,
            _ => Choose::Dirs,
        }
    }

    fn keymap(&self) -> Vec<(junie_tui::core::keymap::KeyBinding, FileCmd)> {
        if self.opts.value("filebrowser.keys") == "vim" {
            FileCmd::vim_keymap()
        } else {
            FileCmd::default_keymap()
        }
    }

    fn browser(&self) -> FileBrowser {
        let title = match self.choose_kind() {
            Choose::Dirs => "Choose a folder",
            Choose::Files => "Open a file",
            Choose::Any => "Choose a file or folder",
        };
        let start = format!("{HOME}/src");
        let mut b = if self.opts.value("filebrowser.source") == "real" {
            let fs = FsSource::new();
            let start = fs.home().unwrap_or_else(|| "/".into());
            FileBrowser::new(ID.sub("browser"), title, fs, &start)
        } else {
            FileBrowser::new(ID.sub("browser"), title, fixture(), &start)
        };
        b = b
            .choose(self.choose_kind())
            .path_bar(self.opts.on("filebrowser.path_bar"))
            .filter(self.opts.on("filebrowser.filter"))
            .show_hidden(self.opts.on("filebrowser.show_hidden"))
            .hidden_toggle(self.opts.on("filebrowser.hidden_toggle"))
            .type_ahead(self.opts.on("filebrowser.type_ahead"))
            .keys(self.keymap())
            .dismiss(match self.opts.value("filebrowser.dismiss") {
                "light" => Dismiss::LIGHT,
                "never" => Dismiss::NEVER,
                _ => Dismiss::ESCAPE,
            });
        if self.opts.on("filebrowser.checkbox") {
            b = b.checkbox("Mount read-only", false);
        }
        if self.opts.on("filebrowser.url_entry") {
            b = b.url_entry(
                UrlEntry::new("Git URL…", "Git URL")
                    .back("Browse folders")
                    .placeholder("github.com/org/repo")
                    .help(&[
                        "Enter a GitHub repository URL; it is cloned at launch.",
                        "Known here: github.com/acme/payments-platform, acme/customer-portal.",
                    ])
                    .hint("Git URL")
                    .empty_error("Enter a repository URL")
                    .validate(|url| {
                        if REPOS.iter().any(|r| url.contains(r)) {
                            Ok(())
                        } else {
                            Err(format!("Could not resolve {url}: repository not found"))
                        }
                    }),
            );
        }
        if self.opts.on("filebrowser.stepper") {
            b = b.stepper("✓ Source · ● Folder · ○ Agent · ○ Review");
        }
        b
    }

    const CMDS: [FileCmd; 7] = [
        FileCmd::Open,
        FileCmd::Parent,
        FileCmd::Choose,
        FileCmd::ToggleHidden,
        FileCmd::GoTo,
        FileCmd::Filter,
        FileCmd::Url,
    ];

    fn source_rows() -> Vec<String> {
        let home = fixture().list(HOME).unwrap_or_default();
        home.iter()
            .map(|e| {
                let kind = if e.dir { "folder" } else { "file" };
                let hidden = if e.hidden { "hidden" } else { "" };
                format!("{:<12} {:<7} {:<10} {hidden}", e.name(), kind, e.meta)
            })
            .collect()
    }

    fn key_rows(&self) -> Vec<String> {
        let keys = self.keymap();
        Self::CMDS
            .iter()
            .map(|c| {
                let k = label_for(&keys, *c).unwrap_or_else(|| "unbound".into());
                format!("{k:<10} {}", c.label())
            })
            .collect()
    }

    fn source_meta() -> String {
        format!("MemorySource · home {HOME} · FsSource reads the real disk")
    }

    fn keys_meta(&self) -> String {
        format!("FileCmd · {} key map", self.opts.value("filebrowser.keys"))
    }

    fn children(&self) -> [StackChild; 4] {
        [
            StackChild::fixed(9),
            StackChild::fixed(self.log.len().clamp(1, 4) as u16 + 3),
            StackChild::fixed(Self::source_rows().len() as u16 + 5),
            StackChild::fixed(Self::CMDS.len() as u16 + 3),
        ]
    }

    fn open(&mut self, cx: &mut Cx) {
        cx.open(self.browser(), LayerTag::new("browser"));
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let focus = ctx.interaction.has_focus(self.browse.id).then_some(0);
        let l = self
            .sections
            .layout(area, &self.children(), focus, buf, ctx);
        let t = ctx.theme;
        let line = |buf: &mut Buffer, x: u16, y: u16, w: u16, s: &str, style| {
            buf.set_string(x, y, truncate(s, w as usize), style);
        };
        match l.slot(0) {
            Some(r) => {
                let panel = Panel::card(Some("New mount")).meta(NEW_META);
                let bg = panel.bg(t);
                let inner = panel.render(r, buf, t);
                let w = inner.width;
                line(buf, inner.x, inner.y, w, INTRO[0], t.secondary().bg(bg));
                line(buf, inner.x, inner.y + 1, w, INTRO[1], t.secondary().bg(bg));
                line(
                    buf,
                    inner.x,
                    inner.y + 3,
                    w,
                    &format!("Host folder   {}", self.chosen),
                    t.primary().bg(bg),
                );
                let y = inner.y + 5;
                if y < inner.bottom() {
                    let bw = self.browse.width();
                    self.browse
                        .render(Rect::new(inner.x, y, bw, 1), buf, ctx, bg);
                }
            }
            None => ctx.focus_stop(self.browse.id),
        }
        if let Some(r) = l.slot(2) {
            let meta = Self::source_meta();
            let panel = Panel::card(Some("Source")).meta(&meta);
            let bg = panel.bg(t);
            let inner = panel.render(r, buf, t);
            line(
                buf,
                inner.x,
                inner.y,
                inner.width,
                SOURCE_HEAD,
                t.secondary().bg(bg),
            );
            for (i, row) in Self::source_rows().iter().enumerate() {
                let y = inner.y + 2 + i as u16;
                if y >= inner.bottom() {
                    break;
                }
                line(buf, inner.x, y, inner.width, row, t.primary().bg(bg));
            }
        }
        if let Some(r) = l.slot(3) {
            let meta = self.keys_meta();
            let panel = Panel::card(Some("Keys")).meta(&meta);
            let bg = panel.bg(t);
            let inner = panel.render(r, buf, t);
            for (i, row) in self.key_rows().iter().enumerate() {
                let y = inner.y + i as u16;
                if y >= inner.bottom() {
                    break;
                }
                line(buf, inner.x, y, inner.width, row, t.primary().bg(bg));
            }
        }
        if let Some(r) = l.slot(1) {
            let panel = Panel::card(Some("Results")).meta(RESULTS_META);
            let bg = panel.bg(t);
            let inner = panel.render(r, buf, t);
            if self.log.is_empty() {
                line(
                    buf,
                    inner.x,
                    inner.y,
                    inner.width,
                    "none yet",
                    t.muted().bg(bg),
                );
            }
            for (i, s) in self.log.iter().rev().take(4).enumerate() {
                line(
                    buf,
                    inner.x,
                    inner.y + i as u16,
                    inner.width,
                    s,
                    t.primary().bg(bg),
                );
            }
        }
    }

    fn on_result(&mut self, r: FileBrowserResult, cx: &mut Cx) {
        let fixture = fixture();
        let entry = match &r {
            FileBrowserResult::Chosen { path, checked } => {
                let shown = fixture.display(path);
                self.chosen = if *checked {
                    format!("{shown} (read-only)")
                } else {
                    shown.clone()
                };
                cx.status(format!("Chose {shown}"));
                format!("Chosen {{ path: {shown}, checked: {checked} }}")
            }
            FileBrowserResult::Url(u) => {
                self.chosen = u.clone();
                cx.status(format!("Will clone {u}"));
                format!("Url({u})")
            }
            FileBrowserResult::Cancelled => "Cancelled".into(),
        };
        self.log.push(entry);
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let b = FileBrowser::new(ID.sub("sample"), "Choose", fixture(), HOME);
    StateGrid::new(ID.sub("states"))
        .label_width(11)
        .column_width(30)
        .narrow_width(16)
        .rows(&[Rest, Hover, Focused, Selected, Disabled, Empty, Narrow])
        .column("Listing", b.list.clone())
        .column(
            "Hidden toggle",
            Checkbox::new(ID.sub("hidden"), "Show hidden files", false),
        )
}

impl Page for FileBrowserPage {
    fn title(&self) -> &'static str {
        "File browser"
    }
    fn blurb(&self) -> &'static str {
        "FileBrowser: choose a file or folder from a FileSource, with a path bar, filter, type-ahead and hidden files"
    }

    fn measure(&self, _avail: Size, _ctx: &RenderCtx) -> Size {
        let w = |s: &str| junie_tui::ui::text::width(s) as u16;
        let widest = |rows: &[String]| rows.iter().map(|r| w(r)).max().unwrap_or(0);
        let host = format!("Host folder   {}", self.chosen);
        let new = INTRO
            .iter()
            .map(|s| w(s))
            .chain([w(&host), self.browse.width()]);
        let keys_meta = self.keys_meta();
        let source_meta = Self::source_meta();
        let mut log = self.log.clone();
        log.push("none yet".into());
        let width = [
            Panel::card(Some("New mount"))
                .meta(NEW_META)
                .outer(Size::new(new.max().unwrap_or(0), 0)),
            Panel::card(Some("Results"))
                .meta(RESULTS_META)
                .outer(Size::new(widest(&log), 0)),
            Panel::card(Some("Source"))
                .meta(&source_meta)
                .outer(Size::new(
                    w(SOURCE_HEAD).max(widest(&Self::source_rows())),
                    0,
                )),
            Panel::card(Some("Keys"))
                .meta(&keys_meta)
                .outer(Size::new(widest(&self.key_rows()), 0)),
        ]
        .iter()
        .map(|s| s.width)
        .max()
        .unwrap_or(0);
        Size::new(width, self.sections.stack.measure(&self.children()))
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => self.render_playground(body, buf, ctx),
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() == Section::Playground
            && let Some(o) = self.sections.handle(ev)
        {
            return o;
        }
        match ev {
            PageEvent::Key(key) if cx.focus.is(self.browse.id) => {
                let (o, act) = self.browse.on_key(key);
                if act {
                    self.open(cx);
                }
                o
            }
            PageEvent::Pointer(ev) => {
                if let Some(r) = self.browse.route(ev)
                    && r.outcome() != Outcome::Ignored
                {
                    if r.into_action().is_some() {
                        self.open(cx);
                    }
                    return Outcome::Changed;
                }
                Outcome::Ignored
            }
            _ => Outcome::Ignored,
        }
    }

    fn on_layer(&mut self, _tag: &LayerTag, report: Report, cx: &mut Cx) -> Outcome {
        if let Some(LayerResult::File(r)) = report.result {
            self.on_result(r, cx);
        }
        Outcome::Changed
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        vec![("Enter", "Browse…")]
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        self.opts.set(key, value);
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }
}
