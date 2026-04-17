//! Elle tree-sitter plugin — multi-language parsing and structural queries.
//!
//! Provides a query-first API for parsing and inspecting syntax trees.
//! Bundled grammars: C, Rust. (Elle grammar: future work.)

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_ERROR, SIG_OK};
use std::rc::Rc;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, Tree};

elle_plugin::define_plugin!("ts/", &PRIMITIVES);

// ---------------------------------------------------------------------------
// Internal data types (stored as external objects)
// ---------------------------------------------------------------------------

/// Parsed tree + its source text. Shared via Rc so nodes can reference it.
struct TsTreeData {
    tree: Tree,
    source: String,
}

/// A node identified by its path from the root (vector of child indices).
struct TsNodeData {
    tree_data: Rc<TsTreeData>,
    path: Vec<usize>,
}

/// A compiled tree-sitter query.
struct TsQueryData {
    query: Query,
}

impl TsNodeData {
    fn resolve(&self) -> Option<Node<'_>> {
        let mut node = self.tree_data.tree.root_node();
        for &idx in &self.path {
            node = node.child(idx)?;
        }
        Some(node)
    }

    fn from_node(node: Node<'_>, tree_data: Rc<TsTreeData>) -> Self {
        TsNodeData {
            tree_data,
            path: compute_path(node),
        }
    }
}

