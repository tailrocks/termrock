#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    pub name: String,
    pub role: HostRole,
    pub os: Os,
    pub user: String,

    pub remote: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostRole {
    Local,
    Development,
    Production,
}

impl HostRole {
    pub fn label(self) -> &'static str {
        match self {
            HostRole::Local => "local",
            HostRole::Development => "development",
            HostRole::Production => "production",
        }
    }

    pub fn sensitive(self) -> bool {
        matches!(self, HostRole::Production)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Debian,
}

impl Os {
    pub fn label(self) -> &'static str {
        match self {
            Os::MacOs => "macOS",
            Os::Debian => "Debian 13",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Scope {
    #[default]
    Here,
    Parent,
    Children,
    System,
}

impl Scope {
    pub fn outward(self) -> Scope {
        match self {
            Scope::Children => Scope::Here,
            Scope::Here => Scope::Parent,
            Scope::Parent | Scope::System => Scope::System,
        }
    }

    pub fn inward(self) -> Scope {
        match self {
            Scope::System => Scope::Parent,
            Scope::Parent => Scope::Here,
            Scope::Here | Scope::Children => Scope::Children,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeTag {
    pub direction: Scope,

    pub defined_at: String,

    pub runs_in: String,

    pub word: String,
}

impl ScopeTag {
    pub fn here(cwd: &str) -> Self {
        Self {
            direction: Scope::Here,
            defined_at: cwd.to_owned(),
            runs_in: cwd.to_owned(),
            word: "here".into(),
        }
    }
    pub fn project(root: &str, runs_in: &str) -> Self {
        Self {
            direction: Scope::Here,
            defined_at: root.to_owned(),
            runs_in: runs_in.to_owned(),
            word: "project".into(),
        }
    }
    pub fn parent(root: &str) -> Self {
        Self {
            direction: Scope::Parent,
            defined_at: root.to_owned(),
            runs_in: root.to_owned(),
            word: "parent".into(),
        }
    }
    pub fn child(path: &str) -> Self {
        Self {
            direction: Scope::Children,
            defined_at: path.to_owned(),
            runs_in: path.to_owned(),
            word: "child".into(),
        }
    }
    pub fn host(host: &str) -> Self {
        Self {
            direction: Scope::System,
            defined_at: host.to_owned(),
            runs_in: host.to_owned(),
            word: "host".into(),
        }
    }

    pub fn remote(host: &str) -> Self {
        Self {
            direction: Scope::System,
            defined_at: host.to_owned(),
            runs_in: host.to_owned(),
            word: format!("on {host}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub name: String,
    pub root: String,
    pub kind: ProjectKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectKind {
    Rust {
        workspace: bool,
        member: Option<String>,
    },

    Node {
        manager: &'static str,
    },

    MiseMonorepo,

    Gradle,

    Python,

    Collection,

    Service,
}

impl ProjectKind {
    pub fn label(&self) -> String {
        match self {
            ProjectKind::Rust { workspace, member } => match (workspace, member) {
                (true, Some(m)) => format!("Rust workspace · member {m}"),
                (true, None) => "Rust workspace".into(),
                _ => "Rust project".into(),
            },
            ProjectKind::Node { manager } => format!("Node project · {manager}"),
            ProjectKind::MiseMonorepo => "mise monorepo".into(),
            ProjectKind::Gradle => "Gradle build".into(),
            ProjectKind::Python => "Python project".into(),
            ProjectKind::Collection => "project collection".into(),
            ProjectKind::Service => "service checkout".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub cwd: String,
    pub home: String,

    pub project: Option<Project>,

    pub workspace: Option<Project>,

    pub children: Vec<Project>,
}

impl Location {
    pub fn short(&self, path: &str) -> String {
        match path.strip_prefix(&self.home) {
            Some(rest) => format!("~{rest}"),
            None => path.to_owned(),
        }
    }

    pub fn cwd_short(&self) -> String {
        self.short(&self.cwd)
    }
}
