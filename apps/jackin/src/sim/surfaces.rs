use crate::domain::doctor::{DoctorCheck, DoctorReport};
use crate::domain::role::{FindingArea, RoleCheckState, RoleFinding, RoleManifestDoc, RoleRecord};
use crate::domain::workspace::RoleSource;
use termrock::theme::StatusKind;

const FILES: [&str; 5] = [
    "Dockerfile",
    "Dockerfile.old",
    "policy.yaml",
    "hooks/setup.sh",
    "hooks/preflight.sh",
];

fn missing(area: FindingArea, key: &str, path: &str) -> RoleFinding {
    RoleFinding {
        area,
        key: Some(key.into()),
        message: format!("{path}: file not found in the role repository"),
        fix: Some("Create the file or choose an existing one with Choose….".into()),
    }
}

pub fn add_role(location: &str, taken: impl Fn(&str) -> bool) -> Result<RoleRecord, String> {
    let location = location.trim();
    let last = location.trim_end_matches('/').rsplit(['/', ':']).next();
    let name = last
        .map(|l| l.strip_suffix(".git").unwrap_or(l))
        .filter(|n| !n.is_empty() && *n != "~")
        .ok_or_else(|| format!("{location}: not a role location"))?;
    if name == "missing" {
        return Err(format!("{location}: no jackin.role.toml found"));
    }
    if taken(name) {
        return Err(format!("A role named {name} is already registered."));
    }
    let doc = RoleManifestDoc {
        name: name.into(),
        dockerfile: "Dockerfile".into(),
        policy: "policy.yaml".into(),
        codex_model: None,
        docker: false,
        hook_setup: None,
        hook_preflight: None,
        env: vec![],
    };
    let git = ["https://", "ssh://", "git@"]
        .iter()
        .any(|p| location.starts_with(p));
    Ok(RoleRecord {
        name: name.into(),
        source: if git {
            RoleSource::Git {
                url: location.into(),
                branch: Some("main".into()),
            }
        } else {
            RoleSource::Local {
                path: location.into(),
            }
        },
        check: RoleCheckState::Done(check_role(&doc)),
        manifest: Ok(doc),
        bindings: Default::default(),
        files: FILES.iter().map(|f| (*f).to_owned()).collect(),
    })
}

pub fn check_role(doc: &RoleManifestDoc) -> Vec<RoleFinding> {
    let mut findings = vec![];
    if doc.dockerfile == "Dockerfile.old" {
        findings.push(RoleFinding {
            area: FindingArea::Dockerfile,
            key: Some("role.dockerfile".into()),
            message: "Dockerfile.old:12: the final stage must start FROM construct, found \"ubuntu:24.04\"".into(),
            fix: Some("Change the last FROM line to FROM construct, then press F5.".into()),
        });
    } else if !FILES.contains(&doc.dockerfile.as_str()) {
        findings.push(missing(
            FindingArea::Dockerfile,
            "role.dockerfile",
            &doc.dockerfile,
        ));
    }
    if !FILES.contains(&doc.policy.as_str()) {
        findings.push(missing(FindingArea::Policy, "role.policy", &doc.policy));
    }
    for (key, hook) in [
        ("hooks.setup", &doc.hook_setup),
        ("hooks.preflight", &doc.hook_preflight),
    ] {
        if let Some(path) = hook.as_deref().filter(|p| !p.is_empty())
            && !FILES.contains(&path)
        {
            findings.push(missing(FindingArea::Hook, key, path));
        }
    }
    findings
}

fn check(
    label: &str,
    status: StatusKind,
    summary: &str,
    found: Option<&str>,
    expected: Option<&str>,
    fix: Option<&str>,
) -> DoctorCheck {
    DoctorCheck {
        label: label.into(),
        status,
        summary: summary.into(),
        found: found.map(Into::into),
        expected: expected.map(Into::into),
        fix: fix.map(Into::into),
    }
}

pub fn doctor_report(run: usize, at_secs: i64) -> DoctorReport {
    use StatusKind::{Fail, Ok, Warn};
    let step = (run.max(1) - 1) % 3;
    let (version_status, version_summary, version_found, version_fix) = if step == 0 {
        (
            Warn,
            "openshell 0.1.7, pinned 0.1.6",
            "0.1.7",
            Some("Install OpenShell 0.1.6 to match the pinned version, then press r."),
        )
    } else {
        (Ok, "openshell 0.1.6", "0.1.6", None)
    };
    let (socket_status, socket_summary, socket_found, socket_fix) = if step == 0 {
        (
            Fail,
            "/var/run/docker.sock not found",
            "no socket",
            Some("Start Docker or OrbStack, then press r."),
        )
    } else {
        (Ok, "/var/run/docker.sock", "/var/run/docker.sock", None)
    };
    let (health_status, health_summary, health_found, health_fix) = if step == 2 {
        (
            Fail,
            "connection refused: jackin-local at 127.0.0.1:8080",
            "connection refused",
            Some("Start the gateway jackin-local, then press r."),
        )
    } else {
        (Ok, "healthy · 12 ms", "healthy", None)
    };
    DoctorReport {
        at_secs,
        gateway: Some("jackin-local".into()),
        checks: vec![
            check(
                "OpenShell binary",
                Ok,
                "/opt/homebrew/bin/openshell",
                Some("/opt/homebrew/bin/openshell"),
                Some("openshell on PATH"),
                None,
            ),
            check(
                "SSH binary",
                Ok,
                "/usr/bin/ssh",
                Some("/usr/bin/ssh"),
                Some("ssh on PATH"),
                None,
            ),
            check(
                "Git binary",
                Ok,
                "/usr/bin/git",
                Some("/usr/bin/git"),
                Some("git on PATH"),
                None,
            ),
            check(
                "Gateway registered",
                Ok,
                "jackin-local",
                Some("jackin-local"),
                Some("one registered gateway"),
                None,
            ),
            check(
                "Gateway health",
                health_status,
                health_summary,
                Some(health_found),
                Some("a healthy gateway at 127.0.0.1:8080"),
                health_fix,
            ),
            check(
                "Pinned version",
                version_status,
                version_summary,
                Some(version_found),
                Some("0.1.6"),
                version_fix,
            ),
            check(
                "Engine socket",
                socket_status,
                socket_summary,
                Some(socket_found),
                Some("a readable Docker or OrbStack socket"),
                socket_fix,
            ),
        ],
    }
}
