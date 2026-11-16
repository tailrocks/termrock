use std::rc::Rc;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

use crate::core::event::{Key, Outcome};
use crate::core::focus::{Focus, FocusChange, FocusRing};
use crate::core::hit::Axis;
use crate::core::id::{Part, WidgetId};
use crate::core::intent::{DismissReason, Phase};
use crate::core::keymap::{Actions, KeyBinding, KeyMap, hint_key, lookup};
use crate::core::pointer::{MouseTarget, PointerEvent, scroll_input};
use crate::core::response::Response;
use crate::core::scroll::Scrollable;
use crate::ui::ctx::RenderCtx;
use crate::ui::layer::{LayerSpec, Overlay};
use crate::ui::popup::{Dismiss, Placement};
use crate::ui::text::truncate;
use crate::widgets::breadcrumb::Breadcrumb;
use crate::widgets::button::{Button, row_layout_right};
use crate::widgets::choice::Checkbox;
use crate::widgets::input::{InputAction, TextInput};
use crate::widgets::keyhint::{Hint, hint};
use crate::widgets::list::{ListBox, ListItem, SelectMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,

    pub dir: bool,

    pub meta: String,

    pub hidden: bool,
}

impl FileEntry {
    fn new(path: &str, dir: bool) -> Self {
        let name = path.rsplit('/').next().unwrap_or(path);
        Self {
            path: path.to_owned(),
            dir,
            meta: String::new(),
            hidden: name.starts_with('.'),
        }
    }

    pub fn dir(path: &str) -> Self {
        Self::new(path, true)
    }

    pub fn file(path: &str) -> Self {
        Self::new(path, false)
    }

    pub fn meta(mut self, meta: &str) -> Self {
        self.meta = meta.to_owned();
        self
    }

    pub fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

pub trait FileSource {
    fn list(&self, dir: &str) -> Result<Vec<FileEntry>, String>;

    fn is_dir(&self, path: &str) -> bool;

    fn home(&self) -> Option<String> {
        None
    }

    /// Lexical slash-path parent. A trailing separator is ignored, `/a` climbs to
    /// `/`, and `/` has no parent. Providers that resolve paths themselves
    /// (filesystems, URIs) override this.
    fn parent(&self, path: &str) -> Option<String> {
        let normalized = path.trim_end_matches('/');
        if normalized.is_empty() {
            return None;
        }
        let (parent, _) = normalized.rsplit_once('/')?;
        if parent.is_empty() {
            normalized.starts_with('/').then(|| "/".to_owned())
        } else {
            Some(parent.to_owned())
        }
    }

    fn display(&self, path: &str) -> String {
        match self.home() {
            Some(home) => match path.strip_prefix(home.as_str()) {
                Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("~{rest}"),
                _ => path.to_owned(),
            },
            None => path.to_owned(),
        }
    }

    fn expand(&self, typed: &str) -> String {
        match (typed.strip_prefix('~'), self.home()) {
            (Some(rest), Some(home)) => format!("{home}{rest}"),
            _ => typed.to_owned(),
        }
    }
}

impl<S: FileSource + ?Sized> FileSource for Box<S> {
    fn list(&self, dir: &str) -> Result<Vec<FileEntry>, String> {
        (**self).list(dir)
    }

    fn is_dir(&self, path: &str) -> bool {
        (**self).is_dir(path)
    }

    fn home(&self) -> Option<String> {
        (**self).home()
    }

    fn parent(&self, path: &str) -> Option<String> {
        (**self).parent(path)
    }

    fn display(&self, path: &str) -> String {
        (**self).display(path)
    }

