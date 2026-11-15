use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use super::{Hint, PageEvent};
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::intent::{DismissReason, Phase};
use junie_tui::core::pointer::MouseTarget;
use junie_tui::core::response::Actions;
use junie_tui::ui::context::ContextConfig;
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layer;
use junie_tui::widgets::menu::{ContextMenu, MenuAction};

pub const MENU_HINTS: [Hint; 3] = [("↑↓", "Move"), ("Enter", "Choose"), ("Esc", "Close")];

pub const OPEN_HINT: Hint = ("Shift+F10", "Menu");

pub struct RowMenu<T> {
    pub config: ContextConfig,
    open: Option<(ContextMenu, T)>,
}

impl<T: Clone> Default for RowMenu<T> {
    fn default() -> Self {
        Self {
            config: ContextConfig::DEFAULT,
            open: None,
        }
    }
}

impl<T: Clone> RowMenu<T> {
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn hides_page_focus(&self) -> bool {
        self.open
            .as_ref()
            .is_some_and(|(m, _)| m.hides_page_focus())
    }

    pub fn open(&mut self, menu: ContextMenu, target: T) {
        self.open = Some((menu, target));
    }

    pub fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        if let Some((m, _)) = self.open.as_mut() {
            m.render(screen, buf, ctx);
        }
    }

    pub fn handle(&mut self, ev: &PageEvent) -> Option<(Outcome, Option<(T, usize)>)> {
        let (menu, _) = self.open.as_mut()?;
        let (out, event) = match ev {
            PageEvent::Key(key) => {
                let (o, e) = menu.on_key(key);
                if o == Outcome::Ignored && e == Some(MenuAction::Dismissed) {
                    self.open = None;
                    return None;
                }

                (o.or(Outcome::Consumed), Actions::from(e))
            }
            PageEvent::Pointer(p) => match menu.route(p) {
                Some(r) if r.outcome() != Outcome::Ignored => r.into_parts(),
                Some(_) => return None,

                None => match p.phase() {
                    Some(Phase::Click) => (
                        Outcome::Changed,
                        layer::dismiss(menu, DismissReason::OutsideClick)
                            .map(|r| r.into_actions())
                            .unwrap_or_default(),
                    ),
                    Some(Phase::Press | Phase::Drag) => (Outcome::Consumed, Actions::None),
                    _ => return None,
                },
            },
            _ => return None,
        };
        let mut result = Some((out, None));
        for ev in event {
            result = match ev {
                MenuAction::Chosen(i) => {
                    let (_, target) = self.open.take()?;
                    Some((Outcome::Changed, Some((target, i))))
                }
                MenuAction::Dismissed => {
                    self.open = None;
                    Some((Outcome::Changed, None))
                }
            };
        }
        result
    }
}

pub fn plain(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}
