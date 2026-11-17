use std::collections::BTreeSet;

use crate::domain::manifest::{Json, json_opt_str, json_str, parse_json};

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
        let cutoff = now_secs.saturating_sub(STALE_DAYS * 86_400);
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
        let actions: Vec<Json> = actions
            .iter()
            .map(|u| {
                serde_json::json!({
                    "item": u.item,
                    "path": u.path,
                    "host": u.host,
                    "stamps": u.stamps,
                })
            })
            .collect();
        let mut queries: Vec<&QueryChoice> = self.queries.iter().collect();
        queries.sort_by(|a, b| a.query.cmp(&b.query).then_with(|| a.host.cmp(&b.host)));
        let queries: Vec<Json> = queries
            .iter()
            .map(|c| {
                serde_json::json!({
                    "query": c.query,
                    "item": c.item,
                    "host": c.host,
                    "last": c.last,
                })
            })
            .collect();
        serde_json::json!({ "v": 1, "actions": actions, "queries": queries }).to_string()
    }

    fn parse_action(it: &Json, now_secs: i64) -> Result<ActionUse, String> {
        let mut stamps: Vec<i64> = match it.get("stamps") {
            None => vec![],
            Some(Json::Array(values)) => values
                .iter()
                .map(|v| {
                    v.as_i64()
                        .map(|s| s.min(now_secs))
                        .ok_or_else(|| "stamps: expected signed integer".to_owned())
                })
                .collect::<Result<_, _>>()?,
            Some(_) => return Err("stamps: expected array".into()),
        };
        stamps.sort();
        if stamps.len() > MAX_USES {
            let drop = stamps.len() - MAX_USES;
            stamps.drain(..drop);
        }
        Ok(ActionUse {
            item: json_str(it, "item")?.into(),
            path: json_opt_str(it, "path")?,
            host: json_str(it, "host")?.into(),
            stamps,
        })
    }

    fn parse_query(it: &Json, now_secs: i64) -> Result<QueryChoice, String> {
        let last = match it.get("last") {
            None => 0,
            Some(v) => v
                .as_i64()
                .ok_or_else(|| "last: expected signed integer".to_owned())?
                .min(now_secs),
        };
        Ok(QueryChoice {
            query: json_str(it, "query")?.into(),
            item: json_str(it, "item")?.into(),
            host: json_str(it, "host")?.into(),
            last,
        })
    }

    pub fn load(text: Option<&str>, now_secs: i64) -> Self {
        let mut store = Self::default();
        let Some(text) = text else {
            return store;
        };
        let json = match parse_json(text) {
            Ok(j) => j,
            Err(e) => {
                store.load_error = Some(format!("frecency.json unreadable: {e} · starting empty"));
                return store;
            }
        };
        match json.get("v") {
            Some(v) => match v.as_i64() {
                Some(1) => {}
                Some(n) => {
                    store.load_error =
                        Some(format!("frecency.json schema {n} unknown · starting empty"));
                    return store;
                }
                None => {
                    store.load_error = Some(
                        "frecency.json schema version is not an integer · starting empty".into(),
                    );
                    return store;
                }
            },
            None => {
                store.load_error =
                    Some("frecency.json has no schema version · starting empty".into());
                return store;
            }
        }
        match json.get("actions") {
            None => {}
            Some(Json::Array(items)) => {
                for (i, it) in items.iter().enumerate() {
                    match Self::parse_action(it, now_secs) {
                        Ok(action) => store.actions.push(action),
                        Err(why) => {
                            if store.load_error.is_none() {
                                store.load_error =
                                    Some(format!("frecency.json action {i} invalid: {why}"));
                            }
                        }
                    }
                }
            }
            Some(_) => store.load_error = Some("frecency.json actions is not an array".into()),
        }
        match json.get("queries") {
            None => {}
            Some(Json::Array(items)) => {
                for (i, it) in items.iter().enumerate() {
                    match Self::parse_query(it, now_secs) {
                        Ok(query) => store.queries.push(query),
                        Err(why) => {
                            if store.load_error.is_none() {
                                store.load_error =
                                    Some(format!("frecency.json query {i} invalid: {why}"));
                            }
                        }
                    }
                }
            }
            Some(_) => store.load_error = Some("frecency.json queries is not an array".into()),
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
            if disk.loaded_from.is_some() {
                self.merge(&disk);
            }
        }
        self.prune(now_secs);
        self.saves += 1;
        Ok(self.serialize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000;

    #[test]
    fn holla_json_strings_roundtrip_all_escape_classes() {
        let item = "run \"fmt\"\\n\tnext\u{1}🚀";
        let mut store = UsageStore::default();
        store.actions.push(ActionUse {
            item: item.into(),
            path: Some("/w/a\\nb\n".into()),
            host: "h\"ost".into(),
            stamps: vec![NOW],
        });
        store.queries.push(QueryChoice {
            query: "q\\n\"x".into(),
            item: item.into(),
            host: "h".into(),
            last: NOW,
        });
        let text = store.serialize();
        assert!(serde_json::from_str::<serde_json::Value>(&text).is_ok());
        let loaded = UsageStore::load(Some(&text), NOW);
        assert_eq!(loaded.load_error, None);
        assert_eq!(loaded.actions, store.actions);
        assert_eq!(loaded.queries, store.queries);
    }

    #[test]
    fn holla_json_integer_fields_are_exact_and_checked() {
        let stamp = 9_007_199_254_740_993;
        let mut store = UsageStore::default();
        store.actions.push(ActionUse {
            item: "i".into(),
            path: None,
            host: "h".into(),
            stamps: vec![stamp],
        });
        store.queries.push(QueryChoice {
            query: "q".into(),
            item: "i".into(),
            host: "h".into(),
            last: stamp,
        });
        let loaded = UsageStore::load(Some(&store.serialize()), i64::MAX);
        assert_eq!(loaded.actions[0].stamps, vec![stamp]);
        assert_eq!(loaded.queries[0].last, stamp);

        let fractional = r#"{"v":1,"actions":[{"item":"i","path":null,"host":"h","stamps":[1.5,1e3]}],"queries":[]}"#;
        let rejected = UsageStore::load(Some(fractional), NOW);
        assert!(rejected.actions.is_empty());
        assert!(rejected.load_error.is_some());
    }

    #[test]
    fn holla_json_extreme_timestamps_do_not_overflow() {
        let text = format!(
            r#"{{"v":1,"actions":[{{"item":"i","path":null,"host":"h","stamps":[{min}]}},{{"item":"f","path":null,"host":"h","stamps":[{max}]}}],"queries":[{{"query":"q","item":"i","host":"h","last":{min}}}]}}"#,
            min = i64::MIN,
            max = i64::MAX
        );
        let loaded = UsageStore::load(Some(&text), NOW);
        assert_eq!(loaded.frecency("i", None, "h", NOW), 0.0);
        assert!((loaded.frecency("f", None, "h", NOW) - 0.5).abs() < 1e-12);
        let mut pruned = loaded;
        pruned.prune(NOW);
        let items: Vec<&str> = pruned.actions.iter().map(|a| a.item.as_str()).collect();
        assert_eq!(items, ["f"]);
        assert!(pruned.queries.is_empty());
    }

    #[test]
    fn holla_json_invalid_entries_keep_valid_siblings_with_diagnostic() {
        let text = r#"{"v":1,"actions":[{"item":"a","path":null,"host":"h","stamps":[1]},{"item":7,"path":null,"host":"h","stamps":[1]},{"item":"c","path":null,"host":"h","stamps":[3]}],"queries":[]}"#;
        let loaded = UsageStore::load(Some(text), NOW);
        let items: Vec<&str> = loaded.actions.iter().map(|a| a.item.as_str()).collect();
        assert_eq!(items, ["a", "c"]);
        assert!(
            loaded
                .load_error
                .as_deref()
                .is_some_and(|e| e.contains("action 1"))
        );
    }

    #[test]
    fn holla_json_wrong_type_actions_report_corruption() {
        let loaded = UsageStore::load(Some(r#"{"v":1,"actions":5,"queries":[]}"#), NOW);
        assert!(loaded.actions.is_empty());
        assert!(loaded.load_error.is_some());
    }

    #[test]
    fn holla_json_partial_disk_store_still_merges_valid_entries() {
        let disk = format!(
            r#"{{"v":1,"actions":[{{"item":"a","path":null,"host":"h","stamps":[{NOW}]}},{{"item":7,"path":null,"host":"h","stamps":[1]}}],"queries":[]}}"#
        );
        let mut mine = UsageStore::default();
        mine.used("b", None, "h", NOW);
        let text = mine.save(Some(&disk), NOW).unwrap();
        let saved = UsageStore::load(Some(&text), NOW);
        assert!(saved.find("a", None, "h").is_some());
        assert!(saved.find("b", None, "h").is_some());
    }
}
