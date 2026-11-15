use std::any::Any;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

use crate::core::event::{Key, Outcome};
use crate::core::focus::{Focus, FocusChange, FocusRing};
use crate::core::hit::Axis;
use crate::core::id::WidgetId;
pub use crate::core::intent::DismissReason;
use crate::core::intent::Phase;
use crate::core::pointer::PointerEvent;
use crate::core::response::Response;
use crate::ui::ctx::{Backdrop, RenderCtx};
use crate::ui::modal::{Modal, ModalStack};
use crate::ui::popup::{Dismiss, Placement};
use crate::widgets::icon_button::IconButton;
use crate::widgets::keyhint::Hint;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerKind {
    Modal,

    Popover,

    Tooltip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerSpec {
    pub kind: LayerKind,

    pub placement: Placement,

    pub anchor: Option<Rect>,

    pub backdrop: Option<Backdrop>,

    pub dismiss: Dismiss,

    pub initial_focus: Option<WidgetId>,

    pub restore_focus: bool,

    pub inert_below: bool,

    pub hides_page_focus: bool,
}

impl LayerSpec {
    pub const fn modal(dismiss: Dismiss) -> Self {
        Self {
            kind: LayerKind::Modal,
            placement: Placement::Center,
            anchor: None,
            backdrop: Some(Backdrop::DEFAULT),
            dismiss,
            initial_focus: None,
            restore_focus: true,
            inert_below: true,
            hides_page_focus: true,
        }
    }

    pub const fn popover(dismiss: Dismiss) -> Self {
        Self {
            kind: LayerKind::Popover,
            placement: Placement::Below,
            anchor: None,
            backdrop: None,
            dismiss,
            initial_focus: None,
            restore_focus: true,
            inert_below: true,
            hides_page_focus: true,
        }
    }

    pub const fn tooltip() -> Self {
        Self {
            kind: LayerKind::Tooltip,
            placement: Placement::Below,
            anchor: None,
            backdrop: None,
            dismiss: Dismiss::LIGHT,
            initial_focus: None,
            restore_focus: false,
            inert_below: false,
            hides_page_focus: false,
        }
    }

    pub const fn anchor(mut self, anchor: Rect, placement: Placement) -> Self {
        self.anchor = Some(anchor);
        self.placement = placement;
        self
    }

    pub const fn backdrop(mut self, backdrop: Option<Backdrop>) -> Self {
        self.backdrop = backdrop;
        self
    }

    pub const fn initial_focus(mut self, id: WidgetId) -> Self {
        self.initial_focus = Some(id);
        self
    }

    pub const fn restore_focus(mut self, on: bool) -> Self {
        self.restore_focus = on;
        self
    }

    pub const fn inert_below(mut self, on: bool) -> Self {
        self.inert_below = on;
        self
    }

    pub const fn hides_page_focus(mut self, on: bool) -> Self {
        self.hides_page_focus = on;
        self
    }
}

pub trait Overlay {
    type Action;

    fn dismiss(&self) -> Dismiss;

    fn spec(&self) -> LayerSpec {
        LayerSpec::modal(self.dismiss())
    }

    fn on_dismiss(&mut self, reason: DismissReason) -> Response<Self::Action>;

    fn close_button(&self) -> Option<&IconButton> {
        None
    }
}

pub fn dismiss<O: Overlay + ?Sized>(
    overlay: &mut O,
    reason: DismissReason,
) -> Option<Response<O::Action>> {
    overlay
        .dismiss()
        .allows(reason)
        .then(|| overlay.on_dismiss(reason))
}

pub struct LayerCx<'a, W: ?Sized = ()> {
    pub focus: &'a mut Focus,
    pub ring: &'a FocusRing,
    pub app: &'a W,
}

