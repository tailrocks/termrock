//! Planning aid for the #14 burn-down: emits one TSV row of facts per
//! `coverage_debt` entry so `scripts/catalog-debt-lanes.py` can classify and
//! split them without hand work.
//!
//! This is not part of the gate. The test is `#[ignore]`d and only writes a
//! file when run with `--run-ignored only`, to `target/debt-facts.tsv`. It reads the same syn
//! inventory the gate uses (the kind of every export) and adds what the
//! inventory does not carry: the shape of each definition (receiver, return
//! type, variant fields) and where it is defined.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use syn::{ImplItem, Item, ReturnType, Type, Visibility};

use super::api_inventory::{self, MATRIX, SourceTree, build};

/// One definition found by the scan, keyed by `(owner ident, member ident)`.
/// Free items and types use an empty owner.
struct Found {
    file: PathBuf,
    shape: String,
}

fn is_pub(vis: &Visibility) -> bool {
    !matches!(vis, Visibility::Inherited)
}

fn self_ident(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()),
        Type::Reference(r) => self_ident(&r.elem),
        _ => None,
    }
}

fn returns(output: &ReturnType, owner: &str) -> &'static str {
    match output {
        ReturnType::Default => "unit",
        ReturnType::Type(_, ty) => {
            let text = quote_free(ty);
            if text == "Self" || text == owner {
                "self"
            } else if text.contains("Self") {
                "wraps_self"
            } else if text == "bool" {
                "bool"
            } else if text.starts_with("Option") {
                "option"
            } else if text.starts_with("Result") {
                "result"
            } else {
                "value"
            }
        }
    }
}

/// A compact type string without the `quote` crate: the debug-free token walk
/// is enough to tell `Self` from other returns.
fn quote_free(ty: &Type) -> String {
    match ty {
        Type::Path(p) => p
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::"),
        Type::Reference(r) => format!("&{}", quote_free(&r.elem)),
        Type::Tuple(t) if t.elems.is_empty() => "()".to_owned(),
        _ => "other".to_owned(),
    }
}

fn fn_shape(sig: &syn::Signature, owner: &str) -> String {
    let recv = match sig.inputs.first() {
        Some(syn::FnArg::Receiver(r)) => match &r.kind {
            syn::ReceiverKind::Reference(_, _, Some(_)) => "mut_self",
            syn::ReceiverKind::Reference(_, _, None) => "ref_self",
            _ => "own_self",
        },
        _ => "assoc",
    };
    let args = sig
        .inputs
        .iter()
        .filter(|a| matches!(a, syn::FnArg::Typed(_)))
        .count();
    format!(
        "fn:{recv}:args={args}:ret={}{}",
        returns(&sig.output, owner),
        if sig.constness.is_some() {
            ":const"
        } else {
            ""
        }
    )
}

fn scan(file: &Path, text: &str, out: &mut BTreeMap<(String, String), Vec<Found>>) {
    let Ok(ast) = syn::parse_file(text) else {
        return;
    };
    fn items(file: &Path, list: &[Item], out: &mut BTreeMap<(String, String), Vec<Found>>) {
        for item in list {
            let mut add = |owner: &str, name: String, shape: String| {
                out.entry((owner.to_owned(), name))
                    .or_default()
                    .push(Found {
                        file: file.to_path_buf(),
                        shape,
                    });
            };
            match item {
                Item::Struct(s) => {
                    let shape = match &s.fields {
                        syn::Fields::Named(_) => "type:struct",
                        syn::Fields::Unnamed(_) => "type:tuple_struct",
                        syn::Fields::Unit => "type:unit_struct",
                    };
                    add("", s.ident.to_string(), shape.to_owned());
                }
                Item::Enum(e) => {
                    add("", e.ident.to_string(), "type:enum".to_owned());
                    for v in &e.variants {
                        let shape = match &v.fields {
                            syn::Fields::Named(f) => format!("variant:struct:{}", f.named.len()),
                            syn::Fields::Unnamed(f) => format!("variant:tuple:{}", f.unnamed.len()),
                            syn::Fields::Unit => "variant:unit".to_owned(),
                        };
                        add(&e.ident.to_string(), v.ident.to_string(), shape);
                    }
                }
                Item::Trait(t) => add("", t.ident.to_string(), "type:trait".to_owned()),
                Item::Type(t) => add("", t.ident.to_string(), "type:alias".to_owned()),
                Item::Const(c) if is_pub(&c.vis) => add("", c.ident.to_string(), "const".into()),
                Item::Static(c) if is_pub(&c.vis) => {
                    add("", c.ident.to_string(), "static".into());
                }
                Item::Fn(f) if is_pub(&f.vis) => {
                    add("", f.sig.ident.to_string(), fn_shape(&f.sig, ""));
                }
                Item::Impl(i) if i.trait_.is_none() => {
                    let owner = self_ident(&i.self_ty).unwrap_or_default();
                    for member in &i.items {
                        match member {
                            ImplItem::Fn(f) if is_pub(&f.vis) => {
                                add(&owner, f.sig.ident.to_string(), fn_shape(&f.sig, &owner))
                            }
                            ImplItem::Const(c) if is_pub(&c.vis) => {
                                add(&owner, c.ident.to_string(), "assoc_const".into());
                            }
                            _ => {}
                        }
                    }
                }
                Item::Mod(m) => {
                    if let Some((_, inner)) = &m.content {
                        items(file, inner, out);
                    }
                }
                _ => {}
            }
        }
    }
    items(file, &ast.items, out);
}

