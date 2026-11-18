//! Source-level inventory of the declared public API of `junie-tui`.
//!
//! The inventory is a syn walk over the exported module graph. It is layer 1
//! of the capability gate: it says which API ids exist and under which
//! feature rows. It never claims that an API behaves correctly.
//!
//! Canonical ids are `module::Type::member`. Tuple fields end in `::0`,
//! `::1`. A re-export resolves to one canonical definition and every other
//! reachable spelling is recorded as an alias.
//!
//! The walk can span several crates (#443). The first `CrateRoot` is the
//! facade: its modules keep their plain paths and every id is spelled the way
//! a consumer reaches the item from the facade. Any other crate is walked too,
//! its modules live under `@<crate name>`, and an item of it is exported only
//! through the facade's `pub use` of it.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use syn::punctuated::Punctuated;
use syn::{Attribute, Fields, ImplItem, Item, Meta, Token, TraitItem, UseTree, Visibility};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApiKind {
    Type,
    Variant,
    Field,
    Function,
    Const,
    TraitItem,
    TraitImpl,
    Expansion,
    Macro,
}

/// One inventory matrix row: the enabled feature set of the build.
#[derive(Clone, Debug)]
pub struct Row {
    pub name: &'static str,
    pub features: &'static [&'static str],
}

pub const MATRIX: &[Row] = &[
    Row {
        name: "minimal",
        features: &[],
    },
    Row {
        name: "default",
        features: &["cli", "terminal"],
    },
    Row {
        name: "all-features",
        features: &["cli", "terminal", "product", "work-metrics"],
    },
];

/// One crate of the walk: its name as other crates write it in a path (`termrock_text`) and the
/// source directory that holds its `lib.rs`, relative to the tree root.
#[derive(Clone, Copy, Debug)]
pub struct CrateRoot<'a> {
    pub name: &'a str,
    pub dir: &'a str,
}

/// The single-crate walk of the repository root package.
pub const ROOT_CRATE: &[CrateRoot<'static>] = &[CrateRoot {
    name: "junie_tui",
    dir: "src",
}];

/// Hand-checked ids that an item-position macro generates.
#[derive(Clone, Debug)]
pub struct Expansion {
    /// `module::macro_name`, for example `core::id::parts`. For a crate other than the facade the
    /// module starts with `@<crate name>`, for example `@termrock_core::id::parts`.
    pub site: &'static str,
    pub ids: &'static [&'static str],
}

/// An in-memory source tree keyed by path relative to the crate root.
#[derive(Clone, Debug, Default)]
pub struct SourceTree {
    pub files: BTreeMap<PathBuf, String>,
}

impl SourceTree {
    pub fn from_dir(root: &Path, dir: &str) -> Self {
        Self::from_dirs(root, &[dir])
    }

    /// Every `.rs` file under each of the `dirs` of `root`, without a `bin` directory.
    pub fn from_dirs(root: &Path, dirs: &[&str]) -> Self {
        fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, String>) {
            let mut entries: Vec<_> = std::fs::read_dir(dir)
                .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
                .map(|e| e.expect("directory entry").path())
                .collect();
            entries.sort();
            for path in entries {
                if path.is_dir() {
                    walk(root, &path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let rel = path.strip_prefix(root).expect("under root").to_path_buf();
                    let text = std::fs::read_to_string(&path)
                        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
                    out.insert(rel, text);
                }
            }
        }
        let mut files = BTreeMap::new();
        for dir in dirs {
            let mut found = BTreeMap::new();
            walk(root, &root.join(dir), &mut found);
            found.retain(|path, _| !path.starts_with(format!("{dir}/bin")));
            files.extend(found);
        }
        Self { files }
    }