fn compute_path(node: Node<'_>) -> Vec<usize> {
    let mut path = Vec::new();
    let mut current = node;
    while let Some(parent) = current.parent() {
        let id = current.id();
        for i in 0..parent.child_count() {
            if let Some(child) = parent.child(i) {
                if child.id() == id {
                    path.push(i);
                    break;
                }
            }
        }
        current = parent;
    }
    path.reverse();
    path
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn get_string(args: *const ElleValue, nargs: usize, idx: usize, prim: &str) -> Result<String, ElleResult> {
    let a = api();
    let val = a.arg(args, nargs, idx);
    a.get_string(val)
        .map(|s| s.to_string())
        .ok_or_else(|| a.err("type-error", &format!("{}: expected string, got {}", prim, a.type_name(val))))
}

fn get_tree(args: *const ElleValue, nargs: usize, idx: usize, prim: &str) -> Result<Rc<TsTreeData>, ElleResult> {
    let a = api();
    let val = a.arg(args, nargs, idx);
    a.get_external::<Rc<TsTreeData>>(val, "ts/tree")
        .cloned()
        .ok_or_else(|| a.err("type-error", &format!("{}: expected ts/tree, got {}", prim, a.type_name(val))))
}

fn get_node<'a>(args: *const ElleValue, nargs: usize, idx: usize, prim: &str) -> Result<&'a TsNodeData, ElleResult> {
    let a = api();
    let val = a.arg(args, nargs, idx);
    a.get_external::<TsNodeData>(val, "ts/node")
        .ok_or_else(|| a.err("type-error", &format!("{}: expected ts/node, got {}", prim, a.type_name(val))))
}

fn get_query<'a>(args: *const ElleValue, nargs: usize, idx: usize, prim: &str) -> Result<&'a TsQueryData, ElleResult> {
    let a = api();
    let val = a.arg(args, nargs, idx);
    a.get_external::<TsQueryData>(val, "ts/query")
        .ok_or_else(|| a.err("type-error", &format!("{}: expected ts/query, got {}", prim, a.type_name(val))))
}

fn get_language(args: *const ElleValue, nargs: usize, idx: usize, prim: &str) -> Result<Language, ElleResult> {
    let a = api();
    let val = a.arg(args, nargs, idx);
    a.get_external::<Language>(val, "ts/language")
        .cloned()
        .ok_or_else(|| a.err("type-error", &format!("{}: expected ts/language, got {}", prim, a.type_name(val))))
}

fn node_to_value(node: Node<'_>, tree_data: Rc<TsTreeData>) -> ElleValue {
    let a = api();
    a.external("ts/node", TsNodeData::from_node(node, tree_data))
}

fn range_to_value(node: &Node<'_>) -> ElleValue {
    let a = api();
    let start = node.start_position();
    let end = node.end_position();
    a.build_struct(&[
        ("start-row", a.int(start.row as i64)),
        ("start-col", a.int(start.column as i64)),
        ("end-row", a.int(end.row as i64)),
        ("end-col", a.int(end.column as i64)),
        ("start-byte", a.int(node.start_byte() as i64)),
        ("end-byte", a.int(node.end_byte() as i64)),
    ])
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

extern "C" fn prim_ts_language(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = match get_string(args, nargs, 0, "ts/language") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let lang: Language = match name.as_str() {
        "c" => tree_sitter_c::LANGUAGE.into(),
        "rust" => tree_sitter_rust::LANGUAGE.into(),
        other => {
            return a.err("value-error", &format!("ts/language: unknown language {:?}", other));
        }
    };
    a.ok(a.external("ts/language", lang))
}

extern "C" fn prim_ts_parse(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let source = match get_string(args, nargs, 0, "ts/parse") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let lang = match get_language(args, nargs, 1, "ts/parse") {
        Ok(l) => l,
        Err(e) => return e,
    };
    let mut parser = Parser::new();
    if let Err(e) = parser.set_language(&lang) {
        return a.err("parse-error", &format!("ts/parse: {}", e));
    }
    match parser.parse(&source, None) {
        Some(tree) => {
            let data = Rc::new(TsTreeData { tree, source });
            a.ok(a.external("ts/tree", data))
        }
        None => a.err("parse-error", "ts/parse: parsing failed"),
    }
}

extern "C" fn prim_ts_root(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let tree_rc = match get_tree(args, nargs, 0, "ts/root") {
        Ok(t) => t,
        Err(e) => return e,
    };
    let root = tree_rc.tree.root_node();
    let path = compute_path(root);
    let nd = TsNodeData {
        tree_data: tree_rc,
        path,
    };
    a.ok(a.external("ts/node", nd))
}

extern "C" fn prim_ts_node_type(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let nd = match get_node(args, nargs, 0, "ts/node-type") {
        Ok(n) => n,
        Err(e) => return e,
    };
    match nd.resolve() {
        Some(node) => a.ok(a.string(node.kind())),
        None => a.err("node-error", "ts/node-type: could not resolve node"),
    }
}

extern "C" fn prim_ts_node_text(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let nd = match get_node(args, nargs, 0, "ts/node-text") {
        Ok(n) => n,
        Err(e) => return e,
    };
    match nd.resolve() {
        Some(node) => {
            let text = &nd.tree_data.source[node.start_byte()..node.end_byte()];
            a.ok(a.string(text))
        }
        None => a.err("node-error", "ts/node-text: could not resolve node"),
    }
}

extern "C" fn prim_ts_node_named(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let nd = match get_node(args, nargs, 0, "ts/node-named?") {
        Ok(n) => n,
        Err(e) => return e,
    };
    match nd.resolve() {
        Some(node) => a.ok(a.boolean(node.is_named())),
        None => a.err("node-error", "ts/node-named?: could not resolve node"),
    }
}

extern "C" fn prim_ts_children(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let nd = match get_node(args, nargs, 0, "ts/children") {
        Ok(n) => n,
        Err(e) => return e,
    };
    match nd.resolve() {
        Some(node) => {
            let tree_data = nd.tree_data.clone();
            let children: Vec<ElleValue> = (0..node.child_count())
                .filter_map(|i| node.child(i))
                .map(|child| node_to_value(child, tree_data.clone()))
                .collect();
            a.ok(a.array(&children))
        }
        None => a.err("node-error", "ts/children: could not resolve node"),
    }
}

extern "C" fn prim_ts_named_children(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let nd = match get_node(args, nargs, 0, "ts/named-children") {
        Ok(n) => n,
        Err(e) => return e,
    };
    match nd.resolve() {
        Some(node) => {
            let tree_data = nd.tree_data.clone();
            let children: Vec<ElleValue> = (0..node.named_child_count())
                .filter_map(|i| node.named_child(i))
                .map(|child| node_to_value(child, tree_data.clone()))
                .collect();
            a.ok(a.array(&children))
        }
        None => a.err("node-error", "ts/named-children: could not resolve node"),
    }
}

extern "C" fn prim_ts_child_by_field(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let nd = match get_node(args, nargs, 0, "ts/child-by-field") {
        Ok(n) => n,
        Err(e) => return e,
    };
    let field = match get_string(args, nargs, 1, "ts/child-by-field") {
        Ok(s) => s,
        Err(e) => return e,
    };
    match nd.resolve() {
        Some(node) => match node.child_by_field_name(&field) {
            Some(child) => a.ok(node_to_value(child, nd.tree_data.clone())),
            None => a.ok(a.nil()),
        },
        None => a.err("node-error", "ts/child-by-field: could not resolve node"),
    }
}

extern "C" fn prim_ts_parent(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let nd = match get_node(args, nargs, 0, "ts/parent") {
        Ok(n) => n,
        Err(e) => return e,
    };
    if nd.path.is_empty() {
        return a.ok(a.nil());
    }
    let parent = TsNodeData {
        tree_data: nd.tree_data.clone(),
        path: nd.path[..nd.path.len() - 1].to_vec(),
    };
    a.ok(a.external("ts/node", parent))
}

extern "C" fn prim_ts_node_range(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let nd = match get_node(args, nargs, 0, "ts/node-range") {
        Ok(n) => n,
        Err(e) => return e,
    };
    match nd.resolve() {
        Some(node) => a.ok(range_to_value(&node)),
        None => a.err("node-error", "ts/node-range: could not resolve node"),
    }
}

extern "C" fn prim_ts_node_sexp(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let nd = match get_node(args, nargs, 0, "ts/node-sexp") {
        Ok(n) => n,
        Err(e) => return e,
    };
    match nd.resolve() {
        Some(node) => {
            let sexp = node.to_sexp();
            a.ok(a.string(&sexp))
        }
        None => a.err("node-error", "ts/node-sexp: could not resolve node"),
    }
}

extern "C" fn prim_ts_query(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let lang = match get_language(args, nargs, 0, "ts/query") {
        Ok(l) => l,
        Err(e) => return e,
    };
    let pattern = match get_string(args, nargs, 1, "ts/query") {
        Ok(s) => s,
        Err(e) => return e,
    };
    match Query::new(&lang, &pattern) {
        Ok(query) => a.ok(a.external("ts/query", TsQueryData { query })),
        Err(e) => a.err("query-error", &format!("ts/query: {}", e)),
    }
}

extern "C" fn prim_ts_matches(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let qd = match get_query(args, nargs, 0, "ts/matches") {
        Ok(q) => q,
        Err(e) => return e,
    };
    let nd = match get_node(args, nargs, 1, "ts/matches") {
        Ok(n) => n,
        Err(e) => return e,
    };
    let node = match nd.resolve() {
        Some(n) => n,
        None => return a.err("node-error", "ts/matches: could not resolve node"),
    };

    let capture_names = qd.query.capture_names();
    let tree_data = nd.tree_data.clone();
    let source = tree_data.source.as_bytes();

    let mut cursor = QueryCursor::new();
    let mut results: Vec<ElleValue> = Vec::new();
    let mut iter = cursor.matches(&qd.query, node, source);
    while let Some(m) = iter.next() {
        let mut cap_kvs: Vec<(String, ElleValue)> = Vec::new();
        for cap in m.captures {
            let name = &capture_names[cap.index as usize];
            cap_kvs.push(((*name).to_string(), node_to_value(cap.node, tree_data.clone())));
        }
        let cap_refs: Vec<(&str, ElleValue)> = cap_kvs.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        let captures = a.build_struct(&cap_refs);
        let match_val = a.build_struct(&[
            ("pattern", a.int(m.pattern_index as i64)),
            ("captures", captures),
        ]);
        results.push(match_val);
    }

    a.ok(a.array(&results))
}

extern "C" fn prim_ts_captures(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let qd = match get_query(args, nargs, 0, "ts/captures") {
        Ok(q) => q,
        Err(e) => return e,
    };
    let nd = match get_node(args, nargs, 1, "ts/captures") {
        Ok(n) => n,
        Err(e) => return e,
    };
    let node = match nd.resolve() {
        Some(n) => n,
        None => return a.err("node-error", "ts/captures: could not resolve node"),
    };

    let capture_names = qd.query.capture_names();
    let tree_data = nd.tree_data.clone();
    let source = tree_data.source.as_bytes();

    let mut cursor = QueryCursor::new();
    let mut results: Vec<ElleValue> = Vec::new();
    let mut iter = cursor.captures(&qd.query, node, source);
    while let Some((m, _capture_idx)) = iter.next() {
        for cap in m.captures {
            let name = &capture_names[cap.index as usize];
            let entry = a.build_struct(&[
                ("name", a.string(name)),
                ("node", node_to_value(cap.node, tree_data.clone())),
            ]);
            results.push(entry);
        }
    }

    a.ok(a.array(&results))
}

extern "C" fn prim_ts_node_count(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let tree_rc = match get_tree(args, nargs, 0, "ts/node-count") {
        Ok(t) => t,
        Err(e) => return e,
    };
    fn count(node: Node<'_>) -> i64 {
        let mut n: i64 = 1;
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                n += count(child);
            }
        }
        n
    }
    a.ok(a.int(count(tree_rc.tree.root_node())))
}

