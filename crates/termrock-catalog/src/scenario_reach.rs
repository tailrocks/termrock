//! Which identifiers a catalog scenario reaches (#14, layer 1).
//!
//! A `CapabilityProof` names scenarios, and a proof is only credible when a
//! scenario really uses the API it is recorded for. This module follows a
//! scenario's calls without type resolution: a callee is reached when its name
//! is one of the identifiers in the caller's body, resolved inside the
//! caller's own module, through its `use` lines, or through a path such as
//! `pages::keyed_store::draw`. The scenario files share one namespace.
//! A fn's signature counts with its body, a `const` is reached by name like a fn, and a `use a::B as C` rename is
//! undone, so `C::Variant` reads as `B::Variant`.
//! Two fns of one name in one module merge, which only over-approximates.
//! An `impl` block is reached by naming its self type: that type's node holds
//! the trait it implements, its methods' names, signatures and bodies, because a
//! scenario that builds a type uses what the type implements even when only
//! the library calls those methods.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use crate::inventory::{Body, flatten_use};
use syn::visit::Visit;

/// The namespace of the scenario files, which call each other's helpers.
const SCENARIOS: &str = "";

type Node = (String, String);

#[derive(Debug, Default)]
pub struct FnIndex {
    /// `(module, name)` to the identifiers of every body filed under it.
    nodes: BTreeMap<Node, BTreeSet<String>>,
    /// `(module, binding)` to the `(module, name)` that a `use` line names.
    imports: BTreeMap<Node, Node>,
    /// `use a::B as C`: `C` to `B`, so a scenario's `C::Variant` is `B::Variant`.
    aliases: BTreeMap<String, String>,
}

/// The module a source file is, by name: its stem, or its directory for `mod.rs`.
fn module_of(path: &str) -> String {
    let path = std::path::Path::new(path);
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
    match stem.as_deref() {
        Some("mod") => path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        other => other.unwrap_or_default().to_owned(),
    }
}

struct Collect<'a> {
    module: &'a str,
    index: &'a mut FnIndex,
}

impl Collect<'_> {
    fn file(&mut self, name: &str, idents: impl IntoIterator<Item = String>) {
        self.index
            .nodes
            .entry((self.module.to_owned(), name.to_owned()))
            .or_default()
            .extend(idents);
    }

    fn body(&mut self, sig: &syn::Signature, block: &syn::Block) {
        let mut body = Body::default();
        body.visit_signature(sig);
        body.visit_block(block);
        self.file(&sig.ident.to_string(), body.0);
    }
}

impl Collect<'_> {
    /// A `const` or `static` is reached by name, like a fn, and holds what its
    /// type and initializer name.
    fn initializer(&mut self, name: &syn::Ident, ty: &syn::Type, expr: &syn::Expr) {
        let mut body = Body::default();
        body.visit_type(ty);
        body.visit_expr(expr);
        self.file(&name.to_string(), body.0);
    }
}

impl<'ast> Visit<'ast> for Collect<'_> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.body(&node.sig, &node.block);
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.body(&node.sig, &node.block);
        syn::visit::visit_impl_item_fn(self, node);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        if let Some(block) = &node.default {
            self.body(&node.sig, block);
        }
        syn::visit::visit_trait_item_fn(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        let mut idents = BTreeSet::new();
        if let Some((path, _)) = &node.trait_ {
            idents.extend(path.segments.iter().map(|s| s.ident.to_string()));
        }
        for item in &node.items {
            if let syn::ImplItem::Fn(f) = item {
                let mut body = Body::default();
                body.visit_signature(&f.sig);
                body.visit_block(&f.block);
                idents.insert(f.sig.ident.to_string());
                idents.extend(body.0);
            }
        }
        if let syn::Type::Path(ty) = &*node.self_ty
            && let Some(last) = ty.path.segments.last()
        {
            self.file(&last.ident.to_string(), idents);
        }
        syn::visit::visit_item_impl(self, node);
    }

    fn visit_item_const(&mut self, node: &'ast syn::ItemConst) {
        self.initializer(&node.ident, &node.ty, &node.expr);
        syn::visit::visit_item_const(self, node);
    }

    fn visit_item_static(&mut self, node: &'ast syn::ItemStatic) {
        self.initializer(&node.ident, &node.ty, &node.expr);
        syn::visit::visit_item_static(self, node);
    }

    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        let mut entries = Vec::new();
        flatten_use(&node.tree, Vec::new(), false, &mut entries);
        for entry in entries {
            if let (Some(binding), Some(original)) = (&entry.binding, entry.path.last())
                && binding != original
            {
                self.index.aliases.insert(binding.clone(), original.clone());
            }
            if let (Some(binding), [.., module, name]) = (entry.binding, entry.path.as_slice()) {
                self.index.imports.insert(
                    (self.module.to_owned(), binding),
                    (module.clone(), name.clone()),
                );
            }
        }
    }
}

