use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::FocusChange;
use junie_tui::core::id::WidgetId;
use junie_tui::core::pointer::PointerEvent;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layer::{Layer, LayerTag};
use junie_tui::ui::layers::LayerResult;
use junie_tui::widgets::dialog::{Dialog, DialogResult};
use junie_tui::widgets::form_dialog::{FieldValue, FormValues};
use junie_tui::widgets::help::HelpOverlay;

pub mod accounts_usage;
#[path = "../../../../apps/showcase/src/pages/activity.rs"]
pub mod activity;
#[path = "../../../../apps/showcase/src/pages/adapter.rs"]
pub mod adapter;
#[path = "../../../../apps/showcase/src/pages/breadcrumbs_rails.rs"]
pub mod breadcrumbs_rails;
#[path = "../../../../apps/showcase/src/pages/buttons.rs"]
pub mod buttons;
pub mod celleditors;
#[path = "../../../../apps/showcase/src/pages/checks_radios.rs"]
pub mod checks_radios;
#[path = "../../../../apps/showcase/src/pages/chips.rs"]
pub mod chips;
#[path = "../../../../apps/showcase/src/pages/chrome.rs"]
pub mod chrome;
pub mod columns;
#[path = "../../../../apps/showcase/src/pages/connections.rs"]
pub mod connections;
#[path = "../../../../apps/showcase/src/pages/dialog_kinds.rs"]
pub mod dialog_kinds;
#[path = "../../../../apps/showcase/src/pages/dialogs.rs"]
pub mod dialogs;
pub mod diff;
#[path = "../../../../apps/showcase/src/pages/disk_cleanup.rs"]
pub mod disk_cleanup;
#[path = "../../../../apps/showcase/src/pages/editable.rs"]
pub mod editable;
#[path = "../../../../apps/showcase/src/pages/editor.rs"]
pub mod editor;
#[path = "../../../../apps/showcase/src/pages/empty_error.rs"]
pub mod empty_error;
#[path = "../../../../apps/showcase/src/pages/file_browser.rs"]
pub mod file_browser;
pub mod finder;
#[path = "../../../../apps/showcase/src/pages/fixed_controls.rs"]
pub mod fixed_controls;
pub mod fixed_dialogs;
#[path = "../../../../apps/showcase/src/pages/focus_navigation.rs"]
pub mod focus_navigation;
#[path = "../../../../apps/showcase/src/pages/forms.rs"]
pub mod forms;
pub mod grid;
#[path = "../../../../apps/showcase/src/pages/help_keys.rs"]
pub mod help_keys;
#[path = "../../../../apps/showcase/src/pages/inputs.rs"]
pub mod inputs;
#[path = "../../../../apps/showcase/src/pages/keyed_store.rs"]
pub mod keyed_store;
#[path = "../../../../apps/showcase/src/pages/keyed_tree.rs"]
pub mod keyed_tree;
pub mod launch_cockpit;
pub mod list_detail;
#[path = "../../../../apps/showcase/src/pages/lists.rs"]
pub mod lists;
#[path = "../../../../apps/showcase/src/pages/master_detail.rs"]
pub mod master_detail;
#[path = "../../../../apps/showcase/src/pages/menus.rs"]
pub mod menus;
#[path = "../../../../apps/showcase/src/pages/meters.rs"]
pub mod meters;
#[path = "../../../../apps/showcase/src/pages/meters_work.rs"]
pub mod meters_work;
#[path = "../../../../apps/showcase/src/pages/notices.rs"]
pub mod notices;
#[path = "../../../../apps/showcase/src/pages/overview.rs"]
pub mod overview;
#[path = "../../../../apps/showcase/src/pages/pane_tree.rs"]
pub mod pane_tree;
#[path = "../../../../apps/showcase/src/pages/panels.rs"]
pub mod panels;
pub mod pickers;
#[path = "../../../../apps/showcase/src/pages/popups_tooltips.rs"]
pub mod popups_tooltips;
#[path = "../../../../apps/showcase/src/pages/progress.rs"]
pub mod progress;
#[path = "../../../../apps/showcase/src/pages/query_workbench.rs"]
pub mod query_workbench;
pub mod quit_leave;
#[path = "../../../../apps/showcase/src/pages/readonly_report.rs"]
pub mod readonly_report;
pub mod row_menu;
#[path = "../../../../apps/showcase/src/pages/scrolling.rs"]
pub mod scrolling;
pub mod selectable;
#[path = "../../../../apps/showcase/src/pages/settings.rs"]
pub mod settings;
#[path = "../../../../apps/showcase/src/pages/sidebars.rs"]
pub mod sidebars;
#[path = "../../../../apps/showcase/src/pages/splits_stacks.rs"]
pub mod splits_stacks;
#[path = "../../../../apps/showcase/src/pages/status_hints.rs"]
pub mod status_hints;
#[path = "../../../../apps/showcase/src/pages/stress.rs"]
pub mod stress;
pub mod tables;
#[path = "../../../../apps/showcase/src/pages/tabs.rs"]
pub mod tabs;
#[path = "../../../../apps/showcase/src/pages/taskrunner.rs"]
pub mod taskrunner;
pub mod template;
#[path = "../../../../apps/showcase/src/pages/terminal.rs"]
pub mod terminal;
#[path = "../../../../apps/showcase/src/pages/terminal_pane.rs"]
pub mod terminal_pane;
#[path = "../../../../apps/showcase/src/pages/terminal_session.rs"]
pub mod terminal_session;
#[path = "../../../../apps/showcase/src/pages/terminal_viewport.rs"]
pub mod terminal_viewport;
#[path = "../../../../apps/showcase/src/pages/text_utilities.rs"]
pub mod text_utilities;
#[path = "../../../../apps/showcase/src/pages/textareas.rs"]
pub mod textareas;
#[path = "../../../../apps/showcase/src/pages/toolbars.rs"]
pub mod toolbars;
#[path = "../../../../apps/showcase/src/pages/trees.rs"]
pub mod trees;
#[path = "../../../../apps/showcase/src/pages/workspaces.rs"]
pub mod workspaces;

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum PageEvent {
    Key(Key),

    Paste(String),

    Pointer(PointerEvent),
    Tick,

    Blur(FocusChange),
    DialogClosed {
        id: WidgetId,
        result: DialogResult,

        value: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum LayerRes {
    Lib(LayerResult),

    Dialog {
        id: WidgetId,
        result: DialogResult,
        value: Option<String>,
    },

    Page(Report),
}

#[derive(Debug, Clone)]
pub struct Report {
    pub result: Option<LayerResult>,

    pub values: Option<FormValues>,

    pub closed: Option<Closed>,
}

impl Report {
    pub fn text(&self, name: &str) -> String {
        self.values
            .iter()
            .flatten()
            .find_map(|(n, v)| match v {
                FieldValue::Text(t) if n == name => Some(t.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }
}

#[derive(Clone)]
pub struct Closed(pub std::rc::Rc<dyn Layer<LayerRes>>);

impl Closed {
    pub fn get<L: Layer<LayerRes>>(&self) -> Option<&L> {
        let any: &dyn std::any::Any = &*self.0;
        any.downcast_ref()
    }
}

impl std::fmt::Debug for Closed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Closed")
    }
}

impl From<LayerResult> for LayerRes {
    fn from(r: LayerResult) -> Self {
        Self::Lib(r)
    }
}

#[derive(Debug)]
pub enum Go {
    SetOption(crate::options::OptSet),

    Nav(junie_tui::shell::Nav<crate::app::PageId>),
}

pub type Request = junie_tui::shell::Request<LayerRes, (), Go>;

pub type Cx<'a> = junie_tui::shell::Cx<'a, LayerRes, (), Go>;

pub const DIALOG_TAG: &str = "dialog";

pub const HELP_TAG: &str = "help";

pub trait CxExt {
    fn open_dialog(&mut self, d: Dialog);

    fn open_help(&mut self, help: HelpOverlay);

    fn set_option(&mut self, set: crate::options::OptSet);

    fn open_page(&mut self, page: crate::app::PageId);
}

impl CxExt for Cx<'_> {
    fn open_dialog(&mut self, d: Dialog) {
        self.open(d, LayerTag::new(DIALOG_TAG));
    }
    fn open_help(&mut self, help: HelpOverlay) {
        self.open(help, LayerTag::new(HELP_TAG));
    }
    fn set_option(&mut self, set: crate::options::OptSet) {
        self.go(Go::SetOption(set));
    }
    fn open_page(&mut self, page: crate::app::PageId) {
        self.go(Go::Nav(junie_tui::shell::Nav::Push(page)));
    }
}

pub type Hint = (&'static str, &'static str);

pub trait Page {
    fn title(&self) -> &'static str;
    fn blurb(&self) -> &'static str;
    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx);

    fn measure(&self, avail: ratatui::layout::Size, _ctx: &RenderCtx) -> ratatui::layout::Size {
        avail
    }
    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome;

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint>;

    fn editing(&self) -> bool {
        false
    }

    fn leave_stakes(&self) -> junie_tui::ui::leave::Stakes {
        junie_tui::ui::leave::Stakes::Clean
    }

    fn animating(&self) -> bool {
        false
    }

    #[allow(dead_code)]
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        None
    }

    fn layer_open(&self) -> bool {
        self.template().is_some_and(|t| t.popup_open())
    }

    fn on_layer_key(&mut self, _key: &Key, _cx: &mut Cx) -> Outcome {
        Outcome::Ignored
    }

    fn on_layer(&mut self, _tag: &LayerTag, _report: Report, _cx: &mut Cx) -> Outcome {
        Outcome::Ignored
    }

    fn external(&self) -> Option<&junie_tui::runtime::ExternalReceiver> {
        None
    }

    fn on_external(
        &mut self,
        _message: &junie_tui::core::event::External,
        _cx: &mut Cx,
    ) -> Outcome {
        Outcome::Ignored
    }

    fn option_value(&self, _key: &str) -> Option<usize> {
        None
    }

    fn set_option(&mut self, _key: &str, _value: usize) {}

    #[allow(dead_code)]
    fn template(&self) -> Option<&template::Template> {
        None
    }
    #[allow(dead_code)]
    fn template_mut(&mut self) -> Option<&mut template::Template> {
        None
    }
}

