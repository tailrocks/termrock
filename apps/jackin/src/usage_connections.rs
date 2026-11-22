use std::fmt;

use crate::backend::{PlainKey, Ticket};

pub const PRODUCTS: [(&str, &str); 12] = [
    ("claude", "Claude"),
    ("codex", "ChatGPT"),
    ("muse", "Muse"),
    ("grok", "Grok"),
    ("kimi", "Kimi"),
    ("zai", "Z.ai"),
    ("minimax", "MiniMax"),
    ("mimo", "MiMo"),
    ("gemini", "Gemini Code Assist"),
    ("antigravity", "Antigravity (Google AI)"),
    ("opencode", "OpenCode"),
    ("openrouter", "OpenRouter"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageEntry {
    pub id: String,
    pub provider: String,
    pub label: String,
    pub summary: Box<[String]>,
    pub details: Vec<String>,
    pub resets: Vec<ResetItem>,
    pub hub: Option<String>,
    pub region: Option<String>,
    pub device_login: bool,
}

impl UsageEntry {
    pub(crate) fn supports_manual_refresh(&self) -> bool {
        self.provider != "antigravity"
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct LoginChoice {
    pub provider: String,
    pub device_login: Option<bool>,
    pub region: Option<String>,
    pub replace: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum UsageAction {
    Board {
        priority: bool,
        edit: BoardEdit,
    },
    List,
    Hubs,
    ConnectHub {
        address: String,
        key: PlainKey,
        replace: Option<String>,
    },
    SyncHub(String),
    RemoveHub(String),
    Begin(LoginChoice),
    Submit(PlainKey),
    Skip,
    ChooseWorkspace(usize),
    OpenBrowser,
    Refresh(String),
    Redeem {
        account: String,
        index: usize,
        identity: String,
    },
    Remove(String),
    Rename {
        id: String,
        name: String,
    },
    Cancel,
}

impl fmt::Debug for UsageAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Board { .. } => "Board",
            Self::List => "List",
            Self::Hubs => "Hubs",
            Self::ConnectHub { .. } => "ConnectHub",
            Self::SyncHub(_) => "SyncHub",
            Self::RemoveHub(_) => "RemoveHub",
            Self::Begin(_) => "Begin",
            Self::Submit(_) => "Submit",
            Self::Skip => "Skip",
            Self::ChooseWorkspace(_) => "ChooseWorkspace",
            Self::OpenBrowser => "OpenBrowser",
            Self::Refresh(_) => "Refresh",
            Self::Redeem { .. } => "Redeem",
            Self::Remove(_) => "Remove",
            Self::Rename { .. } => "Rename",
            Self::Cancel => "Cancel",
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum UsageUpdate {
    Board(Box<UsageBoard>),
    Accounts(Vec<UsageEntry>),
    Hubs(Vec<HubRow>),
    Key {
        url: String,
    },
    Browser {
        url: String,
        accepts_paste: bool,
    },
    Device {
        url: String,
        code: String,
    },
    Field {
        label: String,
        optional: bool,
        masked: bool,
    },
    AntigravityStatusLine {
        command: String,
    },
    Workspaces(Vec<String>),
    Saved(UsageEntry),
    Failed(String),
    OpenFailed,
}

impl fmt::Debug for UsageUpdate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Accounts(_) => "Accounts",
            Self::Hubs(_) => "Hubs",
            Self::Board(_) => "Board",
            Self::Key { .. } => "Key",
            Self::Browser { .. } => "Browser",
            Self::Device { .. } => "Device",
            Self::AntigravityStatusLine { .. } => "AntigravityStatusLine",
            Self::Workspaces(_) => "Workspaces",
            Self::Field { .. } => "Field",
            Self::Saved(_) => "Saved",
            Self::Failed(_) => "Failed",
            Self::OpenFailed => "OpenFailed",
        })
    }
}

pub trait UsageConnections: Send + Sync {
    fn accounts_opened(&self) {}
    fn has_connected_accounts(&self) -> bool;
    fn request(&self, ticket: Ticket, action: UsageAction);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetricPreference {
    pub account: String,
    pub hidden: bool,
    pub pinned: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardMetric {
    pub member_preferences: Vec<MetricPreference>,
    pub account: Option<String>,
    pub members: Vec<UsageEntry>,
    pub provider: String,
    pub metric: String,
    pub title: String,
    pub label: String,
    /// Whether this reading has a finite quota.
    pub bounded: bool,
    pub value: String,
    pub exact_value: String,
    pub group: String,
    pub bar: Option<(u8, Option<u8>)>,
    pub severity: termrock::theme::Tone,
    pub reset: Option<String>,
    pub inspector: Vec<String>,
    pub details: Vec<String>,
    pub pinned: bool,
}

impl BoardMetric {
    pub(crate) fn bounded_bar(&self) -> Option<(u8, Option<u8>)> {
        self.bar.filter(|_| self.bounded)
    }

    pub fn unified_edit(&self, hide: bool) -> Option<(BoardEdit, BoardEdit)> {
        if self.account.is_some() || self.member_preferences.is_empty() {
            return None;
        }
        let changes: Vec<_> = self
            .member_preferences
            .iter()
            .map(|state| MetricPreference {
                account: state.account.clone(),
                hidden: hide || state.hidden,
                pinned: !hide && !self.pinned && !state.hidden,
            })
            .collect();
        let edit = |before, after| BoardEdit::Unified {
            metric: self.metric.clone(),
            before,
            after,
        };
        Some((
            edit(self.member_preferences.clone(), changes.clone()),
            edit(changes, self.member_preferences.clone()),
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardSettings {
    pub flags: Vec<(&'static str, &'static str, bool)>,
    pub warning: u8,
    pub critical: u8,
    pub hidden: Vec<(String, String, String)>,
    pub proxy: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardProvider {
    pub id: String,
    pub name: String,
    pub accounts: Vec<UsageEntry>,
    pub unavailable_reason: Option<String>,
}

impl BoardProvider {
    pub fn connection_unavailable(&self) -> Option<&str> {
        self.unavailable_reason.as_deref().or_else(|| {
            (!PRODUCTS.iter().any(|(id, _)| *id == self.id))
                .then_some("Adding an account for this provider is unavailable.")
        })
    }

    pub fn status(&self) -> String {
        match (self.connection_unavailable(), self.accounts.len()) {
            (Some(_), 0) => "Quota unavailable".into(),
            (None, 0) => "Not connected".into(),
            (_, 1) => "1 account".into(),
            (_, n) => format!("{n} accounts"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageBoard {
    pub providers: Vec<BoardProvider>,
    pub priority: bool,
    pub summary: String,
    pub notices: Vec<String>,
    pub attention: Vec<UsageEntry>,
    pub empty: bool,
    pub alerts: Vec<String>,
    pub rows: Vec<BoardMetric>,
    pub share: String,
    pub settings: BoardSettings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderingScope {
    Metric,
    Account,
    Provider,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoardEdit {
    Read,
    DefaultTab,
    Refresh(Option<String>),
    Pin {
        account: String,
        metric: String,
    },
    Hide {
        account: String,
        metric: String,
        hidden: bool,
        restore_pin: bool,
    },
    Unified {
        metric: String,
        before: Vec<MetricPreference>,
        after: Vec<MetricPreference>,
    },
    Move {
        scope: OrderingScope,
        account: String,
        metric: String,
        up: bool,
    },
    Settings(BoardSettings),
    Proxy(Option<PlainKey>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResetItem {
    pub index: usize,
    pub identity: String,
    pub description: String,
    pub usable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HubRow {
    pub id: String,
    pub address: String,
    pub refused: bool,
}
