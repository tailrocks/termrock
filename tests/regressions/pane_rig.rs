use junie_tui::core::event::{Mouse, MouseKind, Outcome};
use junie_tui::core::focus::{Focus, FocusRing};
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::pointer::{PointerEvent, PointerHost, PointerParts, PointerRouter};
use junie_tui::core::response::Response;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::pane_tree::{PaneId, PaneTree};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

pub struct Rig {
    pub tree: PaneTree,
    pub area: Rect,
    pub hits: HitRegistry,
    pub focus: Focus,
    pub ring: FocusRing,
    pub router: PointerRouter,
    pub inner: Vec<(PaneId, Rect)>,
}

impl PointerHost for Rig {
    fn pointer_parts(&mut self) -> PointerParts<'_> {
        let Self {
            router,
            hits,
            focus,
            ring,
            ..
        } = self;
        PointerParts {
            router,
            hits,
            focus,
            ring,
        }
    }

    fn layer_open(&self) -> bool {
        false
    }

    fn now_ms(&self) -> u64 {
        0
    }

    fn deliver(&mut self, ev: &PointerEvent) -> Response<()> {
        self.tree.on_pointer(ev).unwrap_or(Outcome::Ignored).into()
    }
}

impl Rig {
    pub fn new(tree: PaneTree, area: Rect) -> Self {
        let mut rig = Self {
            tree,
            area,
            hits: HitRegistry::default(),
            focus: Focus::default(),
            ring: FocusRing::default(),
            router: PointerRouter::default(),
            inner: Vec::new(),
        };
        rig.render();
        rig
    }

    pub fn render(&mut self) -> Buffer {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(Rect::new(0, 0, self.area.right(), self.area.bottom()));
        self.hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let interaction = Interaction {
            hover: self.router.hover,
            pressed: self.router.pressed,
            ..Default::default()
        };
        let mut ctx = RenderCtx::new(&theme, interaction, &mut self.hits, &mut ring);
        self.inner = self
            .tree
            .render(self.area, &mut buf, &mut ctx, theme.canvas, &|p| {
                format!("pane {p}")
            });
        buf
    }

    pub fn mouse(&mut self, kind: MouseKind, pos: Position) -> Outcome {
        PointerRouter::route(self, Mouse::new(kind, pos))
    }

    pub fn handle_origin(&self, i: usize) -> Position {
        let h = self.tree.seams()[i].handle;
        Position::new(h.x, h.y)
    }

    pub fn drag_seam(&mut self, i: usize, to: Position) {
        let from = self.handle_origin(i);
        self.mouse(MouseKind::Down, from);
        self.mouse(MouseKind::Drag, to);
        self.mouse(MouseKind::Up, to);
        self.render();
    }

    pub fn double_click_seam(&mut self, i: usize) {
        let at = self.handle_origin(i);
        for _ in 0..2 {
            self.mouse(MouseKind::Down, at);
            self.mouse(MouseKind::Up, at);
        }
        self.render();
    }

    pub fn rects(&self) -> Vec<Rect> {
        self.tree.panes().iter().map(|(_, r)| *r).collect()
    }
}