    pub fn from_files(files: &[(&str, &str)]) -> Self {
        Self {
            files: files
                .iter()
                .map(|(path, text)| (PathBuf::from(path), (*text).to_owned()))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Export {
    pub kind: ApiKind,
}

#[derive(Debug, Default)]
pub struct Inventory {
    pub exports: BTreeMap<String, Export>,
    /// Non canonical spelling to canonical id.
    pub aliases: BTreeMap<String, String>,
    /// Every module that was traversed, public or private, by definition path.
    pub modules: BTreeSet<String>,
    /// The subset of `modules` reachable from the crate root through `pub mod`.
    pub public_modules: BTreeSet<String>,
    /// Source files that were read as module bodies.
    pub files_read: BTreeSet<PathBuf>,
    /// Files of modules that only exist under `cfg(test)`, found through their `mod x;` declarations.
    pub test_only_files: BTreeSet<PathBuf>,
}

impl Inventory {
    /// Files under the tree that no `mod` declaration reaches, neither as API nor as a test-only module.
    pub fn orphan_files(&self, tree: &SourceTree) -> Vec<PathBuf> {
        tree.files
            .keys()
            .filter(|path| {
                !self.files_read.contains(*path) && !self.test_only_files.contains(*path)
            })
            .cloned()
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Target {
    Module(String),
    Def(String),
}

#[derive(Clone, Debug)]
struct Binding {
    target: Target,
    public: bool,
}

#[derive(Clone, Debug)]
pub struct UseEntry {
    pub path: Vec<String>,
    pub binding: Option<String>,
    pub glob: bool,
    pub public: bool,
    pub unsupported: Option<String>,
}

struct Def {
    kind: ApiKind,
    members: BTreeMap<String, ApiKind>,
    is_trait: bool,
}

struct ImplRec {
    module: String,
    self_ty: Vec<String>,
    trait_path: Option<Vec<String>>,
    members: Vec<(String, ApiKind)>,
}

#[derive(Default)]
struct Module {
    parent: Option<String>,
    public: bool,
    own: BTreeMap<String, Binding>,
    uses: Vec<UseEntry>,
}

struct Walker<'a> {
    tree: &'a SourceTree,
    features: BTreeSet<&'static str>,
    manifest: &'a [Expansion],
    /// Crate name to the key of its root module.
    externs: BTreeMap<String, String>,
    modules: BTreeMap<String, Module>,
    defs: BTreeMap<String, Def>,
    impls: Vec<ImplRec>,
    files_read: BTreeSet<PathBuf>,
    test_only_files: BTreeSet<PathBuf>,
    test_cfg: bool,
    errors: Vec<String>,
}

fn join(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.to_owned()
    } else {
        format!("{module}::{name}")
    }
}

/// The key of the root module of the crate that holds `module`.
fn crate_root_of(module: &str) -> String {
    match module.strip_prefix('@') {
        Some(rest) => format!("@{}", rest.split("::").next().unwrap_or(rest)),
        None => String::new(),
    }
}

fn is_pub(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

/// True for an item a consumer can name and rustdoc lists. `#[doc(hidden)] pub` is how an item crosses a
/// crate boundary of the split (#439) without joining the declared API (#444).
fn exported(vis: &Visibility, attrs: &[Attribute]) -> bool {
    is_pub(vis) && !is_doc_hidden(attrs)
}

fn is_doc_hidden(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("doc")
            && attr
                .parse_args::<syn::Ident>()
                .is_ok_and(|ident| ident == "hidden")
    })
}

fn hidden_pub(vis: &Visibility, attrs: &[Attribute]) -> bool {
    is_pub(vis) && is_doc_hidden(attrs)
}

/// Every `#[doc(hidden)] pub` item, field, variant, method and exported macro of `source`, spelled
/// `Owner::name` inside its file. The inventory gives such an item no id (#444), so this list is
/// what a reviewer sees of them: a hidden item nameable by a consumer carries no catalog entry.
pub fn doc_hidden_items(source: &str) -> Result<Vec<String>, String> {
    let file = syn::parse_file(source).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    hidden_in_items(&file.items, "", &mut out);
    out.sort();
    out.dedup();
    Ok(out)
}

fn hidden_in_items(items: &[Item], owner: &str, out: &mut Vec<String>) {
    let at = |name: &str| {
        if owner.is_empty() {
            name.to_owned()
        } else {
            format!("{owner}::{name}")
        }
    };
    for item in items {
        match item {
            Item::Fn(i) if hidden_pub(&i.vis, &i.attrs) => out.push(at(&i.sig.ident.to_string())),
            Item::Const(i) if hidden_pub(&i.vis, &i.attrs) => out.push(at(&i.ident.to_string())),
            Item::Static(i) if hidden_pub(&i.vis, &i.attrs) => out.push(at(&i.ident.to_string())),
            Item::Type(i) if hidden_pub(&i.vis, &i.attrs) => out.push(at(&i.ident.to_string())),
            Item::Union(i) if hidden_pub(&i.vis, &i.attrs) => out.push(at(&i.ident.to_string())),
            Item::Macro(i) if is_doc_hidden(&i.attrs) => {
                if let Some(name) = &i.ident
                    && i.attrs.iter().any(|a| a.path().is_ident("macro_export"))
                {
                    out.push(at(&format!("{name}!")));
                }
            }
            Item::Struct(i) => {
                let name = i.ident.to_string();
                if hidden_pub(&i.vis, &i.attrs) {
                    out.push(at(&name));
                } else if is_pub(&i.vis) {
                    for (index, field) in i.fields.iter().enumerate() {
                        if hidden_pub(&field.vis, &field.attrs) {
                            let field_name = field
                                .ident
                                .as_ref()
                                .map_or_else(|| index.to_string(), ToString::to_string);
                            out.push(at(&format!("{name}::{field_name}")));
                        }
                    }
                }
            }
            Item::Enum(i) => {
                let name = i.ident.to_string();
                if hidden_pub(&i.vis, &i.attrs) {
                    out.push(at(&name));
                } else if is_pub(&i.vis) {
                    for variant in &i.variants {
                        if is_doc_hidden(&variant.attrs) {
                            out.push(at(&format!("{name}::{}", variant.ident)));
                        }
                    }
                }
            }
            Item::Trait(i) => {
                let name = i.ident.to_string();
                if hidden_pub(&i.vis, &i.attrs) {
                    out.push(at(&name));
                } else if is_pub(&i.vis) {
                    for member in &i.items {
                        let (attrs, member_name) = match member {
                            TraitItem::Fn(f) => (&f.attrs, f.sig.ident.to_string()),
                            TraitItem::Const(c) => (&c.attrs, c.ident.to_string()),
                            TraitItem::Type(t) => (&t.attrs, t.ident.to_string()),
                            _ => continue,
                        };
                        if is_doc_hidden(attrs) {
                            out.push(at(&format!("{name}::{member_name}")));
                        }
                    }
                }
            }
            Item::Mod(i) => {
                let name = i.ident.to_string();
                if hidden_pub(&i.vis, &i.attrs) {
                    out.push(at(&name));
                } else if let Some((_, content)) = &i.content {
                    hidden_in_items(content, &at(&name), out);
                }
            }
            Item::Use(i) if hidden_pub(&i.vis, &i.attrs) => {
                let mut entries = Vec::new();
                flatten_use(&i.tree, Vec::new(), true, &mut entries);
                for entry in entries {
                    out.push(at(&format!("use {}", entry.path.join("::"))));
                }
            }
            Item::Impl(i) if i.trait_.is_none() => {
                let self_ty = match &*i.self_ty {
                    syn::Type::Path(p) => p
                        .path
                        .segments
                        .last()
                        .map_or_else(|| "?".to_owned(), |s| s.ident.to_string()),
                    _ => "?".to_owned(),
                };
                for member in &i.items {
                    let (vis, attrs, member_name) = match member {
                        ImplItem::Fn(f) => (&f.vis, &f.attrs, f.sig.ident.to_string()),
                        ImplItem::Const(c) => (&c.vis, &c.attrs, c.ident.to_string()),
                        ImplItem::Type(t) => (&t.vis, &t.attrs, t.ident.to_string()),
                        _ => continue,
                    };
                    if hidden_pub(vis, attrs) {
                        out.push(at(&format!("{self_ty}::{member_name}")));
                    }
                }
            }
            _ => {}
        }
    }
}

impl<'a> Walker<'a> {
    fn eval_meta(&self, meta: &Meta) -> Result<bool, String> {
        match meta {
            Meta::Path(path) => match path.get_ident().map(ToString::to_string).as_deref() {
                Some("test") => Ok(self.test_cfg),
                Some("windows") => Ok(false),
                Some("unix") => Ok(true),
                other => Err(format!("unsupported cfg predicate `{other:?}`")),
            },
            Meta::NameValue(nv) => {
                let key = nv.path.get_ident().map(ToString::to_string);
                let value = match &nv.value {
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s),
                        ..
                    }) => s.value(),
                    _ => return Err("cfg value is not a string".into()),
                };
                match key.as_deref() {
                    Some("feature") => Ok(self.features.contains(value.as_str())),
                    other => Err(format!("unsupported cfg key `{other:?}`")),
                }
            }
            Meta::List(list) => {
                let name = list.path.get_ident().map(ToString::to_string);
                let inner = list
                    .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                    .map_err(|e| format!("cfg does not parse: {e}"))?;
                let mut values = Vec::new();
                for meta in &inner {
                    values.push(self.eval_meta(meta)?);
                }
                match name.as_deref() {
                    Some("all") => Ok(values.iter().all(|v| *v)),
                    Some("any") => Ok(values.iter().any(|v| *v)),
                    Some("not") if values.len() == 1 => Ok(!values[0]),
                    other => Err(format!("unsupported cfg combinator `{other:?}`")),
                }
            }
        }
    }

    /// True when every `#[cfg]` on the item holds in this matrix row.
    fn enabled(&mut self, attrs: &[Attribute], what: &str) -> bool {
        let mut on = true;
        for attr in attrs {
            if attr.path().is_ident("cfg") {
                match attr
                    .parse_args::<Meta>()
                    .map_err(|e| e.to_string())
                    .and_then(|m| self.eval_meta(&m))
                {
                    Ok(value) => on &= value,
                    Err(why) => self.errors.push(format!("{what}: {why}")),
                }
            }
        }
        on
    }

    /// True when the item is disabled in this row but would compile in a test build of it.
    fn test_gated(&mut self, attrs: &[Attribute]) -> bool {
        let kept = self.errors.len();
        self.test_cfg = true;
        let on = self.enabled(attrs, "cfg(test) probe");
        self.test_cfg = false;
        self.errors.truncate(kept);
        on
    }

    /// The file and child directory of an out-of-line module, if the file exists.
    fn module_file(
        &self,
        item: &syn::ItemMod,
        file: &Path,
        dir: &Path,
    ) -> Option<(PathBuf, PathBuf)> {
        let name = item.ident.to_string();
        if let Some(explicit) = Self::path_attr(&item.attrs) {
            let target = file.parent().unwrap_or(Path::new("")).join(explicit);
            let nested = target
                .parent()
                .unwrap_or(Path::new(""))
                .join(target.file_stem().unwrap_or_default());
            return self
                .tree
                .files
                .contains_key(&target)
                .then_some((target, nested));
        }
        let flat = dir.join(format!("{name}.rs"));
        if self.tree.files.contains_key(&flat) {
            return Some((flat, dir.join(&name)));
        }
        let nested = dir.join(format!("{name}/mod.rs"));
        self.tree
            .files
            .contains_key(&nested)
            .then(|| (nested, dir.join(&name)))
    }

    /// Records the file of a `cfg(test)`-only module and of every module it declares.
    fn collect_test_only(&mut self, item: &syn::ItemMod, file: &Path, dir: &Path) {
        if let Some((_, items)) = &item.content {
            let nested = dir.join(item.ident.to_string());
            for inner in items {
                if let Item::Mod(inner) = inner {
                    self.collect_test_only(inner, file, &nested);
                }
            }
            return;
        }
        // A test module outside the tree (`#[path = "../tests/..."]`) is no orphan, and rustc rejects a missing one.
        let Some((target, nested)) = self.module_file(item, file, dir) else {
            return;
        };
        if !self.test_only_files.insert(target.clone()) {
            return;
        }
        let Some(text) = self.tree.files.get(&target) else {
            return;
        };
        match syn::parse_file(text) {
            Ok(parsed) => {
                for inner in parsed.items {
                    if let Item::Mod(inner) = inner {
                        self.collect_test_only(&inner, &target, &nested);
                    }
                }
            }
            Err(e) => self
                .errors
                .push(format!("{} does not parse: {e}", target.display())),
        }
    }

    fn path_attr(attrs: &[Attribute]) -> Option<String> {
        attrs.iter().find_map(|attr| {
            if !attr.path().is_ident("path") {
                return None;
            }
            match &attr.meta {
                Meta::NameValue(nv) => match &nv.value {
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s),
                        ..
                    }) => Some(s.value()),
                    _ => None,
                },
                _ => None,
            }
        })
    }

    fn walk_file(&mut self, file: &Path, module: &str, children_dir: &Path) {
        let Some(text) = self.tree.files.get(file) else {
            self.errors.push(format!(
                "module `{module}` file {} is missing",
                file.display()
            ));
            return;
        };
        self.files_read.insert(file.to_path_buf());
        let parsed = match syn::parse_file(text) {
            Ok(parsed) => parsed,
            Err(e) => {
                self.errors
                    .push(format!("{} does not parse: {e}", file.display()));
                return;
            }
        };
        if !self.enabled(&parsed.attrs, &format!("{}", file.display())) {
            return;
        }
        self.walk_items(parsed.items, module, file, children_dir);
    }

    fn walk_items(&mut self, items: Vec<Item>, module: &str, file: &Path, dir: &Path) {
        for item in items {
            self.walk_item(item, module, file, dir);
        }
    }

    fn declare(&mut self, module: &str, name: &str, def: Def, public: bool) {
        let path = join(module, name);
        if self.defs.contains_key(&path) {
            self.errors.push(format!("duplicate definition `{path}`"));
            return;
        }
        self.defs.insert(path.clone(), def);
        self.modules
            .get_mut(module)
            .expect("module exists")
            .own
            .insert(
                name.to_owned(),
                Binding {
                    target: Target::Def(path),
                    public,
                },
            );
    }

    fn fields(
        fields: &Fields,
        only_pub: bool,
        prefix: &str,
        members: &mut BTreeMap<String, ApiKind>,
        this: &mut Walker<'_>,
        what: &str,
    ) {
        for (index, field) in fields.iter().enumerate() {
            if !this.enabled(&field.attrs, what) {
                continue;
            }
            if only_pub && !exported(&field.vis, &field.attrs) {
                continue;
            }
            let name = field
                .ident
                .as_ref()
                .map_or_else(|| index.to_string(), ToString::to_string);
            members.insert(format!("{prefix}{name}"), ApiKind::Field);
        }
    }

    fn walk_item(&mut self, item: Item, module: &str, file: &Path, dir: &Path) {
        match item {
            Item::Mod(item) => {
                let name = item.ident.to_string();
                let what = join(module, &name);
                if !self.enabled(&item.attrs, &what) {
                    if self.test_gated(&item.attrs) {
                        self.collect_test_only(&item, file, dir);
                    }
                    return;
                }
                let child = join(module, &name);
                let public = exported(&item.vis, &item.attrs);
                self.modules.insert(
                    child.clone(),
                    Module {
                        parent: Some(module.to_owned()),
                        public,
                        ..Module::default()
                    },
                );
                self.modules
                    .get_mut(module)
                    .expect("module exists")
                    .own
                    .insert(
                        name.clone(),
                        Binding {
                            target: Target::Module(child.clone()),
                            public,
                        },
                    );
                if let Some((_, items)) = item.content {
                    let nested = dir.join(&name);
                    self.walk_items(items, &child, file, &nested);
                } else if let Some(explicit) = Self::path_attr(&item.attrs) {
                    let target = file.parent().unwrap_or(Path::new("")).join(explicit);
                    let nested = target
                        .parent()
                        .unwrap_or(Path::new(""))
                        .join(target.file_stem().unwrap_or_default());
                    self.walk_file(&target, &child, &nested);
                } else {
                    let flat = dir.join(format!("{name}.rs"));
                    let nested = dir.join(format!("{name}/mod.rs"));
                    if self.tree.files.contains_key(&flat) {
                        self.walk_file(&flat, &child, &dir.join(&name));
                    } else if self.tree.files.contains_key(&nested) {
                        self.walk_file(&nested, &child, &dir.join(&name));
                    } else {
                        self.errors.push(format!(
                            "module `{child}` has no file {} or {}",
                            flat.display(),
                            nested.display()
                        ));
                    }
                }
            }
            Item::Struct(item) => {
                let name = item.ident.to_string();
                if !self.enabled(&item.attrs, &join(module, &name)) {
                    return;
                }
                let mut members = BTreeMap::new();
                let what = join(module, &name);
                Self::fields(&item.fields, true, "", &mut members, self, &what);
                let def = Def {
                    kind: ApiKind::Type,
                    members,
                    is_trait: false,
                };
                self.declare(module, &name, def, exported(&item.vis, &item.attrs));
            }
            Item::Enum(item) => {
                let name = item.ident.to_string();
                let what = join(module, &name);
                if !self.enabled(&item.attrs, &what) {
                    return;
                }
                let mut members = BTreeMap::new();
                for variant in &item.variants {
                    if !self.enabled(&variant.attrs, &what) {
                        continue;
                    }
                    let vname = variant.ident.to_string();
                    members.insert(vname.clone(), ApiKind::Variant);
                    Self::fields(
                        &variant.fields,
                        false,
                        &format!("{vname}::"),
                        &mut members,
                        self,
                        &what,
                    );
                }
                let def = Def {
                    kind: ApiKind::Type,
                    members,
                    is_trait: false,
                };
                self.declare(module, &name, def, exported(&item.vis, &item.attrs));
            }
            Item::Union(item) => {
                let name = item.ident.to_string();
                if self.enabled(&item.attrs, &join(module, &name))
                    && exported(&item.vis, &item.attrs)
                {
                    self.errors.push(format!(
                        "unsupported public union `{}`",
                        join(module, &name)
                    ));
                }
            }
            Item::Trait(item) => {
                let name = item.ident.to_string();
                let what = join(module, &name);
                if !self.enabled(&item.attrs, &what) {
                    return;
                }
                let mut members = BTreeMap::new();
                for member in &item.items {
                    let (attrs, ident, kind) = match member {
                        TraitItem::Fn(f) => (&f.attrs, f.sig.ident.to_string(), ApiKind::TraitItem),
                        TraitItem::Const(c) => (&c.attrs, c.ident.to_string(), ApiKind::TraitItem),
                        TraitItem::Type(t) => (&t.attrs, t.ident.to_string(), ApiKind::TraitItem),
                        _ => continue,
                    };
                    if self.enabled(attrs, &what) {
                        members.insert(ident, kind);
                    }
                }
                let def = Def {
                    kind: ApiKind::Type,
                    members,
                    is_trait: true,
                };
                self.declare(module, &name, def, exported(&item.vis, &item.attrs));
            }
            Item::Type(item) => self.simple(
                module,
                &item.attrs,
                &item.vis,
                &item.ident.to_string(),
                ApiKind::Type,
            ),
            Item::Const(item) => self.simple(
                module,
                &item.attrs,
                &item.vis,
                &item.ident.to_string(),
                ApiKind::Const,
            ),
            Item::Static(item) => self.simple(
                module,
                &item.attrs,
                &item.vis,
                &item.ident.to_string(),
                ApiKind::Const,
            ),
            Item::Fn(item) => self.simple(
                module,
                &item.attrs,
                &item.vis,
                &item.sig.ident.to_string(),
                ApiKind::Function,
            ),
            Item::Use(item) => {
                if !self.enabled(&item.attrs, &join(module, "use")) {
                    return;
                }
                let public = exported(&item.vis, &item.attrs);
                let mut entries = Vec::new();
                flatten_use(&item.tree, Vec::new(), public, &mut entries);
                self.modules
                    .get_mut(module)
                    .expect("module exists")
                    .uses
                    .extend(entries);
            }
            Item::Impl(item) => {
                let what = format!("{module}::<impl>");
                if !self.enabled(&item.attrs, &what) {
                    return;
                }
                let syn::Type::Path(ty) = &*item.self_ty else {
                    return;
                };
                let self_ty: Vec<String> = ty
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect();
                let trait_path = item
                    .trait_
                    .as_ref()
                    .map(|(path, _)| path.segments.iter().map(|s| s.ident.to_string()).collect());
                let mut members = Vec::new();
                if trait_path.is_none() {
                    for member in &item.items {
                        match member {
                            ImplItem::Fn(f)
                                if exported(&f.vis, &f.attrs) && self.enabled(&f.attrs, &what) =>
                            {
                                members.push((f.sig.ident.to_string(), ApiKind::Function));
                            }
                            ImplItem::Const(c)
                                if exported(&c.vis, &c.attrs) && self.enabled(&c.attrs, &what) =>
                            {
                                members.push((c.ident.to_string(), ApiKind::Const));
                            }
                            ImplItem::Type(t)
                                if exported(&t.vis, &t.attrs) && self.enabled(&t.attrs, &what) =>
                            {
                                members.push((t.ident.to_string(), ApiKind::TraitItem));
                            }
                            _ => {}
                        }
                    }
                }
                self.impls.push(ImplRec {
                    module: module.to_owned(),
                    self_ty,
                    trait_path,
                    members,
                });
            }
            Item::Macro(item) => {
                let name = item
                    .mac
                    .path
                    .segments
                    .last()
                    .map(|s| s.ident.to_string())
                    .unwrap_or_default();
                if !self.enabled(&item.attrs, &join(module, &name)) {
                    return;
                }
                if let Some(ident) = &item.ident {
                    if item.attrs.iter().any(|a| a.path().is_ident("macro_export")) {
                        let root = crate_root_of(module);
                        let def = Def {
                            kind: ApiKind::Macro,
                            members: BTreeMap::new(),
                            is_trait: false,
                        };
                        self.declare(&root, &format!("{ident}!"), def, true);
                    }
                    return;
                }
                let site = join(module, &name);
                // A restricted `pub(super)` or `pub(crate)` static is not export.
                let text = item.mac.tokens.to_string();
                let tokens: Vec<&str> = text.split_whitespace().collect();
                let private_only = name == "thread_local"
                    && !tokens.iter().enumerate().any(|(at, token)| {
                        *token == "pub"
                            && tokens.get(at + 1).is_none_or(|next| !next.starts_with('('))
                    });
                if private_only {
                    return;
                }
                if !self.manifest.iter().any(|e| e.site == site) {
                    self.errors.push(format!(
                        "unsupported item macro `{name}!` in `{module}`: add `{site}` to the expansion manifest with the ids it generates"
                    ));
                }
            }
            _ => {}
        }
    }

    fn simple(
        &mut self,
        module: &str,
        attrs: &[Attribute],
        vis: &Visibility,
        name: &str,
        kind: ApiKind,
    ) {
        if !self.enabled(attrs, &join(module, name)) {
            return;
        }
        let def = Def {
            kind,
            members: BTreeMap::new(),
            is_trait: false,
        };
        self.declare(module, name, def, exported(vis, attrs));
    }
}