fn debt_lines(root: &Path) -> Vec<String> {
    let mut ids = Vec::new();
    let dir = root.join("src/bin/showcase/coverage_debt");
    let mut paths: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(d) => d.flatten().map(|e| e.path()).collect(),
        Err(_) => vec![root.join("src/bin/showcase/coverage_debt.txt")],
    };
    paths.sort();
    for path in paths
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
    {
        let text = std::fs::read_to_string(path).expect("debt file reads");
        for line in text.lines() {
            let id = line.split('#').next().unwrap_or("").trim();
            if !id.is_empty() {
                ids.push(id.to_owned());
            }
        }
    }
    ids
}

#[test]
#[ignore = "planning aid: run with --run-ignored only; writes target/debt-facts.tsv"]
fn catalog_debt_facts_are_emitted() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let target = root.join("target/debt-facts.tsv");
    let tree = SourceTree::from_dir(root, "src");
    let inventory = build(&tree, MATRIX.last().expect("matrix row"), super::EXPANSIONS)
        .expect("inventory builds");
    let mut found: BTreeMap<(String, String), Vec<Found>> = BTreeMap::new();
    for (path, text) in &tree.files {
        scan(path, text, &mut found);
    }
    let tests = api_inventory::test_names(root, &["src", "tests", "apps"]);
    let mut rows = vec![
        "id\tinv_kind\towner\tmember\tshape\tfile\tcandidates\tnamed_by\tfirst_test".to_owned(),
    ];
    for id in debt_lines(root) {
        let segs: Vec<&str> = id.split("::").collect();
        let member = segs.last().copied().unwrap_or("").trim_end_matches('!');
        let owner = if segs.len() >= 2
            && inventory
                .exports
                .get(&segs[..segs.len() - 1].join("::"))
                .is_some_and(|e| e.kind == super::ApiKind::Type)
        {
            segs[segs.len() - 2]
        } else {
            ""
        };
        let module_path = segs[..segs.len() - usize::from(!owner.is_empty()) - 1].join("/");
        let kind = inventory
            .exports
            .get(&id)
            .map_or_else(|| "missing".to_owned(), |e| format!("{:?}", e.kind));
        let candidates = found
            .get(&(owner.to_owned(), member.to_owned()))
            .map(Vec::as_slice)
            .unwrap_or_default();
        let best = candidates
            .iter()
            .find(|c| {
                let p = c.file.to_string_lossy();
                !module_path.is_empty()
                    && (p.contains(&format!("src/{module_path}.rs"))
                        || p.contains(&format!("src/{module_path}/")))
            })
            .or_else(|| candidates.first());
        let (shape, file) = best.map_or(("unresolved".to_owned(), String::new()), |c| {
            (c.shape.clone(), c.file.display().to_string())
        });
        // Live tests whose body uses the member and owner: the exact check an
        // exception's `regression_test` must pass, so a lane sees up front
        // which ids already have a test it could name.
        let named: Vec<&String> = tests
            .iter()
            .filter(|(_, t)| !t.ignored && super::names_api(t, &id, &inventory))
            .map(|(key, _)| key)
            .collect();
        rows.push(format!(
            "{id}\t{kind}\t{owner}\t{member}\t{shape}\t{file}\t{}\t{}\t{}",
            candidates.len(),
            named.len(),
            named.first().map_or("", |k| k.as_str())
        ));
    }
    std::fs::write(target, rows.join("\n") + "\n").expect("facts file writes");
}
