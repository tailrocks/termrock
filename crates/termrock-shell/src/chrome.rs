use ratatui::layout::Rect;

use crate::core::id::WidgetId;
use crate::widgets::dialog::Dialog;
use crate::widgets::menu::{MenuBar, MenuBarAction, MenuItem};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Regions {
    pub header: Rect,

    pub body: Rect,

    pub footer: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chrome {
    pub header: u16,

    pub gap: u16,

    pub footer: u16,

    pub inset: u16,
}

impl Default for Chrome {
    fn default() -> Self {
        Self::new()
    }
}

impl Chrome {
    pub const fn new() -> Self {
        Self {
            header: 1,
            gap: 1,
            footer: 1,
            inset: 0,
        }
    }

    pub const fn header(mut self, rows: u16) -> Self {
        self.header = rows;
        self
    }

    pub const fn gap(mut self, rows: u16) -> Self {
        self.gap = rows;
        self
    }

    pub const fn footer(mut self, rows: u16) -> Self {
        self.footer = rows;
        self
    }

    pub const fn inset(mut self, cols: u16) -> Self {
        self.inset = cols;
        self
    }

    pub fn regions(&self, area: Rect) -> Regions {
        let header_h = self.header.min(area.height);
        let footer_h = self.footer.min(area.height - header_h);
        let header = Rect::new(area.x, area.y, area.width, header_h);
        let footer = Rect::new(area.x, area.bottom() - footer_h, area.width, footer_h);
        let top = if header_h > 0 { header_h + self.gap } else { 0 };
        let bottom = if footer_h > 0 { footer_h + self.gap } else { 0 };
        let body_y = (area.y + top).min(area.bottom());
        let body_h = area.height.saturating_sub(top + bottom);
        let body = Rect::new(
            area.x + self.inset.min(area.width / 2),
            body_y,
            area.width.saturating_sub(self.inset * 2),
            body_h,
        );
        Regions {
            header,
            body,
            footer,
        }
    }

    pub fn rows(&self) -> (u16, u16) {
        (self.header, self.footer)
    }

    pub fn fit(&self, d: &mut Dialog) {
        let (top, bottom) = self.rows();
        d.chrome = (d.chrome.0.max(top), d.chrome.1.max(bottom));
    }
}

#[derive(Debug, Clone)]
pub struct Menus<C> {
    menus: Vec<(String, Vec<(MenuItem, C)>)>,
}

impl<C> Default for Menus<C> {
    fn default() -> Self {
        Self { menus: vec![] }
    }
}

impl<C: Clone> Menus<C> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn menu(mut self, title: impl Into<String>, items: Vec<(MenuItem, C)>) -> Self {
        self.menus.push((title.into(), items));
        self
    }

    pub fn titles(&self) -> impl Iterator<Item = &str> {
        self.menus.iter().map(|(t, _)| t.as_str())
    }

    pub fn items(&self, m: usize) -> Vec<MenuItem> {
        self.menus
            .get(m)
            .map(|(_, v)| v.iter().map(|(i, _)| i.clone()).collect())
            .unwrap_or_default()
    }

    pub fn bar(&self, id: WidgetId) -> MenuBar {
        MenuBar::new(
            id,
            self.menus
                .iter()
                .enumerate()
                .map(|(m, (t, _))| (t.as_str(), self.items(m)))
                .collect(),
        )
    }

    pub fn get(&self, m: usize, i: usize) -> Option<C> {
        self.menus.get(m)?.1.get(i).map(|(_, c)| c.clone())
    }

    pub fn command(&self, action: MenuBarAction) -> Option<C> {
        match action {
            MenuBarAction::Chosen(m, i) => self.get(m, i),
            _ => None,
        }
    }
}