pub fn flatten_use(tree: &UseTree, prefix: Vec<String>, public: bool, out: &mut Vec<UseEntry>) {
    match tree {
        UseTree::Path(p) => {
            let mut next = prefix;
            next.push(p.ident.to_string());
            flatten_use(&p.tree, next, public, out);
        }
        UseTree::Name(n) => {
            let name = n.ident.to_string();
            if name == "self" {
                let binding = prefix.last().cloned();
                out.push(UseEntry {
                    path: prefix,
                    binding,
                    glob: false,
                    public,
                    unsupported: None,
                });
            } else {
                let mut path = prefix;
                path.push(name.clone());
                out.push(UseEntry {
                    path,
                    binding: Some(name),
                    glob: false,
                    public,
                    unsupported: None,
                });
            }
        }
        UseTree::Rename(r) => {
            let mut path = prefix;
            let original = r.ident.to_string();
            let name = r.rename.to_string();
            if original != "self" {
                path.push(original);
            }
            out.push(UseEntry {
                path,
                binding: (name != "_").then_some(name),
                glob: false,
                public,
                unsupported: None,
            });
        }
        UseTree::Glob(_) => out.push(UseEntry {
            path: prefix,
            binding: None,
            glob: true,
            public,
            unsupported: None,
        }),
        UseTree::Group(g) => {
            for item in &g.items {
                flatten_use(item, prefix.clone(), public, out);
            }
        }
    }
}

