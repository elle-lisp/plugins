//! Elle oxigraph plugin — RDF quad storage + SPARQL via the `oxigraph` crate.

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_ERROR};

use oxigraph::io::{RdfFormat, RdfSerializer};
use oxigraph::model::{
    BlankNode, GraphName, GraphNameRef, Literal, NamedNode, Quad, Subject, Term,
};
use oxigraph::sparql::QueryResults;
use oxigraph::store::Store;

// ---------------------------------------------------------------------------
// fd save/restore (RocksDB redirects stdio)
// ---------------------------------------------------------------------------

mod fd_ops {
    extern "C" {
        pub fn dup(fd: i32) -> i32;
        pub fn dup2(oldfd: i32, newfd: i32) -> i32;
        pub fn close(fd: i32) -> i32;
    }
}

fn save_fd(fd: i32) -> Option<i32> {
    let ret = unsafe { fd_ops::dup(fd) };
    if ret >= 0 {
        Some(ret)
    } else {
        None
    }
}

fn restore_fd(saved: i32, target: i32) {
    unsafe {
        fd_ops::dup2(saved, target);
        fd_ops::close(saved);
    };
}

// ---------------------------------------------------------------------------
// Plugin entry point
// ---------------------------------------------------------------------------
elle_plugin::define_plugin!("oxigraph/", &PRIMITIVES);

// ---------------------------------------------------------------------------
// Term conversion helpers
// ---------------------------------------------------------------------------

/// Build an Elle array representing an RDF IRI: `[:iri "http://..."]`.
fn iri_to_elle(n: &NamedNode) -> ElleValue {
    let a = api();
    a.array(&[a.keyword("iri"), a.string(n.as_str())])
}

/// Build an Elle array representing an RDF blank node: `[:bnode "id"]`.
fn bnode_to_elle(b: &BlankNode) -> ElleValue {
    let a = api();
    a.array(&[a.keyword("bnode"), a.string(b.as_str())])
}

/// Build an Elle array representing an RDF literal.
fn literal_to_elle(l: &Literal) -> ElleValue {
    let a = api();
    if let Some(lang) = l.language() {
        a.array(&[
            a.keyword("literal"),
            a.string(l.value()),
            a.keyword("lang"),
            a.string(lang),
        ])
    } else {
        let dt = l.datatype().as_str();
        const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";
        if dt == XSD_STRING {
            a.array(&[a.keyword("literal"), a.string(l.value())])
        } else {
            a.array(&[
                a.keyword("literal"),
                a.string(l.value()),
                a.keyword("datatype"),
                a.string(dt),
            ])
        }
    }
}

/// Convert an oxigraph `Term` to an Elle array.
fn term_to_elle(term: &Term) -> ElleValue {
    let a = api();
    match term {
        Term::NamedNode(n) => iri_to_elle(n),
        Term::BlankNode(b) => bnode_to_elle(b),
        Term::Literal(l) => literal_to_elle(l),
        Term::Triple(_) => a.nil(),
    }
}

