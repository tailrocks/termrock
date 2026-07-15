use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use crate::interaction::HitRegion;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone)]
pub struct StatusSlot<'a, Id> {
    pub id: Id,
    pub content: &'a str,
    pub priority: u8,
    pub min_width: u16,
    pub enabled: bool,
    pub style: Style,
}

#[derive(Debug, Clone, Copy)]
pub struct StatusBar<'a, Id> {
    pub left: &'a [StatusSlot<'a, Id>],
    pub right: &'a [StatusSlot<'a, Id>],
}

impl<Id: Clone> StatusBar<'_, Id> {
    #[must_use]
    pub fn regions(&self, area: Rect) -> Vec<HitRegion<Id>> {
        let mut regions = Vec::new();
        let mut x = area.x;
        for slot in self.left.iter().filter(|slot| slot.enabled) {
            let width = UnicodeWidthStr::width(slot.content)
                .max(slot.min_width as usize)
                .min(u16::MAX as usize) as u16;
            regions.push(HitRegion {
                id: slot.id.clone(),
                area: Rect::new(x, area.y, width.min(area.right().saturating_sub(x)), 1),
            });
            x = x.saturating_add(width);
        }
        let mut right = area.right();
        for slot in self.right.iter().rev().filter(|slot| slot.enabled) {
            let width = UnicodeWidthStr::width(slot.content)
                .max(slot.min_width as usize)
                .min(u16::MAX as usize) as u16;
            let start = right.saturating_sub(width).max(area.x);
            regions.push(HitRegion {
                id: slot.id.clone(),
                area: Rect::new(
                    start,
                    area.y,
                    right.saturating_sub(start),
                    area.height.min(1),
                ),
            });
            right = start;
        }
        regions
    }
}

impl<Id: Clone + PartialEq> Widget for &StatusBar<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        for region in self.regions(area) {
            if let Some(slot) = self
                .left
                .iter()
                .chain(self.right)
                .find(|slot| slot.id == region.id)
            {
                buffer.set_stringn(
                    region.area.x,
                    region.area.y,
                    slot.content,
                    region.area.width as usize,
                    slot.style,
                );
            }
        }
    }
}
