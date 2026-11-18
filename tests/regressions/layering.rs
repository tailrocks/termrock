//! Crate layering (#439, #440): no dependency may point up the layers.
//!
//! The rules live in `docs/layering.toml`. This file enforces them in two
//! stages that read the same table:
//!
//! * Stage A, modules. While the code is one package, every `.rs` file under
//!   `src/` belongs to a target crate (`[files]`). The scan resolves each
//!   `crate::`, `self::` and `super::` path to the file that defines the
//!   module, maps both ends to crates and fails on an edge that `[allow]` does
//!   not permit. Edges that exist today are recorded in
//!   `layering-allowlist.tsv`. That file may only shrink: #441, #442 and the
//!   extraction issues empty it, and a line that no longer matches an edge
//!   fails as stale.
//! * Stage B, crates. Once `crates/termrock-*` exist, `cargo metadata` gives
//!   the real dependency graph, which is checked against the same table. It
//!   runs since the first extraction (#444) and a test asserts that it did.
//!
//! The scan is syntactic, like the module-graph measurement in #439: an item
//! reached through a re-export is attributed to the re-exporting module, and
//! visibility is not modelled. Doc comments and string literals are not code
//! and are ignored. Set `LAYERING_PRINT_EDGES=1` to print the current edges in
//! allowlist format.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use syn::visit::Visit;

const LAYERING: &str = "docs/layering.toml";
const ALLOWLIST: &str = "tests/regressions/layering-allowlist.tsv";
const FIX: &str = "Move the item down or invert the dependency; do not edit the allowlist.";

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

// ---------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------

struct Layering {
    order: Vec<String>,
    layer_order: Vec<String>,
    layers: BTreeMap<String, String>,
    allow: BTreeMap<String, BTreeSet<String>>,
    files: BTreeMap<String, String>,
}

fn strings(value: &toml::Value, what: &str) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{what} must be an array"))
        .iter()
        .map(|v| {
            v.as_str()
                .unwrap_or_else(|| panic!("{what} must hold strings"))
                .to_string()
        })
        .collect()
}

fn string_table(value: &toml::Value, what: &str) -> BTreeMap<String, String> {
    value
        .as_table()
        .unwrap_or_else(|| panic!("{what} must be a table"))
        .iter()
        .map(|(k, v)| {
            let v = v
                .as_str()
                .unwrap_or_else(|| panic!("{what}.{k} must be a string"));
            (k.clone(), v.to_string())
        })
        .collect()
}

impl Layering {
    fn parse(text: &str) -> Self {
        let doc: toml::Table = text.parse().expect("layering table is valid TOML");
        let section = |name: &str| {
            doc.get(name)
                .unwrap_or_else(|| panic!("layering table has no `{name}`"))
        };
        let allow = section("allow")
            .as_table()
            .expect("allow must be a table")
            .iter()
            .map(|(k, v)| (k.clone(), strings(v, k).into_iter().collect()))
            .collect();
        Self {
            order: strings(section("order"), "order"),
            layer_order: strings(section("layer_order"), "layer_order"),
            layers: string_table(section("layers"), "layers"),
            allow,
            files: string_table(section("files"), "files"),
        }
    }

    fn load() -> Self {
        let text = std::fs::read_to_string(root().join(LAYERING)).expect("read docs/layering.toml");
        Self::parse(&text)
    }

    fn position(&self, name: &str) -> Option<usize> {
        self.order.iter().position(|c| c == name)
    }

    /// Everything `krate` may depend on: its listed dependencies and, because
    /// "every row also allows the foundation crates that the row's
    /// dependencies already use", whatever those may depend on in turn.
    fn permitted(&self, krate: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut todo = vec![krate.to_string()];
        while let Some(next) = todo.pop() {
            for dep in self.allow.get(&next).into_iter().flatten() {
                if seen.insert(dep.clone()) {
                    todo.push(dep.clone());
                }
            }
        }
        seen
    }

    /// The package name that stands behind a short crate name.
    fn package(krate: &str) -> String {
        match krate {
            "termrock" | "junie-tui" => krate.to_string(),
            other => format!("termrock-{other}"),
        }
    }

    /// The short crate name of a workspace package, if the table lists it.
    fn member(&self, package: &str) -> Option<String> {
        let short = match package {
            "termrock" | "junie-tui" => package,
            other => other.strip_prefix("termrock-")?,
        };
        self.position(short).map(|_| short.to_string())
    }

    /// The crate a source file belongs to: the longest `[files]` key that is
    /// the file itself or a directory prefix (a key ending in `/`).
    fn crate_of_file(&self, file: &str) -> Option<&str> {
        self.files
            .iter()
            .filter(|(key, _)| *key == file || (key.ends_with('/') && file.starts_with(*key)))
            .max_by_key(|(key, _)| key.len())
            .map(|(_, krate)| krate.as_str())
    }

    /// Problems with the table itself.
    fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let mut seen = BTreeSet::new();
        for name in &self.order {
            if !seen.insert(name) {
                problems.push(format!("order lists `{name}` twice"));
            }
        }
        let rank = |layer: &str| self.layer_order.iter().position(|l| l == layer);
        for name in &self.order {
            let Some(layer) = self.layers.get(name) else {
                problems.push(format!("[layers] has no entry for `{name}`"));
                continue;
            };
            if rank(layer).is_none() {
                problems.push(format!("`{name}` is in unknown layer `{layer}`"));
            }
            if !self.allow.contains_key(name) {
                problems.push(format!("[allow] has no row for `{name}`"));
            }
        }
        for (name, deps) in &self.allow {
            let Some(at) = self.position(name) else {
                problems.push(format!("[allow] row `{name}` is not in `order`"));
                continue;
            };
            for dep in deps {
                match self.position(dep) {
                    None => problems.push(format!("`{name}` may depend on unknown crate `{dep}`")),
                    Some(below) if below >= at => problems.push(format!(
                        "`{name}` may depend on `{dep}`, which is not below it in `order`"
                    )),
                    Some(_) => {
                        let (own, theirs) = (
                            self.layers.get(name).and_then(|l| rank(l)),
                            self.layers.get(dep).and_then(|l| rank(l)),
                        );
                        if own < theirs {
                            problems
                                .push(format!("`{name}` may depend on `{dep}`, a higher layer"));
                        }
                    }
                }
            }
        }
        for name in self.layers.keys() {
            if self.position(name).is_none() {
                problems.push(format!("[layers] entry `{name}` is not in `order`"));
            }
        }
        for (file, krate) in &self.files {
            if self.position(krate).is_none() {
                problems.push(format!("[files] maps `{file}` to unknown crate `{krate}`"));
            }
        }
        problems
    }
}

// ---------------------------------------------------------------------------
// Source tree and module graph
// ---------------------------------------------------------------------------

/// Source files keyed by repository-relative path with `/` separators.
#[derive(Default, Clone)]
struct Tree {
    files: BTreeMap<String, String>,
}

