use std::collections::{BTreeMap, BTreeSet};

use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::{ItemKey, WidgetId};
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::popup::backdrop_id;
use junie_tui::widgets::option_list::Item as ListItem;
use junie_tui::widgets::picker::{Picker, PickerStatus};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use syn::visit::Visit;
use syn::{Expr, ImplItem, Item, Pat, Type, Visibility};

#[path = "../src/bin/showcase/coverage.rs"]
mod coverage;

fn picker_api(source: &str) -> BTreeSet<String> {
    let file = syn::parse_file(source).expect("picker source parses");
    let mut ids = BTreeSet::new();
    let mut public_types = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Enum(item) if matches!(item.vis, Visibility::Public(_)) => {
                let name = item.ident.to_string();
                let root = format!("widgets::picker::{name}");
                ids.insert(root.clone());
                public_types.insert(name);
                ids.extend(
                    item.variants
                        .iter()
                        .map(|variant| format!("{root}::{}", variant.ident)),
                );
            }
            Item::Struct(item) if matches!(item.vis, Visibility::Public(_)) => {
                let name = item.ident.to_string();
                ids.insert(format!("widgets::picker::{name}"));
                public_types.insert(name);
            }
            Item::Fn(item) if matches!(item.vis, Visibility::Public(_)) => {
                ids.insert(format!("widgets::picker::{}", item.sig.ident));
            }
            Item::Const(item) if matches!(item.vis, Visibility::Public(_)) => {
                ids.insert(format!("widgets::picker::{}", item.ident));
            }
            _ => {}
        }
    }
    for item in file.items {
        if let Item::Impl(item) = item
            && item.trait_.is_none()
        {
            let Type::Path(path) = *item.self_ty else {
                continue;
            };
            let Some(ty) = path.path.segments.last() else {
                continue;
            };
            if !public_types.contains(&ty.ident.to_string()) {
                continue;
            }
            for method in item.items {
                match method {
                    ImplItem::Fn(method) if matches!(method.vis, Visibility::Public(_)) => {
                        ids.insert(format!(
                            "widgets::picker::{}::{}",
                            ty.ident, method.sig.ident
                        ));
                    }
                    ImplItem::Const(constant) if matches!(constant.vis, Visibility::Public(_)) => {
                        ids.insert(format!("widgets::picker::{}::{}", ty.ident, constant.ident));
                    }
                    _ => {}
                }
            }
        }
    }
    ids
}

fn option_list_api(source: &str) -> BTreeSet<String> {
    let file = syn::parse_file(source).expect("option list source parses");
    let mut ids = BTreeSet::new();
    let mut public_type = false;
    for item in &file.items {
        if let Item::Struct(item) = item
            && matches!(item.vis, Visibility::Public(_))
            && item.ident == "OptionList"
        {
            public_type = true;
        }
    }
    if !public_type {
        return ids;
    }
    ids.insert("widgets::option_list::OptionList".to_owned());
    for item in &file.items {
        if let Item::Impl(item) = item
            && item.trait_.is_none()
            && let Type::Path(path) = &*item.self_ty
            && path
                .path
                .segments
                .last()
                .is_some_and(|ty| ty.ident == "OptionList")
        {
            for method in &item.items {
                match method {
                    ImplItem::Fn(method) if matches!(method.vis, Visibility::Public(_)) => {
                        ids.insert(format!(
                            "widgets::option_list::OptionList::{}",
                            method.sig.ident
                        ));
                    }
                    ImplItem::Const(constant) if matches!(constant.vis, Visibility::Public(_)) => {
                        ids.insert(format!(
                            "widgets::option_list::OptionList::{}",
                            constant.ident
                        ));
                    }
                    _ => {}
                }
            }
        }
    }
    ids
}

fn picker_catalog(source: &str) -> BTreeMap<String, String> {
    let file = syn::parse_file(source).expect("catalog source parses");
    let mut visitor = CatalogVisitor::default();
    visitor.visit_file(&file);
    visitor.entries
}

#[derive(Default)]
struct CatalogVisitor {
    entries: BTreeMap<String, String>,
}