impl Walker<'_> {
    fn module_parent(&self, module: &str, up: usize) -> Option<String> {
        let mut current = module.to_owned();
        for _ in 0..up {
            current = self.modules.get(&current)?.parent.clone()?;
        }
        Some(current)
    }

    /// What `name` means inside `module`. A macro is bound as `name!`, so it is found by its plain name too.
    fn lookup(&self, module: &str, name: &str) -> Option<Target> {
        let own = &self.modules.get(module)?.own;
        own.get(name)
            .or_else(|| own.get(&format!("{name}!")))
            .map(|b| b.target.clone())
    }

    fn resolve(&self, from: &str, segs: &[String]) -> Option<Target> {
        let (first, rest) = segs.split_first()?;
        let mut current = match first.as_str() {
            "crate" => Target::Module(crate_root_of(from)),
            "self" => Target::Module(from.to_owned()),
            "super" => Target::Module(self.module_parent(from, 1)?),
            name => self.lookup(from, name).or_else(|| {
                // A crate named in the walk is in scope everywhere, after the module's own names.
                self.externs
                    .get(name)
                    .map(|root| Target::Module(root.clone()))
            })?,
        };
        for seg in rest {
            current = match &current {
                Target::Module(m) => {
                    if seg == "super" {
                        Target::Module(self.module_parent(m, 1)?)
                    } else {
                        self.lookup(m, seg)?
                    }
                }
                Target::Def(_) => return None,
            };
        }
        Some(current)
    }

    fn resolve_uses(&mut self) {
        let names: Vec<String> = self.modules.keys().cloned().collect();
        let mut resolved: BTreeSet<(String, usize)> = BTreeSet::new();
        loop {
            let mut changed = false;
            for module in &names {
                let uses = self.modules[module].uses.clone();
                for (index, entry) in uses.iter().enumerate() {
                    if entry.unsupported.is_some() {
                        continue;
                    }
                    let key = (module.clone(), index);
                    if entry.glob {
                        let Some(Target::Module(source)) = self.resolve(module, &entry.path) else {
                            continue;
                        };
                        let incoming: Vec<(String, Binding)> = self.modules[&source]
                            .own
                            .iter()
                            .filter(|(_, b)| b.public)
                            .map(|(k, b)| {
                                (
                                    k.clone(),
                                    Binding {
                                        target: b.target.clone(),
                                        public: entry.public,
                                    },
                                )
                            })
                            .collect();
                        for (name, binding) in incoming {
                            let own = &mut self.modules.get_mut(module).expect("module").own;
                            if let std::collections::btree_map::Entry::Vacant(slot) =
                                own.entry(name)
                            {
                                slot.insert(binding);
                                changed = true;
                            }
                        }
                        resolved.insert(key);
                        continue;
                    }
                    let Some(name) = &entry.binding else {
                        resolved.insert(key);
                        continue;
                    };
                    if let Some(target) = self.resolve(module, &entry.path) {
                        let own = &mut self.modules.get_mut(module).expect("module").own;
                        match own.get_mut(name) {
                            None => {
                                own.insert(
                                    name.clone(),
                                    Binding {
                                        target,
                                        public: entry.public,
                                    },
                                );
                                changed = true;
                            }
                            Some(existing) if existing.target == target => {
                                if entry.public && !existing.public {
                                    existing.public = true;
                                    changed = true;
                                }
                            }
                            Some(_) => {}
                        }
                        resolved.insert(key);
                    }
                }
            }
            if !changed {
                break;
            }
        }
        for module in &names {
            for (index, entry) in self.modules[module].uses.iter().enumerate() {
                if entry.public
                    && !resolved.contains(&(module.clone(), index))
                    && !self.generated_by_a_manifest_macro(module, &entry.path)
                {
                    self.errors.push(format!(
                        "unsupported public re-export `{}` in `{module}`: it does not resolve inside the crate",
                        entry.path.join("::")
                    ));
                }
            }
        }
    }

    /// True when `path` names an item that a macro of the manifest generates, so no definition of it
    /// exists to resolve: the last segment sits in a module that holds a manifest site
    /// (`pub use termrock_metrics::Metric`, where `metrics!` writes `Metric`). The manifest lists the
    /// ids such a macro generates, so the re-export adds nothing to resolve.
    fn generated_by_a_manifest_macro(&self, from: &str, path: &[String]) -> bool {
        let Some((_, parent)) = path.split_last() else {
            return false;
        };
        let Some(Target::Module(source)) = self.resolve(from, parent) else {
            return false;
        };
        self.manifest.iter().any(|e| {
            e.site
                .rsplit_once("::")
                .is_some_and(|(site_module, _)| site_module == source)
        })
    }

    /// Every module a consumer can reach from the facade root, with each path it is spelled by there.
    /// A module another crate defines gets the path of the `pub use` that exposes it.
    fn facade_paths(&self) -> BTreeMap<String, BTreeSet<String>> {
        fn visit(
            walker: &Walker<'_>,
            module: &str,
            path: &str,
            chain: &mut Vec<String>,
            out: &mut BTreeMap<String, BTreeSet<String>>,
        ) {
            if !out
                .entry(module.to_owned())
                .or_default()
                .insert(path.to_owned())
            {
                return;
            }
            chain.push(module.to_owned());
            for (name, binding) in &walker.modules[module].own {
                if let (Target::Module(child), true) = (&binding.target, binding.public)
                    && !chain.contains(child)
                {
                    visit(walker, child, &join(path, name), chain, out);
                }
            }
            chain.pop();
        }
        let mut out = BTreeMap::new();
        visit(self, "", "", &mut Vec::new(), &mut out);
        out
    }
}

