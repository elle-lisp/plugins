//! Elle syn plugin — Rust syntax parsing via the `syn` crate.

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_ERROR};
use quote::ToTokens;

elle_plugin::define_plugin!("syn/", &PRIMITIVES);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn list(items: Vec<ElleValue>) -> ElleValue {
    api().array(&items)
}

// ---------------------------------------------------------------------------
// Parsing primitives
// ---------------------------------------------------------------------------

extern "C" fn prim_syn_parse_file(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let src = match a.get_string(v) {
        Some(s) => s.to_string(),
        None => return a.err("type-error", &format!("syn/parse-file: expected string, got {}", a.type_name(v))),
    };
    match syn::parse_file(&src) {
        Ok(file) => a.ok(a.external("syn-file", file)),
        Err(e) => a.err("parse-error", &format!("syn/parse-file: {}", e)),
    }
}

extern "C" fn prim_syn_parse_expr(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let src = match a.get_string(v) {
        Some(s) => s.to_string(),
        None => return a.err("type-error", &format!("syn/parse-expr: expected string, got {}", a.type_name(v))),
    };
    match syn::parse_str::<syn::Expr>(&src) {
        Ok(expr) => a.ok(a.external("syn-expr", expr)),
        Err(e) => a.err("parse-error", &format!("syn/parse-expr: {}", e)),
    }
}

extern "C" fn prim_syn_parse_type(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let src = match a.get_string(v) {
        Some(s) => s.to_string(),
        None => return a.err("type-error", &format!("syn/parse-type: expected string, got {}", a.type_name(v))),
    };
    match syn::parse_str::<syn::Type>(&src) {
        Ok(ty) => a.ok(a.external("syn-type", ty)),
        Err(e) => a.err("parse-error", &format!("syn/parse-type: {}", e)),
    }
}

extern "C" fn prim_syn_parse_item(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let src = match a.get_string(v) {
        Some(s) => s.to_string(),
        None => return a.err("type-error", &format!("syn/parse-item: expected string, got {}", a.type_name(v))),
    };
    match syn::parse_str::<syn::Item>(&src) {
        Ok(item) => a.ok(a.external("syn-item", item)),
        Err(e) => a.err("parse-error", &format!("syn/parse-item: {}", e)),
    }
}

// ---------------------------------------------------------------------------
// Navigation primitives
// ---------------------------------------------------------------------------

extern "C" fn prim_syn_items(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let file = match a.get_external::<syn::File>(v, "syn-file") {
        Some(f) => f,
        None => return a.err("type-error", &format!("syn/items: expected syn-file, got {}", a.type_name(v))),
    };
    let items: Vec<ElleValue> = file.items.iter()
        .map(|item| a.external("syn-item", item.clone()))
        .collect();
    a.ok(list(items))
}

extern "C" fn prim_syn_item_kind(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let item = match a.get_external::<syn::Item>(v, "syn-item") {
        Some(i) => i,
        None => return a.err("type-error", &format!("syn/item-kind: expected syn-item, got {}", a.type_name(v))),
    };
    a.ok(a.keyword(item_kind_str(item)))
}

extern "C" fn prim_syn_item_name(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let item = match a.get_external::<syn::Item>(v, "syn-item") {
        Some(i) => i,
        None => return a.err("type-error", &format!("syn/item-name: expected syn-item, got {}", a.type_name(v))),
    };
    let name: Option<String> = match item {
        syn::Item::Fn(f) => Some(f.sig.ident.to_string()),
        syn::Item::Struct(s) => Some(s.ident.to_string()),
        syn::Item::Enum(e) => Some(e.ident.to_string()),
        syn::Item::Trait(t) => Some(t.ident.to_string()),
        syn::Item::Mod(m) => Some(m.ident.to_string()),
        syn::Item::Const(c) => Some(c.ident.to_string()),
        syn::Item::Static(s) => Some(s.ident.to_string()),
        syn::Item::Type(t) => Some(t.ident.to_string()),
        syn::Item::Macro(m) => m.ident.as_ref().map(|i| i.to_string()),
        _ => None,
    };
    match name {
        Some(s) => a.ok(a.string(&s)),
        None => a.ok(a.nil()),
    }
}

// ---------------------------------------------------------------------------
// Introspection helpers
// ---------------------------------------------------------------------------

