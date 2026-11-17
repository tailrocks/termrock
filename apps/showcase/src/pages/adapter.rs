use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::{Cx, Go, LayerRes, Page, PageEvent};
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::focus::FocusChange;
use junie_tui::core::id::WidgetId;
use junie_tui::core::pointer::PointerEvent;
use junie_tui::shell::{self, PageTypes};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layer::LayerTag;
use junie_tui::ui::leave::Stakes;
use junie_tui::widgets::keyhint::{Hint, hint};

pub struct Showcase;

impl PageTypes for Showcase {
    type State = ();
    type Msg = ();
    type Result = LayerRes;
    type LayerState = ();
    type Go = Go;
}

impl shell::Page<Showcase> for dyn Page {
    fn on_key(&mut self, key: &Key, _: &mut (), cx: &mut Cx) -> Outcome {
        let out = self.handle(&PageEvent::Key(*key), cx);
        if out == Outcome::Ignored
            && let Some(o) = self.template_mut().and_then(|t| t.on_unused_key(key))
        {
            return o;
        }
        out
    }

    fn pointer(&mut self, ev: &PointerEvent, _: &mut (), cx: &mut Cx) -> Outcome {
        self.handle(&PageEvent::Pointer(*ev), cx)
    }

    fn on_paste(&mut self, text: &str, _: &mut (), cx: &mut Cx) -> Outcome {
        self.handle(&PageEvent::Paste(text.to_owned()), cx)
    }

    fn on_blur(&mut self, ch: FocusChange, _: &mut (), cx: &mut Cx) -> Outcome {
        self.handle(&PageEvent::Blur(ch), cx)
    }

    fn on_tick(&mut self, _: &mut (), cx: &mut Cx) -> Outcome {
        self.handle(&PageEvent::Tick, cx)
    }

    fn on_layer_result(
        &mut self,
        tag: &LayerTag,
        result: LayerRes,
        _: &mut (),
        cx: &mut Cx,
    ) -> Outcome {
        match result {
            LayerRes::Dialog { id, result, value } => {
                self.handle(&PageEvent::DialogClosed { id, result, value }, cx)
            }
            LayerRes::Page(report) => self.on_layer(tag, report, cx),
            LayerRes::Lib(_) => Outcome::Ignored,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _: &()) {
        let avail = super::template::Template::body_of(area).as_size();
        let playground = match self.template() {
            Some(t) => t.view.fit(avail, |a| Page::measure(self, a, ctx)),
            None => Page::measure(self, avail, ctx),
        };
        let Some(mut frame) = self
            .template_mut()
            .map(|t| t.begin(area, playground, buf, ctx))
        else {
            Page::render(self, area, buf, ctx);
            return;
        };
        Page::render(self, area, frame.buffer(buf), ctx);
        if let Some(t) = self.template_mut() {
            t.end(frame, buf, ctx);
        }
    }

    fn hints(&self, focus: Option<WidgetId>, _: &()) -> Vec<Hint> {
        Page::hints(self, focus)
            .into_iter()
            .map(|(key, action)| hint(key, action))
            .collect()
    }

    fn crumb(&self, _: &()) -> String {
        self.title().to_owned()
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        None
    }

    fn is_editing(&self) -> bool {
        self.editing()
    }

    fn animating(&self, _: &()) -> bool {
        Page::animating(self)
    }

    fn stakes(&mut self, _: &()) -> Stakes {
        self.leave_stakes()
    }
}