    fn expand(&self, typed: &str) -> String {
        (**self).expand(typed)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemorySource {
    pub entries: Vec<FileEntry>,
    pub home: Option<String>,
}

impl MemorySource {
    pub fn new(entries: Vec<FileEntry>) -> Self {
        Self {
            entries,
            home: None,
        }
    }

    pub fn home(mut self, home: &str) -> Self {
        self.home = Some(home.to_owned());
        self
    }
}

impl FileSource for MemorySource {
    fn list(&self, dir: &str) -> Result<Vec<FileEntry>, String> {
        let dir = dir.trim_end_matches('/');
        Ok(self
            .entries
            .iter()
            .filter(|e| e.path.rsplit_once('/').is_some_and(|(p, _)| p == dir))
            .cloned()
            .collect())
    }
    fn is_dir(&self, path: &str) -> bool {
        self.entries.iter().any(|e| e.dir && e.path == path)
    }
    fn home(&self) -> Option<String> {
        self.home.clone()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FsSource {
    pub home: Option<String>,
}

impl FsSource {
    pub fn new() -> Self {
        Self {
            home: std::env::var("HOME").ok().filter(|h| !h.is_empty()),
        }
    }
}

impl FileSource for FsSource {
    fn list(&self, dir: &str) -> Result<Vec<FileEntry>, String> {
        let read = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for entry in read.flatten() {
            let path = entry.path().to_string_lossy().into_owned();

            let meta = std::fs::metadata(entry.path()).ok();
            let is_dir = meta.as_ref().is_some_and(std::fs::Metadata::is_dir);
            let mut e = FileEntry::new(&path, is_dir);
            if let Some(m) = meta.filter(|m| m.is_file()) {
                e.meta = crate::ui::text::bytes(m.len());
            }
            out.push(e);
        }
        Ok(out)
    }
    fn is_dir(&self, path: &str) -> bool {
        std::path::Path::new(path).is_dir()
    }
    fn home(&self) -> Option<String> {
        self.home.clone()
    }
    fn parent(&self, path: &str) -> Option<String> {
        std::path::Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Choose {
    #[default]
    Dirs,

    Files,

    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileCmd {
    Open,

    Parent,

    Choose,

    ToggleHidden,

    GoTo,

    Filter,

    Url,
}

impl Actions for FileCmd {
    fn default_keymap() -> KeyMap<Self> {
        vec![
            (KeyBinding::key(KeyCode::Enter), FileCmd::Open),
            (KeyBinding::key(KeyCode::Right), FileCmd::Open),
            (KeyBinding::key(KeyCode::Left), FileCmd::Parent),
            (KeyBinding::key(KeyCode::Backspace), FileCmd::Parent),
            (KeyBinding::char(' '), FileCmd::Choose),
            (KeyBinding::char('.'), FileCmd::ToggleHidden),
            (KeyBinding::char('/'), FileCmd::GoTo),
            (KeyBinding::char('~'), FileCmd::GoTo),
            (KeyBinding::ctrl('f'), FileCmd::Filter),
        ]
    }

    fn label(self) -> &'static str {
        match self {
            FileCmd::Open => "Open",
            FileCmd::Parent => "Parent folder",
            FileCmd::Choose => "Choose",
            FileCmd::ToggleHidden => "Hidden files",
            FileCmd::GoTo => "Go to folder",
            FileCmd::Filter => "Filter",
            FileCmd::Url => "URL entry",
        }
    }
}

impl FileCmd {
    pub fn vim_keymap() -> KeyMap<Self> {
        vec![
            (KeyBinding::key(KeyCode::Enter), FileCmd::Open),
            (KeyBinding::key(KeyCode::Right), FileCmd::Open),
            (KeyBinding::char('l'), FileCmd::Open),
            (KeyBinding::key(KeyCode::Left), FileCmd::Parent),
            (KeyBinding::char('h'), FileCmd::Parent),
            (KeyBinding::key(KeyCode::Backspace), FileCmd::Parent),
            (KeyBinding::char(' '), FileCmd::Choose),
            (KeyBinding::char('s'), FileCmd::Choose),
            (KeyBinding::char('g'), FileCmd::Url),
        ]
    }
}

pub type UrlValidator = Rc<dyn Fn(&str) -> Result<(), String>>;

#[derive(Clone)]
pub struct UrlEntry {
    pub button: String,

    pub back: String,

    pub label: String,
    pub placeholder: String,

    pub help: Vec<String>,

    pub hint: String,

    pub empty_error: String,

    pub validate: UrlValidator,
}

impl UrlEntry {
    pub fn new(button: &str, label: &str) -> Self {
        Self {
            button: button.to_owned(),
            back: "Browse folders".into(),
            label: label.to_owned(),
            placeholder: String::new(),
            help: Vec::new(),
            hint: label.to_owned(),
            empty_error: "Enter a URL".into(),
            validate: Rc::new(|_| Ok(())),
        }
    }
    pub fn back(mut self, s: &str) -> Self {
        self.back = s.to_owned();
        self
    }
    pub fn placeholder(mut self, s: &str) -> Self {
        self.placeholder = s.to_owned();
        self
    }
    pub fn help(mut self, lines: &[&str]) -> Self {
        self.help = lines.iter().map(|l| (*l).to_owned()).collect();
        self
    }
    pub fn hint(mut self, s: &str) -> Self {
        self.hint = s.to_owned();
        self
    }
    pub fn empty_error(mut self, s: &str) -> Self {
        self.empty_error = s.to_owned();
        self
    }
    pub fn validate(mut self, f: impl Fn(&str) -> Result<(), String> + 'static) -> Self {
        self.validate = Rc::new(f);
        self
    }
}

impl std::fmt::Debug for UrlEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UrlEntry")
            .field("button", &self.button)
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileBrowserResult {
    Chosen { path: String, checked: bool },

    Url(String),
    Cancelled,
}

pub struct FileBrowser {
    pub id: WidgetId,

    pub dismiss: Dismiss,
    pub title: String,

    pub cwd: String,
    pub path: TextInput,
    pub list: ListBox,

    pub entries: Vec<FileEntry>,

    pub choose_kind: Choose,

    pub checkbox: Option<Checkbox>,

    pub show_hidden: bool,

    pub hidden_box: Option<Checkbox>,

    pub breadcrumb: Option<Breadcrumb>,

    crumb_paths: Vec<String>,

    pub filter: Option<TextInput>,

    pub type_ahead: bool,
    pub keys: KeyMap<FileCmd>,
    pub url: Option<UrlEntry>,
    url_button: Button,
    pub cancel: Button,
    pub choose: Button,

    pub url_mode: bool,
    pub error: Option<String>,
    pub result: Option<FileBrowserResult>,

    pub area: Rect,

    pub stepper: Option<String>,
    source: Box<dyn FileSource>,
}

impl FileBrowser {
    pub fn new(id: WidgetId, title: &str, source: impl FileSource + 'static, start: &str) -> Self {
        let list = ListBox::new(id.sub("list"), vec![], SelectMode::Single);
        let type_ahead = list.type_ahead;
        let mut b = Self {
            id,
            dismiss: Dismiss::ESCAPE,
            title: title.to_owned(),
            cwd: start.to_owned(),
            path: TextInput::new(id.sub("path"), "Path").plain_label(),
            list,
            entries: vec![],
            choose_kind: Choose::Dirs,
            checkbox: None,
            show_hidden: false,
            hidden_box: Some(Checkbox::new(id.sub("hidden"), "Show hidden files", false)),
            breadcrumb: None,
            crumb_paths: vec![],
            filter: None,
            type_ahead,
            keys: FileCmd::default_keymap(),
            url: None,
            url_button: Button::secondary(id.sub("git"), ""),
            cancel: Button::subtle(id.sub("cancel"), "Cancel"),
            choose: Button::primary(id.sub("choose"), "Choose"),
            url_mode: false,
            error: None,
            result: None,
            area: Rect::ZERO,
            stepper: None,
            source: Box::new(source),
        };
        b.load();
        b
    }

    pub fn dismiss(mut self, dismiss: Dismiss) -> Self {
        self.dismiss = dismiss;
        self
    }

    pub fn choose(mut self, kind: Choose) -> Self {
        self.choose_kind = kind;
        self.load();
        self
    }

    pub fn checkbox(mut self, label: &str, checked: bool) -> Self {
        self.checkbox = Some(Checkbox::new(self.id.sub("ro"), label, checked));
        self
    }

    pub fn show_hidden(mut self, on: bool) -> Self {
        self.show_hidden = on;
        if let Some(c) = self.hidden_box.as_mut() {
            c.checked = on;
        }
        self.load();
        self
    }

    pub fn hidden_toggle(mut self, on: bool) -> Self {
        self.hidden_box =
            on.then(|| Checkbox::new(self.id.sub("hidden"), "Show hidden files", self.show_hidden));
        self
    }

    pub fn path_bar(mut self, on: bool) -> Self {
        self.breadcrumb = on.then(|| Breadcrumb::new(self.id.sub("crumb"), vec![]));
        self.load();
        self
    }

    pub fn filter(mut self, on: bool) -> Self {
        self.filter = on.then(|| {
            TextInput::new(self.id.sub("filter"), "Filter")
                .plain_label()
                .placeholder("Name contains")
        });
        self.load();
        self
    }

    pub fn type_ahead(mut self, on: bool) -> Self {
        self.type_ahead = on;
        self.list.type_ahead = on;
        self
    }

    pub fn keys(mut self, keys: KeyMap<FileCmd>) -> Self {
        self.keys = keys;
        self
    }

    pub fn url_entry(mut self, entry: UrlEntry) -> Self {
        self.url_button.label = entry.button.clone();
        self.url = Some(entry);
        self
    }

    pub fn stepper(mut self, line: &str) -> Self {
        self.stepper = Some(line.to_owned());
        self
    }

    pub fn url_button(&self) -> Option<&Button> {
        self.url.is_some().then_some(&self.url_button)
    }

    pub fn load(&mut self) {
        let cwd = self.cwd.clone();
        let (mut items, err) = match self.source.list(&cwd) {
            Ok(v) => (v, None),
            Err(e) => (vec![], Some(e)),
        };
        let query = self
            .filter
            .as_ref()
            .map(|f| f.text().trim().to_lowercase())
            .unwrap_or_default();
        items.retain(|e| {
            (self.show_hidden || !e.hidden)
                && (query.is_empty() || e.name().to_lowercase().contains(&query))
        });

        items.sort_by(|a, b| b.dir.cmp(&a.dir).then(a.path.cmp(&b.path)));
        let mut rows = vec![ListItem::new("..").meta("parent")];
        for e in &items {
            let label = if e.dir {
                format!("{}/", e.name())
            } else {
                e.name().to_owned()
            };
            let mut li = ListItem::new(&label);
            if !e.meta.is_empty() {
                li = li.meta(&e.meta);
            }
            li.disabled = self.choose_kind == Choose::Dirs && !e.dir;
            rows.push(li);
        }
        self.entries = items;
        let cur = self.list.cursor.min(rows.len().saturating_sub(1));
        let empty = if query.is_empty() {
            "Empty folder"
        } else {
            "No matches"
        };
        self.list = ListBox::new(self.list.id, rows, SelectMode::Single)
            .empty_text(empty)
            .type_ahead(self.type_ahead);
        self.list.cursor = cur;
        if !self.url_mode {
            self.path = TextInput::new(self.path.id, "Path")
                .plain_label()
                .value(&self.source.display(&self.cwd));
        }
        self.error = err;
        if let Some(b) = self.breadcrumb.as_mut() {
            let home = self.source.home();
            let mut chain = vec![cwd.clone()];
            while home.as_deref() != chain.last().map(String::as_str)
                && let Some(p) = self.source.parent(chain.last().expect("non-empty"))
            {
                chain.push(p);
            }
            chain.reverse();
            b.segments = chain
                .iter()
                .map(|p| {
                    if home.as_deref() == Some(p.as_str()) {
                        "~".to_owned()
                    } else {
                        p.trim_end_matches('/')
                            .rsplit('/')
                            .next()
                            .filter(|n| !n.is_empty())
                            .unwrap_or("/")
                            .to_owned()
                    }
                })
                .collect();
            self.crumb_paths = chain;
        }
    }

    fn go(&mut self, dir: String) -> Outcome {
        self.cwd = dir;
        self.list.cursor = 0;
        self.load();
        Outcome::Changed
    }

    fn open_cursor(&mut self) -> Outcome {
        let i = self.list.cursor;
        if i == 0 {
            if let Some(parent) = self.source.parent(&self.cwd) {
                return self.go(parent);
            }
            return Outcome::Changed;
        }
        match self.entries.get(i - 1) {
            Some(e) if e.dir => {
                let p = e.path.clone();
                self.go(p)
            }
            _ => Outcome::Consumed,
        }
    }

    fn choose_cursor(&mut self) -> Outcome {
        if self.url_mode {
            let url = self.path.text().trim().to_owned();
            let Some(entry) = &self.url else {
                return Outcome::Consumed;
            };
            if url.is_empty() {
                self.error = Some(entry.empty_error.clone());
                return Outcome::Changed;
            }
            if let Err(e) = (entry.validate)(&url) {
                self.error = Some(e);
                return Outcome::Changed;
            }
            self.result = Some(FileBrowserResult::Url(url));
            return Outcome::Changed;
        }
        let i = self.list.cursor;
        let entry = i.checked_sub(1).and_then(|i| self.entries.get(i)).cloned();
        let path = match (entry, self.choose_kind) {
            (Some(e), Choose::Files) if e.dir => return self.go(e.path),
            (None, Choose::Files) => {
                self.error = Some("Choose a file, not a folder".into());
                return Outcome::Changed;
            }
            (Some(e), Choose::Dirs) if !e.dir => {
                self.error = Some("Choose a folder, not a file".into());
                return Outcome::Changed;
            }
            (Some(e), _) => e.path,
            (None, _) => self.cwd.clone(),
        };
        self.result = Some(FileBrowserResult::Chosen {
            path,
            checked: self.checkbox.as_ref().is_some_and(|c| c.checked),
        });
        Outcome::Changed
    }

    fn toggle_hidden(&mut self) -> Outcome {
        self.show_hidden = !self.show_hidden;
        if let Some(c) = self.hidden_box.as_mut() {
            c.checked = self.show_hidden;
        }
        self.load();
        Outcome::Changed
    }

    fn toggle_url_mode(&mut self, focus: &mut Focus) {
        let Some(entry) = &self.url else { return };
        self.url_mode = !self.url_mode;
        if self.url_mode {
            self.path = TextInput::new(self.path.id, &entry.label)
                .plain_label()
                .placeholder(&entry.placeholder);
            self.url_button.label = entry.back.clone();
        } else {
            self.url_button.label = entry.button.clone();
            self.load();
        }
        self.error = None;
        focus.focus(self.path.id);
        self.path.begin_edit();
    }

    fn edit(&mut self, focus: &mut Focus, filter: bool) -> Outcome {
        let field = if filter {
            match self.filter.as_mut() {
                Some(f) => f,
                None => return Outcome::Ignored,
            }
        } else {
            &mut self.path
        };
        focus.focus(field.id);
        field.begin_edit();
        Outcome::Changed
    }

    fn cancelled(&mut self) -> Outcome {
        self.result = Some(FileBrowserResult::Cancelled);
        Outcome::Changed
    }

    pub fn on_key(&mut self, key: &Key, focus: &mut Focus, ring: &FocusRing) -> Outcome {
        let cur = focus.current();
        if cur == Some(self.path.id) {
            let editing = self.path.editing;
            let (o, ev) = self.path.on_key(key);
            match ev {
                Some(InputAction::Committed) => {
                    if self.url_mode {
                        return self.choose_cursor();
                    }
                    let typed = self.source.expand(self.path.text().trim());
                    if self.source.is_dir(&typed) {
                        self.go(typed);
                        focus.focus(self.list.id);
                    } else {
                        self.error = Some(format!("Folder not found: {}", self.path.text()));
                    }
                    return Outcome::Changed;
                }
                Some(InputAction::CommittedTab { backward }) => {
                    step(focus, ring, backward);
                    return Outcome::Changed;
                }
                _ => {}
            }
            if o.consumed() || editing {
                return o.or(Outcome::Consumed);
            }
        }
        if let Some(f) = self.filter.as_mut()
            && cur == Some(f.id)
        {
            let editing = f.editing;
            let before = f.text().to_owned();
            let (o, ev) = f.on_key(key);
            let changed = f.text() != before;
            match ev {
                Some(InputAction::Committed) => {
                    focus.focus(self.list.id);
                    return Outcome::Changed;
                }
                Some(InputAction::CommittedTab { backward }) => {
                    step(focus, ring, backward);
                    return Outcome::Changed;
                }
                _ => {}
            }
            if changed {
                self.list.cursor = 0;
                self.load();
                return Outcome::Changed;
            }
            if o.consumed() || editing {
                return o.or(Outcome::Consumed);
            }
        }
        if cur == Some(self.list.id) {
            match lookup(&self.keys, key) {
                Some(FileCmd::Open) => return self.open_cursor(),
                Some(FileCmd::Parent) => {
                    self.list.cursor = 0;
                    return self.open_cursor();
                }
                Some(FileCmd::Choose) => return self.choose_cursor(),
                Some(FileCmd::ToggleHidden) => return self.toggle_hidden(),
                Some(FileCmd::GoTo) if !self.url_mode => return self.edit(focus, false),
                Some(FileCmd::Filter) => {
                    let o = self.edit(focus, true);
                    if o != Outcome::Ignored {
                        return o;
                    }
                }
                Some(FileCmd::Url) if self.url.is_some() => {
                    self.toggle_url_mode(focus);
                    return Outcome::Changed;
                }
                _ => {}
            }
            let o = self.list.on_key(key);
            if o.consumed() {
                return o;
            }
        }
        if let Some(c) = self.checkbox.as_mut()
            && cur == Some(c.id)
        {
            let o = c.on_key(key);
            if o.consumed() {
                return o;
            }
        }
        if let Some(c) = self.hidden_box.as_mut()
            && cur == Some(c.id)
        {
            let was = c.checked;
            let o = c.on_key(key);
            if c.checked != was {
                self.show_hidden = c.checked;
                self.load();
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
        }
        if self.url.is_some() && cur == Some(self.url_button.id) {
            let (o, fired) = self.url_button.on_key(key);
            if fired {
                self.toggle_url_mode(focus);
                return Outcome::Changed;
            }
            if o.consumed() {
                return o;
            }
        }
        if cur == Some(self.cancel.id) {
            let (o, fired) = self.cancel.on_key(key);
            if fired {
                return self.cancelled();
            }
            if o.consumed() {
                return o;
            }
        }
        if cur == Some(self.choose.id) {
            let (o, fired) = self.choose.on_key(key);
            if fired {
                return self.choose_cursor();
            }
            if o.consumed() {
                return o;
            }
        }
        match key.code {
            KeyCode::Esc if self.dismiss.escape => self.cancelled(),
            KeyCode::Tab => {
                focus.next(ring);
                Outcome::Changed
            }
            KeyCode::BackTab => {
                focus.prev(ring);
                Outcome::Changed
            }
            _ => Outcome::Consumed,
        }
    }

    pub fn pointer(&mut self, ev: &PointerEvent, focus: &mut Focus) -> Outcome {
        if let Some((axis, delta)) = ev.wheel() {
            return match axis {
                Axis::Vertical => self.list.wheel(axis, delta).outcome(),
                Axis::Horizontal => Outcome::Consumed,
            };
        }
        if let Some(r) = self.path.route(ev) {
            if ev.is(Phase::Click) {
                focus.focus(self.path.id);
            }
            return r.outcome();
        }
        if let Some(f) = self.filter.as_mut()
            && let Some(r) = f.route(ev)
        {
            if ev.is(Phase::Click) {
                focus.focus(f.id);
            }
            return r.outcome();
        }
        if ev.on(self.list.id)
            && let Some(r) = scroll_input(&mut self.list, ev)
        {
            return r.outcome();
        }
        if !ev.is(Phase::Click) {
            return Outcome::Ignored;
        }
        let id = ev.id;
        if let Some(i) = ev
            .on(self.list.id)
            .then(|| ev.index_of(Part::ROW))
            .flatten()
        {
            focus.focus(self.list.id);

            let open = self.list.click.activates(ev.count(), self.list.cursor == i);
            self.list.cursor = i;
            self.list.chosen = Some(i);
            if open {
                return self.open_cursor();
            }
            return Outcome::Changed;
        }
        if let Some(i) = self.breadcrumb.as_ref().and_then(|b| b.on_click(id))
            && let Some(p) = self.crumb_paths.get(i).cloned()
        {
            return self.go(p);
        }
        if let Some(c) = self.checkbox.as_mut()
            && c.id == id
        {
            focus.focus(id);
            return c.on_click();
        }
        if let Some(c) = self.hidden_box.as_mut()
            && c.id == id
        {
            focus.focus(id);
            c.on_click();
            self.show_hidden = c.checked;
            self.load();
            return Outcome::Changed;
        }
        if self.url.is_some() && id == self.url_button.id {
            focus.focus(id);
            if self.url_button.on_click() {
                self.toggle_url_mode(focus);
            }
            return Outcome::Changed;
        }
        if id == self.cancel.id && self.cancel.on_click() {
            return self.cancelled();
        }
        if id == self.choose.id && self.choose.on_click() {
            return self.choose_cursor();
        }
        Outcome::Consumed
    }

    pub fn on_paste(&mut self, text: &str) -> Outcome {
        if let Some(f) = self.filter.as_mut()
            && f.editing
        {
            let o = f.on_paste(text);
            self.load();
            return o;
        }
        self.path.on_paste(text)
    }

    pub fn is_editing(&self) -> bool {
        self.path.editing || self.filter.as_ref().is_some_and(|f| f.editing)
    }

    pub fn on_blur(&mut self, change: FocusChange) -> Outcome {
        match self.filter.as_mut() {
            Some(f) => crate::core::focus::forward_blur(change, &mut [&mut self.path, f]),
            None => crate::core::focus::forward_blur(change, &mut [&mut self.path]),
        }
    }

    pub fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let (w, h) = if screen.width < 100 {
            (
                screen.width.saturating_sub(4).min(76),
                screen.height.saturating_sub(4).min(20),
            )
        } else {
            (84, 22)
        };
        let (area, inner) = crate::ui::modal::Frame::new(w, h)
            .title(&self.title)
            .meta(None)
            .placement(Placement::Center)
            .dismiss(self.dismiss)
            .render(screen, buf, ctx);
        self.area = area;
        if inner.is_empty() {
            return;
        }
        let t = ctx.theme;
        let bg = t.surface_elevated;
        let mut y = inner.y;
        if let Some(s) = &self.stepper {
            buf.set_string(
                inner.x,
                y,
                truncate(s, inner.width as usize),
                t.muted().bg(bg),
            );
            y += 2;
        }
        if let Some(b) = self.breadcrumb.as_mut()
            && !self.url_mode
        {
            b.render(Rect::new(inner.x, y, inner.width, 1), buf, ctx, bg);
            y += 2;
        }
        let field = Rect::new(inner.x - 1, y, inner.width + 1, 2);
        match self.filter.as_mut() {
            Some(f) if !self.url_mode => {
                let fw = (field.width / 3).max(12).min(field.width / 2);
                let pw = field.width.saturating_sub(fw + 1);
                self.path.render(Rect::new(field.x, y, pw, 2), buf, ctx, bg);
                f.render(Rect::new(field.x + pw + 1, y, fw, 2), buf, ctx, bg);
                ctx.hits.register(f.id, f.area);
            }
            _ => self.path.render(field, buf, ctx, bg),
        }
        y += 3;
        let extra: u16 = 2 + u16::from(self.checkbox.is_some()) + 1;
        let list_h = inner.bottom().saturating_sub(y + extra + 1);
        if self.url_mode {
            if let Some(entry) = &self.url {
                for (i, l) in entry.help.iter().enumerate() {
                    buf.set_string(inner.x + 1, y + i as u16, l, t.muted().bg(bg));
                }
            }
        } else {
            self.list.render(
                Rect::new(inner.x - 1, y, inner.width + 1, list_h),
                buf,
                ctx,
                bg,
            );
        }
        y += list_h + 1;
        if let Some(c) = self.checkbox.as_mut() {
            c.render(Rect::new(inner.x - 1, y, inner.width + 1, 1), buf, ctx, bg);
            if let Some(reason) = c.control.reason() {
                let x = inner.x - 1 + c.width() + 2;
                let room = Rect::new(x, y, inner.right().saturating_sub(x), 1);
                buf.set_string(
                    x,
                    y,
                    crate::ui::reveal::cut(room, &reason.text),
                    t.muted().bg(bg),
                );
            }
            y += 1;
        }
        if let Some(e) = &self.error {
            buf.set_string(
                inner.x,
                y,
                truncate(&format!("! {e}"), inner.width as usize),
                t.error_fg().bg(bg),
            );
        }
        let ay = inner.bottom().saturating_sub(1);
        let row = Rect::new(inner.x, ay, inner.width, 1);
        let mut buttons: Vec<&mut Button> = Vec::new();
        if self.url.is_some() {
            buttons.push(&mut self.url_button);
        }
        buttons.push(&mut self.cancel);
        buttons.push(&mut self.choose);
        let widths: Vec<u16> = buttons.iter().map(|b| b.width()).collect();
        let rects = row_layout_right(row, &widths, 1);
        let used: u16 = widths.iter().sum::<u16>() + widths.len() as u16;
        for (b, r) in buttons.into_iter().zip(rects) {
            b.render(r, buf, ctx, bg);
            ctx.hits.register(b.id, b.area);
        }
        if let Some(c) = self.hidden_box.as_mut() {
            let room = row.width.saturating_sub(used + 1);
            let cw = crate::ui::text::width(&c.label) as u16 + 6;
            if room >= cw {
                c.render(Rect::new(inner.x - 1, ay, cw, 1), buf, ctx, bg);
                ctx.hits.register(c.id, c.area);
            }
        }
        ctx.hits.register(self.path.id, self.path.area);
        if let Some(c) = &self.checkbox {
            ctx.hits.register(c.id, c.area);
        }
    }

    pub fn initial_focus(&self) -> WidgetId {
        self.list.id
    }

    pub fn hints(&self, _focused: Option<WidgetId>) -> Vec<Hint> {
        let mut v = Vec::new();
        let mut push = |cmd: FileCmd, action: &str| {
            if let Some(k) = hint_key(&self.keys, cmd) {
                v.push(Hint::new(k, action.to_owned()));
            }
        };
        push(FileCmd::Open, "Open");
        push(FileCmd::Choose, "Choose");
        if self.filter.is_some() {
            push(FileCmd::Filter, "Filter");
        }
        if self.hidden_box.is_some() {
            push(FileCmd::ToggleHidden, "Hidden files");
        }
        if let Some(u) = &self.url {
            let action = u.hint.clone();
            push(FileCmd::Url, &action);
        }
        v.push(hint("Tab", "Next"));
        if self.dismiss.escape {
            v.push(hint("Esc", "Cancel"));
        }
        v
    }
}

fn step(focus: &mut Focus, ring: &FocusRing, backward: bool) {
    if backward {
        focus.prev(ring)
    } else {
        focus.next(ring)
    }
}

impl Overlay for FileBrowser {
    type Action = FileBrowserResult;
    fn dismiss(&self) -> Dismiss {
        self.dismiss
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::modal(self.dismiss).initial_focus(self.initial_focus())
    }

    fn on_dismiss(&mut self, _: DismissReason) -> Response<FileBrowserResult> {
        self.result = Some(FileBrowserResult::Cancelled);
        Response::emit(FileBrowserResult::Cancelled)
    }
}