impl<'ast> Visit<'ast> for CatalogVisitor {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Expr::Path(function) = node.func.as_ref()
            && let Some(kind) = function.path.segments.last()
            && let Some(Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(id),
                ..
            })) = node.args.first()
        {
            let id = id.value();
            if id.starts_with("widgets::picker::") {
                self.entries.insert(id, kind.ident.to_string());
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}

#[derive(Default)]
struct PageUsage {
    api: BTreeSet<String>,
    picker_vars: BTreeSet<String>,
    builder_names: BTreeSet<String>,
    exported: BTreeSet<String>,
    configured_toggle: bool,
}

impl PageUsage {
    fn from_source(
        source: &str,
        exported: &BTreeSet<String>,
        catalog: &BTreeMap<String, String>,
    ) -> Self {
        let file = syn::parse_file(source).expect("showcase page parses");
        let builder_names = catalog
            .iter()
            .filter_map(|(id, kind)| {
                (kind == "builder")
                    .then(|| id.strip_prefix("widgets::picker::Picker::"))
                    .flatten()
                    .map(str::to_owned)
            })
            .collect();
        let mut usage = Self {
            builder_names,
            exported: exported.clone(),
            ..Self::default()
        };
        usage.visit_file(&file);
        usage
    }

    fn is_picker_type(ty: &Type) -> bool {
        match ty {
            Type::Path(path) => path
                .path
                .segments
                .last()
                .is_some_and(|part| part.ident == "Picker"),
            Type::Reference(reference) => Self::is_picker_type(&reference.elem),
            Type::Paren(paren) => Self::is_picker_type(&paren.elem),
            Type::Group(group) => Self::is_picker_type(&group.elem),
            _ => false,
        }
    }

    fn bind_pattern(&mut self, pattern: &Pat) {
        match pattern {
            Pat::Ident(ident) => {
                self.picker_vars.insert(ident.ident.to_string());
            }
            Pat::Reference(reference) => self.bind_pattern(&reference.pat),
            Pat::Paren(paren) => self.bind_pattern(&paren.pat),
            Pat::Type(typed) => self.bind_pattern(&typed.pat),
            _ => {}
        }
    }

    fn unbind_pattern(&mut self, pattern: &Pat) {
        struct Bindings(Vec<String>);
        impl<'ast> Visit<'ast> for Bindings {
            fn visit_pat_ident(&mut self, node: &'ast syn::PatIdent) {
                self.0.push(node.ident.to_string());
                syn::visit::visit_pat_ident(self, node);
            }
        }
        let mut bindings = Bindings(Vec::new());
        bindings.visit_pat(pattern);
        for name in bindings.0 {
            self.picker_vars.remove(&name);
        }
    }

    fn visit_condition(&mut self, expr: &Expr) {
        match expr {
            Expr::Let(binding) => {
                self.visit_expr(&binding.expr);
                self.unbind_pattern(&binding.pat);
                self.visit_pat(&binding.pat);
            }
            Expr::Binary(binary) if matches!(binary.op, syn::BinOp::And(_)) => {
                self.visit_condition(&binary.left);
                self.visit_condition(&binary.right);
            }
            Expr::Paren(paren) => self.visit_condition(&paren.expr),
            Expr::Group(group) => self.visit_condition(&group.expr),
            _ => self.visit_expr(expr),
        }
    }

    fn is_picker_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Path(path) => path
                .path
                .get_ident()
                .is_some_and(|ident| self.picker_vars.contains(&ident.to_string())),
            Expr::Call(call) => {
                matches!(&*call.func, Expr::Path(path) if path.path.segments.iter().map(|part| part.ident.to_string()).collect::<Vec<_>>().ends_with(&["Picker".to_owned(), "new".to_owned()]))
            }
            Expr::MethodCall(call) => {
                self.builder_names.contains(&call.method.to_string())
                    && self.is_picker_expr(&call.receiver)
            }
            Expr::Paren(paren) => self.is_picker_expr(&paren.expr),
            Expr::Group(group) => self.is_picker_expr(&group.expr),
            Expr::Reference(reference) => self.is_picker_expr(&reference.expr),
            Expr::Match(matched) => {
                !matched.arms.is_empty()
                    && matched
                        .arms
                        .iter()
                        .all(|arm| self.is_picker_expr(&arm.body))
            }
            Expr::If(branch) => {
                branch
                    .else_branch
                    .as_ref()
                    .is_some_and(|(_, expr)| self.is_picker_expr(expr))
                    && branch
                        .then_branch
                        .stmts
                        .last()
                        .is_some_and(|stmt| match stmt {
                            syn::Stmt::Expr(expr, _) => self.is_picker_expr(expr),
                            _ => false,
                        })
            }
            _ => false,
        }
    }

    fn record_type(&mut self, name: &str) {
        if matches!(
            name,
            "Picker"
                | "PickerAction"
                | "PickerCmd"
                | "PickerKeyProfile"
                | "PickerStatus"
                | "VisibleKey"
        ) && self.exported.contains(&format!("widgets::picker::{name}"))
        {
            self.api.insert(format!("widgets::picker::{name}"));
        }
    }

    fn record_path(&mut self, path: &syn::Path) {
        let segments: Vec<String> = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        for (index, segment) in segments.iter().enumerate() {
            self.record_type(segment);
            if index > 0
                && matches!(
                    segments[index - 1].as_str(),
                    "PickerAction" | "PickerCmd" | "PickerKeyProfile" | "PickerStatus"
                )
            {
                let id = format!("widgets::picker::{}::{}", segments[index - 1], segment);
                if self.exported.contains(&id) {
                    self.api.insert(id);
                }
            }
        }
    }

    fn bind_picker_arguments(
        &mut self,
        inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::Token![,]>,
    ) {
        for input in inputs {
            if let syn::FnArg::Typed(typed) = input
                && Self::is_picker_type(&typed.ty)
            {
                self.bind_pattern(&typed.pat);
            }
        }
    }
}

