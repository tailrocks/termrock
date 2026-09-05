// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Applications mount of the same TablePro [`crate::tablepro::App`] used by
// `cargo run -p termrock-catalog --bin tablepro`.

//! Catalog Applications page: live TablePro.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use termrock::style::ColorCapability;

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent, Request};
use crate::tablepro::App as TableProApp;

pub struct TableProPage {
    app: TableProApp,
}

impl TableProPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            app: TableProApp::new(ColorCapability::Truecolor),
        }
    }
}

impl Page for TableProPage {
    fn title(&self) -> &'static str {
        "TablePro"
    }
    fn blurb(&self) -> &'static str {
        "Database workbench: connections, explorer, SQL, grid, Safe Mode"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        self.app.size = (area.width, area.height);
        self.app.render_surface(area, buf, ctx);
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        self.app.handle_surface(ev, cx)
    }

    fn handle_request(&mut self, request: &Request) -> bool {
        let Request::OpenTableFilter {
            index,
            column,
            value,
        } = request
        else {
            return false;
        };
        self.app.open_table_filter(*index, *column, value.clone());
        true
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        self.app.hints(focus)
    }

    fn editing(&self) -> bool {
        self.app.editing()
    }

    fn animating(&self) -> bool {
        self.app.animating()
    }
}