/// Build the inventory for one matrix row.
///
/// `preferred` holds spellings that already carry a catalog line. When a
/// definition has several public spellings, one of those wins, so moving the
/// definition to another module behind a `pub use` keeps its id. Without a
/// preferred spelling the definition's own path wins when it is public, else
/// the shortest spelling.
pub fn build(
    tree: &SourceTree,
    row: &Row,
    manifest: &[Expansion],
    preferred: &BTreeSet<String>,
) -> Result<Inventory, Vec<String>> {
    build_crates(tree, ROOT_CRATE, row, manifest, preferred)
}

/// `build` over several crates. The first of `crates` is the facade; the ids are the facade's spellings.
pub fn build_crates(
    tree: &SourceTree,
    crates: &[CrateRoot<'_>],
    row: &Row,
    manifest: &[Expansion],
    preferred: &BTreeSet<String>,
) -> Result<Inventory, Vec<String>> {
    let root_key = |index: usize| {
        if index == 0 {
            String::new()
        } else {
            format!("@{}", crates[index].name)
        }
    };
    let mut walker = Walker {
        tree,
        features: row.features.iter().copied().collect(),
        manifest,
        externs: crates
            .iter()
            .enumerate()
            .skip(1)
            .map(|(index, c)| (c.name.to_owned(), root_key(index)))
            .collect(),
        modules: BTreeMap::new(),
        defs: BTreeMap::new(),
        impls: Vec::new(),
        files_read: BTreeSet::new(),
        test_only_files: BTreeSet::new(),
        test_cfg: false,
        errors: Vec::new(),
    };
    for (index, krate) in crates.iter().enumerate() {
        let key = root_key(index);
        walker.modules.insert(
            key.clone(),
            Module {
                public: true,
                ..Module::default()
            },
        );
        let dir = Path::new(krate.dir);
        walker.walk_file(&dir.join("lib.rs"), &key, dir);
    }
    if !walker.errors.is_empty() {
        return Err(walker.errors);
    }
    walker.resolve_uses();
    if !walker.errors.is_empty() {
        return Err(walker.errors);
    }

    let reachable = walker.facade_paths();
    let mut spellings: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (module, paths) in &reachable {
        for (name, binding) in &walker.modules[module].own {
            if let (Target::Def(def), true) = (&binding.target, binding.public) {
                // A macro is spelled `name!` however a `use` renames it.
                let bang = if walker.defs[def].kind == ApiKind::Macro && !name.ends_with('!') {
                    "!"
                } else {
                    ""
                };
                for path in paths {
                    spellings
                        .entry(def.clone())
                        .or_default()
                        .insert(format!("{}{bang}", join(path, name)));
                }
            }
        }
    }

    let mut canonical: BTreeMap<String, String> = BTreeMap::new();
    for (def, names) in &spellings {
        let pick = if names.len() > 1
            && let Some(kept) = names.iter().find(|n| preferred.contains(*n))
        {
            kept.clone()
        } else if names.contains(def) {
            def.clone()
        } else {
            names
                .iter()
                .min_by_key(|n| (n.matches("::").count(), (*n).clone()))
                .expect("non-empty")
                .clone()
        };
        canonical.insert(def.clone(), pick);
    }

    let mut inventory = Inventory {
        modules: walker
            .modules
            .keys()
            .filter(|m| !m.is_empty())
            .cloned()
            .collect(),
        public_modules: reachable
            .keys()
            .filter(|m| !m.is_empty())
            .cloned()
            .collect(),
        files_read: walker.files_read.clone(),
        test_only_files: walker.test_only_files.clone(),
        ..Inventory::default()
    };
    let add = |id: String, kind: ApiKind, inv: &mut Inventory| {
        inv.exports.insert(id, Export { kind });
    };
    for (def_path, spelled) in &spellings {
        let def = &walker.defs[def_path];
        let canon = &canonical[def_path];
        add(canon.clone(), def.kind, &mut inventory);
        for member in def.members.iter() {
            add(format!("{canon}::{}", member.0), *member.1, &mut inventory);
        }
        for alias in spelled.iter().filter(|a| *a != canon) {
            inventory.aliases.insert(alias.clone(), canon.clone());
            for member in def.members.keys() {
                inventory
                    .aliases
                    .insert(format!("{alias}::{member}"), format!("{canon}::{member}"));
            }
        }
    }

    for imp in &walker.impls {
        let resolve = |segs: &[String]| match walker.resolve(&imp.module, segs) {
            Some(Target::Def(def)) => Some(def),
            _ => None,
        };
        let Some(ty) = resolve(&imp.self_ty) else {
            if imp.trait_path.is_none() {
                return Err(vec![format!(
                    "inherent impl for `{}` in `{}` does not resolve to a crate type",
                    imp.self_ty.join("::"),
                    imp.module
                )]);
            }
            continue;
        };
        let Some(ty_id) = canonical.get(&ty) else {
            continue;
        };
        if let Some(trait_path) = &imp.trait_path {
            if let Some(tr) = resolve(trait_path)
                && walker.defs[&tr].is_trait
                && let Some(trait_id) = canonical.get(&tr)
            {
                add(
                    format!("{ty_id}::<impl {trait_id}>"),
                    ApiKind::TraitImpl,
                    &mut inventory,
                );
            }
            continue;
        }
        for (name, kind) in &imp.members {
            add(format!("{ty_id}::{name}"), *kind, &mut inventory);
        }
        for (alias, canon) in spellings[&ty]
            .iter()
            .filter(|a| *a != ty_id)
            .map(|a| (a.clone(), ty_id.clone()))
        {
            for (name, _) in &imp.members {
                inventory
                    .aliases
                    .insert(format!("{alias}::{name}"), format!("{canon}::{name}"));
            }
        }
    }

    for expansion in manifest {
        let (module, _) = expansion
            .site
            .rsplit_once("::")
            .unwrap_or(("", expansion.site));
        // A macro invoked at the root of another crate (`@termrock_metrics::metrics`) generates ids
        // the facade spells through its re-exports, so the crate counts as reachable once any of its
        // items is.
        let exposed_root = module.starts_with('@')
            && !module.contains("::")
            && spellings.keys().any(|def| {
                def.strip_prefix(module)
                    .is_some_and(|rest| rest.starts_with("::"))
            });
        if reachable.contains_key(module) || module.is_empty() || exposed_root {
            for id in expansion.ids {
                add((*id).to_owned(), ApiKind::Expansion, &mut inventory);
            }
        }
    }
    Ok(inventory)
}

/// A literal value inside a record: only the shapes the capability records use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Str(String),
    Num(u64),
    Path(Vec<String>),
    List(Vec<Value>),
    Some(Box<Value>),
    None,
    Other,
}

