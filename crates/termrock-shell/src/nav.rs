use crate::core::id::WidgetId;
use crate::theme::Strings;
use crate::ui::leave::{Leave, LeaveGuard, Stakes};
use crate::widgets::breadcrumb::Breadcrumb;
use crate::widgets::dialog::Dialog;

use super::PageKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nav<R> {
    Push(R),

    Replace(R),

    Reset(R),

    Back,

    PopTo(usize),

    Tab(usize),
}

#[derive(Debug)]
struct Entry<P> {
    key: u64,
    page: P,
}

#[derive(Debug)]
pub struct Router<P> {
    tabs: Vec<Vec<Entry<P>>>,
    active: usize,
    next_key: u64,
}

impl<P> Router<P> {
    pub fn new(root: P) -> Self {
        Self::with_tabs(vec![root])
    }

    pub fn with_tabs(roots: Vec<P>) -> Self {
        assert!(!roots.is_empty(), "a router needs at least one tab");
        let mut r = Self {
            tabs: vec![],
            active: 0,
            next_key: 0,
        };
        for p in roots {
            let e = r.entry(p);
            r.tabs.push(vec![e]);
        }
        r
    }

    fn entry(&mut self, page: P) -> Entry<P> {
        self.next_key += 1;
        Entry {
            key: self.next_key,
            page,
        }
    }

    pub fn current(&self) -> &P {
        &self.tabs[self.active]
            .last()
            .expect("a tab is never empty")
            .page
    }

    pub fn current_mut(&mut self) -> &mut P {
        &mut self.tabs[self.active]
            .last_mut()
            .expect("a tab is never empty")
            .page
    }

    pub fn key(&self) -> PageKey {
        PageKey(self.tabs[self.active].last().map_or(0, |e| e.key))
    }

    pub fn stack(&self) -> impl DoubleEndedIterator<Item = &P> + ExactSizeIterator {
        self.tabs[self.active].iter().map(|e| &e.page)
    }

    pub fn stack_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut P> + ExactSizeIterator {
        self.tabs[self.active].iter_mut().map(|e| &mut e.page)
    }

    pub fn depth(&self) -> usize {
        self.tabs[self.active].len()
    }

    pub fn tab(&self, i: usize) -> impl DoubleEndedIterator<Item = &P> + ExactSizeIterator {
        self.tabs[i].iter().map(|e| &e.page)
    }

    pub fn top_of(&self, i: usize) -> Option<&P> {
        self.tabs.get(i)?.last().map(|e| &e.page)
    }

    pub fn top_of_mut(&mut self, i: usize) -> Option<&mut P> {
        self.tabs.get_mut(i)?.last_mut().map(|e| &mut e.page)
    }

    pub fn active(&self) -> usize {
        self.active
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn pages(&self) -> impl Iterator<Item = &P> {
        self.tabs.iter().flatten().map(|e| &e.page)
    }

    pub fn pages_mut(&mut self) -> impl Iterator<Item = &mut P> {
        self.tabs.iter_mut().flatten().map(|e| &mut e.page)
    }

    pub fn push(&mut self, page: P) {
        let e = self.entry(page);
        self.tabs[self.active].push(e);
    }

    pub fn back(&mut self) -> Option<P> {
        let stack = &mut self.tabs[self.active];
        if stack.len() < 2 {
            return None;
        }
        stack.pop().map(|e| e.page)
    }

    pub fn pop_to(&mut self, depth: usize) -> Vec<P> {
        let stack = &mut self.tabs[self.active];
        if depth + 1 >= stack.len() {
            return vec![];
        }
        stack.drain(depth + 1..).map(|e| e.page).collect()
    }

    pub fn replace(&mut self, page: P) -> P {
        let e = self.entry(page);
        let stack = &mut self.tabs[self.active];
        std::mem::replace(stack.last_mut().expect("a tab is never empty"), e).page
    }

    pub fn reset(&mut self, page: P) -> Vec<P> {
        let e = self.entry(page);
        std::mem::replace(&mut self.tabs[self.active], vec![e])
            .into_iter()
            .map(|e| e.page)
            .collect()
    }

    pub fn select(&mut self, i: usize) -> bool {
        if i >= self.tabs.len() || i == self.active {
            return false;
        }
        self.active = i;
        true
    }

    pub fn open_tab(&mut self, root: P) -> usize {
        let e = self.entry(root);
        self.tabs.push(vec![e]);
        self.active = self.tabs.len() - 1;
        self.active
    }

    pub fn close_tab(&mut self, i: usize) -> Option<Vec<P>> {
        if self.tabs.len() < 2 || i >= self.tabs.len() {
            return None;
        }
        let pages = self.tabs.remove(i).into_iter().map(|e| e.page).collect();
        if self.active > i || (self.active == i && i > 0) {
            self.active -= 1;
        }
        Some(pages)
    }

    pub fn drops<R>(&self, nav: &Nav<R>) -> bool {
        match nav {
            Nav::Push(_) | Nav::Tab(_) => false,
            Nav::Replace(_) | Nav::Reset(_) => true,
            Nav::Back => self.depth() > 1,
            Nav::PopTo(d) => d + 1 < self.depth(),
        }
    }

    pub fn dropped_by<R>(&self, nav: &Nav<R>) -> impl Iterator<Item = &P> {
        let stack = &self.tabs[self.active];
        let n = stack.len();
        let from = match nav {
            Nav::Push(_) | Nav::Tab(_) => n,
            Nav::Replace(_) => n - 1,

            Nav::Back if n < 2 => n,
            Nav::Back => n - 1,
            Nav::Reset(_) => 0,
            Nav::PopTo(d) => (*d + 1).min(n),
        };
        stack[from..].iter().rev().map(|e| &e.page)
    }

    pub fn request<R: Clone>(
        &self,
        guard: &mut LeaveGuard<Nav<R>>,
        nav: Nav<R>,
        stakes: impl FnOnce() -> Stakes,
        strings: &Strings,
    ) -> Result<Nav<R>, Box<Dialog>> {
        if !self.drops(&nav) {
            return Ok(nav);
        }
        match guard.request(Leave::To(nav), &stakes(), strings)? {
            Leave::To(nav) => Ok(nav),
            Leave::Quit => unreachable!("a route request resolves to its route"),
        }
    }

    pub fn apply<R>(&mut self, nav: Nav<R>, build: impl FnOnce(R) -> P) -> Option<Vec<P>> {
        match nav {
            Nav::Push(r) => {
                self.push(build(r));
                Some(vec![])
            }
            Nav::Replace(r) => Some(vec![self.replace(build(r))]),
            Nav::Reset(r) => Some(self.reset(build(r))),
            Nav::Back => self.back().map(|p| vec![p]),
            Nav::PopTo(d) => {
                let dropped = self.pop_to(d);
                (!dropped.is_empty()).then_some(dropped)
            }
            Nav::Tab(i) => self.select(i).then(Vec::new),
        }
    }

    pub fn breadcrumb(&self, id: WidgetId, label: impl Fn(&P) -> String) -> Breadcrumb {
        Breadcrumb::new(id, self.stack().map(label).collect())
    }

    pub fn crumb_click<R>(
        &self,
        crumb: &Breadcrumb,
        levels: usize,
        hit: WidgetId,
    ) -> Option<Nav<R>> {
        let level = crumb.on_click(hit)?;
        let depth = level.checked_sub(levels)?;
        (depth + 1 < self.depth()).then_some(Nav::PopTo(depth))
    }
}
