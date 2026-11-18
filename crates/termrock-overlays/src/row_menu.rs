use crate::core::event::{Key, Outcome};
use crate::core::id::WidgetId;
use crate::ui::context::{ContextAnchor, ContextConfig};
use crate::ui::ctx::RenderCtx;
use crate::widgets::keyhint::{Hint, hint};
use crate::widgets::menu::{ContextMenu, MenuAction, MenuItem};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

struct Open<T, C> {
    target: T,
    menu: ContextMenu,
    commands: Vec<C>,
}

pub struct RowMenu<T, C> {
    pub config: ContextConfig,
    id: WidgetId,
    open: Option<Open<T, C>>,
}

impl<T: Clone, C: Clone> RowMenu<T, C> {
    pub fn new(id: WidgetId) -> Self {
        Self {
            config: ContextConfig::DEFAULT,
            id,
            open: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn hides_page_focus(&self) -> bool {
        self.open
            .as_ref()
            .is_some_and(|o| o.menu.hides_page_focus())
    }

    pub fn is_context_key(&self, key: &Key) -> bool {
        self.config.keys.matches(key)
    }

    pub fn open(
        &mut self,
        target: T,
        anchor: ContextAnchor,
        title: impl Into<String>,
        items: Vec<(MenuItem, C)>,
    ) {
        let (items, commands): (Vec<MenuItem>, Vec<C>) = items.into_iter().unzip();
        let menu = anchor.apply(ContextMenu::new(self.id, items).title(title));
        self.open = Some(Open {
            target,
            menu,
            commands,
        });
    }

    fn resolve(&mut self, ev: Option<MenuAction>) -> Option<(T, C)> {
        match ev {
            Some(MenuAction::Chosen(i)) => {
                let o = self.open.take()?;
                o.commands.get(i).cloned().map(|c| (o.target, c))
            }
            Some(MenuAction::Dismissed) => {
                self.open = None;
                None
            }
            None => None,
        }
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<(T, C)>) {
        let Some(o) = self.open.as_mut() else {
            return (Outcome::Ignored, None);
        };
        let (out, ev) = o.menu.on_key(key);
        if out == Outcome::Ignored && ev == Some(MenuAction::Dismissed) {
            self.open = None;
            return (Outcome::Ignored, None);
        }

        (out.or(Outcome::Consumed), self.resolve(ev))
    }

    pub fn on_click(&mut self, id: WidgetId) -> (Outcome, Option<(T, C)>) {
        let Some(o) = self.open.as_mut() else {
            return (Outcome::Ignored, None);
        };
        let ev = o.menu.on_click(id);
        (Outcome::Changed, self.resolve(ev))
    }

    pub fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        if let Some(o) = self.open.as_mut() {
            o.menu.render(screen, buf, ctx);
        }
    }
}

pub fn hints() -> Vec<Hint> {
    vec![
        hint("↑↓", "Move"),
        hint("Enter", "Choose"),
        hint("Esc", "Close"),
    ]
}
