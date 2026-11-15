use crate::domain::context::ScopeTag;
use crate::domain::effect::Effect;
use crate::domain::exec::Command;
use crate::sim::world::BatchMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Mise,
    Git,
    Docker,
    System,
    Disk,
    Files,
    Postgres,
    Rust,
    Ssh,
    Cleanup,
    Plan,
    Activity,
    Explore,
    Personal,
    Node,
    Just,
    Make,
    Taskfile,
    Brew,
    Gradle,
    Idea,
    Upgrade,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Mise => "task",
            Kind::Git => "git",
            Kind::Docker => "docker",
            Kind::System => "system",
            Kind::Disk => "disk",
            Kind::Files => "file",
            Kind::Postgres => "postgres",
            Kind::Rust => "cargo",
            Kind::Ssh => "ssh",
            Kind::Cleanup => "cleanup",
            Kind::Plan => "plan",
            Kind::Activity => "activity",
            Kind::Explore => "explore",
            Kind::Personal => "custom",
            Kind::Node => "script",
            Kind::Just => "recipe",
            Kind::Make => "target",
            Kind::Taskfile => "task",
            Kind::Brew => "service",
            Kind::Gradle => "gradle",
            Kind::Idea => "idea",
            Kind::Upgrade => "upgrade",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultType {
    Action,
    Resource,
    Recommendation,
    Flow,
    Handoff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Risk {
    ReadOnly,
    Mutating,
    Destructive,
    Privileged,
}

impl Risk {
    pub fn label(self) -> &'static str {
        match self {
            Risk::ReadOnly => "read-only",
            Risk::Mutating => "mutating",
            Risk::Destructive => "destructive",
            Risk::Privileged => "privileged",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confirmation {
    None,

    One,

    TwoGate { phrase: String, broad: bool },

    Trust { config: String },
}

impl Confirmation {
    pub fn label(&self) -> String {
        match self {
            Confirmation::None => "runs directly".into(),
            Confirmation::One => "one confirmation".into(),
            Confirmation::TwoGate { broad, .. } => {
                if *broad {
                    "two gates · typed phrase with I UNDERSTAND".into()
                } else {
                    "two gates · typed phrase".into()
                }
            }
            Confirmation::Trust { config } => format!("trust required · {config}"),
        }
    }

    pub fn phrase(&self) -> Option<String> {
        match self {
            Confirmation::TwoGate { phrase, broad } => Some(if *broad {
                format!("I UNDERSTAND: {phrase}")
            } else {
                phrase.clone()
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Freshness {
    Live { age_ms: i64 },
    Cached { age_ms: i64 },
    Partial { loaded: usize, total: usize },
    Loading,
    Unavailable(String),
}

impl Freshness {
    pub fn label(&self) -> String {
        let age = |ms: i64| {
            let s = ms / 1000;
            if s < 60 {
                format!("{s} s ago")
            } else if s < 3600 {
                format!("{} min ago", s / 60)
            } else {
                format!("{} h ago", s / 3600)
            }
        };
        match self {
            Freshness::Live { age_ms } => format!("live · {}", age(*age_ms)),
            Freshness::Cached { age_ms } => format!("cached · {}", age(*age_ms)),
            Freshness::Partial { loaded, total } => {
                format!("partial · {loaded} of {total} sources")
            }
            Freshness::Loading => "still discovering…".into(),
            Freshness::Unavailable(r) => format!("unavailable · {r}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Signal {
    Default,

    GlobalUse,

    LocalUse,

    Context,

    Urgency,

    Pin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reason {
    pub signal: Signal,
    pub text: String,
}

impl Reason {
    pub fn new(signal: Signal, text: impl Into<String>) -> Self {
        Self {
            signal,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgSpec {
    pub name: String,
    pub expected: String,
    pub default: String,
    pub options: Vec<String>,
    pub required: bool,
    pub secret: bool,
    pub help: String,
}

impl ArgSpec {
    pub fn text(name: &str, expected: &str, default: &str) -> Self {
        Self {
            name: name.into(),
            expected: expected.into(),
            default: default.into(),
            options: vec![],
            required: false,
            secret: false,
            help: String::new(),
        }
    }
    pub fn choice(name: &str, options: &[&str], default: usize) -> Self {
        Self {
            name: name.into(),
            expected: "one of".into(),
            default: options
                .get(default)
                .map(|s| (*s).to_owned())
                .unwrap_or_default(),
            options: options.iter().map(|s| (*s).to_owned()).collect(),
            required: true,
            secret: false,
            help: String::new(),
        }
    }
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
    pub fn help(mut self, h: &str) -> Self {
        self.help = h.into();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    Activity { script: Option<String> },

    Batch,

    Disk { path: String },

    Files { path: String },

    Find,

    Cleanup { category: Option<String> },

    TopFiles,

    Copy { value: String },

    Config { path: String },

    Group(&'static str),

    Plan { plan: String },

    Handoff { tool: String },

    Scope(crate::domain::context::Scope),

    Snapshot { snapshot: String },

    OpenActivity { id: String },

    Insert,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: String,
    pub label: String,
    pub kind: Kind,
    pub rtype: ResultType,
    pub scope: ScopeTag,
    pub risk: Risk,
    pub confirmation: Confirmation,

    pub summary: String,

    pub commands: Vec<String>,

    pub effects: Vec<String>,

    pub reasons: Vec<Reason>,

    pub keywords: Vec<String>,

    pub aliases: Vec<String>,

    pub page_aliases: Vec<String>,
    pub freshness: Freshness,
    pub args: Vec<ArgSpec>,

    pub alternatives: Vec<Alternative>,
    pub launch: Launch,

    pub group: &'static str,

    pub used_here: u32,
    pub used_anywhere: u32,

    pub last_used_secs: Option<i64>,
    pub pinned: bool,
    pub hidden: bool,

    pub preferred_tool: Option<String>,

    pub exec: Vec<Command>,

    pub effect: Option<Effect>,

    pub batch: Vec<(String, Vec<Command>, Option<Effect>, ScopeTag)>,
    pub batch_mode: BatchMode,

    pub provenance: String,

    pub trust_key: Option<(String, String)>,

    pub cap_note: Option<String>,

    pub frecency: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alternative {
    pub label: String,

    pub item: String,
}

impl Item {
    #[allow(clippy::too_many_arguments)]
    pub fn new(id: &str, label: &str, kind: Kind, rtype: ResultType, scope: ScopeTag) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind,
            rtype,
            scope,
            risk: Risk::ReadOnly,
            confirmation: Confirmation::None,
            summary: String::new(),
            commands: vec![],
            effects: vec![],
            reasons: vec![],
            keywords: vec![],
            aliases: vec![],
            page_aliases: vec![],
            freshness: Freshness::Live { age_ms: 0 },
            args: vec![],
            alternatives: vec![],
            launch: Launch::Insert,
            group: "",
            used_here: 0,
            used_anywhere: 0,
            last_used_secs: None,
            pinned: false,
            hidden: false,
            preferred_tool: None,
            exec: vec![],
            effect: None,
            batch: vec![],
            batch_mode: BatchMode::Parallel,
            provenance: "built-in".into(),
            trust_key: None,
            cap_note: None,
            frecency: 0.0,
        }
    }

    pub fn exec(mut self, cmds: Vec<Command>) -> Self {
        self.commands = crate::domain::exec::displays(&cmds);
        self.exec = cmds;
        self
    }
    pub fn effect(mut self, e: Effect) -> Self {
        self.effect = Some(e);
        self
    }
    pub fn provenance(mut self, p: &str) -> Self {
        self.provenance = p.into();
        self
    }
    pub fn batch(
        mut self,
        mode: BatchMode,
        members: Vec<(String, Vec<Command>, Option<Effect>, ScopeTag)>,
    ) -> Self {
        self.commands = members
            .iter()
            .flat_map(|(_, c, _, _)| crate::domain::exec::displays(c))
            .collect();
        self.batch = members;
        self.batch_mode = mode;
        self.launch = Launch::Batch;
        self
    }

    pub fn all_exec(&self) -> Vec<Command> {
        if self.batch.is_empty() {
            self.exec.clone()
        } else {
            self.batch
                .iter()
                .flat_map(|(_, c, _, _)| c.clone())
                .collect()
        }
    }
    pub fn risk(mut self, r: Risk) -> Self {
        self.risk = r;
        self
    }
    pub fn confirm(mut self, c: Confirmation) -> Self {
        self.confirmation = c;
        self
    }
    pub fn summary(mut self, s: &str) -> Self {
        self.summary = s.into();
        self
    }
    pub fn effects(mut self, e: &[&str]) -> Self {
        self.effects = e.iter().map(|s| (*s).to_owned()).collect();
        self
    }
    pub fn reason(mut self, signal: Signal, text: &str) -> Self {
        self.reasons.push(Reason::new(signal, text));
        self
    }
    pub fn keywords(mut self, k: &[&str]) -> Self {
        self.keywords = k.iter().map(|s| (*s).to_owned()).collect();
        self
    }
    pub fn aliases(mut self, a: &[&str]) -> Self {
        self.aliases = a.iter().map(|s| (*s).to_owned()).collect();
        self
    }
    pub fn page_aliases(mut self, a: &[&str]) -> Self {
        self.page_aliases = a.iter().map(|s| (*s).to_owned()).collect();
        self
    }
    pub fn freshness(mut self, f: Freshness) -> Self {
        self.freshness = f;
        self
    }
    pub fn args(mut self, a: Vec<ArgSpec>) -> Self {
        self.args = a;
        self
    }
    pub fn alt(mut self, label: &str, item: &str) -> Self {
        self.alternatives.push(Alternative {
            label: label.into(),
            item: item.into(),
        });
        self
    }
    pub fn launch(mut self, l: Launch) -> Self {
        self.launch = l;
        self
    }
    pub fn group(mut self, g: &'static str) -> Self {
        self.group = g;
        self
    }

    pub fn top_reason(&self) -> Option<&Reason> {
        self.reasons.iter().max_by_key(|r| r.signal)
    }

    pub fn signal(&self) -> Signal {
        self.top_reason()
            .map(|r| r.signal)
            .unwrap_or(Signal::Default)
    }

    pub fn is_recommended(&self) -> bool {
        self.signal() >= Signal::LocalUse
    }
}