impl Value {
    pub fn str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The last path segment, for `PageId::Pickers` or `ScenarioId::X`.
    pub fn leaf(&self) -> Option<&str> {
        match self {
            Self::Path(segs) => segs.last().map(String::as_str),
            _ => None,
        }
    }
}

fn value_of(expr: &syn::Expr) -> Value {
    match expr {
        syn::Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Str(s) => Value::Str(s.value()),
            syn::Lit::Int(n) => Value::Num(n.base10_parse().unwrap_or(0)),
            _ => Value::Other,
        },
        syn::Expr::Path(path) => {
            let segs: Vec<String> = path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            if segs == ["None"] {
                Value::None
            } else {
                Value::Path(segs)
            }
        }
        syn::Expr::Reference(r) => value_of(&r.expr),
        syn::Expr::Array(a) => Value::List(a.elems.iter().map(value_of).collect()),
        syn::Expr::Call(call) => match (&*call.func, call.args.first()) {
            (syn::Expr::Path(p), Some(arg)) if p.path.is_ident("Some") => {
                Value::Some(Box::new(value_of(arg)))
            }
            _ => Value::Other,
        },
        syn::Expr::Group(g) => value_of(&g.expr),
        syn::Expr::Paren(p) => value_of(&p.expr),
        _ => Value::Other,
    }
}

