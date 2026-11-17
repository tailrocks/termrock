//! A host that owns its domain records and shows them in a modal picker over
//! a page list, using only the public `termrock` facade.
//!
//! The host keeps one `KeyedVec` per collection. The widgets borrow it for the
//! length of each event or paint call, hold only keys (cursor, selection) and
//! report activations by key. The host never mirrors a selection or a row
//! index: reordering, inserting or removing rows cannot retarget an action.

pub mod layered;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use termrock::core::event::Key;
use termrock::core::id::{ItemKey, WidgetId};
use termrock::core::keyed::{KeyedSourceError, KeyedVec};
use termrock::prelude::SourceStamp;
use termrock::ui::ctx::RenderCtx;
use termrock::widgets::list::{ListAction, ListBox, ListItem, SelectMode};

/// What the host learns from one key press, in the order the widgets emitted it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The user chose the record with this key in the modal.
    Chosen(ItemKey),
    /// The modal's selection changed without a choice.
    SelectionChanged,
}

pub struct Host {
    /// The page behind the modal.
    pub accounts: KeyedVec<ListItem>,
    /// The modal's rows. The domain record is the key's owner; `ListItem` is
    /// the projection the list borrows.
    pub roles: KeyedVec<ListItem>,
    page: ListBox,
    modal: ListBox,
    open: bool,
}

pub const PAGE: WidgetId = WidgetId::of("consumer.page");
pub const MODAL: WidgetId = WidgetId::of("consumer.modal");

impl Host {
    pub fn new(accounts: &[(u64, &str)], roles: &[(u64, &str)]) -> Result<Self, KeyedSourceError> {
        let rows = |rows: &[(u64, &str)]| {
            KeyedVec::try_from_rows(
                rows.iter()
                    .map(|(n, name)| (ItemKey::num(*n), ListItem::new(name))),
            )
        };
        Ok(Self {
            accounts: rows(accounts)?,
            roles: rows(roles)?,
            page: ListBox::new(PAGE, vec![], SelectMode::Single),
            modal: ListBox::new(MODAL, vec![], SelectMode::Single),
            open: false,
        })
    }

