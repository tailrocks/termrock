use std::collections::BTreeSet;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Position;

use super::app::App;
use crate::core::event::{Input, Mouse, MouseKind};
use crate::core::hit::{Axis, Bar, HitRegistry};
use crate::core::id::WidgetId;
use crate::runtime::Application;

pub const SIZES: [(u16, u16); 8] = [
    (40, 12),
    (60, 18),
    (72, 20),
    (80, 24),
    (100, 25),
    (106, 29),
    (120, 40),
    (160, 50),
];

const REFERENCE: (u16, u16) = (300, 100);

const MIN_WORD: usize = 3;

const MAX_PAGES: usize = 200;

const STALL: (usize, usize) = (3, 12);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reach {
    pub size: (u16, u16),

    pub too_small: bool,

    pub missing: BTreeSet<String>,

    pub cut: BTreeSet<String>,

    pub bars: usize,
}

impl Reach {
    pub(crate) fn of(
        size: (u16, u16),
        too_small: bool,
        want: &Words,
        reached: &Words,
        bars: usize,
    ) -> Self {
        if too_small {
            return Self {
                size,
                too_small,
                ..Self::default()
            };
        }
        let (missing, cut) = want.against(reached);
        Self {
            size,
            too_small,
            missing,
            cut,
            bars,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Words {
    pub whole: BTreeSet<String>,
    pub cut: BTreeSet<String>,
}

impl Words {
    pub(crate) fn read(buf: &Buffer) -> Self {
        let mut out = Self::default();
        out.add(buf);
        out
    }

    pub(crate) fn add(&mut self, buf: &Buffer) {
        let area = buf.area;
        self.add_area(
            buf,
            ratatui::layout::Rect::new(area.x, area.y, area.width, area.height.saturating_sub(1)),
        );
    }

    pub fn add_area(&mut self, buf: &Buffer, area: ratatui::layout::Rect) {
        let area = area.intersection(buf.area);
        let rows = area.height;
        for y in 0..rows {
            let mut word = String::new();
            for x in 0..area.width {
                let sym = buf[(area.x + x, area.y + y)].symbol();
                let mut chars = sym.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) if c.is_alphanumeric() => word.push(c),
                    (Some('…'), None) => {
                        if !word.is_empty() {
                            self.cut.insert(std::mem::take(&mut word));
                        }
                    }

                    (None, _) => {}
                    _ => self.push(&mut word),
                }
            }
            self.push(&mut word);
        }
    }

    fn push(&mut self, word: &mut String) {
        if word.chars().count() >= MIN_WORD {
            self.whole.insert(word.clone());
        }
        word.clear();
    }

    pub fn add_overflowed(&mut self) {
        for hidden in crate::ui::reveal::overflowed() {
            for word in hidden.split(|c: char| !c.is_alphanumeric()) {
                if word.chars().count() >= MIN_WORD {
                    self.cut.insert(word.to_owned());
                }
            }
        }
    }

    pub fn extend(&mut self, other: Self) {
        self.whole.extend(other.whole);
        self.cut.extend(other.cut);
    }

    pub fn against(&self, reached: &Self) -> (BTreeSet<String>, BTreeSet<String>) {
        let mut missing = BTreeSet::new();
        let mut cut = BTreeSet::new();
        for w in &self.whole {
            if reached.whole.contains(w) {
                continue;
            }

            if reached.whole.iter().any(|r| r.contains(w.as_str())) {
                continue;
            }
            if reached.cut.iter().any(|p| w.starts_with(p.as_str())) {
                cut.insert(w.clone());
            } else {
                missing.insert(w.clone());
            }
        }
        (missing, cut)
    }
}

pub(crate) struct Probe<A> {
    pub(crate) app: A,
    pub(crate) term: Terminal<TestBackend>,
}

impl<A: App + Application> Probe<A> {
    pub(crate) fn new(app: A, term: Terminal<TestBackend>) -> Self {
        let mut p = Self { app, term };
        p.draw();
        p
    }

