use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

use crate::core::event::{Key, Outcome};
use crate::core::focus::{Focus, FocusChange};
use crate::core::hints::{HasHints, HintCx};
use crate::core::hit::Axis;
use crate::core::id::WidgetId;
use crate::core::pointer::{MouseTarget, PointerEvent};
use crate::core::scroll::Scrollable;
use crate::ui::ctx::RenderCtx;
use crate::ui::layer::{DismissReason, Layer, LayerCx, LayerOut, LayerSpec, Overlay};
use crate::widgets::choice_dialog::ChoiceDialog;
use crate::widgets::dialog::{Dialog, DialogBody, DialogResult};
use crate::widgets::file_browser::{FileBrowser, FileBrowserResult};
use crate::widgets::form_dialog::{FormAction, FormDialog};
use crate::widgets::help::HelpOverlay;
use crate::widgets::info_dialog::{InfoDialog, InfoResult};
use crate::widgets::keyhint::Hint;
use crate::widgets::menu::{ContextMenu, MenuAction};
use crate::widgets::picker::{Picker, PickerAction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerResult {
    Dialog {
        action: Option<usize>,
        text: Option<String>,
    },

    Picker(PickerAction),

    Menu(MenuAction),

    Form(FormAction),

    Choice(Option<usize>),

    Info(InfoResult),

    Help,

    File(FileBrowserResult),
}

fn out<R: From<LayerResult>>(r: LayerResult, closes: bool) -> LayerOut<R> {
    if closes {
        LayerOut::Close(r.into())
    } else {
        LayerOut::Event(r.into())
    }
}

impl<R: From<LayerResult> + 'static, W: ?Sized + 'static> Layer<R, W> for Dialog {
    fn spec(&self) -> LayerSpec {
        Overlay::spec(self)
    }
    fn is_editing(&self) -> bool {
        Dialog::is_editing(self)
    }
    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _: &W) {
        Dialog::render(self, screen, buf, ctx);
    }
    fn on_key(&mut self, key: &Key, cx: &mut LayerCx<'_, W>) -> Outcome {
        Dialog::on_key(self, key, cx.focus, cx.ring).or(Outcome::Consumed)
    }
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut LayerCx<'_, W>) -> Outcome {
        Dialog::pointer_in(self, ev, cx.focus).outcome()
    }
    fn on_click_count(
        &mut self,
        id: WidgetId,
        pos: Position,
        count: u8,
        cx: &mut LayerCx<'_, W>,
    ) -> Outcome {
        Dialog::on_click_count(self, id, pos, count, cx.focus)
    }
    fn on_click(&mut self, id: WidgetId, pos: Position, cx: &mut LayerCx<'_, W>) -> Outcome {
        Dialog::on_click_count(self, id, pos, 1, cx.focus)
    }
    fn on_press(&mut self, id: WidgetId, pos: Position, count: u8) -> Outcome {
        Dialog::on_press(self, id, pos, count)
    }
    fn on_drag(&mut self, pressed: WidgetId, pos: Position) -> Outcome {
        Dialog::on_drag(self, pressed, pos)
    }
    fn on_wheel(&mut self, delta: i32, _: Position) -> Outcome {
        Scrollable::wheel(self, Axis::Vertical, delta).outcome()
    }
    fn on_wheel_h(&mut self, delta: i32, _: Position) -> Outcome {
        Scrollable::wheel(self, Axis::Horizontal, delta).outcome()
    }
    fn on_paste(&mut self, text: &str) -> Outcome {
        Dialog::on_paste(self, text)
    }
    fn on_blur(&mut self, change: FocusChange) -> Outcome {
        Dialog::on_blur(self, change)
    }
    fn hints(&self, focused: Option<WidgetId>) -> Vec<Hint> {
        Dialog::hints(self, focused)
    }
    fn on_dismiss(&mut self, reason: DismissReason) {
        let _ = Overlay::on_dismiss(self, reason);
    }
    fn take_result(&mut self) -> Option<LayerOut<R>> {
        let action = match self.result.take()? {
            DialogResult::Action(i) => Some(i),
            DialogResult::Cancelled => None,
        };
        let text = match &self.body {
            DialogBody::Input(i) => Some(i.text().to_owned()),
            _ => None,
        };
        Some(out(LayerResult::Dialog { action, text }, true))
    }
}

