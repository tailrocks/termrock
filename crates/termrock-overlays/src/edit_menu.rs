use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

use crate::context::{ContextAnchor, ContextKeys};
use crate::menu::{ContextMenu, MenuAction, MenuItem};
use termrock_core::edit::{TextCmd, clipboard};
use termrock_core::event::{Key, Outcome};
use termrock_core::field::edit::{EditField, Editable};
use termrock_core::id::WidgetId;
use termrock_core::intent::Phase;
use termrock_core::keymap::Actions;
use termrock_core::pointer::{MouseTarget, PointerEvent};
use termrock_runtime::ctx::RenderCtx;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EditCaps {
    pub editable: bool,

    pub selection: bool,

    pub text: bool,

    pub secret: bool,
}

impl EditCaps {
    pub fn of(field: &EditField) -> Self {
        Self {
            editable: !field.read_only,
            selection: field.buffer.selected_text().is_some(),
            text: !field.buffer.text().is_empty(),
            secret: false,
        }
    }

    pub fn read_only(selection: bool, text: bool) -> Self {
        Self {
            editable: false,
            selection,
            text,
            secret: false,
        }
    }

    pub fn secret(mut self, on: bool) -> Self {
        self.secret = on;
        self
    }
}

pub fn edit_items(
    caps: EditCaps,
    shortcut: impl Fn(TextCmd) -> Option<String>,
) -> Vec<(TextCmd, MenuItem)> {
    let mut cmds = Vec::with_capacity(4);
    if caps.editable {
        cmds.push(TextCmd::Cut);
    }
    cmds.push(TextCmd::Copy);
    if caps.editable {
        cmds.push(TextCmd::Paste);
    }
    cmds.push(TextCmd::SelectAll);
    let pasteable = clipboard::text().is_some_and(|t| !t.is_empty());
    cmds.into_iter()
        .map(|cmd| {
            let reason = match cmd {
                TextCmd::Cut | TextCmd::Copy if caps.secret => Some("Hidden text is not copied"),
                TextCmd::Cut | TextCmd::Copy if !caps.selection => Some("Nothing is selected"),
                TextCmd::Paste if !pasteable => Some("Nothing copied yet"),
                TextCmd::SelectAll if !caps.text => Some("No text to select"),
                _ => None,
            };
            let mut item = MenuItem::new(cmd.label());
            if let Some(s) = shortcut(cmd) {
                item = item.shortcut(s);
            }
            if let Some(r) = reason {
                item = item.reason(r);
            }

            let item = item
                .separator_if(cmd == TextCmd::Paste || (!caps.editable && cmd == TextCmd::Copy));
            (cmd, item)
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct EditMenu {
    pub enabled: bool,

    pub keys: ContextKeys,

    open: Option<Box<(ContextMenu, Vec<TextCmd>)>>,

    edited: bool,

    area: Rect,
    caret: Option<Position>,
}

impl Default for EditMenu {
    fn default() -> Self {
        Self {
            enabled: true,
            keys: ContextKeys::DEFAULT,
            open: None,
            edited: false,
            area: Rect::ZERO,
            caret: None,
        }
    }
}

impl EditMenu {
    pub fn id_for(owner: WidgetId) -> WidgetId {
        owner.sub("edit-menu")
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn menu(&self) -> Option<&ContextMenu> {
        self.open.as_ref().map(|o| &o.0)
    }

    pub fn close(&mut self) -> Outcome {
        if self.open.take().is_some() {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    pub fn take_edited(&mut self) -> bool {
        std::mem::take(&mut self.edited)
    }

    pub fn owns(&self, ev: &PointerEvent) -> bool {
        self.open.as_ref().is_some_and(|o| ev.on(o.0.id))
    }

    pub fn open(
        &mut self,
        owner: WidgetId,
        anchor: ContextAnchor,
        items: Vec<(TextCmd, MenuItem)>,
    ) {
        let (cmds, rows): (Vec<_>, Vec<_>) = items.into_iter().unzip();
        let menu = anchor.apply(ContextMenu::new(Self::id_for(owner), rows));
        self.open = Some(Box::new((menu, cmds)));
    }

    pub fn key_anchor(&self, owner: WidgetId) -> ContextAnchor {
        match self.caret {
            Some(c) => ContextAnchor::Pointer(c),
            None => ContextAnchor::Control {
                id: owner,
                fallback: Rect::new(self.area.x, self.area.y, self.area.width.max(1), 1),
            },
        }
    }

    pub fn on_key(&mut self, key: &Key) -> Option<(Outcome, Option<TextCmd>)> {
        let menu = &mut self.open.as_mut()?.0;
        let (out, action) = menu.on_key(key);
        match action {
            Some(MenuAction::Dismissed) if out == Outcome::Ignored => {
                self.open = None;
                None
            }
            other => Some(self.settle(out.or(Outcome::Consumed), other)),
        }
    }

    pub fn on_pointer(&mut self, ev: &PointerEvent) -> Option<(Outcome, Option<TextCmd>)> {
        if !self.owns(ev) {
            return None;
        }
        let menu = &mut self.open.as_mut()?.0;
        let (out, actions) = menu.pointer(ev).into_parts();
        if actions.is_empty() {
            return Some(self.settle(out, None));
        }
        let mut settled = (out, None);
        for action in actions {
            let (o, cmd) = self.settle(out, Some(action));
            settled = (settled.0.or(o), settled.1.or(cmd));
        }
        Some(settled)
    }

    fn settle(&mut self, out: Outcome, action: Option<MenuAction>) -> (Outcome, Option<TextCmd>) {
        match action {
            Some(MenuAction::Chosen(i)) => {
                let cmd = self.open.take().and_then(|o| o.1.get(i).copied());
                (Outcome::Changed, cmd)
            }
            Some(MenuAction::Dismissed) => {
                self.open = None;
                (Outcome::Changed, None)
            }
            None => (out, None),
        }
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.area = area;
        self.caret = ctx.cursor.filter(|c| area.contains(*c));
        if let Some(o) = self.open.as_mut() {
            let menu = &mut o.0;
            ctx.cursor = None;
            let screen = *buf.area();
            menu.render(screen, buf, ctx);
        }
    }
}

pub trait EditMenuHost {
    fn edit_menu(&mut self) -> &mut EditMenu;

    fn edit_owner(&self) -> WidgetId;

    fn edit_caps(&self) -> EditCaps;

    fn edit_shortcut(&self, cmd: TextCmd) -> Option<String>;

    fn edit_target(&self, _ev: &PointerEvent) -> bool {
        true
    }

    fn edit_point(&mut self, _pos: Position) {}

    fn edit_run(&mut self, cmd: TextCmd) -> bool;
}

fn run<H: EditMenuHost + ?Sized>(h: &mut H, out: Outcome, cmd: Option<TextCmd>) -> Outcome {
    let Some(c) = cmd else {
        return out;
    };
    if h.edit_run(c) {
        h.edit_menu().edited = true;
    }
    Outcome::Changed
}

fn open_for<H: EditMenuHost + ?Sized>(h: &mut H, anchor: ContextAnchor) {
    let owner = h.edit_owner();
    let items = edit_items(h.edit_caps(), |c| h.edit_shortcut(c));
    h.edit_menu().open(owner, anchor, items);
}

pub fn edit_menu_key<H: EditMenuHost + ?Sized>(h: &mut H, key: &Key) -> Option<Outcome> {
    if let Some((out, cmd)) = h.edit_menu().on_key(key) {
        return Some(run(h, out, cmd));
    }
    let owner = h.edit_owner();
    let menu = h.edit_menu();
    if !menu.enabled || !menu.keys.matches(key) {
        return None;
    }
    let anchor = menu.key_anchor(owner);
    open_for(h, anchor);
    Some(Outcome::Changed)
}

pub fn edit_menu_pointer<H: EditMenuHost + ?Sized>(
    h: &mut H,
    ev: &PointerEvent,
) -> Option<Outcome> {
    if let Some((out, cmd)) = h.edit_menu().on_pointer(ev) {
        return Some(run(h, out, cmd));
    }
    if !ev.is(Phase::Secondary)
        || !ev.on(h.edit_owner())
        || !h.edit_menu().enabled
        || !h.edit_target(ev)
    {
        return None;
    }
    h.edit_point(ev.pos());
    open_for(h, ContextAnchor::Pointer(ev.pos()));
    Some(Outcome::Changed)
}

pub fn edit_menu_route<H>(
    h: &mut H,
    ev: &PointerEvent,
) -> Option<termrock_core::response::Response<H::Action>>
where
    H: EditMenuHost + MouseTarget + ?Sized,
{
    let mine = ev.on(h.mouse_id());
    let menu = h.edit_menu().owns(ev);
    (mine || menu).then(|| h.pointer(ev))
}

pub fn point_editable<T: Editable + ?Sized>(t: &mut T, pos: Position) {
    t.pointer_focus();
    t.pointer_begin();
    let at = t.offset_at(pos);
    point_field(t.field(), at);
    t.pointer_after();
}

pub fn editable_target<T: Editable + ?Sized>(t: &T, ev: &PointerEvent) -> bool {
    t.pointer_enabled() && !t.on_label(ev.pos())
}

pub fn point_field(field: &mut EditField, offset: usize) {
    if field
        .buffer
        .selection()
        .is_some_and(|r| r.start <= offset && offset <= r.end)
    {
        return;
    }
    field.buffer.select_range(offset, offset);
    field.buffer.clear_selection();
}
