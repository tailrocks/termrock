use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::FocusChange;
use junie_tui::core::id::WidgetId;
use junie_tui::core::pointer::PointerEvent;
use junie_tui::shell::{Page, PageTypes};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::leave::Stakes;
use junie_tui::widgets::keyhint::Hint;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::app::{Act, Cx, ModalResult};
use crate::connections::{ConnEvent, ConnectionsScreen};
use crate::model::History;
use crate::workbench::WorkTab;
use crate::workbench::{EXPLORER, TABSTRIP, Workbench};
use junie_tui::theme::StatusKind;
use junie_tui::ui::layout::Maximized;
use junie_tui::widgets::input::TextInput;

pub struct Tablepro;

impl PageTypes for Tablepro {
    type State = History;
    type Msg = ();
    type Result = ModalResult;
    type LayerState = ();
    type Go = Act;
}

impl Page<Tablepro> for ConnectionsScreen {
    fn on_key(&mut self, key: &Key, _h: &mut History, cx: &mut Cx) -> Outcome {
        ConnectionsScreen::on_key(self, key, cx).0
    }

    fn pointer(&mut self, ev: &PointerEvent, _h: &mut History, cx: &mut Cx) -> Outcome {
        ConnectionsScreen::pointer(self, ev, cx).0
    }

    fn on_paste(&mut self, text: &str, _h: &mut History, _cx: &mut Cx) -> Outcome {
        ConnectionsScreen::on_paste(self, text)
    }

    fn on_blur(&mut self, ch: FocusChange, _h: &mut History, _cx: &mut Cx) -> Outcome {
        ConnectionsScreen::on_blur(self, ch)
    }

    fn on_tick(&mut self, _h: &mut History, cx: &mut Cx) -> Outcome {
        match self.tick() {
            Some(ConnEvent::Connected {
                key,
                generation,
                connection,
            }) => {
                cx.go(Act::Connected {
                    key,
                    generation,
                    connection,
                });
                Outcome::Changed
            }
            Some(ConnEvent::Cancelled(why)) => {
                cx.status(why.message());
                Outcome::Changed
            }
            None => Outcome::Ignored,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _h: &History) {
        ConnectionsScreen::render(self, area, buf, ctx);
    }

    fn hints(&self, focus: Option<WidgetId>, _h: &History) -> Vec<Hint> {
        ConnectionsScreen::hints(self, focus)
    }

    fn crumb(&self, _h: &History) -> String {
        "Connections".into()
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        None
    }

    fn is_editing(&self) -> bool {
        ConnectionsScreen::is_editing(self)
    }

    fn animating(&self, _h: &History) -> bool {
        ConnectionsScreen::animating(self)
    }

    fn on_esc_top(&mut self, _h: &mut History, _cx: &mut Cx) -> Outcome {
        Outcome::Consumed
    }

    fn stakes(&mut self, _h: &History) -> Stakes {
        self.form_stakes()
    }
}

impl Page<Tablepro> for Workbench {
    fn on_key(&mut self, key: &Key, h: &mut History, cx: &mut Cx) -> Outcome {
        Workbench::on_key(self, key, cx, h)
    }

    fn pointer(&mut self, ev: &PointerEvent, h: &mut History, cx: &mut Cx) -> Outcome {
        Workbench::pointer(self, ev, cx, h)
    }

    fn on_paste(&mut self, text: &str, _h: &mut History, _cx: &mut Cx) -> Outcome {
        Workbench::on_paste(self, text)
    }

    fn on_blur(&mut self, ch: FocusChange, _h: &mut History, _cx: &mut Cx) -> Outcome {
        Workbench::on_blur(self, ch)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, h: &History) {
        Workbench::render(self, area, buf, ctx, h);
    }

    fn hints(&self, focus: Option<WidgetId>, _h: &History) -> Vec<Hint> {
        Workbench::hints(self, focus)
    }

    fn crumb(&self, _h: &History) -> String {
        self.connection.name.clone()
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Workbench::primary_focus(self)
    }

    fn is_editing(&self) -> bool {
        Workbench::is_editing(self)
    }

    fn animating(&self, _h: &History) -> bool {
        Workbench::animating(self)
    }

    fn on_esc_top(&mut self, _h: &mut History, cx: &mut Cx) -> Outcome {
        if self.maximized {
            self.maximized = false;
            let _ = self.modify_active(|tab| {
                if let WorkTab::Query(q) = tab {
                    q.split.maximized = Maximized::None;
                }
            });
            return Outcome::Changed;
        }
        let f = cx.focus.current();
        if f == Some(TABSTRIP) {
            cx.focus.focus(EXPLORER);
            return Outcome::Changed;
        }
        if f == Some(EXPLORER) {
            if self.explorer_filter.text().is_empty() {
                return Outcome::Consumed;
            }
            self.explorer_filter = TextInput::new(self.explorer_filter.id, "")
                .placeholder("Filter objects")
                .plain_label();
            self.explorer.set_filter(None);
            return Outcome::Changed;
        }
        cx.focus.focus(TABSTRIP);
        Outcome::Changed
    }

    fn stakes(&mut self, _h: &History) -> Stakes {
        match self.pending_total() + self.unsaved_queries() {
            0 => Stakes::Clean,
            changes => Stakes::Unsaved { changes },
        }
    }

    fn on_interrupt(&mut self, _h: &mut History, cx: &mut Cx) -> Outcome {
        if let Some(key) = self.active_key()
            && self.cancel_query(key)
        {
            cx.status_as(StatusKind::Info, "Query cancelled");
            return Outcome::Changed;
        }
        Outcome::Ignored
    }
}