fn picker_events(p: &mut Picker, actions: impl IntoIterator<Item = PickerAction>) {
    p.pending.extend(actions);
}

fn picker_closes(ev: &PickerAction) -> bool {
    matches!(
        ev,
        PickerAction::Chosen(_)
            | PickerAction::ChosenAlt(_)
            | PickerAction::ChosenMany(_)
            | PickerAction::Cancelled
    )
}

impl<R: From<LayerResult> + 'static, W: ?Sized + 'static> Layer<R, W> for Picker {
    fn hints(&self, focused: Option<WidgetId>) -> Vec<Hint> {
        HasHints::hints(self, &HintCx::new(focused))
    }
    fn spec(&self) -> LayerSpec {
        Overlay::spec(self)
    }
    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _: &W) {
        Picker::draw(self, screen, buf, ctx);
    }
    fn on_key(&mut self, key: &Key, _: &mut LayerCx<'_, W>) -> Outcome {
        let (o, ev) = Picker::on_key(self, key);
        picker_events(self, ev);
        o.or(Outcome::Consumed)
    }

    fn pointer(&mut self, ev: &PointerEvent, _: &mut LayerCx<'_, W>) -> Outcome {
        if ev.part().part == crate::core::id::Part::BACKDROP && !self.edit_menu.owns(ev) {
            return Outcome::Ignored;
        }
        let mut r = MouseTarget::pointer(self, ev);
        let actions = r.take_actions();
        picker_events(self, actions);
        r.outcome()
    }
    fn on_click(&mut self, id: WidgetId, _: Position, _: &mut LayerCx<'_, W>) -> Outcome {
        let ev = Picker::on_click(self, id);
        let changed = ev.is_some();
        picker_events(self, ev);
        if changed {
            Outcome::Changed
        } else {
            Outcome::Consumed
        }
    }
    fn on_wheel(&mut self, delta: i32, _: Position) -> Outcome {
        self.wheel(Axis::Vertical, delta)
    }
    fn on_paste(&mut self, text: &str) -> Outcome {
        let (o, ev) = Picker::on_paste(self, text);
        picker_events(self, ev);
        o
    }
    fn on_dismiss(&mut self, reason: DismissReason) {
        let actions = Overlay::on_dismiss(self, reason).into_actions();
        self.pending.clear();
        picker_events(self, actions);
    }
    fn take_result(&mut self) -> Option<LayerOut<R>> {
        let next = self.pending.next(picker_closes)?;
        let closes = next.closes();
        Some(out(LayerResult::Picker(next.into_inner()), closes))
    }
}

impl<R: From<LayerResult> + 'static, W: ?Sized + 'static> Layer<R, W> for ContextMenu {
    fn hints(&self, focused: Option<WidgetId>) -> Vec<Hint> {
        HasHints::hints(self, &HintCx::new(focused))
    }
    fn spec(&self) -> LayerSpec {
        Overlay::spec(self)
    }
    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _: &W) {
        ContextMenu::render(self, screen, buf, ctx);
    }
    fn on_key(&mut self, key: &Key, _: &mut LayerCx<'_, W>) -> Outcome {
        let (o, ev) = ContextMenu::on_key(self, key);
        self.pending.extend(ev);
        o.or(Outcome::Consumed)
    }
    fn pointer(&mut self, ev: &PointerEvent, _: &mut LayerCx<'_, W>) -> Outcome {
        let mut r = MouseTarget::pointer(self, ev);
        let actions = r.take_actions();
        self.pending.extend(actions);
        r.outcome()
    }
    fn on_click(&mut self, id: WidgetId, _: Position, _: &mut LayerCx<'_, W>) -> Outcome {
        let action = ContextMenu::on_click(self, id);
        self.pending.extend(action);
        Outcome::Changed
    }
    fn on_dismiss(&mut self, reason: DismissReason) {
        let actions = Overlay::on_dismiss(self, reason).into_actions();
        self.pending.clear();
        self.pending.extend(actions);
    }
    fn take_result(&mut self) -> Option<LayerOut<R>> {
        let next = self.pending.next(|_| true)?;
        Some(out(LayerResult::Menu(next.into_inner()), true))
    }
}