    pub(crate) fn draw(&mut self) {
        self.term
            .draw(|f| crate::shell::render(&mut self.app, f))
            .expect("test backend draws");
    }

    pub(crate) fn send(&mut self, input: Input) {
        self.app.handle(input);
        self.draw();
    }

    pub(crate) fn resize(&mut self, w: u16, h: u16) {
        self.term.backend_mut().resize(w, h);
        self.send(Input::Resize(w, h));
    }

    pub(crate) fn buffer(&self) -> &Buffer {
        self.term.backend().buffer()
    }

    pub(crate) fn hits(&self) -> &HitRegistry {
        &self.app.shell().hits
    }

    pub(crate) fn words_at(&mut self, size: (u16, u16)) -> Words {
        self.resize(size.0, size.1);
        Words::read(self.buffer())
    }

    pub(crate) fn reference(&mut self, reference: (u16, u16), except: &Words) -> Words {
        let mut want = self.words_at(reference);
        let dims = [reference.0.to_string(), reference.1.to_string()];
        want.whole
            .retain(|w| !dims.contains(w) && !except.whole.iter().any(|e| e.contains(w.as_str())));
        want.cut.clear();
        want
    }

    pub(crate) fn explore(&mut self, want: &Words, reached: &mut Words) -> usize {
        reached.add_overflowed();
        let left = want.against(reached).0;
        let mut acc = Acc {
            reached: std::mem::take(reached),
            left,
            want: want.whole.clone(),
            last: BTreeSet::new(),
        };
        acc.add(self.buffer());
        let mut done: Vec<WidgetId> = Vec::new();
        self.explore_owners(&mut acc, &mut done, 0);
        *reached = acc.reached;
        done.len()
    }

    fn bars(&self) -> Vec<Bar> {
        let hits = self.hits();
        let mut seen: Vec<Bar> = Vec::new();
        for r in hits.regions() {
            if let Some(b) = hits.bar(r.id)
                && !seen.iter().any(|s| s.id == b.id)
            {
                seen.push(b);
            }
        }
        seen
    }

    fn wheel_spot(&self, bar: Bar) -> Position {
        let hits = self.hits();
        let reaches = |p: Position| hits.scroll_stack(p).contains(&bar.owner);
        let middle = hits
            .regions()
            .iter()
            .rev()
            .find(|r| r.owner == bar.owner && r.id == bar.owner && !r.hover_only)
            .map(|r| Position::new(r.area.x + r.area.width / 2, r.area.y + r.area.height / 2));
        if let Some(p) = middle.filter(|&p| reaches(p)) {
            return p;
        }
        let t = bar.track;
        match bar.axis {
            Axis::Vertical => Position::new(t.x.saturating_sub(1), t.y + t.height / 2),
            Axis::Horizontal => Position::new(t.x + t.width / 2, t.y.saturating_sub(1)),
        }
    }

    fn bar_of(&self, owner: WidgetId, axis: Axis) -> Option<Bar> {
        self.bars()
            .into_iter()
            .find(|b| b.owner == owner && b.axis == axis)
    }

    fn explore_owners(&mut self, acc: &mut Acc, done: &mut Vec<WidgetId>, depth: u8) {
        for bar in self.bars() {
            if done.contains(&bar.owner) || acc.done() {
                continue;
            }
            let owner = bar.owner;
            done.push(owner);
            let v = self.bar_of(owner, Axis::Vertical).is_some();
            let h = self.bar_of(owner, Axis::Horizontal).is_some();

            let mut v_pages = 0;
            let mut stalled = 0;
            loop {
                if h {
                    let pages = self.page_to_end(owner, Axis::Horizontal, true, acc);
                    self.page_back(owner, Axis::Horizontal, true, pages);
                }
                if !v || acc.done() || v_pages >= MAX_PAGES || stalled >= STALL.0 {
                    break;
                }
                if !self.page(owner, Axis::Vertical, true) {
                    break;
                }
                v_pages += 1;
                stalled = if acc.add(self.buffer()) {
                    0
                } else {
                    stalled + 1
                };
            }
            if depth == 0 {
                self.explore_owners(acc, done, depth + 1);
            }
            self.page_back(owner, Axis::Vertical, true, v_pages);

            for axis in [Axis::Vertical, Axis::Horizontal] {
                if self.bar_of(owner, axis).is_some() && !acc.done() {
                    let pages = self.page_to_end(owner, axis, false, acc);
                    self.page_back(owner, axis, false, pages);
                }
            }
        }
    }