pub mod layout {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;

    use junie_tui::theme::Theme;

    pub fn caption(x: u16, y: u16, buf: &mut Buffer, t: &Theme, text: &str, bg: Color) {
        buf.set_string(x, y, text, t.muted().bg(bg));
    }

    pub fn rows(area: Rect, heights: &[u16]) -> Vec<Rect> {
        let mut y = area.y;
        let mut out = Vec::new();
        for (i, &h) in heights.iter().enumerate() {
            let last = i == heights.len() - 1;
            let h = if last {
                area.bottom().saturating_sub(y)
            } else {
                h.min(area.bottom().saturating_sub(y))
            };
            out.push(Rect::new(area.x, y, area.width, h));
            y = y.saturating_add(h);
        }
        out
    }

    pub struct Sections {
        pub id: junie_tui::core::id::WidgetId,
        pub stack: junie_tui::ui::layout::Stack,
        pub state: junie_tui::ui::layout::StackScroll,
    }

    impl Sections {
        pub fn new(id: junie_tui::core::id::WidgetId) -> Self {
            Self {
                id,
                stack: junie_tui::ui::layout::Stack::default().gap(1),
                state: Default::default(),
            }
        }

        pub fn layout(
            &mut self,
            area: Rect,
            sections: &[junie_tui::ui::layout::StackChild],
            focus: Option<usize>,
            buf: &mut Buffer,
            ctx: &mut junie_tui::ui::ctx::RenderCtx,
        ) -> junie_tui::ui::layout::StackLayout {
            let l = self.stack.layout(area, sections, &mut self.state, focus);
            junie_tui::ui::layout::Stack::chrome(&l, area, buf, ctx, self.id, &self.state);
            l
        }