/// Convert an Elle term array to an oxigraph `Term`.
fn elle_to_term(val: ElleValue, prim: &str) -> Result<Term, ElleResult> {
    let a = api();
    let len = a.get_array_len(val).ok_or_else(|| {
        a.err("type-error", &format!("{}: expected term array, got {}", prim, a.type_name(val)))
    })?;

    let first = a.get_array_item(val, 0);
    let tag = a.get_keyword_name(first).ok_or_else(|| {
        a.err("type-error", &format!("{}: term array must start with a keyword tag", prim))
    })?;

    match tag {
        "iri" => {
            let s = string_at(val, 1, len, prim, "IRI string")?;
            NamedNode::new(&s)
                .map(Term::from)
                .map_err(|e| oxigraph_err(prim, e))
        }
        "bnode" => {
            let s = string_at(val, 1, len, prim, "blank node id")?;
            BlankNode::new(&s)
                .map(Term::from)
                .map_err(|e| oxigraph_err(prim, e))
        }
        "literal" => {
            let value = string_at(val, 1, len, prim, "literal value")?;
            if len == 2 {
                Ok(Term::from(Literal::new_simple_literal(&value)))
            } else if len == 4 {
                let key_val = a.get_array_item(val, 2);
                let key = a.get_keyword_name(key_val).ok_or_else(|| {
                    a.err("type-error", &format!("{}: expected :lang or :datatype keyword at index 2", prim))
                })?;
                let tag_val = string_at(val, 3, len, prim, "tag value")?;
                match key {
                    "lang" => Literal::new_language_tagged_literal(&value, &tag_val)
                        .map(Term::from)
                        .map_err(|e| oxigraph_err(prim, e)),
                    "datatype" => {
                        let dt = NamedNode::new(&tag_val).map_err(|e| oxigraph_err(prim, e))?;
                        Ok(Term::from(Literal::new_typed_literal(&value, dt)))
                    }
                    _ => Err(a.err("type-error", &format!("{}: expected :lang or :datatype, got :{}", prim, key))),
                }
            } else {
                Err(a.err("type-error", &format!("{}: :literal array must have length 2 or 4, got {}", prim, len)))
            }
        }
        _ => Err(a.err("type-error", &format!("{}: unknown term tag :{}", prim, tag))),
    }
}