impl Tree {
    /// Every `.rs` file under `src/`. The test support that `catalog_contract` used to include
    /// through `#[path]` (`tests/support/*`) moved into `termrock-catalog` (#449), where stage B
    /// checks its dependencies.
    fn load() -> Self {
        fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
            let mut entries: Vec<_> = std::fs::read_dir(dir)
                .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
                .map(|e| e.expect("directory entry").path())
                .collect();
            entries.sort();
            for path in entries {
                if path.is_dir() {
                    walk(root, &path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let rel = path.strip_prefix(root).expect("under root");
                    let rel = rel.to_string_lossy().replace('\\', "/");
                    let text = std::fs::read_to_string(&path)
                        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
                    out.insert(rel, text);
                }
            }
        }
        let mut files = BTreeMap::new();
        walk(root(), &root().join("src"), &mut files);
        Self { files }
    }
}

struct FileInfo {
    modpath: Vec<String>,
    /// Declared under `#[cfg(test)]`, or named like a test file.
    test: bool,
}

#[derive(Default)]
struct Graph {
    /// `a::b` to the file that defines module `a::b`; the crate root is `""`.
    modules: BTreeMap<String, String>,
    files: BTreeMap<String, FileInfo>,
    /// `#[macro_export]` macros live at the crate root but are defined in a
    /// module: macro name to that module.
    macros: BTreeMap<String, String>,
    problems: Vec<String>,
}

fn is_test_name(file: &str) -> bool {
    let name = file.rsplit('/').next().unwrap_or(file);
    name == "tests.rs" || name.ends_with("_tests.rs")
}

/// Splits a `cfg` predicate at its top-level commas.
fn split_top(s: &str) -> Vec<&str> {
    let (mut depth, mut start, mut quoted) = (0usize, 0usize, false);
    let mut parts = Vec::new();
    for (i, c) in s.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth = depth.saturating_sub(1),
            ',' if !quoted && depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

/// True when the predicate holds only in test builds: `test` or an `all(..)`
/// containing it. `any` and `not` are not test-only.
fn cfg_is_test(predicate: &str) -> bool {
    if predicate == "test" {
        return true;
    }
    predicate
        .strip_prefix("all(")
        .and_then(|s| s.strip_suffix(')'))
        .is_some_and(|inner| split_top(inner).into_iter().any(cfg_is_test))
}

fn is_test_attrs(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if attr.path().is_ident("test") {
            return true;
        }
        if !attr.path().is_ident("cfg") {
            return false;
        }
        let syn::Meta::List(list) = &attr.meta else {
            return false;
        };
        let predicate: String = list
            .tokens
            .to_string()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        cfg_is_test(&predicate)
    })
}

fn path_attribute(attrs: &[syn::Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        if !attr.path().is_ident("path") {
            return None;
        }
        let syn::Meta::NameValue(nv) = &attr.meta else {
            return None;
        };
        let syn::Expr::Lit(lit) = &nv.value else {
            return None;
        };
        let syn::Lit::Str(s) = &lit.lit else {
            return None;
        };
        Some(s.value())
    })
}