fn item_start_line(item: &syn::Item) -> Option<usize> {
    use syn::spanned::Spanned;
    let span = item.span();
    let start = span.start();
    if start.line == 0 { None } else { Some(start.line) }
}

fn item_kind_str(item: &syn::Item) -> &'static str {
    match item {
        syn::Item::Fn(_) => "fn",
        syn::Item::Struct(_) => "struct",
        syn::Item::Enum(_) => "enum",
        syn::Item::Trait(_) => "trait",
        syn::Item::Impl(_) => "impl",
        syn::Item::Use(_) => "use",
        syn::Item::Mod(_) => "mod",
        syn::Item::Const(_) => "const",
        syn::Item::Static(_) => "static",
        syn::Item::Type(_) => "type",
        syn::Item::Macro(_) => "macro",
        _ => "other",
    }
}

fn fn_args_to_elle(sig: &syn::Signature) -> ElleValue {
    let a = api();
    let args: Vec<ElleValue> = sig.inputs.iter().map(|arg| {
        match arg {
            syn::FnArg::Receiver(r) => {
                let ty_str = if r.reference.is_some() {
                    if r.mutability.is_some() { "&mut self" } else { "&self" }
                } else { "self" };
                a.build_struct(&[("name", a.string("self")), ("type", a.string(ty_str))])
            }
            syn::FnArg::Typed(pt) => {
                let name_str = match pt.pat.as_ref() {
                    syn::Pat::Ident(pi) => pi.ident.to_string(),
                    _ => pt.pat.to_token_stream().to_string(),
                };
                let type_str = pt.ty.to_token_stream().to_string();
                a.build_struct(&[("name", a.string(&name_str)), ("type", a.string(&type_str))])
            }
        }
    }).collect();
    list(args)
}

fn fields_to_elle(fields: &syn::Fields) -> (ElleValue, ElleValue) {
    let a = api();
    match fields {
        syn::Fields::Named(named) => {
            let fs: Vec<ElleValue> = named.named.iter().map(|f| {
                let name_val = match &f.ident {
                    Some(i) => a.string(&i.to_string()),
                    None => a.nil(),
                };
                let type_str = f.ty.to_token_stream().to_string();
                a.build_struct(&[("name", name_val), ("type", a.string(&type_str))])
            }).collect();
            (a.keyword("named"), list(fs))
        }
        syn::Fields::Unnamed(unnamed) => {
            let fs: Vec<ElleValue> = unnamed.unnamed.iter().map(|f| {
                let type_str = f.ty.to_token_stream().to_string();
                a.build_struct(&[("name", a.nil()), ("type", a.string(&type_str))])
            }).collect();
            (a.keyword("tuple"), list(fs))
        }
        syn::Fields::Unit => (a.keyword("unit"), list(vec![])),
    }
}

// ---------------------------------------------------------------------------
// Introspection primitives
// ---------------------------------------------------------------------------

extern "C" fn prim_syn_fn_info(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let item = match a.get_external::<syn::Item>(v, "syn-item") {
        Some(i) => i,
        None => return a.err("type-error", &format!("syn/fn-info: expected syn-item, got {}", a.type_name(v))),
    };
    let func = match item {
        syn::Item::Fn(f) => f,
        _ => return a.err("type-error", &format!("syn/fn-info: expected fn item, got :{}", item_kind_str(item))),
    };
    let sig = &func.sig;
    let return_type_val = match &sig.output {
        syn::ReturnType::Default => a.nil(),
        syn::ReturnType::Type(_, ty) => a.string(&ty.to_token_stream().to_string()),
    };
    let mut fields: Vec<(&str, ElleValue)> = vec![
        ("name", a.string(&sig.ident.to_string())),
        ("args", fn_args_to_elle(sig)),
        ("return-type", return_type_val),
        ("async?", a.boolean(sig.asyncness.is_some())),
        ("unsafe?", a.boolean(sig.unsafety.is_some())),
        ("const?", a.boolean(sig.constness.is_some())),
    ];
    if let Some(line) = item_start_line(item) {
        fields.push(("line", a.int(line as i64)));
    }
    a.ok(a.build_struct(&fields))
}

