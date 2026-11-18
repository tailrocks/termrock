//! Every public item of the session controller module is a catalog line, and
//! no catalog line names an item that is gone.

use std::collections::BTreeSet;

use syn::{ImplItem, Item, TraitItem, Type, Visibility};
use termrock_catalog::catalog;

const SOURCE: &str = include_str!("../../../../src/terminal/session.rs");

fn public(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

fn source_ids() -> BTreeSet<String> {
    let file = syn::parse_file(SOURCE).expect("session.rs parses");
    let mut ids = BTreeSet::new();
    let mut types = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Struct(item) if public(&item.vis) => {
                types.insert(item.ident.to_string());
                ids.insert(format!("terminal::{}", item.ident));
            }
            Item::Enum(item) if public(&item.vis) => {
                types.insert(item.ident.to_string());
                ids.insert(format!("terminal::{}", item.ident));
                ids.extend(
                    item.variants
                        .iter()
                        .map(|v| format!("terminal::{}::{}", item.ident, v.ident)),
                );
            }
            Item::Trait(item) if public(&item.vis) => {
                types.insert(item.ident.to_string());
                ids.insert(format!("terminal::{}", item.ident));
                for member in &item.items {
                    if let TraitItem::Fn(f) = member {
                        ids.insert(format!("terminal::{}::{}", item.ident, f.sig.ident));
                    }
                }
            }
            Item::Const(item) if public(&item.vis) => {
                ids.insert(format!("terminal::{}", item.ident));
            }
            _ => {}
        }
    }
    for item in &file.items {
        let Item::Impl(item) = item else { continue };
        if item.trait_.is_some() {
            continue;
        }
        let Type::Path(path) = &*item.self_ty else {
            continue;
        };
        let Some(name) = path.path.segments.last().map(|s| s.ident.to_string()) else {
            continue;
        };
        if !types.contains(&name) {
            continue;
        }
        for member in &item.items {
            match member {
                ImplItem::Fn(f) if public(&f.vis) => {
                    ids.insert(format!("terminal::{name}::{}", f.sig.ident));
                }
                ImplItem::Const(c) if public(&c.vis) => {
                    ids.insert(format!("terminal::{name}::{}", c.ident));
                }
                _ => {}
            }
        }
    }
    ids
}

#[test]
fn session_controller_api_is_catalogued() {
    let ids = source_ids();
    assert!(ids.contains("terminal::TerminalSessionController::pump"));
    let missing: Vec<_> = ids
        .iter()
        .filter(|id| catalog::find(id).is_none())
        .collect();
    assert!(
        missing.is_empty(),
        "add these lines to crates/termrock-catalog/src/catalog.rs:\n{}",
        missing
            .iter()
            .map(|id| format!("    Capability::function(\"{id}\"),"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn session_controller_catalog_has_no_stale_lines() {
    let ids = source_ids();
    let own = |id: &str| {
        let name = id.trim_start_matches("terminal::");
        [
            "Session",
            "TerminalSessionController",
            "ReplayTiming",
            "PENDING_EFFECTS",
        ]
        .iter()
        .any(|prefix| name.starts_with(prefix))
    };
    let stale: Vec<_> = catalog::CAPABILITIES
        .iter()
        .map(|cap| cap.id)
        .filter(|id| id.starts_with("terminal::") && own(id) && !ids.contains(*id))
        .collect();
    assert!(stale.is_empty(), "catalog lines without an item: {stale:?}");
}