    fn page_to_end(&mut self, owner: WidgetId, axis: Axis, forward: bool, acc: &mut Acc) -> usize {
        let limit = match axis {
            Axis::Vertical => STALL.0,
            Axis::Horizontal => STALL.1,
        };
        let mut pages = 0;
        let mut stalled = 0;
        while pages < MAX_PAGES && stalled < limit && !acc.done() && self.page(owner, axis, forward)
        {
            pages += 1;
            stalled = if acc.add(self.buffer()) {
                0
            } else {
                stalled + 1
            };
        }
        pages
    }

    fn page_back(&mut self, owner: WidgetId, axis: Axis, forward: bool, pages: usize) {
        if pages == 0 {
            return;
        }
        let Some(bar) = self.bar_of(owner, axis) else {
            return;
        };
        let kind = match (axis, forward) {
            (Axis::Vertical, true) => MouseKind::WheelUp,
            (Axis::Vertical, false) => MouseKind::WheelDown,
            (Axis::Horizontal, true) => MouseKind::WheelLeft,
            (Axis::Horizontal, false) => MouseKind::WheelRight,
        };
        let at = self.wheel_spot(bar);
        for _ in 0..pages * usize::from(notches(bar)) {
            self.app.handle(Input::Mouse(Mouse::new(kind, at)));
        }
        self.draw();
    }

    fn page(&mut self, owner: WidgetId, axis: Axis, forward: bool) -> bool {
        let Some(bar) = self.bar_of(owner, axis) else {
            return false;
        };
        let kind = match (axis, forward) {
            (Axis::Vertical, true) => MouseKind::WheelDown,
            (Axis::Vertical, false) => MouseKind::WheelUp,
            (Axis::Horizontal, true) => MouseKind::WheelRight,
            (Axis::Horizontal, false) => MouseKind::WheelLeft,
        };
        let before = self.buffer().clone();
        let at = self.wheel_spot(bar);
        for _ in 0..notches(bar) {
            self.app.handle(Input::Mouse(Mouse::new(kind, at)));
        }
        self.draw();
        *self.buffer() != before
    }
}

fn notches(bar: Bar) -> u16 {
    match bar.axis {
        Axis::Vertical if bar.track.height < 9 => 1,
        Axis::Vertical => (bar.track.height / 3 - 1).max(1),
        Axis::Horizontal => 1,
    }
}

struct Acc {
    reached: Words,
    left: BTreeSet<String>,

    want: BTreeSet<String>,

    last: BTreeSet<String>,
}

impl Acc {
    fn add(&mut self, buf: &Buffer) -> bool {
        let mut frame = Words::read(buf);
        frame.add_overflowed();
        let fresh = frame
            .whole
            .iter()
            .any(|w| self.want.contains(w) && !self.last.contains(w));
        self.last.clone_from(&frame.whole);
        if !self.left.is_empty() {
            let probe = Words {
                whole: std::mem::take(&mut self.left),
                cut: BTreeSet::new(),
            };
            self.left = probe.against(&frame).0;
        }
        self.reached.extend(frame);
        fresh
    }

