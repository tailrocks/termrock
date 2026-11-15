use std::collections::BTreeSet;

pub const MAX_USES: usize = 20;
pub const HALF_LIFE_DAYS: f64 = 10.0;
pub const STALE_DAYS: i64 = 90;
pub const RECENT_THRESHOLD: f64 = 0.05;
pub const RECENT_MAX: usize = 5;

pub const BOOST_CAP: f64 = 0.25;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionUse {
    pub item: String,

    pub path: Option<String>,
    pub host: String,

    pub stamps: Vec<i64>,
}

impl ActionUse {
    pub fn count(&self) -> u32 {
        self.stamps.len() as u32
    }
    pub fn last_secs(&self) -> i64 {
        self.stamps.last().copied().unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryChoice {
    pub query: String,
    pub item: String,
    pub host: String,
    pub last: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageStore {
    pub actions: Vec<ActionUse>,
    pub queries: Vec<QueryChoice>,

    pub enabled: bool,
    pub load_error: Option<String>,
    pub save_error: Option<String>,
    pub saves: u32,
    pub loaded_from: Option<&'static str>,
}

impl Default for UsageStore {
    fn default() -> Self {
        Self {
            actions: vec![],
            queries: vec![],
            enabled: true,
            load_error: None,
            save_error: None,
            saves: 0,
            loaded_from: None,
        }
    }
}

pub fn normalize_query(q: &str) -> String {
    q.split_whitespace()
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

impl UsageStore {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    fn entry(&mut self, item: &str, path: Option<&str>, host: &str) -> &mut ActionUse {
        if let Some(i) = self
            .actions
            .iter()
            .position(|u| u.item == item && u.path.as_deref() == path && u.host == host)
        {
            return &mut self.actions[i];
        }
        self.actions.push(ActionUse {
            item: item.into(),
            path: path.map(str::to_owned),
            host: host.into(),
            stamps: vec![],
        });
        self.actions.last_mut().unwrap()
    }

    pub fn used(&mut self, item: &str, path: Option<&str>, host: &str, now_secs: i64) {
        if !self.enabled {
            return;
        }
        let e = self.entry(item, path, host);
        e.stamps.push(now_secs);
        e.stamps.sort();
        if e.stamps.len() > MAX_USES {
            let drop = e.stamps.len() - MAX_USES;
            e.stamps.drain(..drop);
        }
    }

    pub fn seed(&mut self, item: &str, path: Option<&str>, host: &str, count: u32, last_secs: i64) {
        let e = self.entry(item, path, host);
        e.stamps = (0..count.min(MAX_USES as u32) as i64)
            .map(|i| last_secs - (count as i64 - 1 - i) * 600)
            .collect();
    }

    pub fn learn_query(&mut self, query: &str, item: &str, host: &str, now_secs: i64) {
        if !self.enabled {
            return;
        }
        let q = normalize_query(query);
        if q.is_empty() {
            return;
        }
        self.queries.retain(|c| !(c.query == q && c.host == host));
        self.queries.push(QueryChoice {
            query: q,
            item: item.into(),
            host: host.into(),
            last: now_secs,
        });
    }

    pub fn learned_for(&self, query: &str, host: &str) -> Option<&str> {
        let q = normalize_query(query);
        self.queries
            .iter()
            .find(|c| c.query == q && c.host == host)
            .map(|c| c.item.as_str())
    }

    pub fn find(&self, item: &str, path: Option<&str>, host: &str) -> Option<&ActionUse> {
        self.actions
            .iter()
            .find(|u| u.item == item && u.path.as_deref() == path && u.host == host)
    }

    pub fn frecency(&self, item: &str, path: Option<&str>, host: &str, now_secs: i64) -> f64 {
        let Some(u) = self.find(item, path, host) else {
            return 0.0;
        };
        let raw: f64 = u
            .stamps
            .iter()
            .map(|s| {
                let age_days = now_secs.saturating_sub(*s).max(0) as f64 / 86_400.0;
                0.5f64.powf(age_days / HALF_LIFE_DAYS)
            })
            .sum();
        raw / (raw + 1.0)
    }

    pub fn frecency_any(&self, item: &str, path: &str, host: &str, now_secs: i64) -> f64 {
        let here = self.frecency(item, Some(path), host, now_secs);
        let global = self.frecency(item, None, host, now_secs);
        here.max(global)
    }

    pub fn recent(&self, path: &str, host: &str, now_secs: i64) -> Vec<(String, f64)> {
        let items: BTreeSet<&str> = self
            .actions
            .iter()
            .filter(|u| u.host == host && (u.path.as_deref() == Some(path) || u.path.is_none()))
            .map(|u| u.item.as_str())
            .collect();
        let mut v: Vec<(String, f64)> = items
            .into_iter()
            .map(|i| (i.to_owned(), self.frecency_any(i, path, host, now_secs)))
            .filter(|(_, s)| *s > RECENT_THRESHOLD)
            .collect();
        v.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        v.truncate(RECENT_MAX);
        v
    }

    pub fn prune(&mut self, now_secs: i64) {
        let cutoff = now_secs - STALE_DAYS * 86_400;
        for u in &mut self.actions {
            u.stamps.retain(|s| *s >= cutoff);
        }
        self.actions.retain(|u| !u.stamps.is_empty());
        let alive: BTreeSet<&str> = self.actions.iter().map(|u| u.item.as_str()).collect();
        self.queries
            .retain(|q| alive.contains(q.item.as_str()) && q.last >= cutoff);
    }

    pub fn forget(&mut self, item: &str) {
        self.actions.retain(|u| u.item != item);
        self.queries.retain(|q| q.item != item);
    }

    pub fn serialize(&self) -> String {
        let mut actions: Vec<&ActionUse> = self.actions.iter().collect();
        actions.sort_by(|a, b| {
            a.item
                .cmp(&b.item)
                .then_with(|| a.host.cmp(&b.host))
                .then_with(|| a.path.cmp(&b.path))
        });
        let a: Vec<String> = actions
            .iter()
            .map(|u| {
                format!(
                    "{{\"item\":{},\"path\":{},\"host\":{},\"stamps\":[{}]}}",
                    js(&u.item),
                    u.path.as_deref().map(js).unwrap_or("null".into()),
                    js(&u.host),
                    u.stamps
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect();
        let mut queries: Vec<&QueryChoice> = self.queries.iter().collect();
        queries.sort_by(|a, b| a.query.cmp(&b.query).then_with(|| a.host.cmp(&b.host)));
        let q: Vec<String> = queries
            .iter()
            .map(|c| {
                format!(
                    "{{\"query\":{},\"item\":{},\"host\":{},\"last\":{}}}",
                    js(&c.query),
                    js(&c.item),
                    js(&c.host),
                    c.last
                )
            })
            .collect();
        format!(
            "{{\"v\":1,\"actions\":[{}],\"queries\":[{}]}}",
            a.join(","),
            q.join(",")
        )
    }

    pub fn load(text: Option<&str>, now_secs: i64) -> Self {
        let mut store = Self::default();
        let Some(text) = text else {
            return store;
        };
        use crate::domain::manifest::{Json, parse_json};
        let json = match parse_json(text) {
            Ok(j) => j,
            Err(e) => {
                store.load_error = Some(format!("frecency.json unreadable: {e} · starting empty"));
                return store;
            }
        };
        match json.get("v") {
            Some(Json::Num(n)) if *n as i64 == 1 => {}
            Some(Json::Num(n)) => {
                store.load_error = Some(format!(
                    "frecency.json schema {} unknown · starting empty",
                    *n as i64
                ));
                return store;
            }
            _ => {
                store.load_error =
                    Some("frecency.json has no schema version · starting empty".into());
                return store;
            }
        }
        if let Some(Json::Arr(items)) = json.get("actions") {
            for it in items {
                let (Some(item), Some(host)) = (
                    it.get("item").and_then(Json::as_str),
                    it.get("host").and_then(Json::as_str),
                ) else {
                    continue;
                };
                let path = it.get("path").and_then(Json::as_str).map(str::to_owned);
                let mut stamps: Vec<i64> = match it.get("stamps") {
                    Some(Json::Arr(s)) => s
                        .iter()
                        .filter_map(|v| match v {
                            Json::Num(n) => Some((*n as i64).min(now_secs)),
                            _ => None,
                        })
                        .collect(),
                    _ => vec![],
                };
                stamps.sort();
                if stamps.len() > MAX_USES {
                    let drop = stamps.len() - MAX_USES;
                    stamps.drain(..drop);
                }
                store.actions.push(ActionUse {
                    item: item.into(),
                    path,
                    host: host.into(),
                    stamps,
                });
            }
        }
        if let Some(Json::Arr(items)) = json.get("queries") {
            for it in items {
                let (Some(query), Some(item), Some(host)) = (
                    it.get("query").and_then(Json::as_str),
                    it.get("item").and_then(Json::as_str),
                    it.get("host").and_then(Json::as_str),
                ) else {
                    continue;
                };
                let last = match it.get("last") {
                    Some(Json::Num(n)) => (*n as i64).min(now_secs),
                    _ => 0,
                };
                store.queries.push(QueryChoice {
                    query: query.into(),
                    item: item.into(),
                    host: host.into(),
                    last,
                });
            }
        }
        store.loaded_from = Some("frecency.json");
        store
    }

    pub fn merge(&mut self, other: &UsageStore) {
        for o in &other.actions {
            let e = self.entry(&o.item, o.path.as_deref(), &o.host);
            let mut all: Vec<i64> = e.stamps.iter().chain(o.stamps.iter()).copied().collect();
            all.sort();
            all.dedup();
            if all.len() > MAX_USES {
                let drop = all.len() - MAX_USES;
                all.drain(..drop);
            }
            e.stamps = all;
        }
        for q in &other.queries {
            match self
                .queries
                .iter_mut()
                .find(|c| c.query == q.query && c.host == q.host)
            {
                Some(c) if c.last < q.last => *c = q.clone(),
                Some(_) => {}
                None => self.queries.push(q.clone()),
            }
        }
    }

    pub fn save(&mut self, on_disk: Option<&str>, now_secs: i64) -> Result<String, String> {
        if !self.enabled {
            return Err("history disabled (HOLLA_NO_HISTORY=1)".into());
        }
        if let Some(e) = &self.save_error {
            return Err(e.clone());
        }
        if let Some(text) = on_disk {
            let disk = UsageStore::load(Some(text), now_secs);
            if disk.load_error.is_none() {
                self.merge(&disk);
            }
        }
        self.prune(now_secs);
        self.saves += 1;
        Ok(self.serialize())
    }
}

fn js(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}