/// Convert an Elle graph-name value to an oxigraph `GraphName`.
fn elle_to_graph_name(val: ElleValue, prim: &str) -> Result<GraphName, ElleResult> {
    let a = api();
    if a.check_nil(val) {
        return Ok(GraphName::DefaultGraph);
    }
    let term = elle_to_term(val, prim)?;
    match term {
        Term::NamedNode(n) => Ok(GraphName::NamedNode(n)),
        Term::BlankNode(b) => Ok(GraphName::BlankNode(b)),
        Term::Literal(_) | Term::Triple(_) => {
            Err(a.err("type-error", &format!("{}: graph name must be an IRI or blank node", prim)))
        }
    }
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

/// Extract a string from array element at index.
fn string_at(
    arr: ElleValue,
    index: usize,
    len: usize,
    prim: &str,
    what: &str,
) -> Result<String, ElleResult> {
    let a = api();
    if index >= len {
        return Err(a.err("type-error", &format!("{}: expected string at index {} ({}), got (missing)", prim, index, what)));
    }
    let v = a.get_array_item(arr, index);
    a.get_string(v)
        .map(|s| s.to_string())
        .ok_or_else(|| {
            a.err("type-error", &format!("{}: expected string at index {} ({}), got {}", prim, index, what, a.type_name(v)))
        })
}

/// Map any `Display` error to an `oxigraph-error` signal.
fn oxigraph_err(prim: &str, e: impl std::fmt::Display) -> ElleResult {
    api().err("oxigraph-error", &format!("{}: {}", prim, e))
}

/// Extract `Store` from args[0], or return a type-error.
fn get_store<'a>(args: *const ElleValue, nargs: usize, prim: &str) -> Result<&'a Store, ElleResult> {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    a.get_external::<Store>(v, "oxigraph/store").ok_or_else(|| {
        a.err("type-error", &format!("{}: expected oxigraph/store, got {}", prim, a.type_name(v)))
    })
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

extern "C" fn prim_store_new(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    let a = api();
    match Store::new() {
        Ok(store) => a.ok(a.external("oxigraph/store", store)),
        Err(e) => oxigraph_err("oxigraph/store-new", e),
    }
}

extern "C" fn prim_store_open(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let path = match a.get_string(v) {
        Some(s) => s.to_string(),
        None => return a.err("type-error", &format!("oxigraph/store-open: expected string path, got {}", a.type_name(v))),
    };
    let saved_stdout = save_fd(1);
    let saved_stderr = save_fd(2);
    let result = Store::open(&path);
    if let Some(fd) = saved_stdout { restore_fd(fd, 1); }
    if let Some(fd) = saved_stderr { restore_fd(fd, 2); }
    match result {
        Ok(store) => a.ok(a.external("oxigraph/store", store)),
        Err(e) => oxigraph_err("oxigraph/store-open", e),
    }
}

extern "C" fn prim_iri(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v = unsafe { a.arg(args, nargs, 0) };
    let s = match a.get_string(v) {
        Some(s) => s.to_string(),
        None => return a.err("type-error", &format!("oxigraph/iri: expected string, got {}", a.type_name(v))),
    };
    match NamedNode::new(&s) {
        Ok(n) => a.ok(iri_to_elle(&n)),
        Err(e) => oxigraph_err("oxigraph/iri", e),
    }
}

extern "C" fn prim_literal(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v0 = unsafe { a.arg(args, nargs, 0) };
    let value = match a.get_string(v0) {
        Some(s) => s.to_string(),
        None => return a.err("type-error", &format!("oxigraph/literal: expected string value, got {}", a.type_name(v0))),
    };

    if nargs == 1 {
        return a.ok(a.array(&[a.keyword("literal"), a.string(&value)]));
    }

    let v1 = unsafe { a.arg(args, nargs, 1) };
    let tag_key = match a.get_keyword_name(v1) {
        Some(k) => k.to_string(),
        None => return a.err("type-error", &format!("oxigraph/literal: expected :lang or :datatype keyword, got {}", a.type_name(v1))),
    };
    let v2 = unsafe { a.arg(args, nargs, 2) };
    let tag_val = match a.get_string(v2) {
        Some(s) => s.to_string(),
        None => return a.err("type-error", &format!("oxigraph/literal: expected string tag value, got {}", a.type_name(v2))),
    };

    match tag_key.as_str() {
        "lang" => {
            match Literal::new_language_tagged_literal(&value, &tag_val) {
                Ok(_) => a.ok(a.array(&[
                    a.keyword("literal"),
                    a.string(&value),
                    a.keyword("lang"),
                    a.string(&tag_val),
                ])),
                Err(e) => oxigraph_err("oxigraph/literal", e),
            }
        }
        "datatype" => {
            a.ok(a.array(&[
                a.keyword("literal"),
                a.string(&value),
                a.keyword("datatype"),
                a.string(&tag_val),
            ]))
        }
        _ => a.err("type-error", &format!("oxigraph/literal: expected :lang or :datatype, got :{}", tag_key)),
    }
}

extern "C" fn prim_blank_node(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs == 0 {
        let b = BlankNode::default();
        return a.ok(bnode_to_elle(&b));
    }
    let v = unsafe { a.arg(args, nargs, 0) };
    let id = match a.get_string(v) {
        Some(s) => s.to_string(),
        None => return a.err("type-error", &format!("oxigraph/blank-node: expected string id, got {}", a.type_name(v))),
    };
    match BlankNode::new(&id) {
        Ok(b) => a.ok(bnode_to_elle(&b)),
        Err(e) => oxigraph_err("oxigraph/blank-node", e),
    }
}

// ---------------------------------------------------------------------------
// Quad conversion helpers
// ---------------------------------------------------------------------------

fn graph_name_to_elle(gn: &GraphName) -> ElleValue {
    let a = api();
    match gn {
        GraphName::DefaultGraph => a.nil(),
        GraphName::NamedNode(n) => iri_to_elle(n),
        GraphName::BlankNode(b) => bnode_to_elle(b),
    }
}

fn subject_to_elle(s: &Subject) -> ElleValue {
    let a = api();
    match s {
        Subject::NamedNode(n) => iri_to_elle(n),
        Subject::BlankNode(b) => bnode_to_elle(b),
        Subject::Triple(_) => a.nil(),
    }
}

fn oxigraph_quad_to_elle(quad: &Quad) -> ElleValue {
    let a = api();
    a.array(&[
        subject_to_elle(&quad.subject),
        iri_to_elle(&quad.predicate),
        term_to_elle(&quad.object),
        graph_name_to_elle(&quad.graph_name),
    ])
}

fn elle_quad_to_oxigraph(val: ElleValue, prim: &str) -> Result<Quad, ElleResult> {
    let a = api();
    let len = a.get_array_len(val).ok_or_else(|| {
        a.err("type-error", &format!("{}: expected quad array, got {}", prim, a.type_name(val)))
    })?;
    if len != 4 {
        return Err(a.err("type-error", &format!("{}: quad array must have length 4, got {}", prim, len)));
    }

    let subject_term = elle_to_term(a.get_array_item(val, 0), prim)?;
    let subject: Subject = match subject_term {
        Term::NamedNode(n) => Subject::NamedNode(n),
        Term::BlankNode(b) => Subject::BlankNode(b),
        _ => return Err(a.err("type-error", &format!("{}: subject must be an IRI or blank node", prim))),
    };

    let pred_term = elle_to_term(a.get_array_item(val, 1), prim)?;
    let predicate = match pred_term {
        Term::NamedNode(n) => n,
        _ => return Err(a.err("type-error", &format!("{}: predicate must be an IRI", prim))),
    };

    let object = elle_to_term(a.get_array_item(val, 2), prim)?;
    let graph_name = elle_to_graph_name(a.get_array_item(val, 3), prim)?;

    Ok(Quad::new(subject, predicate, object, graph_name))
}

// ---------------------------------------------------------------------------
// Quad CRUD primitives
// ---------------------------------------------------------------------------

extern "C" fn prim_insert(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let store = match get_store(args, nargs, "oxigraph/insert") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let quad = match elle_quad_to_oxigraph(unsafe { a.arg(args, nargs, 1) }, "oxigraph/insert") {
        Ok(q) => q,
        Err(e) => return e,
    };
    match store.insert(quad.as_ref()) {
        Ok(_) => a.ok(a.nil()),
        Err(e) => oxigraph_err("oxigraph/insert", e),
    }
}

extern "C" fn prim_remove(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let store = match get_store(args, nargs, "oxigraph/remove") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let quad = match elle_quad_to_oxigraph(unsafe { a.arg(args, nargs, 1) }, "oxigraph/remove") {
        Ok(q) => q,
        Err(e) => return e,
    };
    match store.remove(quad.as_ref()) {
        Ok(_) => a.ok(a.nil()),
        Err(e) => oxigraph_err("oxigraph/remove", e),
    }
}

extern "C" fn prim_contains(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let store = match get_store(args, nargs, "oxigraph/contains") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let quad = match elle_quad_to_oxigraph(unsafe { a.arg(args, nargs, 1) }, "oxigraph/contains") {
        Ok(q) => q,
        Err(e) => return e,
    };
    match store.contains(quad.as_ref()) {
        Ok(result) => a.ok(a.boolean(result)),
        Err(e) => oxigraph_err("oxigraph/contains", e),
    }
}

extern "C" fn prim_quads(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let store = match get_store(args, nargs, "oxigraph/quads") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let mut result = Vec::new();
    for item in store.quads_for_pattern(None, None, None, None) {
        match item {
            Ok(quad) => result.push(oxigraph_quad_to_elle(&quad)),
            Err(e) => return oxigraph_err("oxigraph/quads", e),
        }
    }
    a.ok(a.array(&result))
}

extern "C" fn prim_query(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    const PRIM: &str = "oxigraph/query";
    let store = match get_store(args, nargs, PRIM) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let v1 = unsafe { a.arg(args, nargs, 1) };
    let sparql = match a.get_string(v1) {
        Some(s) => s.to_string(),
        None => return a.err("type-error", &format!("{}: expected string sparql, got {}", PRIM, a.type_name(v1))),
    };
    let results = match store.query(&sparql) {
        Ok(r) => r,
        Err(e) => return a.err("sparql-error", &format!("{}: {}", PRIM, e)),
    };
    match results {
        QueryResults::Solutions(solutions) => {
            let mut rows: Vec<ElleValue> = Vec::new();
            for solution in solutions {
                let solution = match solution {
                    Ok(s) => s,
                    Err(e) => return a.err("sparql-error", &format!("{}: {}", PRIM, e)),
                };
                let fields: Vec<(&str, ElleValue)> = solution
                    .iter()
                    .map(|(variable, term)| (variable.as_str(), term_to_elle(term)))
                    .collect();
                rows.push(a.build_struct(&fields));
            }
            a.ok(a.array(&rows))
        }
        QueryResults::Boolean(b) => a.ok(a.boolean(b)),
        QueryResults::Graph(triples) => {
            let mut rows: Vec<ElleValue> = Vec::new();
            for triple in triples {
                let triple = match triple {
                    Ok(t) => t,
                    Err(e) => return a.err("sparql-error", &format!("{}: {}", PRIM, e)),
                };
                let subject_val = match &triple.subject {
                    Subject::NamedNode(n) => iri_to_elle(n),
                    Subject::BlankNode(b) => bnode_to_elle(b),
                    Subject::Triple(_) => a.nil(),
                };
                rows.push(a.array(&[
                    subject_val,
                    iri_to_elle(&triple.predicate),
                    term_to_elle(&triple.object),
                    a.nil(),
                ]));
            }
            a.ok(a.array(&rows))
        }
    }
}

extern "C" fn prim_update(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    const PRIM: &str = "oxigraph/update";
    let store = match get_store(args, nargs, PRIM) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let v1 = unsafe { a.arg(args, nargs, 1) };
    let sparql = match a.get_string(v1) {
        Some(s) => s.to_string(),
        None => return a.err("type-error", &format!("{}: expected string sparql-update, got {}", PRIM, a.type_name(v1))),
    };
    match store.update(&sparql) {
        Ok(()) => a.ok(a.nil()),
        Err(e) => a.err("sparql-error", &format!("{}: {}", PRIM, e)),
    }
}

/// Map a keyword value to an `RdfFormat`, or return an error.
fn keyword_to_format(val: ElleValue, prim: &str) -> Result<RdfFormat, ElleResult> {
    let a = api();
    let kw = a.get_keyword_name(val).ok_or_else(|| {
        a.err("type-error", &format!("{}: expected format keyword (:turtle :ntriples :nquads :rdfxml), got {}", prim, a.type_name(val)))
    })?;
    match kw {
        "turtle" => Ok(RdfFormat::Turtle),
        "ntriples" => Ok(RdfFormat::NTriples),
        "nquads" => Ok(RdfFormat::NQuads),
        "rdfxml" => Ok(RdfFormat::RdfXml),
        _ => Err(a.err("type-error", &format!("{}: unknown format keyword :{}, expected :turtle :ntriples :nquads :rdfxml", prim, kw))),
    }
}

extern "C" fn prim_load(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    const PRIM: &str = "oxigraph/load";
    let store = match get_store(args, nargs, PRIM) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let v1 = unsafe { a.arg(args, nargs, 1) };
    let data = match a.get_string(v1) {
        Some(s) => s.to_string(),
        None => return a.err("type-error", &format!("{}: expected string data, got {}", PRIM, a.type_name(v1))),
    };
    let v2 = unsafe { a.arg(args, nargs, 2) };
    let format = match keyword_to_format(v2, PRIM) {
        Ok(f) => f,
        Err(e) => return e,
    };
    match store.load_from_reader(format, data.as_bytes()) {
        Ok(()) => a.ok(a.nil()),
        Err(e) => oxigraph_err(PRIM, e),
    }
}

extern "C" fn prim_dump(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    const PRIM: &str = "oxigraph/dump";
    let store = match get_store(args, nargs, PRIM) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let v1 = unsafe { a.arg(args, nargs, 1) };
    let format = match keyword_to_format(v1, PRIM) {
        Ok(f) => f,
        Err(e) => return e,
    };
    let buf: Vec<u8> = if format.supports_datasets() {
        match store.dump_to_writer(RdfSerializer::from_format(format), Vec::new()) {
            Ok(b) => b,
            Err(e) => return oxigraph_err(PRIM, e),
        }
    } else {
        match store.dump_graph_to_writer(
            GraphNameRef::DefaultGraph,
            RdfSerializer::from_format(format),
            Vec::new(),
        ) {
            Ok(b) => b,
            Err(e) => return oxigraph_err(PRIM, e),
        }
    };
    match String::from_utf8(buf) {
        Ok(s) => a.ok(a.string(&s)),
        Err(e) => oxigraph_err(PRIM, e),
    }
}

extern "C" fn prim_store_flush(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let store = match get_store(args, nargs, "oxigraph/store-flush") {
        Ok(s) => s,
        Err(e) => return e,
    };
    match store.flush() {
        Ok(()) => a.ok(a.nil()),
        Err(e) => oxigraph_err("oxigraph/store-flush", e),
    }
}

// ---------------------------------------------------------------------------
// Registration table
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::exact("oxigraph/store-new", prim_store_new, SIG_ERROR, 0,
        "Create a new in-memory RDF store.", "oxigraph", "(oxigraph/store-new)"),
    EllePrimDef::exact("oxigraph/store-open", prim_store_open, SIG_ERROR, 1,
        "Open a persistent on-disk RDF store at the given path.", "oxigraph",
        r#"(oxigraph/store-open "/tmp/my-graph")"#),
    EllePrimDef::exact("oxigraph/iri", prim_iri, SIG_ERROR, 1,
        "Construct and validate an IRI term. Returns [:iri \"http://...\"].", "oxigraph",
        r#"(oxigraph/iri "http://example.org/alice")"#),
    EllePrimDef::range("oxigraph/literal", prim_literal, SIG_ERROR, 1, 3,
        "Construct a literal term. 1 arg = plain. 3 args = :lang or :datatype tagged.", "oxigraph",
        r#"(oxigraph/literal "hello" :lang "en")"#),
    EllePrimDef::range("oxigraph/blank-node", prim_blank_node, SIG_ERROR, 0, 1,
        "Construct a blank node. 0 args = auto-generated ID. 1 arg = explicit ID.", "oxigraph",
        "(oxigraph/blank-node)"),
    EllePrimDef::exact("oxigraph/insert", prim_insert, SIG_ERROR, 2,
        "Insert a quad into the store.", "oxigraph", "(oxigraph/insert store [s p o nil])"),
    EllePrimDef::exact("oxigraph/remove", prim_remove, SIG_ERROR, 2,
        "Remove a quad from the store. No error if quad doesn't exist.", "oxigraph",
        "(oxigraph/remove store quad)"),
    EllePrimDef::exact("oxigraph/contains", prim_contains, SIG_ERROR, 2,
        "Check if a quad exists in the store.", "oxigraph", "(oxigraph/contains store quad)"),
    EllePrimDef::exact("oxigraph/quads", prim_quads, SIG_ERROR, 1,
        "Return all quads in the store as an immutable array.", "oxigraph",
        "(oxigraph/quads store)"),
    EllePrimDef::exact("oxigraph/query", prim_query, SIG_ERROR, 2,
        "Execute a SPARQL query against the store.", "oxigraph",
        r#"(oxigraph/query store "SELECT ?s ?p ?o WHERE { ?s ?p ?o }")"#),
    EllePrimDef::exact("oxigraph/update", prim_update, SIG_ERROR, 2,
        "Execute a SPARQL UPDATE against the store.", "oxigraph",
        r#"(oxigraph/update store "INSERT DATA { ... }")"#),
    EllePrimDef::exact("oxigraph/load", prim_load, SIG_ERROR, 3,
        "Load RDF data from a string into the store. Format: :turtle :ntriples :nquads :rdfxml.", "oxigraph",
        r#"(oxigraph/load store "<http://ex.org/a> <http://ex.org/b> \"hello\" .\n" :ntriples)"#),
    EllePrimDef::exact("oxigraph/dump", prim_dump, SIG_ERROR, 2,
        "Serialize store to a string. Dataset formats (:nquads) dump all graphs; graph formats dump the default graph.", "oxigraph",
        "(oxigraph/dump store :nquads)"),
    EllePrimDef::exact("oxigraph/store-flush", prim_store_flush, SIG_ERROR, 1,
        "Flush pending writes to disk. Call after load/insert/update on persistent stores.", "oxigraph",
        "(oxigraph/store-flush store)"),
];
