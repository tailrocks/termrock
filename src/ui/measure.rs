use ratatui::layout::Size;

use crate::ui::ctx::RenderCtx;

/// Natural size of a widget, before it is fitted to an allocation.
///
/// `measure` returns `u16` cells, so a collection or a long label saturates at
/// `u16::MAX`. A saturated width means "at least this wide", never a request to
/// allocate a buffer of that size, and layout always fits against the actual
/// area. Widgets keep their intrinsic widths, counts, gaps and padding wide
/// (`usize`) until that fit and narrow only the final public result with
/// `terminal_extent`, so a result that saturates still takes the overflow path.
pub trait Measure {
    fn measure(&self, avail: Size, ctx: &RenderCtx) -> Size;

    fn min_width(&self, avail: Size, ctx: &RenderCtx) -> u16 {
        self.measure(avail, ctx).width
    }
}

/// Final saturation of a wide cell count into a public `u16` measurement.
///
/// Intermediate widths, counts, gaps and padding stay in `usize` until they are
/// fitted against a finite allocation. A saturated result means "at least this
/// many cells", never a request to allocate that much. Fitters must read the
/// widget's wide intrinsic value instead of recovering a `usize` from a
/// saturated public width, because saturation is not reversible.
pub(crate) fn terminal_extent(cells: usize) -> u16 {
    u16::try_from(cells).unwrap_or(u16::MAX)
}

pub fn at_least(size: Size, min: Size) -> Size {
    Size::new(size.width.max(min.width), size.height.max(min.height))
}

pub struct Tile<'a> {
    pub min_width: u16,
    pub height: &'a dyn Fn(u16) -> u16,

    pub grow: bool,
}

impl<'a> Tile<'a> {
    pub fn new(min_width: u16, height: &'a dyn Fn(u16) -> u16) -> Self {
        Self {
            min_width,
            height,
            grow: true,
        }
    }

    pub fn fixed(mut self) -> Self {
        self.grow = false;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tiled {
    pub rects: Vec<ratatui::layout::Rect>,

    pub size: Size,
}

impl Tiled {
    pub fn at(&self, i: usize, origin: ratatui::layout::Rect) -> ratatui::layout::Rect {
        let r = self.rects.get(i).copied().unwrap_or_default();
        ratatui::layout::Rect::new(origin.x + r.x, origin.y + r.y, r.width, r.height)
    }
}

pub fn tiles(width: u16, rows: &[&[Tile]], gap: (u16, u16)) -> Tiled {
    use ratatui::layout::Rect;
    let width = rows
        .iter()
        .flat_map(|r| r.iter())
        .map(|t| t.min_width)
        .fold(width, u16::max);
    let mut rects = Vec::new();
    let mut y = 0u16;
    for row in rows.iter().filter(|r| !r.is_empty()) {
        if !rects.is_empty() {
            y = y.saturating_add(gap.1);
        }
        let n = row.len();
        let gaps = usize::from(gap.0).saturating_mul(n - 1);
        let mins = row
            .iter()
            .map(|t| usize::from(t.min_width))
            .fold(0usize, usize::saturating_add);
        let avail = usize::from(width);
        if mins.saturating_add(gaps) <= avail {
            let mut widths: Vec<usize> = row.iter().map(|t| usize::from(t.min_width)).collect();
            let mut spare = avail - gaps - mins;
            let fixed: usize = row
                .iter()
                .filter(|t| !t.grow)
                .map(|t| usize::from(t.min_width))
                .sum();
            let growing = row.iter().filter(|t| t.grow).count();
            if let Some(share) = (avail - gaps - fixed).checked_div(growing) {
                for (w, t) in widths.iter_mut().zip(row.iter()) {
                    if t.grow {
                        let add = share.saturating_sub(*w).min(spare);
                        *w += add;
                        spare -= add;
                    }
                }

                if let Some(i) = row.iter().rposition(|t| t.grow) {
                    widths[i] += spare;
                }
            }
            let h = row
                .iter()
                .zip(&widths)
                .map(|(t, &w)| (t.height)(terminal_extent(w)))
                .max()
                .unwrap_or(0);
            let mut x = 0usize;
            for &w in &widths {
                rects.push(Rect::new(terminal_extent(x), y, terminal_extent(w), h));
                x += w + usize::from(gap.0);
            }
            y = y.saturating_add(h);
        } else {
            for (k, t) in row.iter().enumerate() {
                if k > 0 {
                    y = y.saturating_add(gap.1);
                }
                let w = if t.grow { width } else { t.min_width };
                let h = (t.height)(w);
                rects.push(Rect::new(0, y, w, h));
                y = y.saturating_add(h);
            }
        }
    }
    Tiled {
        rects,
        size: Size::new(width, y),
    }
}
