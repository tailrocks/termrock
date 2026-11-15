use junie_tui::ui::text::fuzzy;

use crate::domain::action::{Item, Signal};
use crate::domain::context::Scope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranked {
    pub index: usize,
    pub score: i64,
    pub matched: Vec<usize>,
    pub reason: String,

    pub tag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Query {
    pub text: String,
    pub scope: Option<ScopeToken>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeToken {
    Here,
    Parent,
    Children,
    System,
    All,
}

impl Query {
    pub fn parse(raw: &str) -> Self {
        let mut scope = None;
        let mut words = vec![];
        for w in raw.split_whitespace() {
            match w.to_lowercase().as_str() {
                "@here" | "@current" => scope = Some(ScopeToken::Here),
                "@parent" | "@ancestors" => scope = Some(ScopeToken::Parent),
                "@children" | "@child" => scope = Some(ScopeToken::Children),
                "@system" | "@host" => scope = Some(ScopeToken::System),
                "@all" => scope = Some(ScopeToken::All),
                _ => words.push(w),
            }
        }
        Self {
            text: words.join(" "),
            scope,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

pub fn effective_scope(readout: Scope, token: Option<ScopeToken>) -> (Option<Scope>, bool) {
    match token {
        Some(ScopeToken::All) => (None, true),
        Some(ScopeToken::Here) => (Some(Scope::Here), true),
        Some(ScopeToken::Parent) => (Some(Scope::Parent), true),
        Some(ScopeToken::Children) => (Some(Scope::Children), true),
        Some(ScopeToken::System) => (Some(Scope::System), true),
        None => (Some(readout), false),
    }
}

fn in_scope(item: &Item, scope: Option<Scope>, strict: bool, has_query: bool) -> bool {
    match scope {
        None => true,
        Some(Scope::Here) if !strict => {
            item.scope.direction == Scope::Here
                || item.signal() >= Signal::Urgency
                || has_query
                || item.kind == crate::domain::action::Kind::Explore
                || item.kind == crate::domain::action::Kind::Activity
        }
        Some(s) => item.scope.direction == s,
    }
}

fn word_score(item: &Item, word: &str) -> Option<(u32, Vec<usize>)> {
    let mut best: Option<(u32, Vec<usize>)> = None;
    let mut consider = |score: u32, matched: Vec<usize>| {
        if best.as_ref().is_none_or(|b| score < b.0) {
            best = Some((score, matched));
        }
    };
    if let Some((s, m)) = fuzzy(&item.label, word) {
        consider(s, m);
    }
    let lw = word.to_lowercase();
    for k in &item.keywords {
        let k = k.to_lowercase();
        if k == lw {
            consider(5, vec![]);
        } else if k.starts_with(&lw) {
            consider(12, vec![]);
        } else if k.contains(&lw) {
            consider(35, vec![]);
        }
    }
    if item.kind.label().starts_with(&lw) || item.group.to_lowercase().starts_with(&lw) {
        consider(40, vec![]);
    }
    if item.scope.word.starts_with(&lw) {
        consider(45, vec![]);
    }
    best
}

pub fn search(items: &[Item], raw: &str, readout: Scope, group: Option<&str>) -> Vec<Ranked> {
    search_with(items, raw, readout, group, None)
}

pub fn search_with(
    items: &[Item],
    raw: &str,
    readout: Scope,
    group: Option<&str>,
    learned: Option<&str>,
) -> Vec<Ranked> {
    let q = Query::parse(raw);
    let (scope, strict) = effective_scope(readout, q.scope);
    let has_query = !q.is_empty();
    let text = q.text.trim().to_lowercase();
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut out = vec![];
    for (i, item) in items.iter().enumerate() {
        if item.hidden && !has_query {
            continue;
        }

        let alias_hit = has_query
            && (item.aliases.iter().any(|a| a.eq_ignore_ascii_case(&text))
                || (group.is_some()
                    && item
                        .page_aliases
                        .iter()
                        .any(|a| a.eq_ignore_ascii_case(&text))));
        if let Some(g) = group {
            if item.group != g || item.kind == crate::domain::action::Kind::Explore {
                continue;
            }
        } else if !alias_hit && !in_scope(item, scope, strict, has_query) {
            continue;
        }
        let mut score: i64 = 0;
        let mut matched = vec![];
        let mut tag = None;
        let mut reason = item
            .top_reason()
            .map(|r| r.text.clone())
            .unwrap_or_default();
        if item.used_here > 0 && item.signal() > Signal::LocalUse {
            reason = format!(
                "{reason} · used {} here",
                if item.used_here == 1 {
                    "once".to_owned()
                } else {
                    format!("{} times", item.used_here)
                }
            );
        }
        if has_query {
            if alias_hit {
                tag = Some(format!("alias {text}"));
                reason = format!("alias {text}");
                score += 5_000_000;
            } else {
                let mut total = 0u32;
                for w in &words {
                    match word_score(item, w) {
                        Some((s, m)) => {
                            total += s;
                            if matched.is_empty() {
                                matched = m;
                            }
                        }
                        None => {
                            total = u32::MAX;
                            break;
                        }
                    }
                }
                if total == u32::MAX {
                    continue;
                }

                let phrase = item.label.to_lowercase().contains(&text);

                let core = item
                    .label
                    .split(" · ")
                    .next()
                    .unwrap_or(&item.label)
                    .to_lowercase();
                let exact = core == text;
                if phrase {
                    total =
                        (core.chars().count().saturating_sub(text.chars().count())).min(199) as u32;
                }

                let bucket: i64 = if exact {
                    10
                } else if phrase {
                    9
                } else {
                    match total / words.len().max(1) as u32 {
                        0..=9 => 8,
                        10..=19 => 7,
                        20..=39 => 6,
                        40..=49 => 4,
                        _ => 2,
                    }
                };
                score += bucket * 100_000 + (200 - (total as i64).min(199));
            }
        }

        let tier = match item.signal() {
            Signal::Pin => 7,
            Signal::Urgency => 6,
            Signal::Context => 5,
            Signal::LocalUse => 3,
            Signal::GlobalUse => 2,
            Signal::Default => 1,
        };

        if item.pinned {
            score += 90_000;
            reason = format!("pinned here · {reason}");
        }
        if has_query && learned == Some(item.id.as_str()) {
            score += 950_000;
            reason = format!("remembered for “{}” · {reason}", text);
        }
        if item.frecency > 0.0 {
            score += (item.frecency.clamp(0.0, 1.0) * crate::domain::usage::BOOST_CAP * 100_000.0)
                as i64;
        }
        score += tier * 5_000;
        score += (item.used_here.min(50) as i64) * 100;
        score += (item.used_anywhere.min(50) as i64) * 10;
        if item.scope.direction == Scope::Here {
            score += 500;
        }
        if item.hidden {
            score -= 2_000_000;
            reason = format!("hidden here · {reason}");
        }
        out.push(Ranked {
            index: i,
            score,
            matched,
            reason,
            tag,
        });
    }
    out.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| items[a.index].label.cmp(&items[b.index].label))
    });
    out
}

pub fn explain(item: &Item, cwd_short: &str) -> Vec<(String, String)> {
    let mut rows = vec![];
    for r in &item.reasons {
        let label = match r.signal {
            Signal::Pin => "pinned",
            Signal::Urgency => "live state",
            Signal::Context => "context",
            Signal::LocalUse => "used here",
            Signal::GlobalUse => "used elsewhere",
            Signal::Default => "default",
        };
        rows.push((label.to_owned(), r.text.clone()));
    }
    if item.used_here > 0 {
        rows.push((
            "used here".into(),
            format!("{} times in {cwd_short}", item.used_here),
        ));
    }
    if item.used_anywhere > 0 {
        rows.push((
            "used anywhere".into(),
            format!("{} times on this host", item.used_anywhere),
        ));
    }
    if !item.aliases.is_empty() {
        rows.push(("aliases".into(), item.aliases.join(", ")));
    }
    rows.push((
        "risk".into(),
        format!(
            "{} · {} · never changes the rank",
            item.risk.label(),
            item.confirmation.label()
        ),
    ));
    rows
}