extern "C" fn prim_syn_fn_args(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let item = match a.get_external::<syn::Item>(v, "syn-item") {
        Some(i) => i,
        None => return a.err("type-error", &format!("syn/fn-args: expected syn-item, got {}", a.type_name(v))),
    };
    let func = match item {
        syn::Item::Fn(f) => f,
        _ => return a.err("type-error", &format!("syn/fn-args: expected fn item, got :{}", item_kind_str(item))),
    };
    a.ok(fn_args_to_elle(&func.sig))
}

extern "C" fn prim_syn_fn_return_type(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let item = match a.get_external::<syn::Item>(v, "syn-item") {
        Some(i) => i,
        None => return a.err("type-error", &format!("syn/fn-return-type: expected syn-item, got {}", a.type_name(v))),
    };
    let func = match item {
        syn::Item::Fn(f) => f,
        _ => return a.err("type-error", &format!("syn/fn-return-type: expected fn item, got :{}", item_kind_str(item))),
    };
    match &func.sig.output {
        syn::ReturnType::Default => a.ok(a.nil()),
        syn::ReturnType::Type(_, ty) => a.ok(a.string(&ty.to_token_stream().to_string())),
    }
}

extern "C" fn prim_syn_struct_fields(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let item = match a.get_external::<syn::Item>(v, "syn-item") {
        Some(i) => i,
        None => return a.err("type-error", &format!("syn/struct-fields: expected syn-item, got {}", a.type_name(v))),
    };
    let st = match item {
        syn::Item::Struct(s) => s,
        _ => return a.err("type-error", &format!("syn/struct-fields: expected struct item, got :{}", item_kind_str(item))),
    };
    let (kind_kw, fields_list) = fields_to_elle(&st.fields);
    a.ok(a.build_struct(&[
        ("name", a.string(&st.ident.to_string())),
        ("kind", kind_kw),
        ("fields", fields_list),
    ]))
}

extern "C" fn prim_syn_enum_variants(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let item = match a.get_external::<syn::Item>(v, "syn-item") {
        Some(i) => i,
        None => return a.err("type-error", &format!("syn/enum-variants: expected syn-item, got {}", a.type_name(v))),
    };
    let en = match item {
        syn::Item::Enum(e) => e,
        _ => return a.err("type-error", &format!("syn/enum-variants: expected enum item, got :{}", item_kind_str(item))),
    };
    let variants: Vec<ElleValue> = en.variants.iter().map(|va| {
        let (kind_kw, fields_list) = fields_to_elle(&va.fields);
        let mut fields: Vec<(&str, ElleValue)> = vec![
            ("name", a.string(&va.ident.to_string())),
            ("kind", kind_kw),
            ("fields", fields_list),
        ];
        if let Some((_, disc_expr)) = &va.discriminant {
            let disc_str = disc_expr.to_token_stream().to_string();
            fields.push(("discriminant", a.string(&disc_str)));
        }
        a.build_struct(&fields)
    }).collect();
    a.ok(a.build_struct(&[
        ("name", a.string(&en.ident.to_string())),
        ("variants", list(variants)),
    ]))
}

extern "C" fn prim_syn_attributes(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let item = match a.get_external::<syn::Item>(v, "syn-item") {
        Some(i) => i,
        None => return a.err("type-error", &format!("syn/attributes: expected syn-item, got {}", a.type_name(v))),
    };
    let attrs: &[syn::Attribute] = match item {
        syn::Item::Fn(f) => &f.attrs,
        syn::Item::Struct(s) => &s.attrs,
        syn::Item::Enum(e) => &e.attrs,
        syn::Item::Trait(t) => &t.attrs,
        syn::Item::Impl(i) => &i.attrs,
        syn::Item::Use(u) => &u.attrs,
        syn::Item::Mod(m) => &m.attrs,
        syn::Item::Const(c) => &c.attrs,
        syn::Item::Static(s) => &s.attrs,
        syn::Item::Type(t) => &t.attrs,
        syn::Item::Macro(m) => &m.attrs,
        _ => return a.ok(list(vec![])),
    };
    let attr_strs: Vec<ElleValue> = attrs.iter()
        .map(|at| a.string(&at.to_token_stream().to_string()))
        .collect();
    a.ok(list(attr_strs))
}

