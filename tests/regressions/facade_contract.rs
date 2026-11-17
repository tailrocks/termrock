//! The `termrock` facade contract of #15: the four applications import only
//! the facade, and each independent consumer fixture resolves its declared
//! features alone.
//!
//! This is source, manifest and lockfile scanning; it builds nothing. The
//! fixtures themselves are compiled and run by `scripts/check-consumers.sh` and
//! by the generated `Rust / termrock-consumer-*` jobs.

use std::path::{Path, PathBuf};

use syn::visit::Visit;

/// The applications that import the facade only. Each pull request that moves an
/// application onto the facade adds it here; the last one lists all four.
const MIGRATED_APPS: &[&str] = &["tablepro", "holla", "showcase"];
const IMPLEMENTATION: &str = "junie_tui";

/// Sources that still sit outside their package directory while the app move
/// lands in steps (docs/api-contract.md). Each entry is deleted by the pull
/// request that moves that source into `apps/<app>/src`.
const UNMOVED_SOURCES: [(&str, &str); 0] = [];

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries
        .map(|e| e.expect("directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Collects every place a file names the implementation crate: a path, a
/// `use` tree, `extern crate`, or a macro argument that spells it.
#[derive(Default)]
struct Names {
    found: Vec<String>,
}

impl<'ast> Visit<'ast> for Names {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        if path
            .segments
            .first()
            .is_some_and(|s| s.ident == IMPLEMENTATION)
        {
            self.found.push("path".into());
        }
        syn::visit::visit_path(self, path);
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        fn root_names(tree: &syn::UseTree) -> bool {
            match tree {
                syn::UseTree::Path(p) => p.ident == IMPLEMENTATION,
                syn::UseTree::Name(n) => n.ident == IMPLEMENTATION,
                syn::UseTree::Rename(r) => r.ident == IMPLEMENTATION,
                syn::UseTree::Group(g) => g.items.iter().any(root_names),
                syn::UseTree::Glob(_) => false,
            }
        }
        if root_names(&item.tree) {
            self.found.push("use".into());
        }
        syn::visit::visit_item_use(self, item);
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        if item.ident == IMPLEMENTATION {
            self.found.push("extern crate".into());
        }
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        let tokens: String = mac
            .tokens
            .to_string()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if tokens.contains(&format!("{IMPLEMENTATION}::")) {
            self.found.push("macro argument".into());
        }
        syn::visit::visit_macro(self, mac);
    }
}

fn implementation_imports(source: &str) -> Result<Vec<String>, String> {
    let file = syn::parse_file(source).map_err(|e| e.to_string())?;
    let mut names = Names::default();
    names.visit_file(&file);
    Ok(names.found)
}

fn manifest_mentions_implementation(manifest: &str) -> Vec<String> {
    manifest
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with('#'))
        .filter(|(_, line)| line.contains("junie-tui") || line.contains(IMPLEMENTATION))
        .map(|(n, line)| format!("line {}: {}", n + 1, line.trim()))
        .collect()
}

#[test]
fn facade_apps_have_no_implementation_imports() {
    let mut violations = Vec::new();
    for &app in MIGRATED_APPS {
        let dir = root().join("apps").join(app);
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml"))
            .unwrap_or_else(|e| panic!("apps/{app}/Cargo.toml: {e}"));
        for hit in manifest_mentions_implementation(&manifest) {
            violations.push(format!("apps/{app}/Cargo.toml {hit}"));
        }
        let mut files = Vec::new();
        rust_files(&dir, &mut files);
        for (_, unmoved) in UNMOVED_SOURCES.iter().filter(|(name, _)| *name == app) {
            rust_files(&root().join(unmoved), &mut files);
        }
        assert!(!files.is_empty(), "apps/{app} has no Rust files");
        for file in files {
            let source = std::fs::read_to_string(&file).expect("read source");
            match implementation_imports(&source) {
                Ok(found) => {
                    violations.extend(found.into_iter().map(|kind| {
                        format!("{} names `{IMPLEMENTATION}` ({kind})", file.display())
                    }))
                }
                Err(why) => violations.push(format!("{}: cannot parse: {why}", file.display())),
            }
        }
    }
    assert!(
        violations.is_empty(),
        "apps import the facade, never the implementation package:\n{}",
        violations.join("\n")
    );
}