fn join_path(dir: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for part in rel.split('/') {
        match part {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

fn dir_of(file: &str) -> &str {
    file.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// The directory that holds the child modules of `file`.
fn child_dir(file: &str) -> String {
    let name = file.rsplit('/').next().unwrap_or(file);
    match name {
        "mod.rs" | "lib.rs" | "main.rs" => dir_of(file).to_string(),
        _ => join_path(dir_of(file), name.trim_end_matches(".rs")),
    }
}

struct Walk<'a> {
    tree: &'a Tree,
    graph: Graph,
}

impl Walk<'_> {
    fn file(&mut self, file: &str, modpath: Vec<String>, test: bool) {
        let Some(source) = self.tree.files.get(file) else {
            self.graph.problems.push(format!("{file}: missing"));
            return;
        };
        self.graph
            .modules
            .insert(modpath.join("::"), file.to_string());
        self.graph.files.insert(
            file.to_string(),
            FileInfo {
                modpath: modpath.clone(),
                test: test || is_test_name(file),
            },
        );
        let parsed = match syn::parse_file(source) {
            Ok(parsed) => parsed,
            Err(e) => {
                self.graph
                    .problems
                    .push(format!("{file}: cannot parse: {e}"));
                return;
            }
        };
        self.items(&parsed.items, file, &modpath, &[], test);
    }

    fn items(
        &mut self,
        items: &[syn::Item],
        file: &str,
        modpath: &[String],
        inline: &[String],
        test: bool,
    ) {
        for item in items {
            if let syn::Item::Macro(m) = item
                && let Some(name) = &m.ident
                && m.mac.path.is_ident("macro_rules")
                && m.attrs.iter().any(|a| a.path().is_ident("macro_export"))
            {
                self.graph
                    .macros
                    .insert(name.to_string(), modpath.join("::"));
            }
            let syn::Item::Mod(m) = item else { continue };
            let name = m.ident.to_string();
            let test = test || is_test_attrs(&m.attrs);
            if let Some((_, nested)) = &m.content {
                let mut deeper = inline.to_vec();
                deeper.push(name);
                self.items(nested, file, modpath, &deeper, test);
                continue;
            }
            let mut child_mod = modpath.to_vec();
            child_mod.extend(inline.iter().cloned());
            child_mod.push(name.clone());
            let mut base = child_dir(file);
            for segment in inline {
                base = join_path(&base, segment);
            }
            let candidates = match path_attribute(&m.attrs) {
                Some(rel) => {
                    let mut dir = dir_of(file).to_string();
                    for segment in inline {
                        dir = join_path(&dir, segment);
                    }
                    vec![join_path(&dir, &rel)]
                }
                None => vec![
                    join_path(&base, &format!("{name}.rs")),
                    join_path(&base, &format!("{name}/mod.rs")),
                ],
            };
            match candidates.iter().find(|c| self.tree.files.contains_key(*c)) {
                Some(found) => {
                    let found = found.clone();
                    self.file(&found, child_mod, test);
                }
                None => self.graph.problems.push(format!(
                    "{file}: `mod {name};` has no file (tried {})",
                    candidates.join(", ")
                )),
            }
        }
    }
}

fn build_graph(tree: &Tree) -> Graph {
    let mut walk = Walk {
        tree,
        graph: Graph::default(),
    };
    walk.file("src/lib.rs", Vec::new(), false);
    for file in tree.files.keys() {
        if !walk.graph.files.contains_key(file) {
            walk.graph.problems.push(format!(
                "{file}: not reachable from src/lib.rs through `mod` declarations"
            ));
        }
    }
    walk.graph
}

// ---------------------------------------------------------------------------
// Reference scan
// ---------------------------------------------------------------------------

/// One flattened `use` leaf: its full path and the name it binds, if renamed.
struct Leaf {
    path: Vec<String>,
    alias: Option<String>,
}

fn flatten(tree: &syn::UseTree, prefix: &mut Vec<String>, out: &mut Vec<Leaf>) {
    match tree {
        syn::UseTree::Path(p) => {
            prefix.push(p.ident.to_string());
            flatten(&p.tree, prefix, out);
            prefix.pop();
        }
        syn::UseTree::Name(n) => {
            let mut path = prefix.clone();
            if n.ident != "self" {
                path.push(n.ident.to_string());
            }
            out.push(Leaf { path, alias: None });
        }
        syn::UseTree::Rename(r) => {
            let mut path = prefix.clone();
            if r.ident != "self" {
                path.push(r.ident.to_string());
            }
            out.push(Leaf {
                path,
                alias: Some(r.rename.to_string()),
            });
        }
        syn::UseTree::Glob(_) => out.push(Leaf {
            path: prefix.clone(),
            alias: None,
        }),
        syn::UseTree::Group(g) => {
            for item in &g.items {
                flatten(item, prefix, out);
            }
        }
    }
}

/// Removes string and character literals so that a path written inside one is
/// not mistaken for code.
fn strip_literals(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '"' => {
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    i += if chars[i] == '\\' { 2 } else { 1 };
                }
                i += 1;
                out.push(' ');
            }
            '\'' if chars.get(i + 2) == Some(&'\'') => i += 3,
            '\'' if chars.get(i + 1) == Some(&'\\') => {
                i += 2;
                while i < chars.len() && chars[i] != '\'' {
                    i += 1;
                }
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

fn ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Finds `crate::`, `self::` and `super::` paths in macro tokens, which `syn`
/// does not parse.
fn macro_paths(tokens: &str) -> Vec<Vec<String>> {
    let text: String = strip_literals(tokens)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let chars: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let rest: String = chars[i..chars.len().min(i + 8)].iter().collect();
        let start_ok = i == 0 || !(ident_char(chars[i - 1]) || chars[i - 1] == ':');
        let keyword = ["crate", "self", "super"]
            .into_iter()
            .find(|k| rest.starts_with(&format!("{k}::")));
        let (true, Some(keyword)) = (start_ok, keyword) else {
            i += 1;
            continue;
        };
        let mut segments = vec![keyword.to_string()];
        let mut at = i + keyword.len();
        while chars.get(at) == Some(&':') && chars.get(at + 1) == Some(&':') {
            let from = at + 2;
            let mut to = from;
            while to < chars.len() && ident_char(chars[to]) {
                to += 1;
            }
            if to == from {
                break;
            }
            segments.push(chars[from..to].iter().collect());
            at = to;
        }
        found.push(segments);
        i = at.max(i + 1);
    }
    found
}

struct Reference {
    test: bool,
    /// Absolute path from the crate root.
    path: Vec<String>,
    spelled: String,
}

struct Refs {
    modpath: Vec<String>,
    children: BTreeSet<String>,
    inline: Vec<String>,
    test_depth: usize,
    file_is_test: bool,
    found: Vec<Reference>,
}

impl Refs {
    fn resolve(&self, raw: &[String]) -> Option<Vec<String>> {
        let first = raw.first()?.as_str();
        let mut here = self.modpath.clone();
        here.extend(self.inline.iter().cloned());
        match first {
            "crate" | "$crate" => Some(raw[1..].to_vec()),
            "self" => {
                here.extend(raw[1..].iter().cloned());
                Some(here)
            }
            "super" => {
                let ups = raw.iter().take_while(|s| *s == "super").count();
                for _ in 0..ups {
                    here.pop()?;
                }
                here.extend(raw[ups..].iter().cloned());
                Some(here)
            }
            child if self.inline.is_empty() && self.children.contains(child) => {
                let mut path = self.modpath.clone();
                path.extend(raw.iter().cloned());
                Some(path)
            }
            _ => None,
        }
    }

    fn record(&mut self, raw: &[String]) {
        if let Some(path) = self.resolve(raw) {
            self.found.push(Reference {
                test: self.test_depth > 0 || self.file_is_test,
                path,
                spelled: raw.join("::"),
            });
        }
    }

    fn scoped(&mut self, attrs: &[syn::Attribute], walk: impl FnOnce(&mut Self)) {
        let test = usize::from(is_test_attrs(attrs));
        self.test_depth += test;
        walk(self);
        self.test_depth -= test;
    }
}

macro_rules! scoped_visits {
    ($($method:ident => $walk:ident($node:ty)),* $(,)?) => {$(
        fn $method(&mut self, node: &'ast $node) {
            self.scoped(&node.attrs, |this| syn::visit::$walk(this, node));
        }
    )*};
}

impl<'ast> Visit<'ast> for Refs {
    scoped_visits! {
        visit_item_fn => visit_item_fn(syn::ItemFn),
        visit_item_impl => visit_item_impl(syn::ItemImpl),
        visit_item_const => visit_item_const(syn::ItemConst),
        visit_item_static => visit_item_static(syn::ItemStatic),
        visit_item_struct => visit_item_struct(syn::ItemStruct),
        visit_item_enum => visit_item_enum(syn::ItemEnum),
        visit_item_trait => visit_item_trait(syn::ItemTrait),
        visit_item_type => visit_item_type(syn::ItemType),
        visit_item_macro => visit_item_macro(syn::ItemMacro),
        visit_impl_item_fn => visit_impl_item_fn(syn::ImplItemFn),
        visit_impl_item_const => visit_impl_item_const(syn::ImplItemConst),
        visit_impl_item_type => visit_impl_item_type(syn::ImplItemType),
        visit_trait_item_fn => visit_trait_item_fn(syn::TraitItemFn),
        visit_local => visit_local(syn::Local),
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let inline = node.content.is_some();
        if inline {
            self.inline.push(node.ident.to_string());
        }
        self.scoped(&node.attrs, |this| syn::visit::visit_item_mod(this, node));
        if inline {
            self.inline.pop();
        }
    }

    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        if node.leading_colon.is_some() {
            return;
        }
        let mut leaves = Vec::new();
        flatten(&node.tree, &mut Vec::new(), &mut leaves);
        self.scoped(&node.attrs, |this| {
            for leaf in &leaves {
                this.record(&leaf.path);
            }
        });
    }

    fn visit_path(&mut self, node: &'ast syn::Path) {
        if node.leading_colon.is_none() {
            let raw: Vec<String> = node.segments.iter().map(|s| s.ident.to_string()).collect();
            self.record(&raw);
        }
        syn::visit::visit_path(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        for raw in macro_paths(&node.tokens.to_string()) {
            self.record(&raw);
        }
        syn::visit::visit_macro(self, node);
    }

    /// `pub(crate)` and `pub(in crate::x)` are visibility, not a dependency.
    fn visit_visibility(&mut self, _: &'ast syn::Visibility) {}
}

fn references(tree: &Tree, graph: &Graph) -> BTreeMap<String, Vec<Reference>> {
    let mut all = BTreeMap::new();
    for (file, info) in &graph.files {
        let Ok(parsed) = syn::parse_file(&tree.files[file]) else {
            continue;
        };
        let children = parsed
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Mod(m) => Some(m.ident.to_string()),
                _ => None,
            })
            .collect();
        let mut refs = Refs {
            modpath: info.modpath.clone(),
            children,
            inline: Vec::new(),
            test_depth: 0,
            file_is_test: info.test,
            found: Vec::new(),
        };
        refs.visit_file(&parsed);
        all.insert(file.clone(), refs.found);
    }
    all
}

/// Module aliases that point into an extracted crate: a transitional
/// `pub use termrock_text::cells as text;` in `ui/mod.rs` makes `ui::text`
/// part of the `text` crate. The shims disappear in #452.
fn crate_aliases(tree: &Tree, graph: &Graph, layering: &Layering) -> BTreeMap<String, String> {
    let mut aliases = BTreeMap::new();
    for (file, info) in &graph.files {
        let Ok(parsed) = syn::parse_file(&tree.files[file]) else {
            continue;
        };
        for item in &parsed.items {
            let syn::Item::Use(u) = item else { continue };
            let mut leaves = Vec::new();
            flatten(&u.tree, &mut Vec::new(), &mut leaves);
            for leaf in leaves {
                let Some(first) = leaf.path.first() else {
                    continue;
                };
                let Some(short) = first.strip_prefix("termrock_") else {
                    continue;
                };
                if layering.position(short).is_none() {
                    continue;
                }
                let Some(name) = leaf.alias.or_else(|| leaf.path.last().cloned()) else {
                    continue;
                };
                let mut at = info.modpath.clone();
                at.push(name);
                aliases.insert(at.join("::"), short.to_string());
            }
        }
    }
    aliases
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Kind {
    Production,
    Test,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Test => "test",
        }
    }
}