impl<R: From<LayerResult> + 'static, W: ?Sized + 'static> Layer<R, W> for FormDialog {
    fn hints(&self, focused: Option<WidgetId>) -> Vec<Hint> {
        HasHints::hints(self, &HintCx::new(focused))
    }
    fn spec(&self) -> LayerSpec {
        Overlay::spec(self)
    }
    fn is_editing(&self) -> bool {
        FormDialog::is_editing(self)
    }
    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _: &W) {
        FormDialog::render(self, screen, buf, ctx);
    }
    fn on_key(&mut self, key: &Key, cx: &mut LayerCx<'_, W>) -> Outcome {
        FormDialog::on_key(self, key, cx.focus, cx.ring).or(Outcome::Consumed)
    }

    fn pointer(&mut self, ev: &PointerEvent, cx: &mut LayerCx<'_, W>) -> Outcome {
        FormDialog::pointer_in(self, ev, cx.focus).outcome()
    }
    fn on_click_count(
        &mut self,
        id: WidgetId,
        pos: Position,
        count: u8,
        cx: &mut LayerCx<'_, W>,
    ) -> Outcome {
        FormDialog::on_click_count(self, id, pos, count, cx.focus)
    }
    fn on_click(&mut self, id: WidgetId, pos: Position, cx: &mut LayerCx<'_, W>) -> Outcome {
        FormDialog::on_click_count(self, id, pos, 1, cx.focus)
    }
    fn on_press(&mut self, id: WidgetId, pos: Position, count: u8) -> Outcome {
        FormDialog::on_press(self, id, pos, count)
    }
    fn on_drag(&mut self, pressed: WidgetId, pos: Position) -> Outcome {
        FormDialog::on_drag(self, pressed, pos)
    }
    fn on_release(&mut self, pressed: WidgetId, pos: Position) -> Outcome {
        FormDialog::on_release(self, pressed, pos)
    }
    fn on_wheel(&mut self, delta: i32, _: Position) -> Outcome {
        Scrollable::wheel(self, Axis::Vertical, delta).outcome()
    }
    fn on_paste(&mut self, _: &str) -> Outcome {
        Outcome::Consumed
    }
    fn on_paste_focused(&mut self, text: &str, focus: &Focus) -> Outcome {
        FormDialog::on_paste(self, text, focus)
    }
    fn on_blur(&mut self, change: FocusChange) -> Outcome {
        FormDialog::on_blur(self, change)
    }
    fn on_dismiss(&mut self, reason: DismissReason) {
        let _ = Overlay::on_dismiss(self, reason);
    }
    fn take_result(&mut self) -> Option<LayerOut<R>> {
        if self.events.is_empty() {
            return None;
        }
        let ev = self.events.remove(0);
        let closes = matches!(ev, FormAction::Save | FormAction::Cancel);
        Some(out(LayerResult::Form(ev), closes))
    }
}

impl<R: From<LayerResult> + 'static, W: ?Sized + 'static> Layer<R, W> for ChoiceDialog {
    fn hints(&self, focused: Option<WidgetId>) -> Vec<Hint> {
        HasHints::hints(self, &HintCx::new(focused))
    }
    fn spec(&self) -> LayerSpec {
        Overlay::spec(self)
    }
    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _: &W) {
        ChoiceDialog::render(self, screen, buf, ctx);
    }
    fn on_key(&mut self, key: &Key, cx: &mut LayerCx<'_, W>) -> Outcome {
        ChoiceDialog::on_key(self, key, cx.focus, cx.ring).or(Outcome::Consumed)
    }
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut LayerCx<'_, W>) -> Outcome {
        ChoiceDialog::pointer_in(self, ev, cx.focus).outcome()
    }
    fn on_click(&mut self, id: WidgetId, _: Position, cx: &mut LayerCx<'_, W>) -> Outcome {
        ChoiceDialog::on_click(self, id, cx.focus)
    }
    fn on_dismiss(&mut self, reason: DismissReason) {
        let _ = Overlay::on_dismiss(self, reason);
    }
    fn take_result(&mut self) -> Option<LayerOut<R>> {
        self.result
            .take()
            .map(|r| out(LayerResult::Choice(r), true))
    }
}