pub trait Layer<R, W: ?Sized = ()>: Any {
    fn spec(&self) -> LayerSpec;

    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, app: &W);

    fn on_key(&mut self, key: &Key, cx: &mut LayerCx<'_, W>) -> Outcome;

    fn pointer(&mut self, ev: &PointerEvent, cx: &mut LayerCx<'_, W>) -> Outcome {
        if let Some((axis, delta)) = ev.wheel() {
            return match axis {
                Axis::Vertical => self.on_wheel(delta, ev.pos()),
                Axis::Horizontal => self.on_wheel_h(delta, ev.pos()),
            };
        }
        match ev.phase() {
            Some(Phase::Press) => self.on_press(ev.id, ev.pos(), ev.count()),
            Some(Phase::Click) => self.on_click_count(ev.id, ev.pos(), ev.count(), cx),
            Some(Phase::Drag) => self.on_drag(ev.id, ev.pos()),
            Some(Phase::Release) => self.on_release(ev.id, ev.pos()),
            _ => Outcome::Ignored,
        }
    }

    fn on_click(&mut self, _id: WidgetId, _pos: Position, _cx: &mut LayerCx<'_, W>) -> Outcome {
        Outcome::Consumed
    }

    fn on_click_count(
        &mut self,
        id: WidgetId,
        pos: Position,
        _count: u8,
        cx: &mut LayerCx<'_, W>,
    ) -> Outcome {
        self.on_click(id, pos, cx)
    }

    fn on_press(&mut self, _id: WidgetId, _pos: Position, _count: u8) -> Outcome {
        Outcome::Ignored
    }

    fn on_drag(&mut self, _pressed: WidgetId, _pos: Position) -> Outcome {
        Outcome::Ignored
    }

    fn on_release(&mut self, _pressed: WidgetId, _pos: Position) -> Outcome {
        Outcome::Ignored
    }

    fn on_wheel(&mut self, _delta: i32, _pos: Position) -> Outcome {
        Outcome::Consumed
    }

    fn on_wheel_h(&mut self, _delta: i32, _pos: Position) -> Outcome {
        Outcome::Consumed
    }

    fn on_paste(&mut self, _text: &str) -> Outcome {
        Outcome::Ignored
    }

    fn on_paste_focused(&mut self, text: &str, _focus: &Focus) -> Outcome {
        self.on_paste(text)
    }

    fn on_blur(&mut self, _change: FocusChange) -> Outcome {
        Outcome::Ignored
    }

    fn on_tick(&mut self, _app: &W) -> Outcome {
        Outcome::Ignored
    }

    fn hints(&self, _focused: Option<WidgetId>) -> Vec<Hint> {
        Vec::new()
    }

    fn is_editing(&self) -> bool {
        false
    }

    fn animating(&self) -> bool {
        false
    }

    fn on_dismiss(&mut self, reason: DismissReason);

    fn take_result(&mut self) -> Option<LayerOut<R>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerOut<R> {
    Close(R),

    Event(R),
}

impl<R> LayerOut<R> {
    pub fn into_inner(self) -> R {
        match self {
            Self::Close(r) | Self::Event(r) => r,
        }
    }

    pub fn closes(&self) -> bool {
        matches!(self, Self::Close(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LayerTag {
    pub kind: &'static str,
    pub key: String,
    pub n: usize,
}

impl LayerTag {
    pub fn new(kind: &'static str) -> Self {
        Self {
            kind,
            key: String::new(),
            n: 0,
        }
    }
    pub fn key(mut self, k: impl Into<String>) -> Self {
        self.key = k.into();
        self
    }
    pub fn n(mut self, n: usize) -> Self {
        self.n = n;
        self
    }
}

pub struct LayerEntry<R, W: ?Sized = (), O = ()> {
    pub layer: Box<dyn Layer<R, W>>,
    pub tag: LayerTag,
    pub owner: O,
}

impl<R: 'static, W: ?Sized + 'static, O> LayerEntry<R, W, O> {
    pub fn get<L: Layer<R, W>>(&self) -> Option<&L> {
        let any: &dyn Any = &*self.layer;
        any.downcast_ref()
    }

    pub fn get_mut<L: Layer<R, W>>(&mut self) -> Option<&mut L> {
        let any: &mut dyn Any = &mut *self.layer;
        any.downcast_mut()
    }
}

impl<R: 'static, W: ?Sized + 'static, O> Modal for LayerEntry<R, W, O> {
    fn dismiss(&self) -> Dismiss {
        self.layer.spec().dismiss
    }
}

pub type ShellLayers<R, W = (), O = ()> = ModalStack<LayerEntry<R, W, O>>;

pub enum Taken<R, W: ?Sized = (), O = ()> {
    Event(R),

    Closed(LayerEntry<R, W, O>, R),
}

pub enum Dismissed<R, W: ?Sized = (), O = ()> {
    None,

    Refused,

    Closed(LayerEntry<R, W, O>, Option<R>),
}

pub enum LayerPointer<R, W: ?Sized = (), O = ()> {
    Handled(Outcome),

    Dismissed(Dismissed<R, W, O>),
}

impl<R: 'static, W: ?Sized + 'static, O> ModalStack<LayerEntry<R, W, O>> {
    pub fn pointer(
        &mut self,
        ev: &PointerEvent,
        focus: &mut Focus,
        ring: &FocusRing,
        app: &W,
    ) -> LayerPointer<R, W, O> {
        let Some(top) = self.top_mut() else {
            return LayerPointer::Handled(Outcome::Ignored);
        };

        if ev.is(Phase::Click) && ev.id == crate::ui::modal::BACKDROP {
            return LayerPointer::Dismissed(self.dismiss(DismissReason::OutsideClick, focus));
        }
        let out = {
            let mut cx = LayerCx { focus, ring, app };
            top.layer.pointer(ev, &mut cx)
        };
        if out == Outcome::Ignored
            && ev.is(Phase::Click)
            && ev.part().part == crate::core::id::Part::BACKDROP
        {
            return LayerPointer::Dismissed(self.dismiss(DismissReason::OutsideClick, focus));
        }
        LayerPointer::Handled(out)
    }

    pub fn open(
        &mut self,
        layer: Box<dyn Layer<R, W>>,
        tag: LayerTag,
        owner: O,
        focus: &mut Focus,
    ) {
        let initial = layer.spec().initial_focus;
        self.push(LayerEntry { layer, tag, owner }, focus, initial);
    }

    pub fn take(&mut self, focus: &mut Focus) -> Option<Taken<R, W, O>> {
        match self.top_mut()?.layer.take_result()? {
            LayerOut::Event(r) => Some(Taken::Event(r)),
            LayerOut::Close(r) => {
                let entry = self.pop(focus)?;
                Some(Taken::Closed(entry, r))
            }
        }
    }

    pub fn dismiss(&mut self, reason: DismissReason, focus: &mut Focus) -> Dismissed<R, W, O> {
        let Some(top) = self.top_mut() else {
            return Dismissed::None;
        };
        if !top.layer.spec().dismiss.allows(reason) {
            return Dismissed::Refused;
        }
        top.layer.on_dismiss(reason);
        let result = top.layer.take_result().map(LayerOut::into_inner);
        match self.pop(focus) {
            Some(entry) => Dismissed::Closed(entry, result),
            None => Dismissed::None,
        }
    }

    pub fn on_blur(&mut self, change: FocusChange) -> Outcome {
        self.iter_mut()
            .fold(Outcome::Ignored, |out, e| out.or(e.layer.on_blur(change)))
    }

    pub fn hides_page_focus(&self) -> bool {
        self.iter().any(|e| e.layer.spec().hides_page_focus)
    }

    pub fn top_as<L: Layer<R, W>>(&self) -> Option<&L> {
        self.top()?.get()
    }

    pub fn top_as_mut<L: Layer<R, W>>(&mut self) -> Option<&mut L> {
        self.top_mut()?.get_mut()
    }
}

use crate::widgets::choice_dialog::ChoiceDialog;
use crate::widgets::completion::{Completion, CompletionAction};
use crate::widgets::dialog::{Dialog, DialogResult};
use crate::widgets::form_dialog::{FormAction, FormDialog};
use crate::widgets::help::HelpOverlay;
use crate::widgets::info_dialog::{InfoDialog, InfoResult};
use crate::widgets::menu::{ContextMenu, MenuAction, MenuBar, MenuBarAction};
use crate::widgets::picker::{Picker, PickerAction};
use crate::widgets::select::{Select, SelectAction};

impl Overlay for Dialog {
    type Action = DialogResult;

    fn dismiss(&self) -> Dismiss {
        self.dismiss_in_force()
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::modal(Overlay::dismiss(self)).initial_focus(self.initial_focus)
    }

    fn on_dismiss(&mut self, _: DismissReason) -> Response<DialogResult> {
        let _ = self.dismissed();
        Response::emit(self.result.unwrap_or(DialogResult::Cancelled))
    }
    fn close_button(&self) -> Option<&IconButton> {
        Some(&self.close_button)
    }
}

impl Overlay for FormDialog {
    type Action = FormAction;
    fn dismiss(&self) -> Dismiss {
        self.dismiss
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::modal(self.dismiss).initial_focus(self.initial_focus())
    }
    fn on_dismiss(&mut self, _: DismissReason) -> Response<FormAction> {
        self.events.push(FormAction::Cancel);
        Response::emit(FormAction::Cancel)
    }
    fn close_button(&self) -> Option<&IconButton> {
        Some(&self.close_button)
    }
}

impl Overlay for ChoiceDialog {
    type Action = Option<usize>;
    fn dismiss(&self) -> Dismiss {
        self.dismiss
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::modal(self.dismiss).initial_focus(self.initial_focus())
    }
    fn on_dismiss(&mut self, _: DismissReason) -> Response<Option<usize>> {
        self.result = Some(None);
        Response::emit(None)
    }
    fn close_button(&self) -> Option<&IconButton> {
        Some(&self.close_button)
    }
}

impl Overlay for InfoDialog {
    type Action = InfoResult;
    fn dismiss(&self) -> Dismiss {
        self.dismiss
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::modal(self.dismiss).initial_focus(self.initial_focus())
    }
    fn on_dismiss(&mut self, _: DismissReason) -> Response<InfoResult> {
        self.result = Some(InfoResult::Closed);
        Response::emit(InfoResult::Closed)
    }
    fn close_button(&self) -> Option<&IconButton> {
        Some(&self.close_button)
    }
}

impl Overlay for HelpOverlay {
    type Action = ();
    fn dismiss(&self) -> Dismiss {
        self.dismiss
    }

    fn spec(&self) -> LayerSpec {
        LayerSpec::modal(self.dismiss).initial_focus(self.done.id)
    }
    fn on_dismiss(&mut self, _: DismissReason) -> Response<()> {
        self.closed = true;
        Response::changed()
    }
    fn close_button(&self) -> Option<&IconButton> {
        Some(&self.close_button)
    }
}

impl Overlay for Picker {
    type Action = PickerAction;
    fn dismiss(&self) -> Dismiss {
        self.list().dismiss
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::modal(self.list().dismiss).initial_focus(self.id)
    }
    fn on_dismiss(&mut self, _: DismissReason) -> Response<PickerAction> {
        Response::emit(PickerAction::Cancelled)
    }
}

impl Overlay for ContextMenu {
    type Action = MenuAction;
    fn dismiss(&self) -> Dismiss {
        self.list.dismiss
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::popover(self.list.dismiss)
            .initial_focus(self.id)
            .hides_page_focus(self.hides_page_focus())
    }
    fn on_dismiss(&mut self, _: DismissReason) -> Response<MenuAction> {
        Response::emit(MenuAction::Dismissed)
    }
}

impl Overlay for MenuBar {
    type Action = MenuBarAction;
    fn dismiss(&self) -> Dismiss {
        self.dismiss
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::popover(self.dismiss).hides_page_focus(self.hides_page_focus())
    }
    fn on_dismiss(&mut self, _: DismissReason) -> Response<MenuBarAction> {
        self.close();
        Response::emit(MenuBarAction::Closed)
    }
}

impl Overlay for Select {
    type Action = SelectAction;
    fn dismiss(&self) -> Dismiss {
        self.list.dismiss
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::popover(self.list.dismiss)
    }
    fn on_dismiss(&mut self, _: DismissReason) -> Response<SelectAction> {
        self.close().into()
    }
}

impl Overlay for Completion {
    type Action = CompletionAction;
    fn dismiss(&self) -> Dismiss {
        self.list.dismiss
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::popover(self.list.dismiss).hides_page_focus(false)
    }
    fn on_dismiss(&mut self, _: DismissReason) -> Response<CompletionAction> {
        self.close();
        Response::emit(CompletionAction::Dismiss)
    }
}

impl Overlay for crate::widgets::option_editor::ChoiceEditor {
    type Action = crate::widgets::option_editor::ChoiceResult;
    fn dismiss(&self) -> Dismiss {
        self.list.dismiss
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::popover(self.list.dismiss)
    }
    fn on_dismiss(&mut self, _: DismissReason) -> Response<Self::Action> {
        Response::emit(crate::widgets::option_editor::ChoiceResult::Cancel)
    }
}