/// One dependency that the table does not allow.
#[derive(Clone, Debug)]
struct Edge {
    kind: Kind,
    file: String,
    target: String,
    from: String,
    to: String,
    spelled: String,
}

impl Edge {
    fn key(&self) -> (Kind, String, String) {
        (self.kind, self.file.clone(), self.target.clone())
    }

    fn line(&self) -> String {
        format!("{}\t{}\t{}", self.kind.name(), self.file, self.target)
    }
}

struct Scan {
    edges: Vec<Edge>,
    problems: Vec<String>,
}

fn scan_tree(tree: &Tree, layering: &Layering) -> Scan {
    let graph = build_graph(tree);
    let mut problems = graph.problems.clone();
    let aliases = crate_aliases(tree, &graph, layering);
    let refs = references(tree, &graph);
    let mut edges: BTreeMap<(Kind, String, String), Edge> = BTreeMap::new();
    for (file, found) in refs {
        let Some(from) = layering.crate_of_file(&file) else {
            problems.push(format!("{file}: no [files] entry in {LAYERING}"));
            continue;
        };
        let permitted = layering.permitted(from);
        for reference in found {
            let exported = match reference.path.as_slice() {
                [name] => graph.macros.get(name),
                _ => None,
            };
            let lookup = |n: usize| {
                let key = match exported {
                    Some(module) => module.clone(),
                    None => reference.path[..n].join("::"),
                };
                if let Some(krate) = aliases.get(&key) {
                    return Some((key, krate.clone()));
                }
                let file = graph.modules.get(&key)?;
                Some((key, layering.crate_of_file(file)?.to_string()))
            };
            let Some((target, to)) = (0..=reference.path.len()).rev().find_map(lookup) else {
                continue;
            };
            if to == from || permitted.contains(&to) {
                continue;
            }
            let kind = if reference.test {
                Kind::Test
            } else {
                Kind::Production
            };
            let edge = Edge {
                kind,
                file: file.clone(),
                target: if target.is_empty() {
                    "crate".to_string()
                } else {
                    target
                },
                from: from.to_string(),
                to,
                spelled: reference.spelled,
            };
            edges.entry(edge.key()).or_insert(edge);
        }
    }
    Scan {
        edges: edges.into_values().collect(),
        problems,
    }
}

/// Files and `[files]` keys must correspond one to one.
fn coverage_problems(tree: &Tree, layering: &Layering) -> Vec<String> {
    let mut problems = Vec::new();
    for file in tree.files.keys() {
        if layering.crate_of_file(file).is_none() {
            problems.push(format!(
                "{file} has no [files] entry in {LAYERING}; map it to the crate that will own it"
            ));
        }
    }
    for key in layering.files.keys() {
        let used = tree
            .files
            .keys()
            .any(|f| f == key || (key.ends_with('/') && f.starts_with(key)));
        if !used {
            problems.push(format!(
                "[files] entry `{key}` matches no source file; delete it (an extraction PR removes the entries of the files it moves)"
            ));
        }
    }
    problems
}

// ---------------------------------------------------------------------------
// Allowlist
// ---------------------------------------------------------------------------

fn parse_allowlist(text: &str) -> Result<Vec<(Kind, String, String)>, String> {
    let mut entries = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let columns: Vec<&str> = line.split('\t').collect();
        let [kind, file, target] = columns[..] else {
            return Err(format!(
                "{ALLOWLIST}:{}: expected `kind<TAB>file<TAB>target`",
                n + 1
            ));
        };
        let kind = match kind {
            "production" => Kind::Production,
            "test" => Kind::Test,
            other => return Err(format!("{ALLOWLIST}:{}: unknown kind `{other}`", n + 1)),
        };
        entries.push((kind, file.to_string(), target.to_string()));
    }
    Ok(entries)
}

/// Compares the scan with the allowlist. A dependency the table forbids and
/// the allowlist does not list is new; a listed line without a dependency is
/// stale. Both fail, so the list can only shrink.
fn judge(edges: &[Edge], allowlist: &[(Kind, String, String)]) -> Vec<String> {
    let mut problems = Vec::new();
    let listed: BTreeSet<_> = allowlist.iter().cloned().collect();
    if listed.len() != allowlist.len() {
        problems.push(format!("{ALLOWLIST} lists a line twice"));
    }
    let mut sorted = allowlist.to_vec();
    sorted.sort();
    if sorted != allowlist {
        problems.push(format!("{ALLOWLIST} must stay sorted (kind, file, target)"));
    }
    for edge in edges {
        if !listed.contains(&edge.key()) {
            problems.push(format!(
                "{file}: `{from}` must not depend on `{to}`. {kind} code names `{spelled}` (module `{target}`). {FIX}",
                file = edge.file,
                from = Layering::package(&edge.from),
                to = Layering::package(&edge.to),
                kind = edge.kind.name(),
                spelled = edge.spelled,
                target = edge.target,
            ));
        }
    }
    let present: BTreeSet<_> = edges.iter().map(Edge::key).collect();
    for entry in &listed {
        if !present.contains(entry) {
            problems.push(format!(
                "stale allowlist line `{} {} {}`: the dependency is gone, so delete the line (the allowlist may only shrink)",
                entry.0.name(),
                entry.1,
                entry.2
            ));
        }
    }
    problems
}