/// One struct literal: its field values by name.
pub type Record = BTreeMap<String, Value>;

/// Every struct literal named `name` in `source`, in source order.
pub fn records_from_source(source: &str, name: &str) -> Vec<Record> {
    use syn::visit::Visit;
    struct Records<'n> {
        name: &'n str,
        out: Vec<Record>,
    }
    impl<'ast> Visit<'ast> for Records<'_> {
        fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
            if node
                .path
                .segments
                .last()
                .is_some_and(|s| s.ident == self.name)
            {
                self.out.push(
                    node.fields
                        .iter()
                        .filter_map(|f| match &f.member {
                            syn::Member::Named(id) => Some((id.to_string(), value_of(&f.expr))),
                            syn::Member::Unnamed(_) => None,
                        })
                        .collect(),
                );
            }
            syn::visit::visit_expr_struct(self, node);
        }
    }
    let file = syn::parse_file(source).expect("record source parses");
    let mut records = Records {
        name,
        out: Vec::new(),
    };
    records.visit_file(&file);
    records.out
}

/// The ids of every `CapabilityProof { api: "..", .. }` literal in `source`.
pub fn proof_ids_from_source(source: &str) -> BTreeSet<String> {
    records_from_source(source, "CapabilityProof")
        .iter()
        .filter_map(|r| r.get("api").and_then(Value::str).map(str::to_owned))
        .collect()
}

/// Exact-id exceptions with their reasons from `CapabilityException { .. }` literals.
pub fn exceptions_from_source(source: &str) -> BTreeMap<String, String> {
    records_from_source(source, "CapabilityException")
        .iter()
        .filter_map(|r| {
            Some((
                r.get("api").and_then(Value::str)?.to_owned(),
                r.get("reason")
                    .and_then(Value::str)
                    .unwrap_or_default()
                    .to_owned(),
            ))
        })
        .collect()
}

fn ignores(attr: &syn::Attribute) -> bool {
    attr.path().is_ident("ignore")
        || (attr.path().is_ident("cfg_attr")
            && attr.meta.require_list().is_ok_and(|list| {
                list.tokens
                    .to_string()
                    .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .any(|t| t == "ignore")
            }))
}

