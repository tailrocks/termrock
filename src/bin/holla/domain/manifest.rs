use std::collections::BTreeSet;

pub const TASK_CAP: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredTask {
    pub id: String,
    pub name: String,
    pub description: String,

    pub argv: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovery {
    pub source: &'static str,

    pub defined_by: String,
    pub runner: String,
    pub tasks: Vec<DiscoveredTask>,

    pub total: usize,

    pub diagnostic: Option<String>,
}

impl Discovery {
    pub fn capped(&self) -> bool {
        self.total > self.tasks.len()
    }
    pub fn title(&self, label: &str) -> String {
        if self.capped() {
            format!("{label} ({} of {})", self.tasks.len(), self.total)
        } else {
            label.to_owned()
        }
    }
}

/// Holla's JSON syntax, string, and number codec. Every persisted store and
/// discovery input decodes through serde_json, so escapes, surrogate pairs,
/// and integers keep their exact meaning.
pub use serde_json::Value as Json;

pub fn parse_json(text: &str) -> Result<Json, String> {
    serde_json::from_str(text).map_err(|error| error.to_string())
}

pub fn json_str<'a>(value: &'a Json, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Json::as_str)
        .ok_or_else(|| format!("{field}: expected string"))
}

pub fn json_opt_str(value: &Json, field: &str) -> Result<Option<String>, String> {
    match value.get(field) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("{field}: expected string or null")),
    }
}

pub fn json_u64(value: &Json, field: &str) -> Result<u64, String> {
    value
        .get(field)
        .and_then(Json::as_u64)
        .ok_or_else(|| format!("{field}: expected unsigned integer"))
}

pub fn json_i64(value: &Json, field: &str) -> Result<i64, String> {
    value
        .get(field)
        .and_then(Json::as_i64)
        .ok_or_else(|| format!("{field}: expected signed integer"))
}

pub fn node_runner(present: &[&str]) -> &'static str {
    if present.contains(&"pnpm-lock.yaml") {
        "pnpm"
    } else if present.contains(&"yarn.lock") {
        "yarn"
    } else if present.contains(&"bun.lock") || present.contains(&"bun.lockb") {
        "bun"
    } else {
        "npm"
    }
}

pub fn node_scripts(cwd: &str, package_json: Option<&str>, lockfiles: &[&str]) -> Discovery {
    let runner = node_runner(lockfiles);
    let file = format!("{cwd}/package.json");
    let mut d = Discovery {
        source: "node",
        defined_by: file.clone(),
        runner: runner.into(),
        tasks: vec![],
        total: 0,
        diagnostic: None,
    };
    let Some(text) = package_json else {
        d.diagnostic = Some("no package.json here".into());
        return d;
    };
    let json = match parse_json(text) {
        Ok(j) => j,
        Err(e) => {
            d.diagnostic = Some(format!("package.json unreadable: {e}"));
            return d;
        }
    };
    let Some(scripts) = json.get("scripts").and_then(Json::as_object) else {
        d.diagnostic = Some("package.json has no scripts object".into());
        return d;
    };
    let mut names: Vec<(String, String)> = scripts
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
        .collect();
    names.sort();
    names.dedup_by(|a, b| a.0 == b.0);
    d.total = names.len();
    if names.is_empty() {
        d.diagnostic = Some("scripts is empty".into());
    }
    d.tasks = names
        .into_iter()
        .take(TASK_CAP)
        .map(|(name, body)| DiscoveredTask {
            id: format!("node.script.{name}"),
            argv: vec![runner.into(), "run".into(), name.clone()],
            description: body,
            name,
        })
        .collect();
    d
}

pub fn just_recipes(cwd: &str, file: &str, summary: Result<&str, &str>) -> Discovery {
    let mut d = Discovery {
        source: "just",
        defined_by: format!("{cwd}/{file}"),
        runner: "just".into(),
        tasks: vec![],
        total: 0,
        diagnostic: None,
    };
    let text = match summary {
        Ok(t) => t,
        Err(e) => {
            d.diagnostic = Some(format!("just --summary failed: {e}"));
            return d;
        }
    };
    let names: BTreeSet<&str> = text.split_whitespace().collect();
    d.total = names.len();
    if names.is_empty() {
        d.diagnostic = Some("just --summary listed no recipes".into());
    }
    d.tasks = names
        .into_iter()
        .take(TASK_CAP)
        .map(|n| DiscoveredTask {
            id: format!("just.recipe.{n}"),
            name: n.into(),
            description: String::new(),
            argv: vec!["just".into(), n.into()],
        })
        .collect();
    d
}