fn load_allowlist() -> Vec<(Kind, String, String)> {
    let text = std::fs::read_to_string(root().join(ALLOWLIST)).expect("read the allowlist");
    parse_allowlist(&text).unwrap_or_else(|e| panic!("{e}"))
}

fn print_edges(edges: &[Edge]) {
    if std::env::var_os("LAYERING_PRINT_EDGES").is_some() {
        for edge in edges {
            eprintln!("{}", edge.line());
        }
    }
}

// ---------------------------------------------------------------------------
// Stage B: crate level, from `cargo metadata`
// ---------------------------------------------------------------------------

/// Checks the workspace packages in `metadata` (the output of `cargo metadata
/// --no-deps`) against the table.
fn check_metadata(metadata: &serde_json::Value, layering: &Layering) -> (usize, Vec<String>) {
    let workspace = metadata["workspace_root"].as_str().unwrap_or_default();
    let mut problems = Vec::new();
    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut checked = 0;
    let packages = metadata["packages"].as_array().cloned().unwrap_or_default();
    for package in &packages {
        let name = package["name"].as_str().unwrap_or_default();
        let manifest = package["manifest_path"].as_str().unwrap_or_default();
        let relative = manifest
            .strip_prefix(workspace)
            .unwrap_or(manifest)
            .trim_start_matches('/');
        let deps: Vec<&str> = package["dependencies"]
            .as_array()
            .map(|d| d.iter().filter_map(|d| d["name"].as_str()).collect())
            .unwrap_or_default();

        if relative.starts_with("apps/") {
            checked += 1;
            if !deps.contains(&"termrock") {
                problems.push(format!("{name}: an application must depend on `termrock`"));
            }
            for dep in &deps {
                if layering.member(dep).is_some() && *dep != "termrock" {
                    problems.push(format!(
                        "{name}: an application depends on `termrock` only, not on `{dep}`. Import `termrock::` paths instead."
                    ));
                }
            }
            continue;
        }

        let Some(short) = layering.member(name) else {
            if relative.starts_with("crates/") && name.starts_with("termrock-") {
                problems.push(format!(
                    "{name}: not in {LAYERING}; add it to `order`, `layers` and `allow` before it ships"
                ));
            }
            continue;
        };
        checked += 1;
        let permitted = layering.permitted(&short);
        for dep in deps {
            let Some(dep_short) = layering.member(dep) else {
                continue;
            };
            if dep_short == short {
                continue;
            }
            graph
                .entry(short.clone())
                .or_default()
                .insert(dep_short.clone());
            if !permitted.contains(&dep_short) {
                problems.push(format!(
                    "{from}: `{from}` must not depend on `{to}` (Cargo.toml dependency `{dep}`). {FIX}",
                    from = Layering::package(&short),
                    to = Layering::package(&dep_short),
                ));
            }
        }
    }
    problems.extend(cycles(&graph));
    problems.extend(feature_forwarding_problems(&packages, layering));
    (checked, problems)
}

/// A crate that declares feature `F` must forward it to every normal dependency
/// that also declares `F`. Without the forward, `cargo check -p <crate>
/// --all-features` fails on its own, and the workspace build only passes by
/// feature unification (#446). A crate that does not declare `F` is exempt, so a
/// dependency's dev-only feature (`termrock-text/test-support`) stays out. `default` is a choice of features, not one.
fn feature_forwarding_problems(packages: &[serde_json::Value], layering: &Layering) -> Vec<String> {
    let declared: BTreeMap<&str, BTreeSet<&str>> = packages
        .iter()
        .filter_map(|p| {
            let features = p["features"].as_object()?;
            Some((
                p["name"].as_str()?,
                features.keys().map(String::as_str).collect(),
            ))
        })
        .collect();
    let mut problems = Vec::new();
    for package in packages {
        let name = package["name"].as_str().unwrap_or_default();
        if layering.member(name).is_none() {
            continue;
        }
        let Some(features) = package["features"].as_object() else {
            continue;
        };
        for (feature, list) in features {
            if feature == "default" {
                continue;
            }
            let forwarded: BTreeSet<&str> = list
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
                .collect();
            for dep in package["dependencies"].as_array().into_iter().flatten() {
                let dep_name = dep["name"].as_str().unwrap_or_default();
                if !dep["kind"].is_null()
                    || !dep_name.starts_with("termrock-") && dep_name != "junie-tui"
                {
                    continue;
                }
                if !declared
                    .get(dep_name)
                    .is_some_and(|f| f.contains(feature.as_str()))
                {
                    continue;
                }
                let rename = dep["rename"].as_str().unwrap_or(dep_name);
                let plain = format!("{rename}/{feature}");
                let weak = format!("{rename}?/{feature}");
                if !forwarded.contains(plain.as_str()) && !forwarded.contains(weak.as_str()) {
                    problems.push(format!(
                        "{name}: feature `{feature}` must forward to `{dep_name}`, which also declares it. Add \"{plain}\" to `{feature}` in its Cargo.toml, or `cargo check -p {name} --all-features` fails on its own."
                    ));
                }
            }
        }
    }
    problems
}

/// A `crates/termrock-*` or `apps/*` directory that `cargo metadata` does not
/// list is outside the workspace, so stage B could not see its dependencies.
/// `directories` are workspace-relative, like `crates/termrock-core`.
fn unlisted_directories(metadata: &serde_json::Value, directories: &[String]) -> Vec<String> {
    let workspace = metadata["workspace_root"].as_str().unwrap_or_default();
    let listed: BTreeSet<String> = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|package| package["manifest_path"].as_str())
        .map(|manifest| {
            manifest
                .strip_prefix(workspace)
                .unwrap_or(manifest)
                .trim_start_matches('/')
                .trim_end_matches("/Cargo.toml")
                .to_string()
        })
        .collect();
    directories
        .iter()
        .filter(|dir| !listed.contains(*dir))
        .map(|dir| {
            format!(
                "{dir} has a Cargo.toml but is not a workspace package, so stage B cannot check it; add it to `members` in the root Cargo.toml"
            )
        })
        .collect()
}

fn cycles(graph: &BTreeMap<String, BTreeSet<String>>) -> Vec<String> {
    fn visit(
        node: &str,
        graph: &BTreeMap<String, BTreeSet<String>>,
        stack: &mut Vec<String>,
        done: &mut BTreeSet<String>,
        out: &mut Vec<String>,
    ) {
        if let Some(at) = stack.iter().position(|n| n == node) {
            let mut cycle = stack[at..].to_vec();
            cycle.push(node.to_string());
            out.push(format!("dependency cycle: {}", cycle.join(" -> ")));
            return;
        }
        if !done.insert(node.to_string()) {
            return;
        }
        stack.push(node.to_string());
        for next in graph.get(node).into_iter().flatten() {
            visit(next, graph, stack, done, out);
        }
        stack.pop();
    }
    let mut out = Vec::new();
    let mut done = BTreeSet::new();
    for node in graph.keys() {
        visit(node, graph, &mut Vec::new(), &mut done, &mut out);
    }
    out
}

