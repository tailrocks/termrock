use crate::domain::digest::sha256_hex;
use crate::domain::manifest::{Json, json_opt_str, json_str, parse_json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Global,
    Project,
}

impl Origin {
    pub fn label(self) -> &'static str {
        match self {
            Origin::Global => "global",
            Origin::Project => "project",
        }
    }
    pub fn default_group(self) -> &'static str {
        match self {
            Origin::Global => "Custom",
            Origin::Project => "Current folder",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Danger {
    Safe,
    Mutating,
    Destructive,
}

impl Danger {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "safe" => Some(Danger::Safe),
            "mutating" => Some(Danger::Mutating),
            "destructive" => Some(Danger::Destructive),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Danger::Safe => "safe",
            Danger::Mutating => "mutating",
            Danger::Destructive => "destructive",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomAction {
    pub id: String,
    pub label: String,

    pub argv: Vec<String>,
    pub danger: Danger,
    pub confirm: bool,
    pub description: String,
    pub keywords: Vec<String>,
    pub group: String,
    pub origin: Origin,

    pub index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub path: String,
    pub index: Option<usize>,
    pub message: String,
}

impl Diagnostic {
    pub fn text(&self) -> String {
        match self.index {
            Some(i) => format!("{} action[{i}]: {}", self.path, self.message),
            None => format!("{}: {}", self.path, self.message),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomConfig {
    pub path: String,
    pub origin: Origin,
    pub text: String,
    pub digest: String,
    pub actions: Vec<CustomAction>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
enum Val {
    Str(String),
    Bool(bool),
    Arr(Vec<String>),
    Other,
}

fn parse_string(s: &str) -> Result<(String, &str), String> {
    let s = s.trim_start();
    let quote = s.chars().next().ok_or("expected a string")?;
    if quote != '"' && quote != '\'' {
        return Err("expected a quoted string".into());
    }
    let body = &s[1..];
    if quote == '\'' {
        let end = body.find('\'').ok_or("unterminated string")?;
        return Ok((body[..end].to_owned(), &body[end + 1..]));
    }
    let mut out = String::new();
    let mut chars = body.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Ok((out, &body[i + 1..])),
            '\\' => match chars.next() {
                Some((_, 'n')) => out.push('\n'),
                Some((_, 't')) => out.push('\t'),
                Some((_, '"')) => out.push('"'),
                Some((_, '\\')) => out.push('\\'),
                Some((_, other)) => {
                    out.push('\\');
                    out.push(other);
                }
                None => return Err("unterminated escape".into()),
            },
            c => out.push(c),
        }
    }
    Err("unterminated string".into())
}

fn parse_value(s: &str) -> Result<Val, String> {
    let s = s.trim();
    if s == "true" {
        return Ok(Val::Bool(true));
    }
    if s == "false" {
        return Ok(Val::Bool(false));
    }
    if let Some(inner) = s.strip_prefix('[') {
        let inner = inner.strip_suffix(']').ok_or("unterminated array")?;
        let mut items = vec![];
        let mut rest = inner.trim();
        while !rest.is_empty() {
            let (item, after) = parse_string(rest)?;
            items.push(item);
            rest = after.trim_start();
            if let Some(r) = rest.strip_prefix(',') {
                rest = r.trim_start();
            } else if !rest.is_empty() {
                return Err("expected ',' in array".into());
            }
        }
        return Ok(Val::Arr(items));
    }
    if s.starts_with('"') || s.starts_with('\'') {
        let (v, rest) = parse_string(s)?;
        if !rest.trim().is_empty() && !rest.trim().starts_with('#') {
            return Err("trailing data after string".into());
        }
        return Ok(Val::Str(v));
    }
    Ok(Val::Other)
}

fn strip_comment(line: &str) -> &str {
    let mut in_str: Option<char> = None;
    for (i, c) in line.char_indices() {
        match in_str {
            Some(q) if c == q => in_str = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => in_str = Some(c),
            None if c == '#' => return &line[..i],
            None => {}
        }
    }
    line
}

const REQUIRED: [&str; 4] = ["id", "label", "command", "danger"];

fn id_ok(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

pub fn parse_config(
    path: &str,
    origin: Origin,
    text: &str,
    builtin_ids: &[&str],
    reserved: &[String],
) -> CustomConfig {
    let mut cfg = CustomConfig {
        path: path.into(),
        origin,
        text: text.into(),
        digest: sha256_hex(text.as_bytes()),
        actions: vec![],
        diagnostics: vec![],
    };
    let mut entries: Vec<Vec<(String, Val, usize)>> = vec![];
    let mut in_action = false;
    for (ln, raw) in text.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if line == "[[action]]" {
            entries.push(vec![]);
            in_action = true;
            continue;
        }
        if line.starts_with("[[") || line.starts_with('[') {
            in_action = false;
            if line.starts_with("[action") && !line.starts_with("[[") {
                cfg.diagnostics.push(Diagnostic {
                    path: path.into(),
                    index: None,
                    message: format!(
                        "line {}: `action` must be an array of tables ([[action]])",
                        ln + 1
                    ),
                });
            }
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            cfg.diagnostics.push(Diagnostic {
                path: path.into(),
                index: None,
                message: format!("line {}: expected `key = value`", ln + 1),
            });
            return cfg;
        };
        let key = k.trim().to_owned();
        let val = match parse_value(v) {
            Ok(val) => val,
            Err(e) => {
                cfg.diagnostics.push(Diagnostic {
                    path: path.into(),
                    index: None,
                    message: format!("line {}: {e}", ln + 1),
                });
                return cfg;
            }
        };
        if key == "action" && !in_action {
            cfg.diagnostics.push(Diagnostic {
                path: path.into(),
                index: None,
                message: format!("line {}: `action` must be an array of tables", ln + 1),
            });
            return cfg;
        }
        if in_action && let Some(e) = entries.last_mut() {
            e.push((key, val, ln + 1));
        }
    }
    let mut ids_here: Vec<String> = vec![];
    for (index, e) in entries.iter().enumerate() {
        let get = |k: &str| {
            e.iter()
                .find(|(key, _, _)| key == k)
                .map(|(_, v, _)| v.clone())
        };
        let mut problems = vec![];
        for r in REQUIRED {
            if get(r).is_none() {
                problems.push(format!("missing `{r}`"));
            }
        }
        let id = match get("id") {
            Some(Val::Str(s)) => s,
            Some(_) => {
                problems.push("`id` must be a string".into());
                String::new()
            }
            None => String::new(),
        };
        if !id.is_empty() && !id_ok(&id) {
            problems.push(format!(
                "invalid id {id:?}: lowercase ASCII letters, digits, `.`, `_`, `-` only"
            ));
        }
        let label = match get("label") {
            Some(Val::Str(s)) if !s.trim().is_empty() => s,
            Some(Val::Str(_)) => {
                problems.push("`label` is blank".into());
                String::new()
            }
            Some(_) => {
                problems.push("`label` must be a string".into());
                String::new()
            }
            None => String::new(),
        };
        let argv = match get("command") {
            Some(Val::Arr(a)) if !a.is_empty() && !a[0].trim().is_empty() => a,
            Some(Val::Arr(_)) => {
                problems.push("`command` needs a nonempty program".into());
                vec![]
            }
            Some(Val::Str(_)) => {
                problems.push("`command` must be an argv array, not a shell string".into());
                vec![]
            }
            Some(_) => {
                problems.push("`command` must be an array of strings".into());
                vec![]
            }
            None => vec![],
        };
        let danger = match get("danger") {
            Some(Val::Str(s)) => match Danger::parse(&s) {
                Some(d) => Some(d),
                None => {
                    problems.push(format!(
                        "unknown danger {s:?}: safe, mutating or destructive (lowercase)"
                    ));
                    None
                }
            },
            Some(_) => {
                problems.push("`danger` must be a string".into());
                None
            }
            None => None,
        };
        let confirm = match get("confirm") {
            Some(Val::Bool(b)) => b,
            Some(_) => {
                problems.push("`confirm` must be true or false".into());
                false
            }
            None => false,
        };
        let description = match get("description") {
            Some(Val::Str(s)) => s,
            Some(_) => {
                problems.push("`description` must be a string".into());
                String::new()
            }
            None => String::new(),
        };
        let keywords = match get("keywords") {
            Some(Val::Arr(a)) => a,
            Some(_) => {
                problems.push("`keywords` must be an array of strings".into());
                vec![]
            }
            None => vec![],
        };
        let group = match get("group") {
            Some(Val::Str(s)) if !s.trim().is_empty() => s,
            Some(Val::Str(_)) => {
                problems.push("`group` is blank".into());
                String::new()
            }
            Some(_) => {
                problems.push("`group` must be a string".into());
                String::new()
            }
            None => origin.default_group().to_owned(),
        };
        if !id.is_empty() && problems.is_empty() {
            if builtin_ids.contains(&id.as_str()) {
                problems.push(format!("id {id:?} collides with a built-in action"));
            } else if reserved.iter().any(|r| r == &id) {
                problems.push(format!(
                    "id {id:?} is already defined by the global configuration"
                ));
            } else if ids_here.contains(&id) {
                problems.push(format!("duplicate id {id:?} in this file"));
            }
        }
        if !problems.is_empty() {
            cfg.diagnostics.push(Diagnostic {
                path: path.into(),
                index: Some(index),
                message: problems.join(" · "),
            });
            continue;
        }
        ids_here.push(id.clone());
        cfg.actions.push(CustomAction {
            id,
            label,
            argv,
            danger: danger.unwrap_or(Danger::Safe),
            confirm,
            description,
            keywords,
            group,
            origin,
            index,
        });
    }
    cfg
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustEntry {
    pub digest: String,
    pub path: Option<String>,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrustStore {
    pub entries: Vec<TrustEntry>,

    pub corrupt: Option<String>,

    pub save_failure: Option<String>,
    pub saves: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustStatus {
    Trusted,

    Moved,

    LegacyDigestOnly,
    Untrusted,
}

impl TrustStore {
    pub fn status(&self, digest: &str, path: &str, cwd: &str) -> TrustStatus {
        let mut best = TrustStatus::Untrusted;
        for e in &self.entries {
            if e.digest != digest {
                continue;
            }
            match (&e.path, &e.cwd) {
                (Some(p), Some(c)) if p == path && c == cwd => return TrustStatus::Trusted,
                (Some(_), _) => best = TrustStatus::Moved,
                (None, _) => {
                    if best == TrustStatus::Untrusted {
                        best = TrustStatus::LegacyDigestOnly;
                    }
                }
            }
        }
        best
    }

    pub fn approve(&mut self, digest: &str, path: &str, cwd: &str) -> Result<String, String> {
        if let Some(e) = &self.save_failure {
            return Err(format!("trust store not saved: {e}"));
        }
        self.entries
            .retain(|e| !(e.digest == digest && e.path.as_deref() == Some(path)));
        self.entries.push(TrustEntry {
            digest: digest.into(),
            path: Some(path.into()),
            cwd: Some(cwd.into()),
        });
        self.entries
            .sort_by(|a, b| a.digest.cmp(&b.digest).then_with(|| a.path.cmp(&b.path)));
        self.saves += 1;
        Ok(self.serialize())
    }

    pub fn revoke(&mut self, digest: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.digest != digest);
        before != self.entries.len()
    }

    pub fn serialize(&self) -> String {
        let entries: Vec<Json> = self
            .entries
            .iter()
            .map(|e| serde_json::json!({ "digest": e.digest, "path": e.path, "cwd": e.cwd }))
            .collect();
        serde_json::json!({ "v": 2, "entries": entries }).to_string()
    }

    pub fn load(text: Option<&str>) -> Self {
        let Some(text) = text else {
            return Self::default();
        };
        let json = match parse_json(text) {
            Ok(j) => j,
            Err(e) => {
                return Self {
                    corrupt: Some(format!("trusted.json unreadable: {e}")),
                    ..Default::default()
                };
            }
        };
        let Some(version) = json.get("v") else {
            return Self {
                corrupt: Some("trusted.json has no version".into()),
                ..Default::default()
            };
        };
        let Some(version) = version.as_i64() else {
            return Self {
                corrupt: Some("trusted.json version is not an integer".into()),
                ..Default::default()
            };
        };
        let (field, parse): (&str, EntryParser) = match version {
            1 => ("hashes", |h: &Json| {
                h.as_str()
                    .map(|digest| TrustEntry {
                        digest: digest.into(),
                        path: None,
                        cwd: None,
                    })
                    .ok_or_else(|| "expected digest string".to_owned())
            }),
            2 => ("entries", trust_entry),
            other => {
                return Self {
                    corrupt: Some(format!("trusted.json version {other} is unknown")),
                    ..Default::default()
                };
            }
        };
        let mut store = Self::default();
        match json.get(field) {
            None => {}
            Some(Json::Array(items)) => {
                for (i, item) in items.iter().enumerate() {
                    match parse(item) {
                        Ok(entry) => store.entries.push(entry),
                        Err(why) => {
                            if store.corrupt.is_none() {
                                store.corrupt =
                                    Some(format!("trusted.json {field} entry {i} invalid: {why}"));
                            }
                        }
                    }
                }
            }
            Some(_) => store.corrupt = Some(format!("trusted.json {field} is not an array")),
        }
        store
    }
}

type EntryParser = fn(&Json) -> Result<TrustEntry, String>;

fn trust_entry(e: &Json) -> Result<TrustEntry, String> {
    Ok(TrustEntry {
        digest: json_str(e, "digest")?.to_owned(),
        path: json_opt_str(e, "path")?,
        cwd: json_opt_str(e, "cwd")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holla_json_strings_roundtrip_all_escape_classes() {
        let store = TrustStore {
            entries: vec![
                TrustEntry {
                    digest: "abc".into(),
                    path: Some("/w/\"a\"\\n\nb\u{1}🚀".into()),
                    cwd: Some("/w\t".into()),
                },
                TrustEntry {
                    digest: "def".into(),
                    path: None,
                    cwd: None,
                },
            ],
            ..Default::default()
        };
        let text = store.serialize();
        assert!(serde_json::from_str::<serde_json::Value>(&text).is_ok());
        let loaded = TrustStore::load(Some(&text));
        assert_eq!(loaded.corrupt, None);
        assert_eq!(loaded.entries, store.entries);
    }

    #[test]
    fn holla_json_duplicate_keys_use_last_value() {
        let text = r#"{"v":2,"entries":[{"digest":"d","path":"/a","path":"/b","cwd":null}]}"#;
        let loaded = TrustStore::load(Some(text));
        assert_eq!(loaded.entries[0].path.as_deref(), Some("/b"));
    }

    #[test]
    fn holla_json_wrong_type_entries_report_corruption() {
        let loaded = TrustStore::load(Some(r#"{"v":2,"entries":"nope"}"#));
        assert!(loaded.entries.is_empty());
        assert!(loaded.corrupt.is_some());
    }

    #[test]
    fn holla_json_invalid_entries_keep_valid_siblings_with_diagnostic() {
        let text = r#"{"v":2,"entries":[{"digest":"a"},{"path":"/no-digest"},{"digest":"c","path":"/c"}]}"#;
        let loaded = TrustStore::load(Some(text));
        let digests: Vec<&str> = loaded.entries.iter().map(|e| e.digest.as_str()).collect();
        assert_eq!(digests, ["a", "c"]);
        assert!(loaded.corrupt.is_some());
    }

    #[test]
    fn holla_json_integer_schema_version_is_exact() {
        let loaded = TrustStore::load(Some(r#"{"v":2.0,"entries":[]}"#));
        assert!(loaded.corrupt.is_some());
    }
}