    fn done(&self) -> bool {
        self.left.is_empty()
    }
}

fn gate(app: &str, found: &[(String, Reach)], allow: &str) -> Result<(), String> {
    use std::collections::BTreeMap;
    let mut allowed: BTreeMap<(String, String), (BTreeSet<String>, String)> = BTreeMap::new();
    for line in allow.lines() {
        let (body, note) = line.split_once('#').unwrap_or((line, ""));
        let mut it = body.split_whitespace();
        if it.next() != Some(app) {
            continue;
        }
        let (Some(scenario), Some(size)) = (it.next(), it.next()) else {
            continue;
        };

        let (scenario, only) = match scenario.split_once('@') {
            Some((name, plat)) => (name, Some(plat)),
            None => (scenario, None),
        };
        let here = match crate::core::event::ClientPlatform::current() {
            crate::core::event::ClientPlatform::MacOs => "macos",
            crate::core::event::ClientPlatform::Other => "other",
        };
        if only.is_some_and(|p| p != here) {
            continue;
        }
        if only.is_none() && allowed.contains_key(&(scenario.to_string(), size.to_string())) {
            continue;
        }

        let shown = |r: &Reach| format!("{}x{}", r.size.0, r.size.1) == size;
        if !found.iter().any(|(s, r)| s == scenario && shown(r)) {
            continue;
        }
        allowed.insert(
            (scenario.to_string(), size.to_string()),
            (it.map(str::to_string).collect(), note.trim().to_string()),
        );
    }
    let mut want = String::new();
    let mut bad = Vec::new();
    for (scenario, r) in found {
        let size = format!("{}x{}", r.size.0, r.size.1);
        let key = (scenario.clone(), size.clone());
        let (ok, note) = allowed.remove(&key).unwrap_or_default();
        if r.missing.is_empty() {
            if !ok.is_empty() {
                bad.push(format!("{scenario} {size}: fixed, drop its line"));
            }
            continue;
        }
        let words: Vec<&str> = r.missing.iter().map(String::as_str).collect();
        let note = if note.is_empty() {
            "R-apps-? <reason>"
        } else {
            &note
        };
        want.push_str(&format!(
            "{app} {scenario} {size} {} # {note}\n",
            words.join(" ")
        ));
        if ok != r.missing {
            let new: Vec<_> = r.missing.difference(&ok).collect();
            let gone: Vec<_> = ok.difference(&r.missing).collect();
            bad.push(format!(
                "{scenario} {size}: newly hidden {new:?}, now reached {gone:?}"
            ));
        }
    }
    for (scenario, size) in allowed.keys() {
        bad.push(format!("{scenario} {size}: no such row, drop its line"));
    }
    if bad.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{app}: reachability differs from tests/reach_allow.txt\n  {}\n\
             {app}'s lines should read:\n{want}",
            bad.join("\n  ")
        ))
    }
}

pub trait Harness: Sized {
    type App: App + Application;

    fn fresh() -> Self;
    fn into_parts(self) -> (Self::App, Terminal<TestBackend>);
    fn from_parts(app: Self::App, term: Terminal<TestBackend>) -> Self;
}

pub type Setup<H> = fn(&mut H);

pub struct Reveal<H> {
    pub step: Setup<H>,
    pub times: usize,
}

impl<H> Clone for Reveal<H> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<H> Copy for Reveal<H> {}

pub struct Case<H: 'static> {
    pub name: &'static str,
    pub base: Setup<H>,
    pub overlay: Option<Setup<H>>,
    pub reveals: &'static [Reveal<H>],
}

impl<H> Case<H> {
    pub const fn page(name: &'static str, base: Setup<H>) -> Self {
        Self {
            name,
            base,
            overlay: None,
            reveals: &[],
        }
    }

    pub const fn layer(name: &'static str, base: Setup<H>, open: Setup<H>) -> Self {
        Self {
            name,
            base,
            overlay: Some(open),
            reveals: &[],
        }
    }

    pub const fn reveals(mut self, reveals: &'static [Reveal<H>]) -> Self {
        self.reveals = reveals;
        self
    }
}

