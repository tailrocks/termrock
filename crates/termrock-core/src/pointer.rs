use ratatui::layout::Position;

use ratatui::crossterm::event::KeyModifiers;

use super::event::{Mouse, MouseKind, Outcome};
use super::focus::{Focus, FocusRing};
use super::hit::{Axis, Bar, Hit, HitRegistry, ReleaseFrom};
use super::id::{ItemKey, Part, PartRef, WidgetId};
use super::intent::{Intent, Phase};
use super::response::Response;
use super::scroll::Scrollable;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusOn {
    #[default]
    Press,

    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SecondaryOutside {
    #[default]
    Reopen,

    Close,

    Ignore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondaryRoute {
    Nothing,

    Target(WidgetId),

    Outside {
        backdrop: WidgetId,
        reopen: Option<WidgetId>,
    },

    Swallowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointerConfig {
    pub focus_on: FocusOn,

    pub double_click_ms: u64,

    pub double_click_slop: u16,

    pub wheel_lines: i32,

    pub hover_refresh_after_scroll: bool,

    pub flash_ms: u64,

    pub secondary_outside: SecondaryOutside,

    pub secondary_focuses: bool,

    pub end_edit_on_dead_space: bool,

    pub autoscroll: bool,

    pub autoscroll_ms: u64,

    pub track_repeat: bool,

    pub track_repeat_delay_ms: u64,

    pub track_repeat_ms: u64,

    pub pointer_shapes: bool,

    pub caret_shape: super::shape::CaretShape,

    pub click_skips_animation: bool,
}

impl PointerConfig {
    pub const DEFAULT: Self = Self {
        focus_on: FocusOn::Press,
        double_click_ms: 500,
        double_click_slop: 1,
        wheel_lines: 3,
        hover_refresh_after_scroll: true,
        flash_ms: 140,
        secondary_outside: SecondaryOutside::Reopen,
        secondary_focuses: true,
        end_edit_on_dead_space: true,
        autoscroll: true,
        autoscroll_ms: 50,
        track_repeat: true,
        track_repeat_delay_ms: 300,
        track_repeat_ms: 50,
        pointer_shapes: true,
        caret_shape: super::shape::CaretShape::Bar,
        click_skips_animation: true,
    };

    pub const fn focus_on(mut self, focus_on: FocusOn) -> Self {
        self.focus_on = focus_on;
        self
    }
    pub const fn double_click_ms(mut self, ms: u64) -> Self {
        self.double_click_ms = ms;
        self
    }
    pub const fn double_click_slop(mut self, cells: u16) -> Self {
        self.double_click_slop = cells;
        self
    }
    pub const fn wheel_lines(mut self, lines: i32) -> Self {
        self.wheel_lines = lines;
        self
    }
    pub const fn hover_refresh_after_scroll(mut self, on: bool) -> Self {
        self.hover_refresh_after_scroll = on;
        self
    }
    pub const fn flash_ms(mut self, ms: u64) -> Self {
        self.flash_ms = ms;
        self
    }
    pub const fn secondary_outside(mut self, policy: SecondaryOutside) -> Self {
        self.secondary_outside = policy;
        self
    }

    pub const fn secondary_focuses(mut self, on: bool) -> Self {
        self.secondary_focuses = on;
        self
    }
    pub const fn pointer_shapes(mut self, on: bool) -> Self {
        self.pointer_shapes = on;
        self
    }

    pub const fn caret_shape(mut self, shape: super::shape::CaretShape) -> Self {
        self.caret_shape = shape;
        self
    }

    pub const fn end_edit_on_dead_space(mut self, on: bool) -> Self {
        self.end_edit_on_dead_space = on;
        self
    }
    pub const fn click_skips_animation(mut self, on: bool) -> Self {
        self.click_skips_animation = on;
        self
    }
    pub const fn autoscroll(mut self, on: bool) -> Self {
        self.autoscroll = on;
        self
    }
    pub const fn autoscroll_ms(mut self, ms: u64) -> Self {
        self.autoscroll_ms = ms;
        self
    }

    pub const fn track_repeat(mut self, on: bool) -> Self {
        self.track_repeat = on;
        self
    }

    pub const fn track_repeat_ms(mut self, delay_ms: u64, every_ms: u64) -> Self {
        self.track_repeat_delay_ms = delay_ms;
        self.track_repeat_ms = every_ms;
        self
    }
}

impl Default for PointerConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Release {
    Click(WidgetId),

    Outside,

    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LastClick {
    id: WidgetId,
    pos: Position,
    at: u64,
    count: u8,
}

#[derive(Debug, Clone, Default)]
pub struct PointerState {
    pub config: PointerConfig,

    pub hover: Option<WidgetId>,

    pub pressed: Option<WidgetId>,

    pub hover_suppressed: bool,

    pub pos: Option<Position>,

    pub pointer_focus: bool,
    flash: Option<(WidgetId, u64)>,

    grab: Option<BarCapture>,

    edge: Option<ratatui::layout::Rect>,

    autoscroll_at: Option<u64>,

    track_at: Option<u64>,

    secondary_at: Option<Position>,

    accepted: Option<WidgetId>,

    motion_seen: bool,
    last_click: Option<LastClick>,
    hover_refresh: Option<Position>,

    hover_mark: std::cell::Cell<Option<(WidgetId, u64)>>,
}

impl PointerState {
    pub fn new(config: PointerConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    pub fn suppress_hover(&mut self) -> bool {
        let changed = (!self.hover_suppressed && self.hover.is_some()) || self.pointer_focus;
        self.hover_suppressed = true;
        self.pointer_focus = false;
        changed
    }

    pub fn focused_by_click(&mut self) {
        self.pointer_focus = true;
    }

    pub fn on_move(&mut self, hits: &HitRegistry, pos: Position) -> Outcome {
        self.pos = Some(pos);
        self.motion_seen = true;
        let was = self.hover;
        let suppressed = self.hover_suppressed;
        self.hover_suppressed = false;
        self.hover = hits.hover_at(pos);
        if self.hover != was || suppressed {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    pub fn on_drag(&mut self, hits: &HitRegistry, pos: Position) -> Option<WidgetId> {
        self.pos = Some(pos);

        self.hover = hits.hit(pos);
        self.pressed
    }

    pub fn on_down(&mut self, hits: &HitRegistry, pos: Position) -> Option<WidgetId> {
        self.pos = Some(pos);
        let hit = hits.hit(pos);
        let grab = hit.and_then(|id| BarCapture::of(hits, id));

        if grab.is_none() || grab != self.grab || hit != self.pressed {
            self.track_at = None;
        }
        self.pressed = hit;
        self.hover = hit;
        self.grab = grab;
        self.edge = hit.and_then(|_| hits.autoscroll_at(pos));
        self.autoscroll_at = None;
        hit
    }

    pub fn autoscrolling(&self) -> bool {
        self.config.autoscroll
            && self.pressed.is_some()
            && self.grabbed().is_none()
            && matches!((self.edge, self.pos), (Some(edge), Some(pos)) if !edge.contains(pos))
    }

    pub fn autoscroll_due(&mut self, now_ms: u64) -> Option<(WidgetId, Position)> {
        if !self.autoscrolling() {
            self.autoscroll_at = None;
            return None;
        }
        if let Some(at) = self.autoscroll_at
            && now_ms.saturating_sub(at) < self.config.autoscroll_ms
        {
            return None;
        }
        self.autoscroll_at = Some(now_ms);
        Some((self.pressed?, self.pos?))
    }

    pub fn track_repeating(&self) -> bool {
        self.config.track_repeat
            && self.pressed.is_some()
            && matches!((self.grab, self.pos), (Some(c), Some(pos)) if c.screen.track.contains(pos))
    }

    pub fn track_repeat_due(&mut self, now_ms: u64) -> Option<Position> {
        if !self.track_repeating() {
            self.track_at = None;
            return None;
        }
        match self.track_at {
            None => {
                self.track_at = Some(now_ms + self.config.track_repeat_delay_ms);
                None
            }
            Some(at) if now_ms >= at => {
                self.track_at = Some(now_ms + self.config.track_repeat_ms);
                self.pos
            }
            Some(_) => None,
        }
    }

    pub fn surface_press(
        &self,
        hits: &HitRegistry,
        id: WidgetId,
        focus: &mut super::focus::Focus,
    ) -> bool {
        if !hits.is_surface(id) {
            return false;
        }
        self.dead_space(focus);
        true
    }

    pub fn dead_space(&self, focus: &mut super::focus::Focus) {
        if self.config.end_edit_on_dead_space {
            focus.end_edit();
        }
    }

    pub fn grabbed(&self) -> Option<super::hit::Bar> {
        self.capture().map(|c| c.screen)
    }

    fn capture(&self) -> Option<BarCapture> {
        self.grab.filter(|c| self.pressed == Some(c.screen.id))
    }

    pub fn reports_motion(&self) -> bool {
        self.motion_seen
    }

    pub fn on_secondary(&mut self, hits: &HitRegistry, pos: Position) -> Option<WidgetId> {
        self.pos = Some(pos);
        self.pressed = None;
        self.secondary_at = Some(pos);
        let hit = hits.hit(pos);
        self.hover = hit;
        hit
    }

    pub fn route_secondary(&mut self, hits: &HitRegistry, pos: Position) -> SecondaryRoute {
        let hit = self.on_secondary(hits, pos);
        let Some(id) = hit else {
            return SecondaryRoute::Nothing;
        };
        let Some((backdrop, beneath)) = hits.backdrop_at(pos) else {
            return SecondaryRoute::Target(id);
        };
        match self.config.secondary_outside {
            SecondaryOutside::Ignore => SecondaryRoute::Swallowed,
            SecondaryOutside::Close => SecondaryRoute::Outside {
                backdrop,
                reopen: None,
            },
            SecondaryOutside::Reopen => {
                self.hover = beneath;
                SecondaryRoute::Outside {
                    backdrop,
                    reopen: beneath,
                }
            }
        }
    }

    pub fn on_secondary_drag(&mut self, hits: &HitRegistry, pos: Position) -> Outcome {
        self.pos = Some(pos);
        let was = self.hover;
        self.hover_suppressed = false;
        self.hover = hits.hit(pos);
        if self.hover != was {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    pub fn on_secondary_up(&mut self, hits: &HitRegistry, pos: Position) -> Option<WidgetId> {
        self.pos = Some(pos);
        let from = self.secondary_at.take()?;
        if from == pos {
            return None;
        }
        let hit = hits.hit(pos)?;
        hits.accepts_release(hit, ReleaseFrom::Secondary)
            .then_some(hit)
    }

    pub fn on_up(
        &mut self,
        hits: &HitRegistry,
        pos: Position,
    ) -> (Option<WidgetId>, Option<WidgetId>) {
        let hit = hits.hit(pos);
        self.accepted = match (hit, self.pressed) {
            (Some(h), Some(p)) if h != p && hits.accepts_release(h, ReleaseFrom::Press(p)) => {
                Some(h)
            }
            _ => None,
        };
        let beneath = hit
            .filter(|id| self.pressed == Some(*id))
            .and_then(|id| hits.pass_through_target(id, pos));
        (hit, beneath)
    }

    pub fn release(&mut self, hit: Option<WidgetId>) -> Release {
        let pressed = self.pressed.take();
        let accepted = self.accepted.take();
        self.grab = None;
        self.edge = None;
        self.track_at = None;
        match hit {
            None if pressed.is_none() => Release::Outside,
            None => Release::Cancelled,
            Some(id) if pressed == Some(id) || accepted == Some(id) => Release::Click(id),
            Some(_) => Release::Cancelled,
        }
    }

    pub fn press(&mut self, id: WidgetId) {
        self.pressed = Some(id);
    }

    pub fn count_click(&mut self, id: WidgetId, pos: Position, now_ms: u64) -> u8 {
        let count = self.press_count(id, pos, now_ms);
        self.last_click = Some(LastClick {
            id,
            pos,
            at: now_ms,
            count,
        });
        count
    }

    pub fn press_count(&self, id: WidgetId, pos: Position, now_ms: u64) -> u8 {
        let slop = self.config.double_click_slop;
        match self.last_click {
            Some(l)
                if l.id == id
                    && now_ms.saturating_sub(l.at) < self.config.double_click_ms
                    && l.pos.x.abs_diff(pos.x) <= slop
                    && l.pos.y.abs_diff(pos.y) <= slop =>
            {
                l.count.saturating_add(1)
            }
            _ => 1,
        }
    }

    pub fn hover_ms(&self, now_ms: u64) -> u64 {
        let Some(id) = self.hover.filter(|_| !self.hover_suppressed) else {
            self.hover_mark.set(None);
            return 0;
        };
        match self.hover_mark.get() {
            Some((h, since)) if h == id => now_ms.saturating_sub(since),
            _ => {
                self.hover_mark.set(Some((id, now_ms)));
                0
            }
        }
    }

    pub fn reset_clicks(&mut self) {
        self.last_click = None;
    }

    pub fn flash(&mut self, id: WidgetId, now_ms: u64) {
        self.flash = Some((id, now_ms.saturating_add(self.config.flash_ms)));
    }

    pub fn flashing(&self, now_ms: u64) -> Option<WidgetId> {
        self.flash
            .filter(|&(_, until)| now_ms < until)
            .map(|(id, _)| id)
    }

    pub fn has_flash(&self) -> bool {
        self.flash.is_some()
    }

    pub fn expire_flash(&mut self, now_ms: u64) -> bool {
        match self.flash {
            Some((_, until)) if now_ms >= until => {
                self.flash = None;
                true
            }
            _ => false,
        }
    }

    pub fn wheel_delta(&self, kind: MouseKind) -> i32 {
        match kind {
            MouseKind::WheelUp | MouseKind::WheelLeft => -self.config.wheel_lines,
            _ => self.config.wheel_lines,
        }
    }

    pub fn scrolled(&mut self, pos: Position, out: Outcome) {
        if out == Outcome::Changed && self.config.hover_refresh_after_scroll {
            self.hover_refresh = Some(pos);
        }
    }

    pub fn refresh_hover(&mut self, hits: &HitRegistry) {
        if let Some(pos) = self.hover_refresh.take()
            && !self.hover_suppressed
        {
            self.hover = hits.hover_at(pos);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BarCapture {
    screen: Bar,
    logical: Bar,
    shift: (u16, u16),
}

impl BarCapture {
    fn of(hits: &HitRegistry, id: WidgetId) -> Option<Self> {
        let screen = hits.bar(id)?;
        Some(Self {
            screen,
            logical: hits.logical_bar(id).unwrap_or(screen),
            shift: hits.shift_of(id),
        })
    }

    fn local_pos(self, screen: Position) -> Position {
        Position::new(
            screen.x.saturating_add(self.shift.0),
            screen.y.saturating_add(self.shift.1),
        )
    }

    fn apply(self, ev: &mut PointerEvent, screen: Position) {
        ev.bar = Some(self.logical);
        if let Intent::Pointer { pos, .. } = &mut ev.intent {
            *pos = self.local_pos(screen);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Held {
    id: WidgetId,
    hit: Hit,
    bar: Option<BarCapture>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointerEvent {
    pub id: WidgetId,

    pub hit: Hit,

    pub intent: Intent,

    pub bar: Option<Bar>,
}

impl PointerEvent {
    pub fn new(
        hits: &HitRegistry,
        phase: Phase,
        id: WidgetId,
        pos: Position,
        count: u8,
        mods: KeyModifiers,
    ) -> Self {
        let hit = hit_of(hits, id);

        let pos = hits.local(id, pos);
        Self {
            id,
            hit,
            intent: Intent::Pointer {
                phase,
                part: hit.part,
                pos,
                count,
                mods,
            },
            bar: hits.logical_bar(id),
        }
    }

    pub fn wheel_on(
        hits: &HitRegistry,
        id: WidgetId,
        axis: Axis,
        delta: i32,
        pos: Position,
        mods: KeyModifiers,
    ) -> Self {
        let hit = hit_of(hits, id);
        let pos = hits.local(id, pos);
        Self {
            id,
            hit,
            intent: Intent::Wheel {
                axis,
                delta: delta.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
                part: hit.part,
                pos,
                mods,
            },
            bar: None,
        }
    }

    pub fn owner(&self) -> WidgetId {
        self.hit.owner
    }

    pub fn on(&self, owner: WidgetId) -> bool {
        self.hit.owner == owner
    }

    pub fn part(&self) -> PartRef {
        self.hit.part
    }

    pub fn index_of(&self, part: Part) -> Option<usize> {
        if self.hit.part.part != part {
            return None;
        }
        self.hit.part.item.and_then(ItemKey::as_index)
    }

    /// The logical key of the hit item when this event is on `part`.
    pub fn key_of(&self, part: Part) -> Option<ItemKey> {
        if self.hit.part.part != part {
            return None;
        }
        self.hit.part.item
    }

    pub fn phase(&self) -> Option<Phase> {
        match self.intent {
            Intent::Pointer { phase, .. } => Some(phase),
            _ => None,
        }
    }

    pub fn is(&self, phase: Phase) -> bool {
        self.phase() == Some(phase)
    }

    pub fn pos(&self) -> Position {
        self.intent.pos().unwrap_or_default()
    }

    pub fn count(&self) -> u8 {
        match self.intent {
            Intent::Pointer { count, .. } => count,
            _ => 0,
        }
    }

    pub fn mods(&self) -> KeyModifiers {
        match self.intent {
            Intent::Pointer { mods, .. } | Intent::Wheel { mods, .. } => mods,
        }
    }

    pub fn wheel(&self) -> Option<(Axis, i32)> {
        match self.intent {
            Intent::Wheel { axis, delta, .. } => Some((axis, delta as i32)),
            _ => None,
        }
    }
}

fn hit_of(hits: &HitRegistry, id: WidgetId) -> Hit {
    hits.part_of(id).unwrap_or(Hit {
        owner: id,
        part: PartRef::of(Part::CONTAINER),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(hidden)]
pub enum BarPhase {
    Press,
    Drag,
    Release,
}

impl BarPhase {
    #[doc(hidden)]
    pub fn of(ev: &PointerEvent) -> Option<Self> {
        match ev.phase()? {
            Phase::Press => Some(Self::Press),
            Phase::Drag | Phase::DragStart => Some(Self::Drag),
            Phase::Click | Phase::Release | Phase::DragEnd => Some(Self::Release),
            _ => None,
        }
    }
}

pub fn scroll_input<S: Scrollable + ?Sized>(w: &mut S, ev: &PointerEvent) -> Option<Response<()>> {
    if let Some((axis, delta)) = ev.wheel() {
        return Some(w.wheel(axis, delta));
    }
    let bar = ev.bar.filter(|b| b.owner == ev.owner())?;
    Some(match BarPhase::of(ev)? {
        BarPhase::Press => w.bar_input(&bar, ev.pos(), true),
        BarPhase::Drag => w.bar_input(&bar, ev.pos(), false),
        BarPhase::Release => {
            w.pane().release();
            Response::consumed()
        }
    })
}

pub trait MouseTarget {
    type Action;

    fn mouse_id(&self) -> WidgetId;

    fn pointer(&mut self, ev: &PointerEvent) -> Response<Self::Action>;

    fn route(&mut self, ev: &PointerEvent) -> Option<Response<Self::Action>> {
        ev.on(self.mouse_id()).then(|| self.pointer(ev))
    }
}

pub struct PointerParts<'a> {
    pub router: &'a mut PointerRouter,
    pub hits: &'a HitRegistry,
    pub focus: &'a mut Focus,
    pub ring: &'a FocusRing,
}

pub trait PointerHost {
    fn pointer_parts(&mut self) -> PointerParts<'_>;

    fn layer_open(&self) -> bool;

    fn now_ms(&self) -> u64;

    fn deliver(&mut self, ev: &PointerEvent) -> Response<()>;

    fn outside(&mut self, _pos: Position) -> Response<()> {
        Response::changed()
    }
}

#[derive(Debug, Clone, Default)]
pub struct PointerRouter {
    pub state: PointerState,

    dragged: bool,

    held: Option<Held>,
}

impl std::ops::Deref for PointerRouter {
    type Target = PointerState;
    fn deref(&self) -> &PointerState {
        &self.state
    }
}

impl std::ops::DerefMut for PointerRouter {
    fn deref_mut(&mut self) -> &mut PointerState {
        &mut self.state
    }
}

impl PointerRouter {
    pub fn new(config: PointerConfig) -> Self {
        Self {
            state: PointerState::new(config),
            dragged: false,
            held: None,
        }
    }

    fn held_event(
        &self,
        hits: &HitRegistry,
        phase: Phase,
        id: WidgetId,
        pos: Position,
        mods: KeyModifiers,
    ) -> PointerEvent {
        let mut ev = PointerEvent::new(hits, phase, id, pos, 1, mods);
        if let Some(held) = self.held.filter(|h| h.id == id) {
            if hits.part_of(id).is_none() {
                ev.hit = held.hit;
                if let Intent::Pointer { part, .. } = &mut ev.intent {
                    *part = held.hit.part;
                }
            }
            if let Some(bar) = held.bar {
                bar.apply(&mut ev, pos);
            }
        }
        ev
    }

    fn captured(&self, mut ev: PointerEvent, pos: Position) -> PointerEvent {
        if let Some(bar) = self.held.filter(|h| h.id == ev.id).and_then(|h| h.bar) {
            bar.apply(&mut ev, pos);
        }
        ev
    }

    pub fn route<H: PointerHost + ?Sized>(host: &mut H, m: Mouse) -> Outcome {
        let before = host.pointer_parts().focus.current();
        let out = Self::dispatch(host, m);

        let p = host.pointer_parts();
        let pressed = m.kind == MouseKind::Down && p.router.state.pressed.is_some();
        if pressed || (m.kind == MouseKind::Up && p.focus.current() != before) {
            p.router.state.focused_by_click();
        }
        out
    }

    pub fn tick<H: PointerHost + ?Sized>(host: &mut H) -> Outcome {
        let now = host.now_ms();
        let mut out = if host.pointer_parts().router.state.expire_flash(now) {
            Outcome::Changed
        } else {
            Outcome::Ignored
        };
        let due = host.pointer_parts().router.state.autoscroll_due(now);
        if let Some((_, pos)) = due {
            out = out.or(Self::route(host, Mouse::new(MouseKind::Drag, pos)));
        }
        let due = host.pointer_parts().router.state.track_repeat_due(now);
        match due {
            Some(pos) => out.or(Self::route(host, Mouse::new(MouseKind::Down, pos))),
            None => out,
        }
    }

    fn dispatch<H: PointerHost + ?Sized>(host: &mut H, m: Mouse) -> Outcome {
        let layer = host.layer_open();

        let swallow = |out: Outcome| {
            if layer && out == Outcome::Ignored {
                Outcome::Consumed
            } else {
                out
            }
        };
        host.pointer_parts().router.state.pos = Some(m.pos);
        match m.kind {
            MouseKind::Move => {
                let p = host.pointer_parts();
                p.router.state.on_move(p.hits, m.pos)
            }
            MouseKind::Down => Self::down(host, m, layer),
            MouseKind::Drag => {
                let p = host.pointer_parts();
                let Some(pressed) = p.router.state.on_drag(p.hits, m.pos) else {
                    return Outcome::Ignored;
                };
                p.router.dragged = true;
                let ev = p
                    .router
                    .held_event(p.hits, Phase::Drag, pressed, m.pos, m.mods);
                swallow(host.deliver(&ev).outcome())
            }
            MouseKind::Up => {
                let now = host.now_ms();
                let p = host.pointer_parts();
                let pressed = p.router.state.pressed;
                let dragged = std::mem::take(&mut p.router.dragged);
                let (hit, beneath) = p.router.state.on_up(p.hits, m.pos);
                let mut out = Outcome::Ignored;
                if let Some(pressed) = pressed.filter(|_| dragged) {
                    let ev = p
                        .router
                        .held_event(p.hits, Phase::DragEnd, pressed, m.pos, m.mods);
                    out = host.deliver(&ev).outcome();
                }
                out = out.or(Self::release(host, m, hit, layer, now));

                if let Some(below) = beneath {
                    host.pointer_parts().router.state.press(below);
                    out = out.or(Self::release(host, m, Some(below), layer, now));
                }
                out
            }
            MouseKind::Secondary => Self::secondary(host, m, layer),
            MouseKind::SecondaryDrag => {
                let p = host.pointer_parts();
                p.router.state.on_secondary_drag(p.hits, m.pos)
            }

            MouseKind::SecondaryUp => {
                let now = host.now_ms();
                let p = host.pointer_parts();
                match p.router.state.on_secondary_up(p.hits, m.pos) {
                    Some(id) => {
                        p.router.state.press(id);
                        Self::release(host, m, Some(id), layer, now)
                    }
                    None => Outcome::Ignored,
                }
            }

            MouseKind::Middle => {
                let p = host.pointer_parts();
                let Some(id) = p.hits.hit(m.pos) else {
                    return swallow(Outcome::Ignored);
                };
                let ev = PointerEvent::new(p.hits, Phase::Middle, id, m.pos, 1, m.mods);
                swallow(host.deliver(&ev).outcome())
            }
            MouseKind::WheelUp
            | MouseKind::WheelDown
            | MouseKind::WheelLeft
            | MouseKind::WheelRight => swallow(Self::wheel(host, m)),
        }
    }

    fn down<H: PointerHost + ?Sized>(host: &mut H, m: Mouse, layer: bool) -> Outcome {
        let now = host.now_ms();
        let p = host.pointer_parts();
        p.router.dragged = false;
        p.router.held = None;
        let Some(id) = p.router.state.on_down(p.hits, m.pos) else {
            if layer {
                return Outcome::Consumed;
            }
            p.router.state.dead_space(p.focus);
            return Outcome::Ignored;
        };
        if layer {
            p.router.state.surface_press(p.hits, id, p.focus);
        } else if p.router.state.config.focus_on == FocusOn::Press && p.ring.contains(id) {
            p.focus.focus(id);
        }
        p.router.held = Some(Held {
            id,
            hit: hit_of(p.hits, id),
            bar: p.router.state.capture(),
        });

        let count = p.router.state.press_count(id, m.pos, now);
        let ev = PointerEvent::new(p.hits, Phase::Press, id, m.pos, count, m.mods);
        host.deliver(&ev).outcome().or(Outcome::Changed)
    }

    fn secondary<H: PointerHost + ?Sized>(host: &mut H, m: Mouse, layer: bool) -> Outcome {
        let now = host.now_ms();
        let p = host.pointer_parts();
        let route = p.router.state.route_secondary(p.hits, m.pos);
        let at = |host: &mut H, id: WidgetId| {
            let p = host.pointer_parts();

            if !layer && p.router.state.config.secondary_focuses && p.ring.contains(id) {
                p.focus.focus(id);
            }
            let ev = PointerEvent::new(p.hits, Phase::Secondary, id, m.pos, 1, m.mods);
            host.deliver(&ev).outcome()
        };
        let out = match route {
            SecondaryRoute::Nothing => Outcome::Ignored,
            SecondaryRoute::Swallowed => Outcome::Consumed,
            SecondaryRoute::Target(id) => at(host, id),
            SecondaryRoute::Outside { backdrop, reopen } => {
                host.pointer_parts().router.state.press(backdrop);
                let out = Self::release(host, m, Some(backdrop), layer, now).or(Outcome::Changed);

                match reopen {
                    Some(id) => out.or(at(host, id)),
                    None => out,
                }
            }
        };
        if layer && out == Outcome::Ignored {
            Outcome::Consumed
        } else {
            out
        }
    }

    fn wheel<H: PointerHost + ?Sized>(host: &mut H, m: Mouse) -> Outcome {
        let axis = if matches!(m.kind, MouseKind::WheelLeft | MouseKind::WheelRight) {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        let p = host.pointer_parts();
        let delta = p.router.state.wheel_delta(m.kind);

        let events: Vec<PointerEvent> = p
            .hits
            .scroll_stack(m.pos)
            .into_iter()
            .map(|id| PointerEvent::wheel_on(p.hits, id, axis, delta, m.pos, m.mods))
            .collect();
        let mut out = Outcome::Ignored;
        let mut tried: Vec<WidgetId> = Vec::new();
        for ev in events {
            if tried.contains(&ev.owner()) {
                continue;
            }
            tried.push(ev.owner());
            out = host.deliver(&ev).outcome();
            if out != Outcome::Ignored {
                break;
            }
        }

        host.pointer_parts().router.state.scrolled(m.pos, out);
        out
    }

    fn release<H: PointerHost + ?Sized>(
        host: &mut H,
        m: Mouse,
        hit: Option<WidgetId>,
        layer: bool,
        now: u64,
    ) -> Outcome {
        let p = host.pointer_parts();
        let pressed = p.router.state.pressed;
        let id = match p.router.state.release(hit) {
            Release::Click(id) => id,
            Release::Outside => return host.outside(m.pos).outcome().or(Outcome::Changed),
            Release::Cancelled => {
                let Some(pressed) = pressed else {
                    return Outcome::Changed;
                };
                let ev = p
                    .router
                    .held_event(p.hits, Phase::Release, pressed, m.pos, m.mods);
                return host.deliver(&ev).outcome().or(Outcome::Changed);
            }
        };

        p.router.state.flash(id, now);
        let count = p.router.state.count_click(id, m.pos, now);
        let ev = p.router.captured(
            PointerEvent::new(p.hits, Phase::Click, id, m.pos, count, m.mods),
            m.pos,
        );
        let before = p.focus.moves();
        let out = host.deliver(&ev).outcome();

        let p = host.pointer_parts();
        if !layer
            && p.router.state.config.focus_on == FocusOn::Release
            && p.focus.moves() == before
            && p.ring.contains(id)
        {
            p.focus.focus(id);
        }
        out.or(Outcome::Changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(hover: Option<WidgetId>, suppressed: bool, pointer_focus: bool) -> PointerState {
        PointerState {
            hover,
            hover_suppressed: suppressed,
            pointer_focus,
            ..PointerState::default()
        }
    }

    #[test]
    fn suppress_hover_reports_only_visual_transition() {
        let id = WidgetId::of("pointer.tests.target");
        for hover in [None, Some(id)] {
            for suppressed in [false, true] {
                for pointer_focus in [false, true] {
                    let mut s = state(hover, suppressed, pointer_focus);
                    let live_hover = hover.is_some() && !suppressed;
                    let changed = s.suppress_hover();
                    let label =
                        format!("hover={hover:?} suppressed={suppressed} focus={pointer_focus}");
                    assert_eq!(changed, live_hover || pointer_focus, "{label}");
                    assert!(s.hover_suppressed, "{label}");
                    assert!(!s.pointer_focus, "{label}");
                    assert_eq!(s.hover, hover, "the raw hover identity survives: {label}");
                    assert!(!s.suppress_hover(), "a repeat is not a transition: {label}");
                }
            }
        }
    }

    #[test]
    fn suppress_hover_does_not_decide_on_enabled_state() {
        let mut hits = HitRegistry::default();
        let id = WidgetId::of("pointer.tests.disabled");
        hits.register(id, ratatui::layout::Rect::new(0, 0, 4, 1));
        let mut s = PointerState::default();
        s.on_move(&hits, Position::new(1, 0));
        assert_eq!(s.hover, Some(id));
        assert!(s.suppress_hover());
        assert_eq!(s.hover, Some(id));
    }

    #[test]
    fn a_move_after_suppression_restores_hover() {
        let mut hits = HitRegistry::default();
        let id = WidgetId::of("pointer.tests.move");
        hits.register(id, ratatui::layout::Rect::new(0, 0, 4, 1));
        let mut s = PointerState::default();
        s.on_move(&hits, Position::new(1, 0));
        assert!(s.suppress_hover());
        assert_eq!(s.on_move(&hits, Position::new(2, 0)), Outcome::Changed);
        assert!(!s.hover_suppressed);
        assert_eq!(s.hover, Some(id));
        assert!(
            s.suppress_hover(),
            "hover is live again, so a key is a transition"
        );
    }
}
