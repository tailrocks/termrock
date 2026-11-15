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
pub mod activity;
pub mod adapter;
pub mod breadcrumbs_rails;
pub mod buttons;
pub mod celleditors;
pub mod checks_radios;
pub mod chips;
pub mod chrome;
pub mod columns;
pub mod connections;
pub mod dialog_kinds;
pub mod dialogs;
pub mod diff;
pub mod disk_cleanup;
pub mod editable;
pub mod editor;
pub mod empty_error;
pub mod file_browser;
pub mod finder;
pub mod fixed_controls;
pub mod fixed_dialogs;
pub mod focus_navigation;
pub mod forms;
pub mod grid;
pub mod help_keys;
pub mod inputs;
pub mod launch_cockpit;
pub mod list_detail;
pub mod lists;
pub mod master_detail;
pub mod menus;
pub mod meters;
pub mod notices;
pub mod overview;
pub mod pane_tree;
pub mod panels;
pub mod pickers;
pub mod popups_tooltips;
pub mod progress;
pub mod query_workbench;
pub mod quit_leave;
pub mod readonly_report;
pub mod row_menu;
pub mod scrolling;
pub mod selectable;
pub mod settings;
pub mod sidebars;
pub mod splits_stacks;
pub mod status_hints;
pub mod tables;
pub mod tabs;
pub mod taskrunner;
pub mod template;
pub mod terminal;
pub mod terminal_pane;
pub mod terminal_session;
pub mod terminal_viewport;
pub mod text_utilities;
pub mod textareas;
pub mod toolbars;
pub mod trees;
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