impl<H: Harness> Case<H> {
    fn build(&self) -> (H, Words) {
        let mut h = H::fresh();
        (self.base)(&mut h);
        let mut except = Words::default();
        if let Some(open) = self.overlay {
            let (app, term) = h.into_parts();
            let mut p = Probe::new(app, term);
            except = p.reference(REFERENCE, &Words::default());
            p.resize(120, 40);
            h = H::from_parts(p.app, p.term);
            open(&mut h);
        }
        (h, except)
    }

    pub fn run(&self, size: (u16, u16)) -> Reach {
        let (h, except) = self.build();
        let (app, term) = h.into_parts();
        let mut p = Probe::new(app, term);
        let want = p.reference(REFERENCE, &except);
        p.resize(size.0, size.1);
        let small = p.app.shell().too_small;
        let mut reached = Words::default();
        let mut bars = p.explore(&want, &mut reached);
        for reveal in self.reveals {
            if small || want.against(&reached).0.is_empty() {
                break;
            }
            let (h, _) = self.build();
            let (app, term) = h.into_parts();
            let mut p = Probe::new(app, term);
            p.resize(size.0, size.1);
            for _ in 0..reveal.times {
                let (app, term) = (p.app, p.term);
                let mut h = H::from_parts(app, term);
                (reveal.step)(&mut h);
                let (app, term) = h.into_parts();
                p = Probe::new(app, term);
                bars += p.explore(&want, &mut reached);
            }
        }
        Reach::of(size, small, &want, &reached, bars)
    }
}

pub fn check<H: Harness>(app: &str, cases: &[Case<H>], name: &str, size: (u16, u16), allow: &str) {
    let case = cases
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no case {name}"));
    let found = [(name.to_string(), case.run(size))];
    if let Err(e) = gate(app, &found, allow) {
        panic!("{e}");
    }
}

pub fn dump<H: Harness>(cases: &[Case<H>]) {
    let Ok(spec) = std::env::var("REACH_DUMP") else {
        return;
    };
    let mut it = spec.split(':');
    let name = it.next().unwrap_or_default();
    let mut num = || it.next().and_then(|n| n.parse().ok()).unwrap_or(80);
    let size = (num(), num());
    let case = cases
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no case {name}"));
    let (h, _) = case.build();
    let (app, term) = h.into_parts();
    let mut p = Probe::new(app, term);
    p.resize(size.0, size.1);
    let buf = p.buffer();
    let mut text = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            text.push_str(buf[(x, y)].symbol());
        }
        text.push('\n');
    }
    let r = case.run(size);
    eprintln!("{text}\nmissing {:?}\ncut {:?}", r.missing, r.cut);
}

#[macro_export]
macro_rules! reach_tests {
    ($app:literal, $h:ty, $cases:expr; $($test:ident = $name:literal,)*) => {
        const REACH_ALLOW: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/reach_allow.txt"
        ));

        $(
            mod $test {
                use super::*;

                fn at(size: (u16, u16)) {
                    $crate::shell::reach::check::<$h>($app, &$cases, $name, size, REACH_ALLOW);
                }

                #[test]
                fn s40x12() {
                    at((40, 12));
                }

                #[test]
                fn s60x18() {
                    at((60, 18));
                }

                #[test]
                fn s72x20() {
                    at((72, 20));
                }

                #[test]
                fn s80x24() {
                    at((80, 24));
                }

                #[test]
                fn s100x25() {
                    at((100, 25));
                }

                #[test]
                fn s106x29() {
                    at((106, 29));
                }

                #[test]
                fn s120x40() {
                    at((120, 40));
                }

                #[test]
                fn s160x50() {
                    at((160, 50));
                }
            }
        )*

        #[test]
        fn every_case_has_its_test() {

            assert_eq!(
                $crate::shell::reach::SIZES,
                [(40, 12), (60, 18), (72, 20), (80, 24), (100, 25), (106, 29), (120, 40), (160, 50)]
            );
            let tested = [$($name),*];
            for case in $cases {
                assert!(tested.contains(&case.name), "{} has no reach test", case.name);
            }
        }

        #[test]
        fn reach_dump() {
            $crate::shell::reach::dump::<$h>(&$cases);
        }
    };
}