fn make_target_ok(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn make_targets(text: &str) -> (Vec<String>, Vec<String>) {
    let mut out: Vec<String> = vec![];
    let mut rejected = vec![];
    for line in text.lines() {
        if line.starts_with(' ') || line.starts_with('\t') || line.starts_with('#') {
            continue;
        }
        let Some(colon) = line.find(':') else {
            continue;
        };
        let head = &line[..colon];
        if line[colon..].starts_with(":=") || head.contains('=') {
            continue;
        }
        for name in head.split_whitespace() {
            if name.contains('%')
                || name.contains('$')
                || name.contains('/')
                || !make_target_ok(name)
            {
                rejected.push(name.to_owned());
                continue;
            }
            if !out.iter().any(|o| o == name) {
                out.push(name.to_owned());
            }
        }
    }
    (out, rejected)
}

pub fn make_discovery(cwd: &str, makefile: Option<&str>) -> Discovery {
    let mut d = Discovery {
        source: "make",
        defined_by: format!("{cwd}/Makefile"),
        runner: "make".into(),
        tasks: vec![],
        total: 0,
        diagnostic: None,
    };
    let Some(text) = makefile else {
        d.diagnostic = Some("no Makefile here (exact name)".into());
        return d;
    };
    let (targets, rejected) = make_targets(text);
    d.total = targets.len();
    if !rejected.is_empty() {
        d.diagnostic = Some(format!(
            "{} unsupported target {} skipped ({})",
            rejected.len(),
            if rejected.len() == 1 {
                "syntax"
            } else {
                "syntaxes"
            },
            rejected.join(", ")
        ));
    }
    if targets.is_empty() && d.diagnostic.is_none() {
        d.diagnostic = Some("Makefile declares no plain targets".into());
    }
    d.tasks = targets
        .into_iter()
        .take(TASK_CAP)
        .map(|n| DiscoveredTask {
            id: format!("make.target.{n}"),
            argv: vec!["make".into(), n.clone()],
            description: String::new(),
            name: n,
        })
        .collect();
    d
}

pub fn taskfile_tasks(cwd: &str, file: &str, listing: Result<&str, &str>) -> Discovery {
    let mut d = Discovery {
        source: "taskfile",
        defined_by: format!("{cwd}/{file}"),
        runner: "task".into(),
        tasks: vec![],
        total: 0,
        diagnostic: None,
    };
    let text = match listing {
        Ok(t) => t,
        Err(e) => {
            d.diagnostic = Some(format!("task --list --json failed: {e}"));
            return d;
        }
    };
    let json = match parse_json(text) {
        Ok(j) => j,
        Err(e) => {
            d.diagnostic = Some(format!("task --list --json: {e}"));
            return d;
        }
    };
    let Some(tasks) = json.get("tasks").and_then(Json::as_array) else {
        d.diagnostic = Some("task --list --json: no tasks array".into());
        return d;
    };
    let mut names: Vec<String> = tasks
        .iter()
        .filter_map(|t| t.get("name").and_then(Json::as_str))
        .filter(|n| !n.trim().is_empty())
        .map(|n| n.to_owned())
        .collect();
    names.sort();
    names.dedup();
    d.total = names.len();
    if names.is_empty() {
        d.diagnostic = Some("task --list --json listed no tasks".into());
    }
    d.tasks = names
        .into_iter()
        .take(TASK_CAP)
        .map(|n| DiscoveredTask {
            id: format!("taskfile.task.{n}"),
            argv: vec!["task".into(), n.clone()],
            description: String::new(),
            name: n,
        })
        .collect();
    d
}

pub fn mise_tasks(cwd: &str, output: Result<&str, &str>) -> Discovery {
    let mut d = Discovery {
        source: "mise",
        defined_by: format!("{cwd}/mise.toml"),
        runner: "mise".into(),
        tasks: vec![],
        total: 0,
        diagnostic: None,
    };
    let text = match output {
        Ok(t) => t,
        Err(e) => {
            d.diagnostic = Some(format!("mise tasks ls failed: {e}"));
            return d;
        }
    };
    for l in text.lines() {
        let l = l.trim();
        if l.is_empty() {
            continue;
        }
        let (name, rest) = l.split_once(char::is_whitespace).unwrap_or((l, ""));
        let desc = rest.trim().trim_start_matches('#').trim().to_owned();
        d.tasks.push(DiscoveredTask {
            id: format!("mise.task.{name}"),
            name: name.into(),
            description: if desc.is_empty() {
                "Run mise task".into()
            } else {
                desc
            },
            argv: vec!["mise".into(), "run".into(), name.into()],
        });
    }
    d.total = d.tasks.len();
    if d.tasks.is_empty() {
        d.diagnostic = Some("mise tasks ls listed nothing".into());
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holla_json_surrogate_pair_is_one_scalar() {
        let json = parse_json(r#"{"k\ud83d\ude80":"\ud83d\ude80"}"#).unwrap();
        assert_eq!(json.get("k🚀").and_then(|v| v.as_str()), Some("🚀"));
        assert!(parse_json(r#"{"a":"\ud83d"}"#).is_err());
        assert!(parse_json(r#"{"a":"\ude80\ud83d"}"#).is_err());
    }

    #[test]
    fn holla_json_rejects_unknown_escapes_raw_controls_and_trailing_data() {
        assert!(parse_json(r#"{"a":"\q"}"#).is_err());
        assert!(parse_json("{\"a\":\"x\u{1}\"}").is_err());
        assert!(parse_json("[1] trailing").is_err());
        assert!(parse_json("NaN").is_err());
    }

    #[test]
    fn holla_json_duplicate_keys_use_last_value() {
        let d = node_scripts(
            "/w",
            Some(r#"{"scripts":{"a":"old"},"scripts":{"b":"new"}}"#),
            &[],
        );
        let names: Vec<&str> = d.tasks.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["b"]);
    }

    #[test]
    fn holla_json_legacy_discovery_shapes_still_decode() {
        let d = taskfile_tasks(
            "/w",
            "Taskfile.yml",
            Ok(r#"{"tasks":[{"name":"build"},{"name":"test"}]}"#),
        );
        let names: Vec<&str> = d.tasks.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["build", "test"]);
        assert_eq!(
            crate::domain::catalog::parse_brew_services(r#"{"services":[{"name":"redis"}]}"#),
            Ok(vec!["redis".to_owned()])
        );
    }
}