impl<'ast> Visit<'ast> for PageUsage {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let outer = std::mem::take(&mut self.picker_vars);
        self.bind_picker_arguments(&node.sig.inputs);
        syn::visit::visit_item_fn(self, node);
        self.picker_vars = outer;
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let outer = std::mem::take(&mut self.picker_vars);
        self.bind_picker_arguments(&node.sig.inputs);
        syn::visit::visit_impl_item_fn(self, node);
        self.picker_vars = outer;
    }

    fn visit_block(&mut self, node: &'ast syn::Block) {
        let outer = self.picker_vars.clone();
        syn::visit::visit_block(self, node);
        self.picker_vars = outer;
    }

    fn visit_expr_closure(&mut self, node: &'ast syn::ExprClosure) {
        let outer = self.picker_vars.clone();
        for pattern in &node.inputs {
            self.unbind_pattern(pattern);
            if let Pat::Type(typed) = pattern
                && Self::is_picker_type(&typed.ty)
            {
                self.bind_pattern(pattern);
            }
        }
        syn::visit::visit_expr_closure(self, node);
        self.picker_vars = outer;
    }

    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        let outer = self.picker_vars.clone();
        self.visit_condition(&node.cond);
        self.visit_block(&node.then_branch);
        self.picker_vars = outer.clone();
        if let Some((_, branch)) = &node.else_branch {
            self.visit_expr(branch);
        }
        self.picker_vars = outer;
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        let outer = self.picker_vars.clone();
        self.visit_condition(&node.cond);
        self.visit_block(&node.body);
        self.picker_vars = outer;
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.visit_expr(&node.expr);
        let outer = self.picker_vars.clone();
        self.unbind_pattern(&node.pat);
        self.visit_pat(&node.pat);
        self.visit_block(&node.body);
        self.picker_vars = outer;
    }

    fn visit_arm(&mut self, node: &'ast syn::Arm) {
        let outer = self.picker_vars.clone();
        self.unbind_pattern(&node.pat);
        syn::visit::visit_arm(self, node);
        self.picker_vars = outer;
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        let typed = match &node.pat {
            Pat::Type(typed) => Some(Self::is_picker_type(&typed.ty)),
            _ => None,
        };
        let mut picker = typed.unwrap_or(false);
        if let Some(init) = &node.init {
            self.visit_expr(&init.expr);
            if typed.is_none() {
                picker = self.is_picker_expr(&init.expr);
            }
        }
        if let Some(init) = &node.init
            && let Some((_, diverge)) = &init.diverge
        {
            self.visit_expr(diverge);
        }
        self.unbind_pattern(&node.pat);
        if picker {
            self.bind_pattern(&node.pat);
        }
        self.visit_pat(&node.pat);
    }

    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        for segment in &node.path.segments {
            self.record_type(&segment.ident.to_string());
        }
        syn::visit::visit_type_path(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        self.record_path(&node.path);
        syn::visit::visit_expr_path(self, node);
    }

    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        self.record_path(&node.path);
        syn::visit::visit_expr_struct(self, node);
    }

    fn visit_pat_struct(&mut self, node: &'ast syn::PatStruct) {
        self.record_path(&node.path);
        syn::visit::visit_pat_struct(self, node);
    }

    fn visit_pat_tuple_struct(&mut self, node: &'ast syn::PatTupleStruct) {
        self.record_path(&node.path);
        syn::visit::visit_pat_tuple_struct(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Expr::Path(path) = node.func.as_ref()
            && path
                .path
                .segments
                .iter()
                .map(|part| part.ident.to_string())
                .collect::<Vec<_>>()
                == ["Picker", "new"]
        {
            self.api.insert("widgets::picker::Picker::new".to_owned());
            self.record_type("Picker");
        }
        if let Expr::Path(path) = node.func.as_ref()
            && path
                .path
                .segments
                .iter()
                .map(|part| part.ident.to_string())
                .collect::<Vec<_>>()
                == ["VisibleKey", "new"]
        {
            self.api
                .insert("widgets::picker::VisibleKey::new".to_owned());
            self.record_type("VisibleKey");
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "matched"
            && let Expr::Call(call) = node.receiver.as_ref()
            && let Expr::Path(path) = call.func.as_ref()
            && path
                .path
                .segments
                .iter()
                .map(|part| part.ident.to_string())
                .collect::<Vec<_>>()
                == ["VisibleKey", "new"]
        {
            self.api
                .insert("widgets::picker::VisibleKey::matched".to_owned());
        }
        let id = format!("widgets::picker::Picker::{}", node.method);
        let is_picker = self.is_picker_expr(&node.receiver);
        if is_picker && self.exported.contains(&id) {
            self.api.insert(id);
        }
        if is_picker && node.method == "key_profile" && configured_profile_match(&node.args) {
            self.configured_toggle = true;
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

fn configured_profile_match(args: &syn::punctuated::Punctuated<Expr, syn::Token![,]>) -> bool {
    let Some(Expr::Match(matched)) = args.first() else {
        return false;
    };
    let configured = match matched.expr.as_ref() {
        Expr::MethodCall(call) if call.method == "value" => matches!(
            call.args.first(),
            Some(Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            })) if value.value() == "pickers.nav"
        ),
        _ => false,
    };
    let profiles: BTreeSet<String> = matched
        .arms
        .iter()
        .filter_map(|arm| match &*arm.body {
            Expr::Path(path) => path
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string()),
            _ => None,
        })
        .collect();
    configured && profiles.contains("Desktop") && profiles.contains("Vim")
}

fn validate_picker_sources(
    picker_source: &str,
    catalog_source: &str,
    page_source: &str,
    option_list_source: &str,
) -> Result<(), Vec<String>> {
    let exported = picker_api(picker_source);
    let option_list_exported = option_list_api(option_list_source);
    let catalog = picker_catalog(catalog_source);
    let page = PageUsage::from_source(page_source, &exported, &catalog);
    let coverage_ids: BTreeSet<String> = coverage::PICKER_COVERAGE
        .iter()
        .map(|(id, _)| (*id).to_owned())
        .collect();
    let allowlist: BTreeMap<String, String> = coverage::PICKER_ALLOWLIST
        .iter()
        .map(|(id, reason)| ((*id).to_owned(), (*reason).to_owned()))
        .collect();
    let catalog_ids: BTreeSet<String> = catalog.keys().cloned().collect();
    let mut errors = Vec::new();

    for id in exported.difference(&catalog_ids) {
        errors.push(format!("exported API is missing from catalog: {id}"));
    }
    for id in catalog_ids.difference(&exported) {
        errors.push(format!("catalog contains a non-exported picker API: {id}"));
    }
    for id in coverage_ids.difference(&page.api) {
        errors.push(format!("showcase claim has no page usage: {id}"));
    }
    for id in coverage_ids.intersection(&allowlist.keys().cloned().collect()) {
        errors.push(format!("API is both showcased and allowlisted: {id}"));
    }
    for (id, reason) in &allowlist {
        if reason.trim().is_empty() {
            errors.push(format!("allowlist entry has no reason: {id}"));
        }
        if !exported.contains(id) && !option_list_exported.contains(id) {
            errors.push(format!("allowlist entry is not a public API: {id}"));
        }
    }
    for id in exported
        .difference(&coverage_ids)
        .filter(|id| !allowlist.contains_key(*id))
    {
        errors.push(format!(
            "public API lacks showcase coverage or a reason: {id}"
        ));
    }
    for id in page.api.difference(&exported) {
        if id.starts_with("widgets::picker::") {
            errors.push(format!("page usage is not an exported API: {id}"));
        }
    }
    if !page.configured_toggle {
        errors
            .push("picker page does not demonstrate configured Desktop/Vim toggle profiles".into());
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn repository_sources() -> (String, String, String, String) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    (
        std::fs::read_to_string(root.join("src/widgets/picker.rs")).expect("picker source reads"),
        std::fs::read_to_string(root.join("src/catalog.rs")).expect("catalog source reads"),
        std::fs::read_to_string(root.join("src/bin/showcase/pages/pickers.rs"))
            .expect("picker showcase page reads"),
        std::fs::read_to_string(root.join("src/widgets/option_list.rs"))
            .expect("option list source reads"),
    )
}

#[test]
fn picker_repository_guard_matches_actual_catalog_and_showcase_usage() {
    let (picker, catalog, page, option_list) = repository_sources();
    let result = validate_picker_sources(&picker, &catalog, &page, &option_list);
    assert!(result.is_ok(), "{}", result.unwrap_err().join("\n"));
    assert!(coverage::PICKER_ALLOWLIST.iter().any(|(id, reason)| {
        *id == "widgets::picker::Picker::reset_selection" && !reason.trim().is_empty()
    }));
}

#[test]
fn picker_repository_guard_rejects_a_removed_configured_toggle_demonstration() {
    let (picker, catalog, page, option_list) = repository_sources();
    let old = ".key_profile(match o.value(\"pickers.nav\") {\n                \"vim\" => PickerKeyProfile::Vim,\n                _ => PickerKeyProfile::Desktop,\n            })";
    let broken_page = page.replace(old, ".key_profile(PickerKeyProfile::Desktop)");
    assert_ne!(
        broken_page, page,
        "configured profile demo mutation matched source"
    );
    let result = validate_picker_sources(&picker, &catalog, &broken_page, &option_list);
    assert!(
        result.as_ref().is_err_and(|errors| errors
            .iter()
            .any(|error| error.contains("configured Desktop/Vim"))),
        "removed configuration demo unexpectedly passed: {result:?}"
    );
}

#[test]
fn picker_repository_guard_rejects_a_removed_exported_catalog_entry() {
    let (picker, catalog, page, option_list) = repository_sources();
    let entry = "    Capability::function(\"widgets::picker::Picker::query\"),\n";
    let broken_catalog = catalog.replace(entry, "");
    assert_ne!(broken_catalog, catalog, "catalog mutation matched source");
    let result = validate_picker_sources(&picker, &broken_catalog, &page, &option_list);
    assert!(
        result.as_ref().is_err_and(|errors| errors
            .iter()
            .any(|error| error.contains("missing from catalog: widgets::picker::Picker::query"))),
        "removed catalog entry unexpectedly passed: {result:?}"
    );
}

#[test]
fn typed_picker_bindings_respect_lexical_scope_and_shadowing() {
    let (picker, catalog, _, _) = repository_sources();
    let exported = picker_api(&picker);
    let catalog = picker_catalog(&catalog);
    let has_list = |source: &str| {
        PageUsage::from_source(source, &exported, &catalog)
            .api
            .contains("widgets::picker::Picker::list")
    };
    assert!(has_list(
        "fn show(&self) { let p: Picker = self.style(); p.list(); }"
    ));
    assert!(!has_list(
        "fn show(&self) { let p: Other = self.style(); p.list(); }"
    ));
    assert!(!has_list(
        "fn show(&self) { let p: Picker::Other = self.style(); p.list(); }"
    ));
    assert!(has_list(
        "fn show(&self) { let p: crate::Picker = self.style(); p.list(); }"
    ));
    assert!(!has_list(
        "fn first(p: Picker) {} fn second(p: Other) { p.list(); }"
    ));
    assert!(!has_list(
        "fn show(p: Picker) { { let p: Other = other(); p.list(); } }"
    ));
    assert!(has_list(
        "fn show(p: Picker) { { let p: Other = other(); p.list(); } p.list(); }"
    ));
}

#[test]
fn typed_picker_closures_respect_shadowing_and_restore_outer_bindings() {
    let (picker, catalog, _, _) = repository_sources();
    let exported = picker_api(&picker);
    let catalog = picker_catalog(&catalog);
    let has_items = |source: &str| {
        PageUsage::from_source(source, &exported, &catalog)
            .api
            .contains("widgets::picker::Picker::visible_items")
    };
    assert!(has_items(
        "fn show() { value.map(|p: &Picker| p.visible_items()); }"
    ));
    assert!(!has_items(
        "fn show(p: Picker) { value.map(|p: &Other| p.visible_items()); }"
    ));
    assert!(!has_items(
        "fn show() { value.map(|p: &Other| p.visible_items()); } fn other(p: &Other) { p.visible_items(); }"
    ));
    assert!(has_items(
        "fn show(p: Picker) { value.map(|p: &Other| p.visible_items()); p.visible_items(); }"
    ));
}

fn page_uses_picker_list(source: &str) -> bool {
    let (picker, catalog, _, _) = repository_sources();
    PageUsage::from_source(source, &picker_api(&picker), &picker_catalog(&catalog))
        .api
        .contains("widgets::picker::Picker::list")
}

#[test]
fn picker_scope_respects_loop_pattern_shadow() {
    assert!(!page_uses_picker_list(
        "fn show(p: Picker) { for p in items { p.list(); } }"
    ));
    assert!(page_uses_picker_list(
        "fn show(p: Picker) { for p in items { p.list(); } p.list(); }"
    ));
}

#[test]
fn picker_scope_respects_match_pattern_shadow() {
    assert!(!page_uses_picker_list(
        "fn show(p: Picker) { match item { Some(p) => p.list(), None => () } }"
    ));
    assert!(page_uses_picker_list(
        "fn show(p: Picker) { match item { Some(p) => p.list(), None => () } p.list(); }"
    ));
}

#[test]
fn picker_scope_respects_if_let_pattern_shadow() {
    assert!(!page_uses_picker_list(
        "fn show(p: Picker) { if let Some(p) = value { p.list(); } }"
    ));
    assert!(!page_uses_picker_list(
        "fn show(p: Picker) { if let Some(p) = value && p.list() { } }"
    ));
    assert!(page_uses_picker_list(
        "fn show(p: Picker) { if let Some(p) = value { p.list(); } else { p.list(); } }"
    ));
    assert!(page_uses_picker_list(
        "fn show(p: Picker) { if let Some(p) = value { p.list(); } p.list(); }"
    ));
}

#[test]
fn picker_scope_respects_while_let_pattern_shadow() {
    assert!(!page_uses_picker_list(
        "fn show(p: Picker) { while let Some(p) = value { p.list(); } }"
    ));
    assert!(page_uses_picker_list(
        "fn show(p: Picker) { while let Some(p) = value { p.list(); } p.list(); }"
    ));
}

#[test]
fn picker_scope_respects_let_else_pattern_shadow() {
    assert!(page_uses_picker_list(
        "fn show(p: Picker) { let Some(p) = value else { p.list(); return; }; }"
    ));
    assert!(!page_uses_picker_list(
        "fn show(p: Picker) { let Some(p) = value else { return; }; p.list(); }"
    ));
}

#[test]
fn terminal_session_coverage_declares_catalog_capabilities() {
    let mut seen = BTreeSet::new();
    for id in coverage::TERMINAL_SESSION_COVERAGE {
        assert!(
            junie_tui::catalog::find(id).is_some(),
            "declared id is not a catalog capability: {id}"
        );
        assert!(seen.insert(*id), "duplicate coverage entry: {id}");
    }
}

fn picker_with_items(id: &str, count: usize) -> Picker {
    let mut picker = Picker::new(WidgetId::of(id), "Rows");
    picker
        .set_items(
            (0..count)
                .map(|index| ListItem::new(format!("Item {index}")).key(ItemKey::Index(index)))
                .collect(),
        )
        .expect("valid picker update");
    picker
}

fn draw(picker: &mut Picker, area: Rect) -> (Buffer, HitRegistry) {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buffer = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    picker.draw(area, &mut buffer, &mut ctx);
    ctx.flush_overlays(&mut buffer);
    drop(ctx);
    (buffer, hits)
}

fn popup_rows(buffer: &Buffer, popup: Rect) -> Vec<String> {
    (popup.y..popup.bottom())
        .map(|y| {
            (popup.x..popup.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect()
        })
        .collect()
}

fn assert_hits_inside_popup(picker: &Picker, hits: &HitRegistry, context: &str) {
    let popup = picker.area;
    assert!(!popup.is_empty(), "{context}: popup was not drawn");
    let backdrop = backdrop_id(picker.id);
    let mut inside = 0;
    for region in hits.regions() {
        if region.id == backdrop || region.area.is_empty() {
            continue;
        }
        let a = region.area;
        assert!(
            a.x >= popup.x
                && a.y >= popup.y
                && a.right() <= popup.right()
                && a.bottom() <= popup.bottom(),
            "{context}: region {a:?} escapes popup {popup:?}"
        );
        inside += 1;
    }
    assert!(inside > 0, "{context}: popup registered no hit regions");
}

#[test]
fn picker_one_row_provider_status_transition() {
    let area = Rect::new(7, 3, 60, 14);
    for searchable in [false, true] {
        let mut picker = picker_with_items("one-row-transition", 5).max_rows(1);
        picker
            .set_searchable(searchable)
            .expect("indexed items are searchable");
        let transition = [
            (PickerStatus::Ready, "Item 0"),
            (PickerStatus::Loading("Fetching".into()), "Fetching"),
            (
                PickerStatus::Error {
                    message: "Offline".into(),
                    detail: Some("Retry later".into()),
                },
                "Offline",
            ),
            (PickerStatus::Ready, "Item 0"),
        ];
        let mut heights = Vec::new();
        for (step, (status, label)) in transition.into_iter().enumerate() {
            let context = format!("searchable={searchable} step={step} label={label}");
            picker.status = status;
            let (buffer, hits) = draw(&mut picker, area);
            let rows = popup_rows(&buffer, picker.area);
            assert_eq!(
                rows.iter().filter(|row| row.contains(label)).count(),
                1,
                "{context}: expected exactly one row with the label in {rows:?}"
            );
            assert!(
                !rows.iter().any(|row| row.contains("Retry later")),
                "{context}: the status detail needs a second row and must be omitted: {rows:?}"
            );
            if step % 3 != 0 {
                assert!(
                    !rows.iter().any(|row| row.contains("Item ")),
                    "{context}: a non-ready status must replace the items: {rows:?}"
                );
            }
            assert_hits_inside_popup(&picker, &hits, &context);
            assert!(
                picker.area.right() <= area.right() && picker.area.bottom() <= area.bottom(),
                "{context}: popup {:?} leaves the buffer {area:?}",
                picker.area
            );
            heights.push(picker.height());
        }
        assert_eq!(
            heights.first(),
            heights.last(),
            "returning to Ready restores the Ready height"
        );
    }
}

#[test]
fn picker_large_count_uses_bounded_height() {
    let statuses = || {
        [
            PickerStatus::Ready,
            PickerStatus::Loading("Fetching".into()),
            PickerStatus::Error {
                message: "Offline".into(),
                detail: Some("Retry later".into()),
            },
        ]
    };
    let mut large = picker_with_items("large-count", 65_536).max_rows(8);
    let mut eight = picker_with_items("eight-count", 8).max_rows(8);
    for status in statuses() {
        large.status = status.clone();
        eight.status = status.clone();
        assert_eq!(
            large.height(),
            eight.height(),
            "{status:?}: 65,536 items must size like eight"
        );
    }
    large.status = PickerStatus::Ready;
    eight.status = PickerStatus::Ready;
    let area = Rect::new(2, 1, 60, 24);
    let (buffer, hits) = draw(&mut large, area);
    let (_, eight_hits) = draw(&mut eight, area);
    let rows = popup_rows(&buffer, large.area);
    assert!(rows.iter().any(|row| row.contains("Item 0")), "{rows:?}");
    assert!(
        !rows.iter().any(|row| row.contains("Item 20")),
        "a 65,536-item picker draws a bounded window: {rows:?}"
    );
    assert_eq!(
        large.area, eight.area,
        "popup geometry matches the eight-item picker"
    );
    assert_hits_inside_popup(&large, &hits, "large count");
    assert_hits_inside_popup(&eight, &eight_hits, "eight count");
}