/// The `crates/termrock-*` and `apps/*` directories that hold a `Cargo.toml`.
fn package_directories() -> Vec<String> {
    let mut found = Vec::new();
    for (parent, prefix) in [("crates", Some("termrock-")), ("apps", None)] {
        let Ok(entries) = std::fs::read_dir(root().join(parent)) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            if prefix.is_some_and(|p| !name.starts_with(p)) {
                continue;
            }
            if entry.path().join("Cargo.toml").is_file() {
                found.push(format!("{parent}/{name}"));
            }
        }
    }
    found.sort();
    found
}

/// Stage B: the real crate graph from `cargo metadata`, checked against the table.
fn crate_level(layering: &Layering) -> Result<usize, Vec<String>> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = std::process::Command::new(cargo)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
        ])
        .arg("--manifest-path")
        .arg(root().join("Cargo.toml"))
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata prints JSON");
    let (packages, mut problems) = check_metadata(&metadata, layering);
    problems.extend(unlisted_directories(&metadata, &package_directories()));
    if problems.is_empty() {
        Ok(packages)
    } else {
        Err(problems)
    }
}

// ---------------------------------------------------------------------------
// Tests on the real repository
// ---------------------------------------------------------------------------

fn report(what: &str, problems: &[String]) {
    assert!(problems.is_empty(), "{what}:\n  {}", problems.join("\n  "));
}

#[test]
fn layering_table_is_consistent() {
    report(
        "docs/layering.toml is inconsistent",
        &Layering::load().validate(),
    );
}

#[test]
fn layering_files_cover_the_source_tree() {
    let tree = Tree::load();
    assert!(!tree.files.is_empty(), "src/ has no Rust files");
    report(
        "src/ and [files] disagree",
        &coverage_problems(&tree, &Layering::load()),
    );
}

#[test]
fn layering_modules_follow_the_table() {
    let layering = Layering::load();
    let tree = Tree::load();
    let scan = scan_tree(&tree, &layering);
    print_edges(&scan.edges);
    report("module scan failed", &scan.problems);
    report(
        "layering violations (stage A, modules)",
        &judge(&scan.edges, &load_allowlist()),
    );
}

#[test]
fn layering_crates_follow_the_table() {
    match crate_level(&Layering::load()) {
        Ok(packages) => {
            eprintln!("layering stage B (crates) checked {packages} packages");
        }
        Err(problems) => report("layering violations (stage B, crates)", &problems),
    }
}

/// The check is not vacuous on the real tree: a file of a crate below
/// `controls` that imports a controls widget fails, and the message names the file, both crates and
/// the fix.
#[test]
fn layering_flags_core_importing_a_widget_in_the_real_tree() {
    let layering = Layering::load();
    let mut tree = Tree::load();
    // A file of a crate below `controls` that is still a module of the package. The extraction
    // issues move the files of the lowest crates out one crate at a time, so the test takes
    // whichever remains rather than naming one.
    let controls = layering
        .order
        .iter()
        .position(|c| c == "controls")
        .expect("controls is in the order");
    let (file, owner) = layering
        .files
        .iter()
        .filter(|(file, _)| tree.files.contains_key(*file))
        .find(|(_, owner)| layering.order.iter().position(|c| c == *owner) < Some(controls))
        .expect("a file of a crate below controls is still in the package");
    tree.files
        .get_mut(file)
        .expect("the file is in the tree")
        .push_str("\nuse crate::widgets::button::Button;\n");
    let scan = scan_tree(&tree, &layering);
    let problems = judge(&scan.edges, &load_allowlist());
    let message = problems.join("\n");
    let owner = format!("`termrock-{owner}`");
    for needle in [
        file.as_str(),
        owner.as_str(),
        "`termrock-controls`",
        "move the item down or invert the dependency; do not edit the allowlist",
    ] {
        assert!(
            message.to_lowercase().contains(&needle.to_lowercase()),
            "message lacks `{needle}`:\n{message}"
        );
    }
}

// ---------------------------------------------------------------------------
// Tests of the check itself, on small synthetic inputs
// ---------------------------------------------------------------------------

const MINI: &str = r#"
schema = 1
order = ["text", "core", "controls", "dialogs", "termrock"]
layer_order = ["foundations", "components", "facade"]

[layers]
text = "foundations"
core = "foundations"
controls = "components"
dialogs = "components"
termrock = "facade"

[allow]
text = []
core = ["text"]
controls = ["core"]
dialogs = ["controls"]
termrock = ["text", "core", "controls", "dialogs"]

[files]
"src/lib.rs" = "termrock"
"src/core/mod.rs" = "termrock"
"src/widgets/mod.rs" = "termrock"
"src/core/id.rs" = "core"
"src/core/text.rs" = "text"
"src/core/tests.rs" = "core"
"src/widgets/button.rs" = "controls"
"src/widgets/dialog.rs" = "dialogs"
"#;