    pub fn open_modal(&mut self) {
        self.open = true;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The modal's source revision, which only the provider advances.
    pub fn roles_stamp(&self) -> SourceStamp {
        self.roles.stamp()
    }

    /// The key under the modal's cursor, held by the list, not by the host.
    pub fn modal_cursor(&self) -> Option<ItemKey> {
        self.modal.keyed().cursor()
    }

    pub fn page_cursor(&self) -> Option<ItemKey> {
        self.page.keyed().cursor()
    }

    /// Paints the page and, when open, the modal on top of it. Returns the
    /// widget ids that keyboard focus can reach after the frame.
    pub fn render(
        &mut self,
        screen: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
    ) -> Result<Vec<WidgetId>, KeyedSourceError> {
        self.page
            .render_source(screen, &self.accounts, buf, ctx, ctx.theme.canvas)?;
        if self.open {
            ctx.modal_backdrop(buf, screen);
            let area =
                Rect::new(screen.x + 4, screen.y + 2, screen.width / 2, 6).intersection(screen);
            self.modal
                .render_source(area, &self.roles, buf, ctx, ctx.theme.canvas)?;
        }
        Ok(ctx.ring.reachable().to_vec())
    }

    /// Routes a key to the topmost layer. A closed modal sends it to the page;
    /// an open one swallows it, and Escape closes it.
    pub fn on_key(&mut self, code: KeyCode) -> Result<Vec<Outcome>, KeyedSourceError> {
        let key = Key {
            code,
            mods: KeyModifiers::NONE,
        };
        if !self.open {
            let out = self.page.on_key_source(&key, &self.accounts)?;
            return Ok(outcomes(out.actions().copied()));
        }
        if code == KeyCode::Esc {
            self.open = false;
            return Ok(Vec::new());
        }
        let out = self.modal.on_key_source(&key, &self.roles)?;
        let outcomes = outcomes(out.actions().copied());
        if outcomes.iter().any(|o| matches!(o, Outcome::Chosen(_))) {
            self.open = false;
        }
        Ok(outcomes)
    }
}

fn outcomes(actions: impl Iterator<Item = ListAction>) -> Vec<Outcome> {
    actions
        .map(|action| match action {
            ListAction::Activated(key) => Outcome::Chosen(key),
            ListAction::SelectionChanged => Outcome::SelectionChanged,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use ratatui::buffer::Buffer;
    use termrock::core::focus::FocusRing;
    use termrock::core::hit::HitRegistry;
    use termrock::theme::{ColorLevel, Theme};
    use termrock::ui::ctx::{Interaction, RenderCtx};

    use super::*;

    const SCREEN: Rect = Rect::new(0, 0, 60, 14);

    fn frame(host: &mut Host) -> (Vec<WidgetId>, String) {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(SCREEN);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        let reachable = host
            .render(SCREEN, &mut buf, &mut ctx)
            .expect("valid sources");
        ctx.flush_overlays(&mut buf);
        let text = (0..SCREEN.height)
            .map(|y| {
                (0..SCREEN.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        (reachable, text)
    }

    fn host() -> Host {
        Host::new(
            &[(1, "personal"), (2, "work")],
            &[(10, "reader"), (20, "writer"), (30, "owner")],
        )
        .expect("unique keys")
    }

    #[test]
    fn an_open_modal_owns_focus_and_the_page_keeps_its_cursor() {
        let mut host = host();
        let (reachable, text) = frame(&mut host);
        assert!(reachable.contains(&PAGE), "{reachable:?}");
        assert!(text.contains("personal"));
        host.on_key(KeyCode::Down).expect("valid");
        assert_eq!(host.page_cursor(), Some(ItemKey::num(2)));

        host.open_modal();
        let (reachable, text) = frame(&mut host);
        assert_eq!(reachable, vec![MODAL], "the page left the focus ring");
        assert!(text.contains("writer"));

        host.on_key(KeyCode::Down).expect("valid");
        assert_eq!(host.page_cursor(), Some(ItemKey::num(2)), "page untouched");
        assert_eq!(host.modal_cursor(), Some(ItemKey::num(20)));
    }

    #[test]
    fn a_choice_names_the_record_even_after_the_rows_move() {
        let mut host = host();
        host.open_modal();
        frame(&mut host);
        host.on_key(KeyCode::Down).expect("valid");
        assert_eq!(host.modal_cursor(), Some(ItemKey::num(20)));

        // A new first row shifts every position; the cursor stays on "writer".
        host.roles
            .insert(0, ItemKey::num(5), ListItem::new("guest"))
            .expect("new key");
        let stamp = host.roles_stamp();
        frame(&mut host);
        assert_eq!(
            host.roles_stamp(),
            stamp,
            "painting never advances a revision"
        );
        assert_eq!(host.modal_cursor(), Some(ItemKey::num(20)));

        let outcomes = host.on_key(KeyCode::Enter).expect("valid");
        assert_eq!(outcomes[0], Outcome::Chosen(ItemKey::num(20)));
        assert!(!host.is_open(), "a choice closes the modal");
    }

    #[test]
    fn deleting_the_cursor_row_moves_the_cursor_to_a_neighbour() {
        let mut host = host();
        host.open_modal();
        frame(&mut host);
        host.on_key(KeyCode::Down).expect("valid");
        host.roles.remove(ItemKey::num(20));
        frame(&mut host);
        assert_eq!(host.modal_cursor(), Some(ItemKey::num(30)));
    }

    #[test]
    fn a_provider_that_breaks_the_contract_is_rejected_whole() {
        let duplicate = Host::new(&[(1, "a"), (1, "b")], &[]);
        assert_eq!(
            duplicate.err(),
            Some(KeyedSourceError::DuplicateAuthoritative {
                key: ItemKey::num(1)
            })
        );
    }

    #[test]
    fn escape_closes_the_modal_without_a_choice() {
        let mut host = host();
        host.open_modal();
        frame(&mut host);
        assert_eq!(host.on_key(KeyCode::Esc).expect("valid"), Vec::new());
        assert!(!host.is_open());
    }
}