extern "C" fn prim_syn_visibility(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let item = match a.get_external::<syn::Item>(v, "syn-item") {
        Some(i) => i,
        None => return a.err("type-error", &format!("syn/visibility: expected syn-item, got {}", a.type_name(v))),
    };
    let vis: Option<&syn::Visibility> = match item {
        syn::Item::Fn(f) => Some(&f.vis),
        syn::Item::Struct(s) => Some(&s.vis),
        syn::Item::Enum(e) => Some(&e.vis),
        syn::Item::Trait(t) => Some(&t.vis),
        syn::Item::Const(c) => Some(&c.vis),
        syn::Item::Static(s) => Some(&s.vis),
        syn::Item::Type(t) => Some(&t.vis),
        syn::Item::Mod(m) => Some(&m.vis),
        _ => None,
    };
    let kw = match vis {
        None => "private",
        Some(syn::Visibility::Public(_)) => "public",
        Some(syn::Visibility::Restricted(r)) => {
            let path_str = r.path.to_token_stream().to_string();
            if path_str == "crate" { "pub-crate" }
            else if path_str == "super" { "pub-super" }
            else { "pub-in" }
        }
        Some(syn::Visibility::Inherited) => "private",
    };
    a.ok(a.keyword(kw))
}

// ---------------------------------------------------------------------------
// Call site extraction
// ---------------------------------------------------------------------------

fn collect_calls(expr: &syn::Expr, calls: &mut Vec<String>) {
    match expr {
        syn::Expr::Call(call) => {
            if let syn::Expr::Path(ep) = &*call.func {
                let name = path_to_string(&ep.path);
                if !name.is_empty() { calls.push(name); }
            }
            collect_calls(&call.func, calls);
            for arg in &call.args { collect_calls(arg, calls); }
        }
        syn::Expr::MethodCall(mc) => {
            calls.push(mc.method.to_string());
            collect_calls(&mc.receiver, calls);
            for arg in &mc.args { collect_calls(arg, calls); }
        }
        syn::Expr::Block(b) => { for stmt in &b.block.stmts { collect_calls_stmt(stmt, calls); } }
        syn::Expr::If(ei) => {
            collect_calls(&ei.cond, calls);
            for stmt in &ei.then_branch.stmts { collect_calls_stmt(stmt, calls); }
            if let Some((_, else_branch)) = &ei.else_branch { collect_calls(else_branch, calls); }
        }
        syn::Expr::Match(m) => {
            collect_calls(&m.expr, calls);
            for arm in &m.arms {
                collect_calls(&arm.body, calls);
                if let Some(guard) = &arm.guard { collect_calls(&guard.1, calls); }
            }
        }
        syn::Expr::Let(l) => { collect_calls(&l.expr, calls); }
        syn::Expr::Binary(b) => { collect_calls(&b.left, calls); collect_calls(&b.right, calls); }
        syn::Expr::Unary(u) => { collect_calls(&u.expr, calls); }
        syn::Expr::Reference(r) => { collect_calls(&r.expr, calls); }
        syn::Expr::Return(r) => { if let Some(expr) = &r.expr { collect_calls(expr, calls); } }
        syn::Expr::Paren(p) => { collect_calls(&p.expr, calls); }
        syn::Expr::Field(f) => { collect_calls(&f.base, calls); }
        syn::Expr::Index(i) => { collect_calls(&i.expr, calls); collect_calls(&i.index, calls); }
        syn::Expr::Tuple(t) => { for elem in &t.elems { collect_calls(elem, calls); } }
        syn::Expr::Array(ar) => { for elem in &ar.elems { collect_calls(elem, calls); } }
        syn::Expr::Struct(s) => { for field in &s.fields { collect_calls(&field.expr, calls); } }
        syn::Expr::Closure(c) => { collect_calls(&c.body, calls); }
        syn::Expr::Assign(as_) => { collect_calls(&as_.left, calls); collect_calls(&as_.right, calls); }
        syn::Expr::Range(r) => {
            if let Some(start) = &r.start { collect_calls(start, calls); }
            if let Some(end) = &r.end { collect_calls(end, calls); }
        }
        syn::Expr::Try(t) => { collect_calls(&t.expr, calls); }
        syn::Expr::Await(aw) => { collect_calls(&aw.base, calls); }
        syn::Expr::Cast(c) => { collect_calls(&c.expr, calls); }
        syn::Expr::ForLoop(f) => {
            collect_calls(&f.expr, calls);
            for stmt in &f.body.stmts { collect_calls_stmt(stmt, calls); }
        }
        syn::Expr::While(w) => {
            collect_calls(&w.cond, calls);
            for stmt in &w.body.stmts { collect_calls_stmt(stmt, calls); }
        }
        syn::Expr::Loop(l) => { for stmt in &l.body.stmts { collect_calls_stmt(stmt, calls); } }
        syn::Expr::Unsafe(u) => { for stmt in &u.block.stmts { collect_calls_stmt(stmt, calls); } }
        _ => {}
    }
}