fn mini_tree(extra: &[(&str, &str)]) -> Tree {
    let mut files: BTreeMap<String, String> = [
        ("src/lib.rs", "pub mod core;\npub mod widgets;\n"),
        (
            "src/core/mod.rs",
            "pub mod id;\npub mod text;\n#[cfg(test)]\nmod tests;\n",
        ),
        ("src/core/id.rs", "pub struct Id;\n"),
        ("src/core/text.rs", "pub struct Text;\n"),
        ("src/core/tests.rs", "use super::id::Id;\n"),
        ("src/widgets/mod.rs", "pub mod button;\npub mod dialog;\n"),
        (
            "src/widgets/button.rs",
            "use crate::core::id::Id;\npub struct Button(Id);\n",
        ),
        (
            "src/widgets/dialog.rs",
            "use super::button::Button;\npub struct Dialog(Button);\n",
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    for (path, text) in extra {
        files.insert((*path).to_string(), (*text).to_string());
    }
    Tree { files }
}

fn mini_problems(tree: &Tree) -> Vec<String> {
    let layering = Layering::parse(MINI);
    assert!(layering.validate().is_empty(), "{:?}", layering.validate());
    let scan = scan_tree(tree, &layering);
    assert!(scan.problems.is_empty(), "{:?}", scan.problems);
    judge(&scan.edges, &[])
}

#[test]
fn layering_accepts_downward_edges() {
    assert_eq!(mini_problems(&mini_tree(&[])), Vec::<String>::new());
}

#[test]
fn layering_rejects_an_upward_use_and_names_the_fix() {
    let tree = mini_tree(&[(
        "src/core/id.rs",
        "use crate::widgets::button::Button;\npub struct Id(Button);\n",
    )]);
    let problems = mini_problems(&tree);
    assert_eq!(problems.len(), 1, "{problems:?}");
    let message = &problems[0];
    assert!(message.contains("src/core/id.rs"), "{message}");
    assert!(message.contains("`termrock-core`"), "{message}");
    assert!(message.contains("`termrock-controls`"), "{message}");
    assert!(message.contains(FIX), "{message}");
}

#[test]
fn layering_rejects_an_upward_path_in_code_types_and_macros() {
    for body in [
        "pub fn f() -> crate::widgets::button::Button { todo!() }\n",
        "pub fn f() { let _ = crate::widgets::button::Button::new(); }\n",
        "pub fn f() { assert!(true, \"x\"); let _ = vec![crate::widgets::button::Button]; }\n",
        "pub fn f(x: &dyn crate::widgets::button::Draw) {}\n",
    ] {
        let tree = mini_tree(&[("src/core/id.rs", body)]);
        let problems = mini_problems(&tree);
        assert_eq!(problems.len(), 1, "{body}: {problems:?}");
    }
}

#[test]
fn layering_resolves_super_self_and_grouped_uses() {
    let tree = mini_tree(&[
        (
            "src/core/id.rs",
            "use super::super::widgets::{button::Button, dialog};\npub struct Id;\n",
        ),
        (
            "src/widgets/button.rs",
            "use crate::{core::id::Id, widgets::dialog::Dialog};\npub struct Button(Id, Dialog);\n",
        ),
    ]);
    let problems = mini_problems(&tree);
    let joined = problems.join("\n");
    assert!(joined.contains("`termrock-controls`"), "{joined}");
    assert!(joined.contains("`termrock-dialogs`"), "{joined}");
    assert_eq!(problems.len(), 3, "{joined}");
}

#[test]
fn layering_ignores_comments_strings_visibility_and_same_crate_paths() {
    let tree = mini_tree(&[(
        "src/core/id.rs",
        "/// See [`crate::widgets::button::Button`].\n\
         pub(crate) struct Id;\n\
         pub fn f() { let _ = \"crate::widgets::button::Button\"; \
         println!(\"{}\", \"crate::widgets::dialog\"); }\n\
         pub use crate::core::text::Text as T;\n",
    )]);
    assert_eq!(mini_problems(&tree), Vec::<String>::new());
}

#[test]
fn layering_classifies_test_code_by_attribute_and_file_name() {
    let layering = Layering::parse(MINI);
    let tree = mini_tree(&[
        (
            "src/core/id.rs",
            "pub struct Id;\n#[cfg(test)]\nmod tests {\n    use crate::widgets::button::Button;\n}\n\
             #[cfg(all(test, feature = \"x\"))]\nfn only_test() { let _ = crate::widgets::dialog::Dialog; }\n",
        ),
        ("src/core/tests.rs", "use crate::widgets::button::Button;\n"),
    ]);
    let scan = scan_tree(&tree, &layering);
    let kinds: BTreeSet<_> = scan
        .edges
        .iter()
        .map(|e| (e.file.as_str(), e.kind, e.target.as_str()))
        .collect();
    assert!(kinds.contains(&("src/core/id.rs", Kind::Test, "widgets::button")));
    assert!(kinds.contains(&("src/core/id.rs", Kind::Test, "widgets::dialog")));
    assert!(
        scan.edges
            .iter()
            .filter(|e| e.file == "src/core/tests.rs")
            .all(|e| e.kind == Kind::Test)
    );
    assert!(scan.edges.iter().all(|e| e.kind == Kind::Test));
}

#[test]
fn layering_follows_path_attributes_and_transitional_reexports() {
    let mut layering = Layering::parse(MINI);
    for file in [
        "src/core/text.rs",
        "src/core/tests.rs",
        "src/widgets/button.rs",
        "src/widgets/dialog.rs",
    ] {
        layering.files.remove(file);
    }
    layering
        .files
        .insert("src/widgets/b.rs".to_string(), "controls".to_string());
    let tree = |core_mod: &str, id: &str| Tree {
        files: [
            ("src/lib.rs", "pub mod core;\npub mod widgets;\n"),
            ("src/core/mod.rs", core_mod),
            ("src/core/id.rs", id),
            // `button` is declared with `#[path]`; its file is `b.rs`.
            (
                "src/widgets/mod.rs",
                "#[path = \"b.rs\"]\npub mod button;\n",
            ),
            (
                "src/widgets/b.rs",
                "use crate::core::text::Text;\nuse crate::core::id::Id;\n",
            ),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect(),
    };

    // `core::text` moved to the `text` crate and `core/mod.rs` re-exports it:
    // a reference through that shim is attributed to `text`, which `core` and
    // `controls` may use.
    let shim = "pub use termrock_text::cells as text;\npub mod id;\n";
    let scan = scan_tree(&tree(shim, "use crate::core::text::Text;\n"), &layering);
    assert!(scan.problems.is_empty(), "{:?}", scan.problems);
    assert!(scan.edges.is_empty(), "{:?}", scan.edges);

    // A shim that points into a higher crate is an upward edge for `core`.
    let up = "pub use termrock_text::cells as text;\npub use termrock_controls::button as btn;\npub mod id;\n";
    let scan = scan_tree(&tree(up, "use crate::core::btn::Button;\n"), &layering);
    assert_eq!(scan.edges.len(), 1, "{:?}", scan.edges);
    assert_eq!(
        (scan.edges[0].from.as_str(), scan.edges[0].to.as_str()),
        ("core", "controls")
    );
    assert_eq!(scan.edges[0].target, "core::btn");

    // A module reached through the `#[path]` attribute resolves to its file.
    let scan = scan_tree(
        &tree(shim, "use crate::widgets::button::Button;\n"),
        &layering,
    );
    assert_eq!(scan.edges.len(), 1, "{:?}", scan.edges);
    assert_eq!(scan.edges[0].target, "widgets::button");
}

#[test]
fn layering_reports_unmapped_unreachable_and_missing_files() {
    let layering = Layering::parse(MINI);
    let mut tree = mini_tree(&[("src/widgets/extra.rs", "pub struct Extra;\n")]);
    let problems = coverage_problems(&tree, &layering);
    assert!(
        problems.iter().any(|p| p.contains("src/widgets/extra.rs")),
        "{problems:?}"
    );
    let scan = scan_tree(&tree, &layering);
    assert!(
        scan.problems.iter().any(|p| p.contains("not reachable")),
        "{:?}",
        scan.problems
    );
    tree.files.remove("src/widgets/button.rs");
    let problems = coverage_problems(&tree, &layering);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("matches no source file")),
        "{problems:?}"
    );
    let scan = scan_tree(&tree, &layering);
    assert!(
        scan.problems.iter().any(|p| p.contains("has no file")),
        "{:?}",
        scan.problems
    );
}

#[test]
fn layering_allowlist_must_match_the_scan_exactly() {
    let layering = Layering::parse(MINI);
    let tree = mini_tree(&[(
        "src/core/id.rs",
        "use crate::widgets::button::Button;\nuse crate::widgets::dialog::Dialog;\n",
    )]);
    let scan = scan_tree(&tree, &layering);
    assert_eq!(scan.edges.len(), 2);
    let full: Vec<_> = scan.edges.iter().map(Edge::key).collect();

    assert!(judge(&scan.edges, &full).is_empty(), "exact list is green");

    let missing = judge(&scan.edges, &full[1..]);
    assert_eq!(missing.len(), 1, "{missing:?}");
    assert!(missing[0].contains("must not depend"), "{missing:?}");

    let mut stale = full.clone();
    stale.push((
        Kind::Production,
        "src/core/id.rs".into(),
        "widgets::gone".into(),
    ));
    stale.sort();
    let problems = judge(&scan.edges, &stale);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("stale"), "{problems:?}");

    let mut unsorted = full.clone();
    unsorted.reverse();
    assert!(
        judge(&scan.edges, &unsorted)
            .iter()
            .any(|p| p.contains("sorted"))
    );

    let parsed = parse_allowlist("# c\nproduction\tsrc/a.rs\tx::y\n\ntest\tsrc/b.rs\tz\n");
    assert_eq!(parsed.expect("parses").len(), 2);
    assert!(parse_allowlist("production\tonly-two\n").is_err());
    assert!(parse_allowlist("sometimes\ta\tb\n").is_err());
}

