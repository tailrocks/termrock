use std::collections::BTreeMap;

use termrock::ui::text::plural;

use super::account::{Account, AccountId, Lifecycle};
use super::agent::{Provider, UsageSurface};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Freshness {
    Current,
    Stale,
    Refreshing,
    Failed,
}

impl Freshness {
    pub fn label(self) -> &'static str {
        match self {
            Freshness::Current => "current",
            Freshness::Stale => "stale",
            Freshness::Refreshing => "refreshing",
            Freshness::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuotaStatus {
    Available,
    NotStarted,
    Warning,
    Exhausted,
    Unsupported,
    Unavailable,
    Error,
}

impl QuotaStatus {
    pub fn label(self) -> &'static str {
        match self {
            QuotaStatus::Available => "available",
            QuotaStatus::NotStarted => "not started",
            QuotaStatus::Warning => "warning",
            QuotaStatus::Exhausted => "exhausted",
            QuotaStatus::Unsupported => "unsupported",
            QuotaStatus::Unavailable => "unavailable",
            QuotaStatus::Error => "error",
        }
    }

    pub fn from_pct(used_pct: u8) -> Self {
        if used_pct >= 100 {
            QuotaStatus::Exhausted
        } else if used_pct >= 75 {
            QuotaStatus::Warning
        } else {
            QuotaStatus::Available
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowCategory {
    Session,
    LongRange,
    Model,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowUnit {
    Percent,
    Tokens,
    Credits,
    Usd,
}

impl WindowUnit {
    pub fn label(self) -> &'static str {
        match self {
            WindowUnit::Percent => "%",
            WindowUnit::Tokens => "tokens",
            WindowUnit::Credits => "credits",
            WindowUnit::Usd => "USD",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaWindow {
    pub id: &'static str,
    pub label: String,
    pub category: WindowCategory,
    pub unit: WindowUnit,
    pub used: Option<u64>,
    pub limit: Option<u64>,
    pub used_pct: Option<u8>,

    pub reset_secs: Option<i64>,
    pub spend_label: Option<String>,
    pub status: QuotaStatus,
    pub note: Option<String>,
}

impl QuotaWindow {
    pub fn pct(id: &'static str, label: &str, category: WindowCategory, used_pct: u8) -> Self {
        Self {
            id,
            label: label.to_owned(),
            category,
            unit: WindowUnit::Percent,
            used: None,
            limit: None,
            used_pct: Some(used_pct.min(100)),
            reset_secs: None,
            spend_label: None,
            status: QuotaStatus::from_pct(used_pct),
            note: None,
        }
    }

    pub fn counted(
        id: &'static str,
        label: &str,
        category: WindowCategory,
        unit: WindowUnit,
        used: u64,
        limit: u64,
    ) -> Self {
        let pct = (used * 100).checked_div(limit).unwrap_or(0).min(100) as u8;
        Self {
            id,
            label: label.to_owned(),
            category,
            unit,
            used: Some(used),
            limit: Some(limit),
            used_pct: Some(pct),
            reset_secs: None,
            spend_label: None,
            status: QuotaStatus::from_pct(pct),
            note: None,
        }
    }

    pub fn sentinel(id: &'static str, label: &str, status: QuotaStatus, note: &str) -> Self {
        Self {
            id,
            label: label.to_owned(),
            category: WindowCategory::Other,
            unit: WindowUnit::Percent,
            used: None,
            limit: None,
            used_pct: None,
            reset_secs: None,
            spend_label: None,
            status,
            note: Some(note.to_owned()),
        }
    }

    pub fn not_started(id: &'static str, label: &str, category: WindowCategory) -> Self {
        let mut w = Self::pct(id, label, category, 0);
        w.status = QuotaStatus::NotStarted;
        w
    }

    pub fn reset(mut self, secs: i64) -> Self {
        self.reset_secs = Some(secs);
        self
    }

    pub fn spend(mut self, label: &str) -> Self {
        self.spend_label = Some(label.to_owned());
        self
    }

    pub fn status(mut self, s: QuotaStatus) -> Self {
        self.status = s;
        self
    }

    pub fn remaining_pct(&self) -> Option<u8> {
        self.used_pct.map(|p| 100 - p)
    }

    pub fn value_label(&self) -> String {
        match (self.used, self.limit, self.used_pct, self.status) {
            (_, _, _, QuotaStatus::NotStarted) => "not started".into(),
            (Some(u), Some(l), _, _) if self.unit != WindowUnit::Percent => format!(
                "{} / {} {}",
                termrock::ui::text::thousands(u as usize),
                termrock::ui::text::thousands(l as usize),
                self.unit.label()
            ),
            (_, _, Some(p), _) => format!("{p}% used"),
            _ => self
                .note
                .clone()
                .unwrap_or_else(|| self.status.label().to_owned()),
        }
    }

    pub fn has_meter(&self) -> bool {
        self.used_pct.is_some()
            && !matches!(
                self.status,
                QuotaStatus::Unsupported | QuotaStatus::Unavailable | QuotaStatus::Error
            )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreshnessInfo {
    pub phase: Freshness,
    pub last_good_secs: Option<i64>,
    pub retry_secs: Option<i64>,
}

impl FreshnessInfo {
    pub fn current(at: i64) -> Self {
        Self {
            phase: Freshness::Current,
            last_good_secs: Some(at),
            retry_secs: None,
        }
    }
    pub fn stale(last_good: i64, retry: i64) -> Self {
        Self {
            phase: Freshness::Stale,
            last_good_secs: Some(last_good),
            retry_secs: Some(retry),
        }
    }
    pub fn refreshing(last_good: Option<i64>) -> Self {
        Self {
            phase: Freshness::Refreshing,
            last_good_secs: last_good,
            retry_secs: None,
        }
    }
    pub fn failed(last_good: Option<i64>, retry: Option<i64>) -> Self {
        Self {
            phase: Freshness::Failed,
            last_good_secs: last_good,
            retry_secs: retry,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountUsage {
    pub freshness: FreshnessInfo,
    pub windows: Vec<QuotaWindow>,
}

impl AccountUsage {
    pub fn none() -> Self {
        Self {
            freshness: FreshnessInfo::failed(None, None),
            windows: vec![],
        }
    }

    pub fn worst_status(&self) -> Option<QuotaStatus> {
        self.windows
            .iter()
            .map(|w| w.status)
            .max_by_key(|s| match s {
                QuotaStatus::Exhausted => 6,
                QuotaStatus::Error => 5,
                QuotaStatus::Warning => 4,
                QuotaStatus::Unavailable => 3,
                QuotaStatus::Unsupported => 2,
                QuotaStatus::Available => 1,
                QuotaStatus::NotStarted => 0,
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthWord {
    Empty,
    Blocked,
    Degraded,
    Attention,
    Healthy,
}

impl HealthWord {
    pub fn label(self) -> &'static str {
        match self {
            HealthWord::Empty => "empty",
            HealthWord::Blocked => "blocked",
            HealthWord::Degraded => "degraded",
            HealthWord::Attention => "attention",
            HealthWord::Healthy => "healthy",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OverallCounts {
    pub accounts: usize,
    pub enabled: usize,
    pub disabled: usize,
    pub providers: usize,
    pub warnings: usize,
    pub exhausted: usize,
    pub stale: usize,
    pub refreshing: usize,
    pub failed: usize,
    pub unsupported: usize,
    pub unresolved_identity: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparableRollup {
    pub surface: UsageSurface,
    pub window_id: &'static str,
    pub label: String,
    pub unit: WindowUnit,
    pub accounts: usize,
    pub min_remaining_pct: u8,
    pub max_remaining_pct: u8,

    pub summed: Option<(u64, u64)>,
    pub last_good_count: usize,
    pub not_visible: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotComparableNote {
    pub surface: UsageSurface,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverallSummary {
    pub health: HealthWord,
    pub counts: OverallCounts,
    pub comparable: Vec<ComparableRollup>,
    pub not_comparable: Vec<NotComparableNote>,
    pub stale_sources: Vec<AccountId>,
    pub failed_sources: Vec<AccountId>,
}

impl OverallSummary {
    pub fn compute(accounts: &[Account]) -> Self {
        let mut counts = OverallCounts {
            accounts: accounts.len(),
            ..Default::default()
        };
        let mut providers: BTreeMap<Provider, ()> = BTreeMap::new();
        let mut stale_sources = vec![];
        let mut failed_sources = vec![];

        let mut groups: BTreeMap<(UsageSurface, &'static str), Vec<(&QuotaWindow, bool)>> =
            BTreeMap::new();
        let mut not_visible: BTreeMap<UsageSurface, usize> = BTreeMap::new();
        let mut blocked = 0usize;
        for a in accounts {
            providers.insert(a.provider, ());
            if !a.enabled {
                counts.disabled += 1;
                continue;
            }
            counts.enabled += 1;
            if a.identity.subject.is_none() {
                counts.unresolved_identity += 1;
            }
            if matches!(
                a.lifecycle,
                Lifecycle::NeedsLogin | Lifecycle::NeedsSecret | Lifecycle::Error
            ) {
                blocked += 1;
            }
            match a.usage.freshness.phase {
                Freshness::Stale => {
                    counts.stale += 1;
                    stale_sources.push(a.id.clone());
                }
                Freshness::Refreshing => counts.refreshing += 1,
                Freshness::Failed => {
                    counts.failed += 1;
                    failed_sources.push(a.id.clone());
                }
                Freshness::Current => {}
            }
            let last_good = a.usage.freshness.phase != Freshness::Current;
            let all_unsupported = !a.usage.windows.is_empty()
                && a.usage
                    .windows
                    .iter()
                    .all(|w| w.status == QuotaStatus::Unsupported);
            if all_unsupported || a.lifecycle == Lifecycle::Unsupported {
                counts.unsupported += 1;
            }
            for w in &a.usage.windows {
                match w.status {
                    QuotaStatus::Warning => counts.warnings += 1,
                    QuotaStatus::Exhausted => counts.exhausted += 1,
                    QuotaStatus::Unsupported => {
                        *not_visible.entry(a.surface).or_default() += 1;
                        continue;
                    }
                    _ => {}
                }
                if w.used_pct.is_some() {
                    groups
                        .entry((a.surface, w.id))
                        .or_default()
                        .push((w, last_good));
                }
            }
        }
        counts.providers = providers.len();
        let mut comparable = vec![];
        let mut not_comparable = vec![];
        let mut seen_single: BTreeMap<UsageSurface, Vec<&'static str>> = BTreeMap::new();
        for ((surface, id), ws) in &groups {
            if ws.len() < 2 {
                seen_single.entry(*surface).or_default().push(id);
                continue;
            }
            let unit = ws[0].0.unit;
            let category = ws[0].0.category;
            if ws
                .iter()
                .any(|(w, _)| w.unit != unit || w.category != category)
            {
                not_comparable.push(NotComparableNote {
                    surface: *surface,
                    reason: "different units",
                });
                continue;
            }
            let rem: Vec<u8> = ws.iter().filter_map(|(w, _)| w.remaining_pct()).collect();
            let summed = if unit != WindowUnit::Percent
                && ws
                    .iter()
                    .all(|(w, lg)| !lg && w.used.is_some() && w.limit.is_some())
            {
                Some((
                    ws.iter().map(|(w, _)| w.used.unwrap_or(0)).sum(),
                    ws.iter().map(|(w, _)| w.limit.unwrap_or(0)).sum(),
                ))
            } else {
                None
            };
            comparable.push(ComparableRollup {
                surface: *surface,
                window_id: id,
                label: ws[0].0.label.clone(),
                unit,
                accounts: ws.len(),
                min_remaining_pct: rem.iter().copied().min().unwrap_or(0),
                max_remaining_pct: rem.iter().copied().max().unwrap_or(0),
                summed,
                last_good_count: ws.iter().filter(|(_, lg)| *lg).count(),
                not_visible: not_visible.get(surface).copied().unwrap_or(0),
            });
        }
        for (surface, ids) in seen_single {
            if !comparable.iter().any(|c| c.surface == surface) {
                let _ = ids;
                not_comparable.push(NotComparableNote {
                    surface,
                    reason: "single account or different windows",
                });
            }
        }
        let health = if counts.accounts == 0 {
            HealthWord::Empty
        } else if counts.enabled > 0
            && (blocked == counts.enabled || counts.failed == counts.enabled)
        {
            HealthWord::Blocked
        } else if counts.exhausted > 0
            || counts.failed > 0
            || counts.stale * 2 >= counts.enabled.max(1)
        {
            HealthWord::Degraded
        } else if counts.warnings > 0
            || counts.stale > 0
            || counts.unresolved_identity > 0
            || counts.unsupported > 0
        {
            HealthWord::Attention
        } else {
            HealthWord::Healthy
        };
        Self {
            health,
            counts,
            comparable,
            not_comparable,
            stale_sources,
            failed_sources,
        }
    }

    pub fn counts_line(&self) -> String {
        let c = &self.counts;
        let mut parts = vec![plural(c.accounts, "account", "accounts")];
        parts.push(format!("{} enabled", c.enabled));
        if c.disabled > 0 {
            parts.push(format!("{} disabled", c.disabled));
        }
        parts.push(plural(c.providers, "provider", "providers"));
        parts.join(" · ")
    }

    pub fn issues_line(&self) -> String {
        let parts: Vec<String> = self.issue_parts().into_iter().map(|(p, _)| p).collect();
        parts.join(" · ")
    }

    pub fn issue_parts(&self) -> Vec<(String, termrock::theme::Tone)> {
        use termrock::theme::Tone;
        let c = &self.counts;
        let mut parts = vec![];
        if c.warnings > 0 {
            parts.push((plural(c.warnings, "warning", "warnings"), Tone::Warning));
        }
        if c.exhausted > 0 {
            parts.push((format!("{} exhausted", c.exhausted), Tone::Error));
        }
        if c.stale > 0 {
            parts.push((format!("{} stale", c.stale), Tone::Warning));
        }
        if c.refreshing > 0 {
            parts.push((format!("{} refreshing", c.refreshing), Tone::Info));
        }
        if c.failed > 0 {
            parts.push((format!("{} failed", c.failed), Tone::Error));
        }
        if c.unsupported > 0 {
            parts.push((format!("{} quota not visible", c.unsupported), Tone::Normal));
        }
        if c.unresolved_identity > 0 {
            parts.push((
                format!(
                    "{} identit{} unresolved",
                    c.unresolved_identity,
                    if c.unresolved_identity == 1 {
                        "y"
                    } else {
                        "ies"
                    }
                ),
                Tone::Normal,
            ));
        }
        if parts.is_empty() {
            parts.push(("no warnings".into(), Tone::Normal));
        }
        parts
    }
}