fn collect_calls_stmt(stmt: &syn::Stmt, calls: &mut Vec<String>) {
    match stmt {
        syn::Stmt::Expr(expr, _) => collect_calls(expr, calls),
        syn::Stmt::Local(local) => {
            if let Some(init) = &local.init {
                collect_calls(&init.expr, calls);
                if let Some((_, diverge)) = &init.diverge { collect_calls(diverge, calls); }
            }
        }
        syn::Stmt::Item(_) => {}
        syn::Stmt::Macro(m) => {
            let name = path_to_string(&m.mac.path);
            if !name.is_empty() { calls.push(name); }
        }
    }
}

fn path_to_string(path: &syn::Path) -> String {
    path.segments.iter().map(|seg| seg.ident.to_string()).collect::<Vec<_>>().join("::")
}

extern "C" fn prim_syn_fn_calls(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let item = match a.get_external::<syn::Item>(v, "syn-item") {
        Some(i) => i,
        None => return a.err("type-error", &format!("syn/fn-calls: expected syn-item, got {}", a.type_name(v))),
    };
    let block = match item {
        syn::Item::Fn(f) => &f.block,
        _ => return a.err("type-error", "syn/fn-calls: item must be a function"),
    };
    let mut calls = Vec::new();
    for stmt in &block.stmts { collect_calls_stmt(stmt, &mut calls); }
    let mut seen = std::collections::HashSet::new();
    let unique: Vec<ElleValue> = calls.into_iter()
        .filter(|c| seen.insert(c.clone()))
        .map(|c| a.string(&c))
        .collect();
    a.ok(a.array(&unique))
}

extern "C" fn prim_syn_static_strings(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let item = match a.get_external::<syn::Item>(v, "syn-item") {
        Some(i) => i,
        None => return a.err("type-error", &format!("syn/static-strings: expected syn-item, got {}", a.type_name(v))),
    };
    let expr = match item {
        syn::Item::Static(s) => &*s.expr,
        syn::Item::Const(c) => &*c.expr,
        _ => return a.err("type-error", "syn/static-strings: item must be static or const"),
    };
    let mut strings = Vec::new();
    collect_string_lits(expr, &mut strings);
    let values: Vec<ElleValue> = strings.iter().map(|s| a.string(s)).collect();
    a.ok(a.array(&values))
}

fn collect_string_lits(expr: &syn::Expr, strings: &mut Vec<String>) {
    match expr {
        syn::Expr::Lit(lit) => {
            if let syn::Lit::Str(s) = &lit.lit { strings.push(s.value()); }
        }
        syn::Expr::Array(ar) => { for elem in &ar.elems { collect_string_lits(elem, strings); } }
        syn::Expr::Reference(r) => { collect_string_lits(&r.expr, strings); }
        syn::Expr::Struct(s) => { for field in &s.fields { collect_string_lits(&field.expr, strings); } }
        syn::Expr::Block(b) => {
            for stmt in &b.block.stmts {
                if let syn::Stmt::Expr(e, _) = stmt { collect_string_lits(e, strings); }
            }
        }
        _ => {}
    }
}

extern "C" fn prim_syn_primitive_defs(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let item = match a.get_external::<syn::Item>(v, "syn-item") {
        Some(i) => i,
        None => return a.err("type-error", &format!("syn/primitive-defs: expected syn-item, got {}", a.type_name(v))),
    };
    let expr = match item {
        syn::Item::Const(c) => &*c.expr,
        syn::Item::Static(s) => &*s.expr,
        _ => return a.err("type-error", "syn/primitive-defs: item must be static or const"),
    };
    let mut results = Vec::new();
    collect_primitive_defs(expr, &mut results);
    a.ok(a.array(&results))
}