#[test]
fn layering_table_validation_catches_a_bad_table() {
    let upward = MINI.replace("text = []", "text = [\"core\"]");
    let problems = Layering::parse(&upward).validate();
    assert!(
        problems.iter().any(|p| p.contains("not below it")),
        "{problems:?}"
    );
    let unknown = MINI.replace(
        "\"src/core/id.rs\" = \"core\"",
        "\"src/core/id.rs\" = \"nope\"",
    );
    assert!(
        Layering::parse(&unknown)
            .validate()
            .iter()
            .any(|p| p.contains("unknown crate"))
    );
    let higher = MINI.replace("controls = \"components\"", "controls = \"facade\"");
    assert!(
        Layering::parse(&higher)
            .validate()
            .iter()
            .any(|p| p.contains("higher layer"))
    );
}

#[test]
fn layering_permits_what_a_dependency_already_uses() {
    let layering = Layering::parse(MINI);
    let permitted = layering.permitted("dialogs");
    for krate in ["controls", "core", "text"] {
        assert!(permitted.contains(krate), "{krate}");
    }
    assert!(!permitted.contains("termrock"));
    assert!(layering.permitted("text").is_empty());
}

fn metadata(packages: &[(&str, &str, &[&str])]) -> serde_json::Value {
    serde_json::json!({
        "workspace_root": "/w",
        "packages": packages.iter().map(|(name, manifest, deps)| serde_json::json!({
            "name": name,
            "manifest_path": format!("/w/{manifest}"),
            "dependencies": deps
                .iter()
                .map(|d| serde_json::json!({"name": d, "kind": null}))
                .collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

#[test]
fn layering_crate_level_accepts_the_table() {
    let layering = Layering::parse(MINI);
    let meta = metadata(&[
        ("termrock-text", "crates/termrock-text/Cargo.toml", &[]),
        (
            "termrock-core",
            "crates/termrock-core/Cargo.toml",
            &["termrock-text", "serde_json"],
        ),
        (
            "termrock-dialogs",
            "crates/termrock-dialogs/Cargo.toml",
            &["termrock-controls", "termrock-core"],
        ),
        (
            "termrock-controls",
            "crates/termrock-controls/Cargo.toml",
            &["termrock-core"],
        ),
        (
            "termrock",
            "crates/termrock/Cargo.toml",
            &["termrock-dialogs", "termrock-text"],
        ),
        (
            "termrock-showcase",
            "apps/showcase/Cargo.toml",
            &["termrock"],
        ),
    ]);
    let (checked, problems) = check_metadata(&meta, &layering);
    assert_eq!(problems, Vec::<String>::new());
    assert_eq!(checked, 6);
}

#[test]
fn layering_crate_level_rejects_an_upward_dependency_and_names_the_fix() {
    let layering = Layering::parse(MINI);
    let meta = metadata(&[
        (
            "termrock-controls",
            "crates/termrock-controls/Cargo.toml",
            &["termrock-core", "termrock-dialogs"],
        ),
        ("termrock-core", "crates/termrock-core/Cargo.toml", &[]),
        (
            "termrock-dialogs",
            "crates/termrock-dialogs/Cargo.toml",
            &["termrock-controls"],
        ),
    ]);
    let (_, problems) = check_metadata(&meta, &layering);
    let joined = problems.join("\n");
    assert!(
        joined.contains("`termrock-controls` must not depend on `termrock-dialogs`"),
        "{joined}"
    );
    assert!(joined.contains(FIX), "{joined}");
    assert!(joined.contains("dependency cycle"), "{joined}");
}

#[test]
fn layering_crate_level_checks_applications_and_unlisted_crates() {
    let layering = Layering::parse(MINI);
    let meta = metadata(&[
        (
            "termrock-showcase",
            "apps/showcase/Cargo.toml",
            &["termrock", "termrock-core"],
        ),
        ("termrock-holla", "apps/holla/Cargo.toml", &["ratatui"]),
        (
            "termrock-surprise",
            "crates/termrock-surprise/Cargo.toml",
            &[],
        ),
    ]);
    let (_, problems) = check_metadata(&meta, &layering);
    let joined = problems.join("\n");
    assert!(joined.contains("termrock-showcase: an application depends on `termrock` only"));
    assert!(joined.contains("termrock-holla: an application must depend on `termrock`"));
    assert!(joined.contains("termrock-surprise: not in docs/layering.toml"));
}

#[test]
fn layering_crate_level_rejects_a_directory_outside_the_workspace() {
    let meta = metadata(&[
        ("termrock-core", "crates/termrock-core/Cargo.toml", &[]),
        (
            "termrock-showcase",
            "apps/showcase/Cargo.toml",
            &["termrock"],
        ),
    ]);
    let dirs = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
    assert!(
        unlisted_directories(&meta, &dirs(&["crates/termrock-core", "apps/showcase"])).is_empty()
    );
    let problems = unlisted_directories(
        &meta,
        &dirs(&["crates/termrock-core", "crates/termrock-ghost"]),
    );
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("crates/termrock-ghost"),
        "{problems:?}"
    );
    assert!(
        problems[0].contains("not a workspace package"),
        "{problems:?}"
    );
}

#[test]
fn layering_stage_b_runs_on_the_extracted_crates() {
    let extracted: Vec<_> = std::fs::read_dir(root().join("crates"))
        .expect("crates/")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("termrock-"))
        .collect();
    assert!(
        extracted.iter().any(|name| name == "termrock-metrics"),
        "crates/termrock-metrics must exist: #444 extracted it first"
    );
    match crate_level(&Layering::load()) {
        Ok(packages) => assert!(
            packages > extracted.len(),
            "stage B checked {packages} packages, fewer than the {} extracted crates plus the facade",
            extracted.len()
        ),
        Err(problems) => panic!("{problems:?}"),
    }
}
