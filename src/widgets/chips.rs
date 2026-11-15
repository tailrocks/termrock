use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::core::event::{Key, Outcome};
use crate::core::hit::Axis;
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keymap::unassigned_chord;
use crate::core::keymap::{Actions, KeyBinding, KeyMap, NavKeys, lookup};
use crate::core::list::{Cursor, Navigable, RowSource};
use crate::theme::{ButtonKind, Kind};
use crate::ui::context::ContextConfig;
use crate::ui::ctx::{RenderCtx, fill};
use crate::ui::measure::terminal_extent;
use crate::ui::text::{truncate, width};
use crate::ui::tooltip::Tip;
use crate::widgets::icon_button::IconButton;
use crate::widgets::menu::{ContextMenu, MenuAction, MenuItem};

pub fn overflow_run(widths: &[u16], avail: u16, gap: u16) -> (usize, Option<String>) {
    let wide: Vec<usize> = widths.iter().map(|&w| usize::from(w)).collect();
    overflow_run_cells(&wide, usize::from(avail), usize::from(gap))
}

/// How many chips of `widths` show in `avail` cells, and the `+N` marker for the
/// rest. Every width, the gap and the budget stay wide, so a chip wider than any
/// `u16` never looks like it fits.
///
/// One pass sums the widths and a second walks the prefix down from the full
/// run, so the fit is O(N). The sums are exact in `u128`: a saturated total
/// could turn falsely small once a trailing gap is subtracted.
pub(crate) fn overflow_run_cells(
    widths: &[usize],
    avail: usize,
    gap: usize,
) -> (usize, Option<String>) {
    let n = widths.len();
    let (gap, avail) = (gap as u128, avail as u128);
    let mut prefix = 0u128;
    for &w in widths {
        crate::record_work!(FitVisits, 1usize);
        prefix += w as u128 + gap;
    }
    if prefix - if n > 0 { gap } else { 0 } <= avail {
        return (n, None);
    }
    for k in (0..n).rev() {
        crate::record_work!(FitVisits, 1usize);
        prefix -= widths[k] as u128 + gap;
        let hidden = n - k;
        let marker_w = 1 + u128::from(hidden.ilog10() + 1);
        if prefix + marker_w <= avail {
            return (k, Some(format!("+{hidden}")));
        }
    }
    (0, None)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    pub label: String,
    pub enabled: bool,
    pub removable: bool,
    pub error: bool,
}

impl Chip {
    fn intrinsic_width(&self) -> usize {
        1usize
            .saturating_add(width(&self.label))
            .saturating_add(1 + if self.removable { 2 } else { 0 } + 1)
    }

    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            enabled: true,
            removable: true,
            error: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChipBar {
    pub id: WidgetId,
    chips: Vec<Chip>,
    pub cursor: usize,
    add_label: Option<String>,

    lead: Option<String>,
    pub area: Rect,

    pub overflow_marker: bool,

    pub nav: NavKeys,

    pub keymap: KeyMap<ChipCmd>,

    pub close_button: IconButton,

    pub overflow_menu: bool,

    pub more_button: IconButton,

    pub digit_toggles: bool,

    pub context_menu: bool,

    pub context: ContextConfig,

    menu: Option<(ContextMenu, BarMenu)>,

    shown: usize,

    more_area: Rect,

    /// The scalar policy the last complete render laid out with. `None` after
    /// any collection or label change, so stale hit targets cannot act.
    layout: Option<BarPolicy>,
}

/// Scalar fields that shape the rendered geometry or the menus. Direct writes
/// to these public fields are caught by comparing against the key captured at
/// render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BarPolicy {
    overflow_menu: bool,
    overflow_marker: bool,
    context_menu: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipCmd {
    Remove,

    Add,

    ClearAll,

    ShowHidden,

    Lead,
}

impl Actions for ChipCmd {
    fn label(self) -> &'static str {
        match self {
            ChipCmd::Remove => "Remove",
            ChipCmd::Add => "Add",
            ChipCmd::ClearAll => "Clear all",
            ChipCmd::ShowHidden => "Show hidden",
            ChipCmd::Lead => "Switch the match mode",
        }
    }