/// Collects identifiers, walking macro arguments token by token. Literals
/// (string, raw string, char) and comments are never identifiers.
fn walk_tokens(mut cursor: syn::buffer::Cursor<'_>, out: &mut BTreeSet<String>) {
    while !cursor.eof() {
        if let Some((ident, rest)) = cursor.ident() {
            out.insert(ident.to_string());
            cursor = rest;
        } else if let Some((inner, _, _, rest)) = cursor.any_group() {
            walk_tokens(inner, out);
            cursor = rest;
        } else if let Some((_, rest)) = cursor.token_tree() {
            cursor = rest;
        } else {
            break;
        }
    }
}

/// Every identifier a syn walk meets, macro arguments included.
#[derive(Default)]
pub struct Body(pub BTreeSet<String>);

impl<'ast> syn::visit::Visit<'ast> for Body {
    fn visit_ident(&mut self, node: &'ast syn::Ident) {
        self.0.insert(node.to_string());
    }
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        let buffer = syn::buffer::TokenBuffer::new2(node.tokens.clone());
        walk_tokens(buffer.begin(), &mut self.0);
        syn::visit::visit_macro(self, node);
    }
    /// A path of two or more segments also records `second_to_last::last`,
    /// which names a callee in another module (`pages::keyed_store::draw`).
    fn visit_path(&mut self, node: &'ast syn::Path) {
        if let [.., owner, last] = node.segments.iter().collect::<Vec<_>>()[..] {
            self.0.insert(format!("{}::{}", owner.ident, last.ident));
        }
        syn::visit::visit_path(self, node);
    }
}

/// One `#[test]` fn found by the syn scan.
#[derive(Clone, Debug, Default)]
pub struct TestFn {
    /// `#[ignore]`, or `ignore` inside a `cfg_attr`.
    pub ignored: bool,
    /// Every identifier in the body, including the tokens of macro calls such
    /// as `assert_eq!`, which syn does not parse. String literals and comments
    /// are not identifiers.
    pub body: BTreeSet<String>,
    /// Every identifier in the file that holds the test, without comments and
    /// string literals.
    pub file: std::rc::Rc<BTreeSet<String>>,
}

/// The identifiers of free text: runs of alphanumerics and `_`.
pub fn idents(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Adds every `#[test]` fn of `text` (the source of `file`) to `found`, keyed
/// `file::name`, with its ignore flag and the identifiers of its body. Tests
/// generated by macros are invisible to this syn scan.
pub fn scan_tests(file: &str, text: &str, found: &mut BTreeMap<String, TestFn>) {
    use syn::visit::Visit;
    struct Tests<'a> {
        file: &'a str,
        idents: std::rc::Rc<BTreeSet<String>>,
        found: &'a mut BTreeMap<String, TestFn>,
    }
    impl<'ast> Visit<'ast> for Tests<'_> {
        fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
            if node.attrs.iter().any(|a| a.path().is_ident("test")) {
                let mut body = Body::default();
                body.visit_block(&node.block);
                let key = format!("{}::{}", self.file, node.sig.ident);
                let entry = self.found.entry(key).or_default();
                entry.ignored |= node.attrs.iter().any(ignores);
                entry.body.extend(body.0);
                entry.file = self.idents.clone();
            }
            syn::visit::visit_item_fn(self, node);
        }
    }
    if let Ok(parsed) = syn::parse_file(text) {
        let mut names = Body::default();
        names.visit_file(&parsed);
        let idents = std::rc::Rc::new(names.0);
        Tests {
            file,
            idents,
            found,
        }
        .visit_file(&parsed);
    }
}

/// `scan_tests` over every `.rs` file under the `dirs` of `root`.
pub fn test_names(root: &Path, dirs: &[&str]) -> BTreeMap<String, TestFn> {
    fn walk(root: &Path, dir: &Path, found: &mut BTreeMap<String, TestFn>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, found);
            } else if path.extension().is_some_and(|e| e == "rs")
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                let file = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                scan_tests(&file, &text, found);
            }
        }
    }
    let mut found = BTreeMap::new();
    for dir in dirs {
        walk(root, &root.join(dir), &mut found);
    }
    found
}

/// The `(id, reason)` pairs of a `const NAME: &[(&str, &str)]` table in `source`.
pub fn string_pairs_from_source(source: &str, name: &str) -> Vec<(String, String)> {
    let file = syn::parse_file(source).expect("table source parses");
    let Some(table) = file.items.iter().find_map(|item| match item {
        Item::Const(c) if c.ident == name => Some(&*c.expr),
        _ => None,
    }) else {
        return Vec::new();
    };
    let syn::Expr::Reference(reference) = table else {
        return Vec::new();
    };
    let syn::Expr::Array(rows) = &*reference.expr else {
        return Vec::new();
    };
    rows.elems
        .iter()
        .filter_map(|row| match row {
            syn::Expr::Tuple(pair) if pair.elems.len() == 2 => Some((
                value_of(&pair.elems[0]).str()?.to_owned(),
                value_of(&pair.elems[1]).str()?.to_owned(),
            )),
            _ => None,
        })
        .collect()
}

/// The non-inherited export ids of one source file mounted at `module` under a crate root.
pub fn module_api(source: &str, module: &str) -> BTreeSet<String> {
    let segments: Vec<&str> = module.split("::").collect();
    let (leaf, parents) = segments.split_last().expect("module path");
    let mut lib = String::new();
    let mut dir = String::from("src");
    for parent in parents {
        lib.push_str(&format!("pub mod {parent} {{ "));
        dir.push('/');
        dir.push_str(parent);
    }
    lib.push_str(&format!("pub mod {leaf};"));
    lib.push_str(&" }".repeat(parents.len()));
    // A one-file scratch crate has no other crate to resolve `pub use termrock_core::...` in, so
    // those re-exports are left out of it.
    let own: String = source
        .lines()
        .filter(|line| !line.starts_with("pub use termrock_"))
        .map(|line| format!("{line}\n"))
        .collect();
    let tree = SourceTree::from_files(&[("src/lib.rs", &lib), (&format!("{dir}/{leaf}.rs"), &own)]);
    let inventory = build(&tree, &MATRIX[2], &[], &BTreeSet::new())
        .unwrap_or_else(|e| panic!("{}", e.join("\n")));
    inventory
        .exports
        .into_iter()
        .filter(|(_, export)| {
            !matches!(
                export.kind,
                ApiKind::Field | ApiKind::TraitItem | ApiKind::TraitImpl
            )
        })
        .map(|(id, _)| id)
        .collect()
}