impl FnIndex {
    /// Indexes the scenario sources into one namespace, and each `(path, text)`
    /// of the rest of the crate into the module its file is.
    pub fn from_sources<'a>(
        scenarios: impl IntoIterator<Item = &'a str>,
        crate_files: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Self {
        let mut index = Self::default();
        let files = scenarios
            .into_iter()
            .map(|text| (SCENARIOS.to_owned(), text))
            .chain(
                crate_files
                    .into_iter()
                    .map(|(path, text)| (module_of(path), text)),
            );
        for (module, text) in files {
            if let Ok(parsed) = syn::parse_file(text) {
                Collect {
                    module: &module,
                    index: &mut index,
                }
                .visit_file(&parsed);
            }
        }
        index
    }

    /// Every node an identifier met in `module` stands for.
    fn resolve(&self, module: &str, ident: &str) -> Vec<Node> {
        let mut found = Vec::new();
        let own = (module.to_owned(), ident.to_owned());
        if self.nodes.contains_key(&own) {
            found.push(own.clone());
        }
        if let Some(target) = self.imports.get(&own)
            && self.nodes.contains_key(target)
        {
            found.push(target.clone());
        }
        if let Some((owner, name)) = ident.split_once("::") {
            let target = (owner.to_owned(), name.to_owned());
            if self.nodes.contains_key(&target) {
                found.push(target);
            }
        }
        found
    }

    /// The identifiers in the body of scenario fn `root` and of every fn it
    /// reaches, transitively. Empty when `root` is not an indexed fn.
    pub fn reach(&self, root: &str) -> BTreeSet<String> {
        let mut seen: BTreeSet<Node> = BTreeSet::new();
        let mut out = BTreeSet::new();
        let mut queue = vec![(SCENARIOS.to_owned(), root.to_owned())];
        while let Some(node) = queue.pop() {
            if !seen.insert(node.clone()) {
                continue;
            }
            let Some(body) = self.nodes.get(&node) else {
                continue;
            };
            out.extend(body.iter().cloned());
            for ident in body {
                queue.extend(self.resolve(&node.0, ident));
            }
        }
        for (alias, original) in &self.aliases {
            let spelled: Vec<String> = out
                .iter()
                .filter_map(|ident| match ident.split_once("::") {
                    Some((owner, rest)) if owner == alias => Some(format!("{original}::{rest}")),
                    _ if ident == alias => Some(original.clone()),
                    _ => None,
                })
                .collect();
            out.extend(spelled);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::FnIndex;

    fn reach(scenario: &str, pages: &[(&str, &str)], root: &str) -> Vec<String> {
        FnIndex::from_sources([scenario], pages.iter().copied())
            .reach(root)
            .into_iter()
            .collect()
    }

    fn has(tokens: &[String], ident: &str) -> bool {
        tokens.iter().any(|t| t == ident)
    }

    #[test]
    fn catalog_reach_follows_helpers_in_the_scenario_files() {
        let tokens = reach(
            "fn run() { help(); } fn help() { deeper(); } fn deeper() { store.remove(1); } fn unrelated() { gone(); }",
            &[],
            "run",
        );
        assert!(has(&tokens, "remove"));
        assert!(!has(&tokens, "gone"));
    }

    #[test]
    fn catalog_reach_reads_macro_arguments_and_skips_literals() {
        let tokens = reach(
            "fn run() { assert_eq!(store.remove(1), None, \"drop\"); }",
            &[],
            "run",
        );
        assert!(has(&tokens, "remove") && !has(&tokens, "drop"));
    }

    #[test]
    fn catalog_reach_enters_another_module_only_by_import_or_path() {
        let page = (
            "src/pages/keyed.rs",
            "pub fn build() { list.remove(0); } pub fn other() { secret(); }",
        );
        let imported = reach(
            "fn run() { use crate::pages::keyed::build; build(); }",
            &[page],
            "run",
        );
        assert!(has(&imported, "remove") && !has(&imported, "secret"));
        let pathed = reach("fn run() { crate::pages::keyed::build(); }", &[page], "run");
        assert!(has(&pathed, "remove"));
        let collision = reach("fn run() { widget.other(); build(); }", &[page], "run");
        assert!(
            !has(&collision, "remove") && !has(&collision, "secret"),
            "a bare name that no use line brings in is not the page's fn"
        );
    }

    #[test]
    fn catalog_reach_counts_what_a_built_type_implements() {
        let tokens = reach(
            "fn run() { Local::new(); } struct Local; impl Source for Local { fn item(&self) -> ItemRef { fetch() } }",
            &[],
            "run",
        );
        for want in ["Source", "item", "ItemRef", "fetch"] {
            assert!(has(&tokens, want), "{want} in {tokens:?}");
        }
    }

    #[test]
    fn catalog_reach_counts_a_signature_bound_and_a_const() {
        let tokens = reach(
            "const KINDS: [Kind; 1] = [Kind::Tab]; fn run() { draw(KINDS); } fn draw<S: Source + ?Sized>(s: S) {}",
            &[],
            "run",
        );
        assert!(has(&tokens, "Source") && has(&tokens, "Kind::Tab"));
    }

    #[test]
    fn catalog_reach_undoes_a_use_rename() {
        let tokens = reach(
            "use a::Overflow as Bar; fn run() { let _ = Bar::Hide; }",
            &[],
            "run",
        );
        assert!(has(&tokens, "Overflow::Hide"));
    }
}