    fn default_keymap() -> KeyMap<Self> {
        vec![
            (KeyBinding::key(KeyCode::Delete), ChipCmd::Remove),
            (KeyBinding::key(KeyCode::Backspace), ChipCmd::Remove),
            (KeyBinding::char('x'), ChipCmd::Remove),
            (KeyBinding::char('+'), ChipCmd::Add),
            (KeyBinding::char('X'), ChipCmd::ClearAll),
            (KeyBinding::key(KeyCode::Down), ChipCmd::ShowHidden),
            (KeyBinding::char('m'), ChipCmd::Lead),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipAction {
    Activate(usize),
    Toggle(usize),
    Remove(usize),
    Add,
    Lead,
    ClearAll,
}

impl ChipBar {
    pub fn new(id: WidgetId) -> Self {
        Self {
            id,
            chips: vec![],
            cursor: 0,
            add_label: Some("+ Add filter".into()),
            lead: None,
            area: Rect::ZERO,
            overflow_marker: true,
            nav: NavKeys::default(),
            keymap: ChipCmd::default_keymap(),
            close_button: IconButton::close(WidgetId::of("chips.remove")),
            overflow_menu: true,
            more_button: IconButton::new(WidgetId::of("chips.more"), "+"),
            digit_toggles: false,
            context_menu: true,
            context: ContextConfig::DEFAULT,
            menu: None,
            shown: 0,
            more_area: Rect::ZERO,
            layout: None,
        }
    }

    pub fn chips(&self) -> &[Chip] {
        &self.chips
    }

    /// Replaces every chip. Returns `false` when the collection is unchanged.
    pub fn set_chips(&mut self, chips: Vec<Chip>) -> bool {
        if self.chips == chips {
            return false;
        }
        self.chips = chips;
        self.invalidate_chips();
        true
    }

    /// Inserts at `index`, which may equal the length. Returns `false` for a
    /// position past the end.
    pub fn insert_chip(&mut self, index: usize, chip: Chip) -> bool {
        if index > self.chips.len() {
            return false;
        }
        self.chips.insert(index, chip);
        self.invalidate_chips();
        true
    }

    pub fn remove_chip(&mut self, index: usize) -> Option<Chip> {
        if index >= self.chips.len() {
            return None;
        }
        let chip = self.chips.remove(index);
        self.invalidate_chips();
        Some(chip)
    }

    /// Edits a copy of one chip and commits it only when the copy differs. A
    /// panicking `update` leaves the collection untouched.
    pub fn update_chip(&mut self, index: usize, update: impl FnOnce(&mut Chip)) -> bool {
        let Some(current) = self.chips.get(index) else {
            return false;
        };
        let mut next = current.clone();
        update(&mut next);
        if next == *current {
            return false;
        }
        self.chips[index] = next;
        self.invalidate_chips();
        true
    }

    pub fn add_label(&self) -> Option<&str> {
        self.add_label.as_deref()
    }

    /// Returns `false` when the label is unchanged.
    pub fn set_add_label(&mut self, label: Option<String>) -> bool {
        if self.add_label == label {
            return false;
        }
        self.add_label = label;
        self.invalidate_chips();
        true
    }

    pub fn lead(&self) -> Option<&str> {
        self.lead.as_deref()
    }

    /// Returns `false` when the lead is unchanged.
    pub fn set_lead(&mut self, lead: Option<String>) -> bool {
        if self.lead == lead {
            return false;
        }
        self.lead = lead;
        self.invalidate_chips();
        true
    }

    /// Drops every target derived from the chips and labels: the open menu,
    /// the overflow geometry and the hit regions. Only a fresh render
    /// re-enables them.
    fn invalidate_chips(&mut self) {
        self.menu = None;
        self.shown = 0;
        self.more_area = Rect::ZERO;
        self.area = Rect::ZERO;
        self.layout = None;
        self.cursor = self.cursor.min(self.stops().saturating_sub(1));
    }

    fn policy(&self) -> BarPolicy {
        BarPolicy {
            overflow_menu: self.overflow_menu,
            overflow_marker: self.overflow_marker,
            context_menu: self.context_menu,
        }
    }

    fn layout_current(&self) -> bool {
        self.layout == Some(self.policy())
    }

    /// Scalar fields are public, so a direct write can change the policy
    /// without a checked method. Close the menu on the next input.
    fn close_stale_menu(&mut self) {
        if self.menu.is_some() && !self.layout_current() {
            self.menu = None;
        }
    }

    fn owns(&self, id: WidgetId) -> bool {
        id == self.more_id()
            || id == self.add_id()
            || id == self.lead_id()
            || (0..self.chips.len()).any(|i| id == self.chip_id(i) || id == self.close_id(i))
    }

    /// Re-checks an action against the live collection at resolution.
    fn eligible(&self, action: ChipAction) -> bool {
        match action {
            ChipAction::Activate(i) | ChipAction::Toggle(i) => i < self.chips.len(),
            ChipAction::Remove(i) => self.chips.get(i).is_some_and(|chip| chip.removable),
            ChipAction::ClearAll => !self.chips.is_empty(),
            ChipAction::Add => self.add_label.is_some(),
            ChipAction::Lead => self.lead.is_some(),
        }
    }

    pub fn close_target_w(mut self, cells: u16) -> Self {
        self.close_button = self.close_button.plate(cells);
        self
    }

    pub fn close_button(mut self, button: IconButton) -> Self {
        self.close_button = button;
        self
    }

    pub fn more_button(mut self, button: IconButton) -> Self {
        self.more_button = button;
        self
    }

    pub fn overflow_menu(mut self, on: bool) -> Self {
        self.overflow_menu = on;
        self
    }

    pub fn more_id(&self) -> WidgetId {
        self.id.part(PartRef::of(Part::OVERFLOW))
    }

    pub fn menu_id(&self) -> WidgetId {
        self.id.sub("more-menu")
    }

    pub fn menu_open(&self) -> bool {
        self.menu.is_some()
    }

    fn open_menu(&mut self) {
        if !self.layout_current() {
            return;
        }
        let hidden: Vec<usize> = (self.shown.min(self.chips.len())..self.chips.len()).collect();
        if hidden.is_empty() {
            return;
        }
        let rows: Option<Vec<MenuItem>> = hidden
            .iter()
            .map(|&i| Some(MenuItem::new(self.chips.get(i)?.label.clone())))
            .collect();
        // One missing chip abandons the menu rather than misaligning its rows
        // with the hidden indices.
        let Some(items) = rows else {
            return;
        };
        let menu =
            ContextMenu::new(self.menu_id(), items).anchor_control(self.more_id(), self.more_area);
        self.menu = Some((menu, BarMenu::Hidden(hidden)));
    }

    pub fn context_items(&self, i: usize) -> Vec<(ChipAction, MenuItem)> {
        use crate::ui::context::{ContextCmd, command_items};
        let Some(chip) = self.chips.get(i) else {
            return Vec::new();
        };
        let toggle = MenuItem::new(if chip.enabled { "Turn off" } else { "Turn on" })
            .shortcut("Space")
            .separator();
        let cmds = [
            ContextCmd::new(ChipCmd::Remove)
                .unless((!chip.removable).then_some("This filter stays")),
            ContextCmd::new(ChipCmd::ClearAll).danger(),
        ];
        let mut items = vec![(ChipAction::Toggle(i), toggle)];
        items.extend(
            command_items(&self.keymap, &cmds)
                .into_iter()
                .map(|(c, it)| {
                    let a = match c {
                        ChipCmd::Remove => ChipAction::Remove(i),
                        _ => ChipAction::ClearAll,
                    };
                    (a, it)
                }),
        );
        items
    }

    fn open_chip_menu(&mut self, i: usize, anchor: crate::ui::context::ContextAnchor) {
        if !self.layout_current() {
            return;
        }
        let (actions, items): (Vec<_>, Vec<_>) = self.context_items(i).into_iter().unzip();
        if items.is_empty() {
            return;
        }
        let menu = anchor.apply(ContextMenu::new(self.menu_id(), items));
        self.menu = Some((menu, BarMenu::Chip(actions)));
    }

    fn menu_event(&mut self, ev: Option<MenuAction>) -> Option<ChipAction> {
        match ev? {
            MenuAction::Chosen(k) => match self.menu.take()?.1 {
                BarMenu::Hidden(hidden) => {
                    let i = *hidden.get(k)?;
                    if i >= self.chips.len() {
                        return None;
                    }
                    self.cursor = i;
                    Some(ChipAction::Activate(i))
                }
                BarMenu::Chip(actions) => actions.get(k).copied().filter(|&a| self.eligible(a)),
            },
            MenuAction::Dismissed => {
                self.menu = None;
                None
            }
        }
    }

    pub fn digit_toggles(mut self, on: bool) -> Self {
        self.digit_toggles = on;
        self
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn keymap(mut self, keymap: KeyMap<ChipCmd>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn overflow_marker(mut self, on: bool) -> Self {
        self.overflow_marker = on;
        self
    }

    pub fn chip_id(&self, i: usize) -> WidgetId {
        self.id.part(PartRef::item(Part::ROW, ItemKey::index(i)))
    }
    pub fn close_id(&self, i: usize) -> WidgetId {
        self.id.part(PartRef::item(Part::CLOSE, ItemKey::index(i)))
    }
    pub fn add_id(&self) -> WidgetId {
        self.id.part(PartRef::of(Part::NEW))
    }

    pub fn lead_id(&self) -> WidgetId {
        self.id.part(PartRef::of(Part::TITLE))
    }

    fn stops(&self) -> usize {
        self.chips.len() + usize::from(self.add_label.is_some())
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<ChipAction>) {
        self.close_stale_menu();
        if let Some((menu, _)) = self.menu.as_mut() {
            let (o, ev) = menu.on_key(key);
            return (o, self.menu_event(ev));
        }
        let n = self.stops();
        if n == 0 {
            return (Outcome::Ignored, None);
        }
        self.cursor = self.cursor.min(n - 1);
        if self.digit_toggles
            && key.plain()
            && let KeyCode::Char(c @ '1'..='9') = key.code
        {
            let i = c as usize - '1' as usize;
            if i < self.chips.len() {
                self.cursor = i;
                return (Outcome::Changed, Some(ChipAction::Toggle(i)));
            }
        }
        if self.context_menu && self.cursor < self.chips.len() && self.context.keys.matches(key) {
            let anchor = crate::ui::context::ContextAnchor::Control {
                id: self.chip_id(self.cursor),
                fallback: self.area,
            };
            self.open_chip_menu(self.cursor, anchor);
            return (Outcome::Changed, None);
        }
        let action = lookup(&self.keymap, key);
        if unassigned_chord(key, &self.keymap) {
            return (Outcome::Ignored, None);
        }
        match action {
            Some(ChipCmd::Remove)
                if self.cursor < self.chips.len() && self.chips[self.cursor].removable =>
            {
                return (Outcome::Changed, Some(ChipAction::Remove(self.cursor)));
            }
            Some(ChipCmd::Add) if self.add_label.is_some() => {
                return (Outcome::Changed, Some(ChipAction::Add));
            }
            Some(ChipCmd::ClearAll) => return (Outcome::Changed, Some(ChipAction::ClearAll)),
            Some(ChipCmd::ShowHidden)
                if self.overflow_menu && self.overflow_marker && self.shown < self.chips.len() =>
            {
                self.open_menu();
                return (Outcome::Changed, None);
            }
            Some(ChipCmd::Lead) if self.lead.is_some() => {
                return (Outcome::Changed, Some(ChipAction::Lead));
            }
            _ => {}
        }
        let to = match self.nav.action(key).filter(|_| key.plain()) {
            Some(nav) => self.cursor().apply(nav, self, 1).index(),
            None => None,
        };
        if let Some(to) = to {
            self.cursor = to;
            return (Outcome::Changed, None);
        }
        match key.code {
            KeyCode::Enter => {
                if self.cursor < self.chips.len() {
                    (Outcome::Changed, Some(ChipAction::Activate(self.cursor)))
                } else {
                    (Outcome::Changed, Some(ChipAction::Add))
                }
            }
            KeyCode::Char(' ') if self.cursor < self.chips.len() => {
                (Outcome::Changed, Some(ChipAction::Toggle(self.cursor)))
            }
            _ => (Outcome::Ignored, None),
        }
    }

    pub fn on_click(&mut self, id: WidgetId) -> (Outcome, Option<ChipAction>) {
        self.close_stale_menu();
        if let Some((menu, _)) = self.menu.as_mut()
            && menu.contains_id(id)
        {
            let ev = menu.on_click(id);
            return (Outcome::Changed, self.menu_event(ev));
        }
        if !self.layout_current() && self.owns(id) {
            return (Outcome::Consumed, None);
        }
        if id == self.more_id() {
            if self.menu.take().is_none() {
                self.open_menu();
            }
            return (Outcome::Changed, None);
        }

        self.menu = None;
        if id == self.add_id() {
            self.cursor = self.chips.len();
            return (Outcome::Changed, Some(ChipAction::Add));
        }
        if id == self.lead_id() {
            return (Outcome::Changed, Some(ChipAction::Lead));
        }
        for i in 0..self.chips.len() {
            if self.close_id(i) == id {
                return (Outcome::Changed, Some(ChipAction::Remove(i)));
            }
            if self.chip_id(i) == id {
                self.cursor = i;
                return (Outcome::Changed, Some(ChipAction::Activate(i)));
            }
        }
        (Outcome::Ignored, None)
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        let area = area.intersection(*buf.area());
        self.layout = None;
        if area.is_empty() {
            return;
        }
        self.area = area;
        let t = ctx.theme;
        let focused = ctx.interaction.focused_under(self.id, self.menu.is_some());
        let plane = t.plane_of(bg);
        let mut x = area.x;
        let y = area.y;
        if let Some(lead) = &self.lead {
            let lid = self.lead_id();
            let mut s = ctx.state(lid);
            s.focused = false;
            let st = t.interactive(Kind::Title, s, plane).style;
            let text = format!(" {lead} ");
            buf.set_string(x, y, &text, st);
            ctx.register_part(
                self.id,
                PartRef::of(Part::TITLE),
                Rect::new(x, y, terminal_extent(width(&text)), 1).intersection(area),
            );
            x = x
                .saturating_add(terminal_extent(width(&text)))
                .saturating_add(1);
        }
        let widths: Vec<usize> = self.chips.iter().map(Chip::intrinsic_width).collect();
        let (shown, marker) =
            overflow_run_cells(&widths, usize::from(area.right().saturating_sub(x)), 1);
        self.shown = shown;
        let mut more_area = Rect::ZERO;
        for (i, chip) in self.chips.iter().enumerate() {
            if i >= shown {
                if let Some(m) = marker.as_ref().filter(|_| self.overflow_marker) {
                    if self.overflow_menu {
                        let left = x.saturating_sub(1).max(area.x);
                        let r = Rect::new(left, y, x + width(m) as u16 + 1 - left, 1)
                            .intersection(area);
                        more_area = self
                            .more_button
                            .with_glyph(m)
                            .part_of(self.id, PartRef::of(Part::OVERFLOW))
                            .render_in(
                                r,
                                x,
                                buf,
                                ctx,
                                ratatui::style::Style::new().bg(bg),
                                Some(
                                    match crate::core::keymap::label_for(
                                        &self.keymap,
                                        ChipCmd::ShowHidden,
                                    ) {
                                        Some(k) => Tip::new(t.strings.tip_overflow).shortcut(&k),
                                        None => Tip::new(t.strings.tip_overflow),
                                    },
                                ),
                            );
                    } else {
                        buf.set_string(x, y, m, t.muted().bg(bg));
                    }
                    x += width(m) as u16 + 1;
                }
                break;
            }
            let cid = self.chip_id(i);

            let mut s = ctx.child_state(self.id, cid, i == self.cursor);
            s.focused = focused && i == self.cursor;
            s.selected = chip.enabled;
            let it = t.interactive(Kind::Chip, s, plane);
            let mut st = it.style;
            if chip.error {
                st = st.fg(t.legible(t.error, st.bg.unwrap_or(bg)));
            }
            let label_w = terminal_extent(width(&chip.label));
            let w = terminal_extent(widths[i]);
            let r = Rect::new(x, y, w, 1);
            fill(buf, r, st);
            buf.set_string(x, y, it.gutter, it.gutter_style);
            buf.set_string(x + 1, y, truncate(&chip.label, label_w as usize), st);
            if chip.removable {
                ctx.register_part(self.id, PartRef::item(Part::ROW, ItemKey::index(i)), r);

                let gx = x + 2 + label_w;
                let cw = self.close_button.width(t).max(1).min(w);
                let left = gx
                    .saturating_sub(self.close_button.pad)
                    .max(x)
                    .min(x + w - cw);
                let tip = Tip::new(t.strings.tip_remove);
                let tip = match crate::core::keymap::label_for(&self.keymap, ChipCmd::Remove) {
                    Some(k) => tip.shortcut(&k),
                    None => tip,
                };
                self.close_button
                    .part_of(self.id, PartRef::item(Part::CLOSE, ItemKey::index(i)))
                    .render_in(Rect::new(left, y, cw, 1), gx, buf, ctx, st, Some(tip));
            } else {
                ctx.register_part(self.id, PartRef::item(Part::ROW, ItemKey::index(i)), r);
            }
            x = x.saturating_add(w).saturating_add(1);
        }
        if let Some(add) = &self.add_label {
            let aid = self.add_id();
            let mut s = ctx.child_state(self.id, aid, self.cursor == self.chips.len());
            s.focused = focused && self.cursor == self.chips.len();
            let it = t.interactive(Kind::Button(ButtonKind::Subtle), s, plane);
            let st = it.style;
            let w = width(add).saturating_add(2);
            if w <= usize::from(area.right().saturating_sub(x)) {
                let w = terminal_extent(w);
                let r = Rect::new(x, y, w, 1);
                fill(buf, r, st);
                buf.set_string(x, y, it.gutter, it.gutter_style);
                buf.set_string(x + 1, y, add, st);
                ctx.register_part(self.id, PartRef::of(Part::NEW), r);
            }
        }
        if !ctx.inert {
            ctx.ring.register(self.id);
        }
        self.more_area = more_area;
        self.layout = Some(self.policy());
        if more_area.is_empty() && matches!(self.menu, Some((_, BarMenu::Hidden(_)))) {
            self.menu = None;
        }
        if let Some((menu, _)) = self.menu.as_mut() {
            menu.render(*buf.area(), buf, ctx);
        }
    }
}

impl crate::ui::measure::Measure for ChipBar {
    fn measure(&self, _avail: ratatui::layout::Size, _ctx: &RenderCtx) -> ratatui::layout::Size {
        ratatui::layout::Size::new(terminal_extent(self.intrinsic_width()), 1)
    }
}

impl ChipBar {
    pub(crate) fn intrinsic_width(&self) -> usize {
        let lead = self.lead.as_ref().map_or(0, |l| width(l).saturating_add(3));
        let chips = self
            .chips
            .iter()
            .map(|c| c.intrinsic_width().saturating_add(1))
            .fold(0usize, usize::saturating_add);
        let add = self
            .add_label
            .as_ref()
            .map_or(0, |a| width(a).saturating_add(2));

        lead.saturating_add(if add > 0 {
            chips.saturating_add(add)
        } else {
            chips.saturating_sub(1)
        })
    }
}

impl RowSource for ChipBar {
    fn len(&self) -> usize {
        self.stops()
    }
    fn label(&self, i: usize) -> &str {
        self.chips.get(i).map_or("", |c| c.label.as_str())
    }
}

impl Navigable for ChipBar {
    fn cursor(&self) -> Cursor {
        Cursor::new(self.cursor).axis(Axis::Horizontal)
    }
}

impl crate::ui::specimen::Specimen for ChipBar {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        matches!(
            state,
            Rest | Hover | Focused | FocusedHover | Error | Overflow | Narrow
        )
    }

    fn specimen(
        &self,
        state: crate::ui::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        use crate::ui::specimen::SpecimenState;
        let mut b = self.clone();
        b.menu = None;
        let v = state.visual();
        if v.hovered && !b.chips.is_empty() {
            ctx.interaction.hover = Some(b.chip_id(0));
        }
        match state {
            SpecimenState::Error => {
                if let Some(c) = b.chips.first_mut() {
                    c.error = true;
                }
            }
            SpecimenState::Overflow => {
                for l in ["region = 'eu'", "plan = 'pro'", "seats > 5"] {
                    b.chips.push(Chip::new(l));
                }
            }
            _ => {}
        }
        b.render(area, buf, ctx, bg);
    }
}

impl crate::core::pointer::MouseTarget for ChipBar {
    type Action = ChipAction;

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn route(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> Option<crate::core::response::Response<ChipAction>> {
        let menu = self.menu.as_ref().map(|(m, _)| m.id);
        (ev.on(self.id) || menu.is_some_and(|m| ev.on(m))).then(|| self.pointer(ev))
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<ChipAction> {
        use crate::core::intent::Phase;

        if let Some((axis, delta)) = ev.wheel()
            && let Some((menu, _)) = self.menu.as_mut()
        {
            return crate::core::scroll::Scrollable::wheel(&mut menu.list, axis, delta).lift();
        }
        match ev.phase() {
            Some(Phase::Click) => self.on_click(ev.id).into(),

            Some(Phase::Secondary) if self.context_menu => {
                let Some(i) = (0..self.chips.len())
                    .find(|&i| ev.id == self.chip_id(i) || ev.id == self.close_id(i))
                else {
                    return crate::core::response::Response::ignored();
                };
                if !self.layout_current() {
                    return crate::core::response::Response::consumed();
                }
                if self.context.moves_cursor {
                    self.cursor = i;
                }
                self.open_chip_menu(i, crate::ui::context::ContextAnchor::Pointer(ev.pos()));
                crate::core::response::Response::changed()
            }
            _ => crate::core::response::Response::ignored(),
        }
    }
}

#[derive(Debug, Clone)]
enum BarMenu {
    Hidden(Vec<usize>),

    Chip(Vec<ChipAction>),
}