fn collect_primitive_defs(expr: &syn::Expr, results: &mut Vec<ElleValue>) {
    let a = api();
    match expr {
        syn::Expr::Struct(s) => {
            let mut name_val: Option<String> = None;
            let mut func_val: Option<String> = None;
            for field in &s.fields {
                if let syn::Member::Named(ident) = &field.member {
                    let field_name = ident.to_string();
                    if field_name == "name" {
                        if let syn::Expr::Lit(lit) = &field.expr {
                            if let syn::Lit::Str(s) = &lit.lit { name_val = Some(s.value()); }
                        }
                    } else if field_name == "func" {
                        func_val = Some(field.expr.to_token_stream().to_string());
                    }
                }
            }
            if let (Some(name), Some(func)) = (name_val, func_val) {
                results.push(a.build_struct(&[
                    ("name", a.string(&name)),
                    ("func", a.string(&func)),
                ]));
            }
        }
        syn::Expr::Array(ar) => { for elem in &ar.elems { collect_primitive_defs(elem, results); } }
        syn::Expr::Reference(r) => { collect_primitive_defs(&r.expr, results); }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Serialization primitives
// ---------------------------------------------------------------------------

extern "C" fn prim_syn_to_string(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    if let Some(file) = a.get_external::<syn::File>(v, "syn-file") {
        return a.ok(a.string(&file.to_token_stream().to_string()));
    }
    if let Some(item) = a.get_external::<syn::Item>(v, "syn-item") {
        return a.ok(a.string(&item.to_token_stream().to_string()));
    }
    if let Some(expr) = a.get_external::<syn::Expr>(v, "syn-expr") {
        return a.ok(a.string(&expr.to_token_stream().to_string()));
    }
    if let Some(ty) = a.get_external::<syn::Type>(v, "syn-type") {
        return a.ok(a.string(&ty.to_token_stream().to_string()));
    }
    a.err("type-error", &format!("syn/to-string: expected syn-file, syn-item, syn-expr, or syn-type, got {}", a.type_name(v)))
}

extern "C" fn prim_syn_to_pretty_string(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    if let Some(file) = a.get_external::<syn::File>(v, "syn-file") {
        let s = prettyplease::unparse(file);
        return a.ok(a.string(s.trim_end()));
    }
    if let Some(item) = a.get_external::<syn::Item>(v, "syn-item") {
        let file = syn::File { shebang: None, attrs: vec![], items: vec![item.clone()] };
        let s = prettyplease::unparse(&file);
        return a.ok(a.string(s.trim_end()));
    }
    a.err("type-error", &format!("syn/to-pretty-string: expected syn-file or syn-item, got {}", a.type_name(v)))
}

extern "C" fn prim_syn_item_line(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let item = match a.get_external::<syn::Item>(v, "syn-item") {
        Some(i) => i,
        None => return a.err("type-error", &format!("syn/item-line: expected syn-item, got {}", a.type_name(v))),
    };
    match item_start_line(item) {
        Some(line) => a.ok(a.int(line as i64)),
        None => a.ok(a.nil()),
    }
}

// ---------------------------------------------------------------------------
// Registration table
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::exact("syn/parse-file", prim_syn_parse_file, SIG_ERROR, 1,
        "Parse a Rust source string into an opaque File node", "syn",
        r#"(syn/parse-file "fn foo() {}")"#),
    EllePrimDef::exact("syn/parse-expr", prim_syn_parse_expr, SIG_ERROR, 1,
        "Parse a Rust expression string into an opaque Expr node", "syn",
        r#"(syn/parse-expr "1 + 2")"#),
    EllePrimDef::exact("syn/parse-type", prim_syn_parse_type, SIG_ERROR, 1,
        "Parse a Rust type string into an opaque Type node", "syn",
        r#"(syn/parse-type "Vec<String>")"#),
    EllePrimDef::exact("syn/parse-item", prim_syn_parse_item, SIG_ERROR, 1,
        "Parse a Rust item string (fn, struct, enum, etc.) into an opaque Item node", "syn",
        r#"(syn/parse-item "fn foo() {}")"#),
    EllePrimDef::exact("syn/items", prim_syn_items, SIG_ERROR, 1,
        "Extract top-level items from a parsed file as a list of Item nodes", "syn",
        r#"(syn/items (syn/parse-file "fn foo() {} fn bar() {}"))"#),
    EllePrimDef::exact("syn/item-kind", prim_syn_item_kind, SIG_ERROR, 1,
        "Return the kind of an item as a keyword (:fn :struct :enum :trait :impl :use :mod :const :static :type :macro :other)", "syn",
        r#"(syn/item-kind (syn/parse-item "fn foo() {}"))"#),
    EllePrimDef::exact("syn/item-name", prim_syn_item_name, SIG_ERROR, 1,
        "Return the name (ident) of a named item as a string, or nil for unnamed items", "syn",
        r#"(syn/item-name (syn/parse-item "fn foo() {}"))"#),
    EllePrimDef::exact("syn/item-line", prim_syn_item_line, SIG_ERROR, 1,
        "Return the start line number of an item (1-indexed), or nil if unavailable", "syn",
        r#"(syn/item-line (syn/parse-item "fn foo() {}"))"#),
    EllePrimDef::exact("syn/fn-info", prim_syn_fn_info, SIG_ERROR, 1,
        "Return {:name :args :return-type :async? :unsafe? :const?} for a function item", "syn",
        r#"(syn/fn-info (syn/parse-item "pub fn add(x: i32) -> i32 { x }"))"#),
    EllePrimDef::exact("syn/fn-args", prim_syn_fn_args, SIG_ERROR, 1,
        "Return the argument list of a function item as ({:name string :type string} ...)", "syn",
        r#"(syn/fn-args (syn/parse-item "fn foo(x: i32, y: String) {}"))"#),
    EllePrimDef::exact("syn/fn-return-type", prim_syn_fn_return_type, SIG_ERROR, 1,
        "Return the return type of a function as a string, or nil if implicit ()", "syn",
        r#"(syn/fn-return-type (syn/parse-item "fn foo() -> i32 { 42 }"))"#),
    EllePrimDef::exact("syn/struct-fields", prim_syn_struct_fields, SIG_ERROR, 1,
        "Return {:name :kind :fields} for a struct item; :kind is :named :tuple or :unit", "syn",
        r#"(syn/struct-fields (syn/parse-item "struct Foo { x: i32 }"))"#),
    EllePrimDef::exact("syn/enum-variants", prim_syn_enum_variants, SIG_ERROR, 1,
        "Return {:name :variants} for an enum item; each variant has :name :kind :fields and optional :discriminant", "syn",
        r#"(syn/enum-variants (syn/parse-item "enum Color { Red, Green, Blue }"))"#),
    EllePrimDef::exact("syn/attributes", prim_syn_attributes, SIG_ERROR, 1,
        "Return the attributes on an item as a list of strings", "syn",
        r##"(syn/attributes (syn/parse-item "#[derive(Debug)] struct Foo {}"))"##),
    EllePrimDef::exact("syn/visibility", prim_syn_visibility, SIG_ERROR, 1,
        "Return the visibility of an item as a keyword (:public :pub-crate :pub-super :pub-in :private)", "syn",
        r#"(syn/visibility (syn/parse-item "pub fn foo() {}"))"#),
    EllePrimDef::exact("syn/to-string", prim_syn_to_string, SIG_ERROR, 1,
        "Convert any parsed syn node back to a compact token string", "syn",
        r#"(syn/to-string (syn/parse-item "fn foo(){}"))"#),
    EllePrimDef::exact("syn/to-pretty-string", prim_syn_to_pretty_string, SIG_ERROR, 1,
        "Pretty-print a parsed File or Item node using prettyplease", "syn",
        r#"(syn/to-pretty-string (syn/parse-item "fn foo(){}"))"#),
    EllePrimDef::exact("syn/fn-calls", prim_syn_fn_calls, SIG_ERROR, 1,
        "Extract deduplicated function/method call names from a function body", "syn",
        r#"(syn/fn-calls (syn/parse-item "fn foo() { bar(); baz::qux(); }"))"#),
    EllePrimDef::exact("syn/static-strings", prim_syn_static_strings, SIG_ERROR, 1,
        "Extract all string literals from a static/const item (e.g. PrimitiveDef arrays)", "syn",
        r#"(syn/static-strings (syn/parse-item "static X: &[&str] = &[\"a\", \"b\"];"))"#),
    EllePrimDef::exact("syn/primitive-defs", prim_syn_primitive_defs, SIG_ERROR, 1,
        "Extract name->func pairs from a PrimitiveDef const/static array", "syn",
        r#"(syn/primitive-defs (syn/parse-item "const PRIMITIVES: &[PrimitiveDef] = &[...]"))"#),
];