        pub fn handle(&mut self, ev: &super::PageEvent) -> Option<junie_tui::core::event::Outcome> {
            match ev {
                super::PageEvent::Pointer(p) => self.state.pointer(self.id, p),
                _ => None,
            }
        }
    }

    pub fn sheet_width(
        panel: &junie_tui::widgets::panel::Panel,
        facts: &[junie_tui::widgets::props::Prop],
    ) -> u16 {
        let w = junie_tui::widgets::props::width(facts);
        panel.outer(ratatui::layout::Size::new(w, 0)).width
    }

    pub fn sheet_height(
        panel: &junie_tui::widgets::panel::Panel,
        facts: &[junie_tui::widgets::props::Prop],
        w: u16,
    ) -> u16 {
        let inner = panel.inner(ratatui::layout::Size::new(w, 0)).width;
        let rows = junie_tui::widgets::props::measure(facts, inner);
        panel.outer(ratatui::layout::Size::new(inner, rows)).height
    }

    pub fn columns(area: Rect, left_w: u16, gap: u16) -> (Rect, Rect) {
        if area.width < left_w + gap + 20 {
            let h = area.height / 2;
            return (
                Rect::new(area.x, area.y, area.width, h),
                Rect::new(area.x, area.y + h, area.width, area.height - h),
            );
        }
        (
            Rect::new(area.x, area.y, left_w, area.height),
            Rect::new(
                area.x + left_w + gap,
                area.y,
                area.width - left_w - gap,
                area.height,
            ),
        )
    }
}

#[cfg(test)]
pub fn page_text(page: &mut dyn Page, width: u16, height: u16) -> String {
    use junie_tui::core::focus::FocusRing;
    use junie_tui::core::hit::HitRegistry;
    use junie_tui::theme::{ColorLevel, Theme};
    use junie_tui::ui::ctx::Interaction;

    let theme = Theme::for_level(ColorLevel::TrueColor);
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    page.render(area, &mut buf, &mut ctx);
    (0..height)
        .map(|y| (0..width).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}