impl<R: From<LayerResult> + 'static, W: ?Sized + 'static> Layer<R, W> for InfoDialog {
    fn hints(&self, focused: Option<WidgetId>) -> Vec<Hint> {
        HasHints::hints(self, &HintCx::new(focused))
    }
    fn spec(&self) -> LayerSpec {
        Overlay::spec(self)
    }
    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _: &W) {
        InfoDialog::render(self, screen, buf, ctx);
    }
    fn on_key(&mut self, key: &Key, cx: &mut LayerCx<'_, W>) -> Outcome {
        InfoDialog::on_key(self, key, cx.focus, cx.ring).or(Outcome::Consumed)
    }
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut LayerCx<'_, W>) -> Outcome {
        InfoDialog::pointer_in(self, ev, cx.focus).outcome()
    }
    fn on_click(&mut self, id: WidgetId, pos: Position, cx: &mut LayerCx<'_, W>) -> Outcome {
        InfoDialog::on_click(self, id, pos, cx.focus)
    }

    fn on_drag(&mut self, pressed: WidgetId, pos: Position) -> Outcome {
        if pressed != crate::widgets::scrollbar::id_for(self.id) {
            return Outcome::Ignored;
        }

        InfoDialog::on_click(
            self,
            pressed,
            pos,
            &mut crate::core::focus::Focus::default(),
        )
    }
    fn on_wheel(&mut self, delta: i32, _: Position) -> Outcome {
        Scrollable::wheel(self, Axis::Vertical, delta).outcome()
    }
    fn on_dismiss(&mut self, reason: DismissReason) {
        let _ = Overlay::on_dismiss(self, reason);
    }
    fn take_result(&mut self) -> Option<LayerOut<R>> {
        let r = self.result.take()?;
        let closes = !matches!(r, InfoResult::Copy(_));
        Some(out(LayerResult::Info(r), closes))
    }
}

impl<R: From<LayerResult> + 'static, W: ?Sized + 'static> Layer<R, W> for HelpOverlay {
    fn hints(&self, focused: Option<WidgetId>) -> Vec<Hint> {
        HasHints::hints(self, &HintCx::new(focused))
    }
    fn spec(&self) -> LayerSpec {
        Overlay::spec(self)
    }
    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _: &W) {
        HelpOverlay::render(self, screen, buf, ctx);
    }
    fn on_key(&mut self, key: &Key, _: &mut LayerCx<'_, W>) -> Outcome {
        HelpOverlay::on_key(self, key).or(Outcome::Consumed)
    }
    fn pointer(&mut self, ev: &PointerEvent, _: &mut LayerCx<'_, W>) -> Outcome {
        MouseTarget::pointer(self, ev).outcome()
    }
    fn on_click(&mut self, id: WidgetId, _: Position, _: &mut LayerCx<'_, W>) -> Outcome {
        HelpOverlay::on_click(self, id)
    }
    fn on_wheel(&mut self, delta: i32, _: Position) -> Outcome {
        Scrollable::wheel(self, Axis::Vertical, delta).outcome()
    }
    fn on_dismiss(&mut self, reason: DismissReason) {
        let _ = Overlay::on_dismiss(self, reason);
    }
    fn take_result(&mut self) -> Option<LayerOut<R>> {
        if !self.closed {
            return None;
        }
        self.closed = false;
        Some(out(LayerResult::Help, true))
    }
}

impl<R: From<LayerResult> + 'static, W: ?Sized + 'static> Layer<R, W> for FileBrowser {
    fn hints(&self, focused: Option<WidgetId>) -> Vec<Hint> {
        FileBrowser::hints(self, focused)
    }
    fn is_editing(&self) -> bool {
        FileBrowser::is_editing(self)
    }
    fn spec(&self) -> LayerSpec {
        Overlay::spec(self)
    }
    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _: &W) {
        FileBrowser::render(self, screen, buf, ctx);
    }
    fn on_key(&mut self, key: &Key, cx: &mut LayerCx<'_, W>) -> Outcome {
        FileBrowser::on_key(self, key, cx.focus, cx.ring).or(Outcome::Consumed)
    }
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut LayerCx<'_, W>) -> Outcome {
        FileBrowser::pointer(self, ev, cx.focus)
    }
    fn on_paste(&mut self, text: &str) -> Outcome {
        FileBrowser::on_paste(self, text)
    }
    fn on_blur(&mut self, change: FocusChange) -> Outcome {
        FileBrowser::on_blur(self, change)
    }
    fn on_dismiss(&mut self, reason: DismissReason) {
        let _ = Overlay::on_dismiss(self, reason);
    }
    fn take_result(&mut self) -> Option<LayerOut<R>> {
        self.result.take().map(|r| out(LayerResult::File(r), true))
    }
}
