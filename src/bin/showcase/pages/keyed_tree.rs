//! The keyed connections card of the Trees page (#301).
//!
//! A `TreeView` shows a `KeyedTree` through the keyed source methods. A fault
//! wraps the store in a provider that breaks the tree contract (a child that
//! does not exist, a node under its own descendant), and the view rejects it
//! with the matching `KeyedSourceError` while keeping its previous state.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::pages::Cx;
use termrock::core::event::{Key, Outcome};
use termrock::core::id::{ItemKey, WidgetId};
use termrock::core::keyed::KeyedSourceError;
use termrock::core::list::SourceStamp;
use termrock::core::pointer::PointerEvent;
use termrock::core::response::Response;
use termrock::ui::ctx::RenderCtx;
use termrock::widgets::empty::{self, EmptyState};
use termrock::widgets::tree::{
    KeyedTree, KeyedTreeNode, TreeAction, TreeNodeRef, TreeSource, TreeView,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeFault {
    None,
    MissingNode,
    Cycle,
}

impl TreeFault {
    pub fn from_value(value: &str) -> Self {
        match value {
            "missing node" => Self::MissingNode,
            "cycle" => Self::Cycle,
            _ => Self::None,
        }
    }
}

fn node(label: &str) -> KeyedTreeNode {
    KeyedTreeNode::new(label)
}

fn key(name: &str) -> ItemKey {
    ItemKey::text(name)
}

fn seed() -> KeyedTree {
    let mut tree = KeyedTree::new();
    let rows: [(Option<&str>, &str, KeyedTreeNode); 7] = [
        (None, "production", node("production").glyph("◆")),
        (
            Some("production"),
            "orders-db",
            node("orders-db").meta("postgres"),
        ),
        (
            Some("production"),
            "billing-db",
            node("billing-db").meta("postgres"),
        ),
        (
            Some("production"),
            "replica-eu",
            node("replica-eu").meta("lag 4.2 s"),
        ),
        (None, "staging", node("staging").glyph("◇")),
        (
            Some("staging"),
            "staging-orders",
            node("orders-db").meta("postgres"),
        ),
        (None, "archive", node("archive").glyph("○")),
    ];
    for (parent, name, node) in rows {
        tree.insert(parent.map(key), usize::MAX, key(name), node)
            .expect("the seed tree is well formed");
    }
    tree
}

/// The store seen through a fault. With `TreeFault::None` it is the store.
struct Source<'a> {
    tree: &'a KeyedTree,
    fault: TreeFault,
    altered: Vec<ItemKey>,
}

impl<'a> Source<'a> {
    fn new(tree: &'a KeyedTree, fault: TreeFault) -> Self {
        let altered = match fault {
            TreeFault::None => Vec::new(),
            TreeFault::MissingNode => {
                let mut kids = tree
                    .get(key("production"))
                    .map_or_else(Vec::new, |n| n.children().to_vec());
                kids.push(key("decommissioned"));
                kids
            }
            TreeFault::Cycle => vec![key("production")],
        };
        Self {
            tree,
            fault,
            altered,
        }
    }
}

impl TreeSource for Source<'_> {
    fn stamp(&self) -> SourceStamp {
        let stamp = TreeSource::stamp(self.tree);
        match self.fault {
            TreeFault::None => stamp,
            _ => SourceStamp {
                data: stamp.data + 1_000,
                ..stamp
            },
        }
    }

    fn root_count(&self) -> usize {
        self.tree.root_count()
    }

    fn root_key(&self, index: usize) -> ItemKey {
        self.tree.root_key(index)
    }

    fn node(&self, at: ItemKey) -> Option<TreeNodeRef<'_>> {
        let found = TreeSource::node(self.tree, at)?;
        let target = match self.fault {
            TreeFault::None => return Some(found),
            TreeFault::MissingNode => key("production"),
            TreeFault::Cycle => key("orders-db"),
        };
        Some(if at == target {
            TreeNodeRef {
                children: &self.altered,
                ..found
            }
        } else {
            found
        })
    }
}

pub fn describe_error(error: &KeyedSourceError) -> String {
    match error {
        KeyedSourceError::MissingNode { .. } => {
            "MissingNode: a node lists a child the store does not hold".to_owned()
        }
        KeyedSourceError::Cycle { .. } => "Cycle: a node sits under its own descendant".to_owned(),
        other => format!("{other:?}"),
    }
}

pub struct KeyedConnections {
    tree: KeyedTree,
    pub view: TreeView,
    fault: TreeFault,
    error: Option<KeyedSourceError>,
    pub last: String,
}

impl KeyedConnections {
    pub fn new(id: WidgetId) -> Self {
        Self {
            tree: seed(),
            view: TreeView::new(id, Vec::new()),
            fault: TreeFault::None,
            error: None,
            last: "—".into(),
        }
    }

    pub fn set_fault(&mut self, fault: TreeFault) {
        self.fault = fault;
    }

    pub fn meta(&self) -> String {
        format!("{} nodes · keyed", self.tree.len())
    }

    pub fn status(&self) -> String {
        match &self.error {
            Some(error) => format!("rejected: {}", describe_error(error)),
            None => format!(
                "accepted: {} nodes, every child reachable once",
                self.tree.len()
            ),
        }
    }

    fn react(&mut self, response: Response<TreeAction<ItemKey>>, cx: &mut Cx) -> Outcome {
        let outcome = response.outcome();
        let tree = &self.tree;
        let mut steps = Vec::new();
        let name = |k: ItemKey| tree.get(k).map_or_else(String::new, |n| n.label.clone());
        response
            .on_actions(|action| match action {
                TreeAction::Expand(k) => steps.push(format!("Expand({})", name(k))),
                TreeAction::Activate(k) => {
                    cx.status(format!("Opened {}", name(k)));
                    steps.push(format!("Activate({})", name(k)));
                }
            })
            .outcome();
        if !steps.is_empty() {
            self.last = steps.join(" → ");
        }
        outcome
    }

    pub fn on_key(&mut self, ev: &Key, cx: &mut Cx) -> Outcome {
        let source = Source::new(&self.tree, self.fault);
        match self.view.on_key_source(ev, &source) {
            Ok(response) => {
                self.error = None;
                self.react(response, cx)
            }
            Err(error) => {
                self.error = Some(error);
                Outcome::Changed
            }
        }
    }

    pub fn on_pointer(&mut self, ev: &PointerEvent, cx: &mut Cx) -> Option<Outcome> {
        let source = Source::new(&self.tree, self.fault);
        match self.view.on_pointer_source(ev, &source) {
            Ok(response) => {
                self.error = None;
                let out = self.react(response, cx);
                (out != Outcome::Ignored).then_some(out)
            }
            Err(error) => {
                self.error = Some(error);
                Some(Outcome::Changed)
            }
        }
    }

    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: ratatui::style::Color,
    ) {
        let source = Source::new(&self.tree, self.fault);
        match self.view.render_source(area, &source, buf, ctx, bg) {
            Ok(()) => self.error = None,
            Err(error) => {
                ctx.control(self.view.id, area, false);
                empty::render_ctx(
                    area,
                    buf,
                    ctx,
                    &EmptyState::new("Source rejected").hint(&format!(
                        "{} · the tree keeps its state",
                        describe_error(&error)
                    )),
                    bg,
                );
                self.error = Some(error);
            }
        }
    }
}