// ---------------------------------------------------------------------------
// Plugin entry point
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::exact("ts/language", prim_ts_language, SIG_ERROR, 1, "Load a built-in tree-sitter grammar by name. Supported: \"c\", \"rust\"", "tree-sitter", r#"(ts/language "c")"#),
    EllePrimDef::exact("ts/parse", prim_ts_parse, SIG_ERROR, 2, "Parse source code with a language grammar, returning a tree", "tree-sitter", r#"(ts/parse "int main() {}" (ts/language "c"))"#),
    EllePrimDef::exact("ts/root", prim_ts_root, SIG_ERROR, 1, "Get the root node of a parsed tree", "tree-sitter", r#"(ts/root tree)"#),
    EllePrimDef::exact("ts/node-type", prim_ts_node_type, SIG_ERROR, 1, "Return the grammar node type as a string", "tree-sitter", r#"(ts/node-type node)"#),
    EllePrimDef::exact("ts/node-text", prim_ts_node_text, SIG_ERROR, 1, "Return the source text spanned by a node", "tree-sitter", r#"(ts/node-text node)"#),
    EllePrimDef::exact("ts/node-named?", prim_ts_node_named, SIG_ERROR, 1, "Return true if the node is a named node", "tree-sitter", r#"(ts/node-named? node)"#),
    EllePrimDef::exact("ts/children", prim_ts_children, SIG_ERROR, 1, "Return all child nodes as an array", "tree-sitter", r#"(ts/children node)"#),
    EllePrimDef::exact("ts/named-children", prim_ts_named_children, SIG_ERROR, 1, "Return only named child nodes as an array", "tree-sitter", r#"(ts/named-children node)"#),
    EllePrimDef::exact("ts/child-by-field", prim_ts_child_by_field, SIG_ERROR, 2, "Get a child node by its field name, or nil if absent", "tree-sitter", r#"(ts/child-by-field node "name")"#),
    EllePrimDef::exact("ts/parent", prim_ts_parent, SIG_OK, 1, "Return the parent node, or nil for the root", "tree-sitter", r#"(ts/parent node)"#),
    EllePrimDef::exact("ts/node-range", prim_ts_node_range, SIG_ERROR, 1, "Return {:start-row :start-col :end-row :end-col :start-byte :end-byte} for a node", "tree-sitter", r#"(ts/node-range node)"#),
    EllePrimDef::exact("ts/node-sexp", prim_ts_node_sexp, SIG_ERROR, 1, "Return the S-expression representation of a node", "tree-sitter", r#"(ts/node-sexp node)"#),
    EllePrimDef::exact("ts/query", prim_ts_query, SIG_ERROR, 2, "Compile a tree-sitter query pattern for a language", "tree-sitter", r#"(ts/query lang "(function_definition name: (identifier) @name)")"#),
    EllePrimDef::exact("ts/matches", prim_ts_matches, SIG_ERROR, 2, "Run a query against a node, returning an array of match structs", "tree-sitter", r#"(ts/matches query (ts/root tree))"#),
    EllePrimDef::exact("ts/captures", prim_ts_captures, SIG_ERROR, 2, "Run a query, returning a flat array of capture structs", "tree-sitter", r#"(ts/captures query (ts/root tree))"#),
    EllePrimDef::exact("ts/node-count", prim_ts_node_count, SIG_ERROR, 1, "Return the total number of nodes in a parsed tree", "tree-sitter", r#"(ts/node-count tree)"#),
];