#[test]
fn facade_scan_sees_each_way_to_name_the_implementation() {
    for (kind, source) in [
        ("use", "use junie_tui::ui::ctx::RenderCtx;"),
        ("use", "use junie_tui;"),
        ("use", "use ::std::fmt::{self}; use junie_tui::{core, ui};"),
        ("path", "fn f(_: junie_tui::core::id::WidgetId) {}"),
        ("extern crate", "extern crate junie_tui;"),
        (
            "macro argument",
            "fn f() { vec![junie_tui::core::id::WidgetId::of(\"x\")]; }",
        ),
    ] {
        let found = implementation_imports(source).expect("fixture parses");
        assert!(
            found.iter().any(|k| k == kind),
            "{kind} not found in `{source}`: {found:?}"
        );
    }
    let clean = "use termrock::ui::ctx::RenderCtx; const S: &str = \"junie_tui::x\";";
    assert_eq!(
        implementation_imports(clean).expect("parses"),
        Vec::<String>::new()
    );
    assert_eq!(
        manifest_mentions_implementation("# junie-tui\n[dependencies]\njunie-tui = {}\n"),
        ["line 3: junie-tui = {}"]
    );
}

/// Every package the committed lockfile of one consumer fixture resolves.
///
/// The fixtures declare no dev-dependencies, so the lockfile holds exactly their
/// normal graph with the declared features applied. Reading it needs no network
/// and no build. `scripts/check-consumers.sh` and the generated
/// `Rust / termrock-consumer-*` jobs build each fixture with `--locked`, which
/// fails when the lockfile and the manifest disagree, and print `cargo tree`.
fn resolved_packages(fixture: &str) -> Vec<String> {
    let lock = root()
        .join("tests/consumers")
        .join(fixture)
        .join("Cargo.lock");
    let text = std::fs::read_to_string(&lock)
        .unwrap_or_else(|e| panic!("{} is committed: {e}", lock.display()));
    let mut names: Vec<String> = text
        .lines()
        .filter_map(|line| line.strip_prefix("name = \""))
        .filter_map(|rest| rest.strip_suffix('"'))
        .map(str::to_owned)
        .collect();
    names.sort();
    names.dedup();
    names
}

fn has(names: &[String], package: &str) -> bool {
    names.iter().any(|n| n == package)
}

#[test]
fn consumer_minimal_excludes_clap_and_termpane() {
    let names = resolved_packages("minimal");
    assert!(
        has(&names, "termrock") && has(&names, "junie-tui"),
        "{names:?}"
    );
    assert!(
        !has(&names, "clap"),
        "clap leaked into the minimal consumer"
    );
    assert!(
        !has(&names, "termpane"),
        "termpane leaked into the minimal consumer"
    );
}

#[test]
fn consumer_keyed_modal_excludes_clap_and_termpane() {
    let names = resolved_packages("keyed-modal");
    assert!(
        has(&names, "termrock") && has(&names, "junie-tui"),
        "{names:?}"
    );
    assert!(
        !has(&names, "clap"),
        "clap leaked into the keyed-modal consumer"
    );
    assert!(
        !has(&names, "termpane"),
        "termpane leaked into the keyed-modal consumer"
    );
}

#[test]
fn consumer_terminal_has_termpane_without_clap() {
    let names = resolved_packages("terminal");
    assert!(
        has(&names, "termpane"),
        "the terminal feature carries termpane"
    );
    assert!(
        !has(&names, "clap"),
        "clap leaked into the terminal consumer"
    );
}

#[test]
fn consumer_jackin_host_excludes_demo() {
    let names = resolved_packages("jackin-host");
    assert!(has(&names, "jackin-console"), "{names:?}");
    assert!(
        has(&names, "termpane"),
        "the host keeps its intentional terminal dependency"
    );
    assert!(
        !has(&names, "clap"),
        "the demo feature leaked clap into the host"
    );
}
