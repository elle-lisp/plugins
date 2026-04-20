//! Elle xml plugin — XML parsing and serialization via the `quick-xml` crate.

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Reader;
use quick_xml::Writer;
use std::cell::RefCell;
use std::io::Cursor;

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_ERROR};

elle_plugin::define_plugin!("xml/", &PRIMITIVES);

// ---------------------------------------------------------------------------
// DOM parser helpers
// ---------------------------------------------------------------------------

/// Internal element node during parsing.
struct ParsedElement {
    tag: String,
    attrs: Vec<(String, ElleValue)>,
    children: Vec<ElleValue>,
}

fn attrs_from_start(e: &BytesStart) -> Result<Vec<(String, ElleValue)>, String> {
    let a = api();
    let mut attrs = Vec::new();
    for attr_result in e.attributes() {
        match attr_result {
            Ok(attr) => {
                let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
                let val = match attr.unescape_value() {
                    Ok(v) => v.into_owned(),
                    Err(e) => return Err(format!("xml/parse: attribute decode error: {}", e)),
                };
                attrs.push((key, a.string(&val)));
            }
            Err(e) => return Err(format!("xml/parse: attribute error: {}", e)),
        }
    }
    Ok(attrs)
}

fn element_to_value(elem: ParsedElement) -> ElleValue {
    let a = api();
    let attrs_kvs: Vec<(&str, ElleValue)> = elem
        .attrs
        .iter()
        .map(|(k, v)| (k.as_str(), *v))
        .collect();
    let attrs_val = a.build_struct(&attrs_kvs);
    let children_val = a.array(&elem.children);
    a.build_struct(&[
        ("tag", a.string(&elem.tag)),
        ("attrs", attrs_val),
        ("children", children_val),
    ])
}

/// Parse an XML string into an Elle element struct.
fn parse_xml(input: &str) -> Result<ElleValue, String> {
    let a = api();
    let mut reader = Reader::from_reader(Cursor::new(input.as_bytes().to_vec()));
    reader.config_mut().trim_text(false);

    let mut stack: Vec<ParsedElement> = Vec::new();
    let mut buf = Vec::new();
    let mut roots: Vec<ElleValue> = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let attrs = attrs_from_start(e)?;
                stack.push(ParsedElement {
                    tag,
                    attrs,
                    children: Vec::new(),
                });
            }
            Ok(Event::End(_)) => {
                let elem = match stack.pop() {
                    Some(e) => e,
                    None => return Err("xml/parse: unexpected closing tag".to_string()),
                };
                let value = element_to_value(elem);
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(value);
                } else {
                    roots.push(value);
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let attrs = attrs_from_start(e)?;
                let value = element_to_value(ParsedElement {
                    tag,
                    attrs,
                    children: Vec::new(),
                });
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(value);
                } else {
                    roots.push(value);
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = match e.unescape() {
                    Ok(t) => t.into_owned(),
                    Err(err) => return Err(format!("xml/parse: text decode error: {}", err)),
                };
                if !text.is_empty() {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(a.string(&text));
                    }
                }
            }
            Ok(Event::CData(ref e)) => {
                let text = match e.decode() {
                    Ok(t) => t.into_owned(),
                    Err(err) => return Err(format!("xml/parse: CDATA decode error: {}", err)),
                };
                if !text.is_empty() {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(a.string(&text));
                    }
                }
            }
            Ok(Event::Comment(_))
            | Ok(Event::PI(_))
            | Ok(Event::Decl(_))
            | Ok(Event::DocType(_)) => {
                // Skip comments, processing instructions, XML declarations, DOCTYPE
            }
            Ok(Event::Eof) => {
                if !stack.is_empty() {
                    return Err(format!(
                        "xml/parse: unclosed element '{}'",
                        stack.last().unwrap().tag
                    ));
                }
                break;
            }
            Err(e) => return Err(format!("xml/parse: {}", e)),
        }
    }

    if roots.is_empty() {
        Err("xml/parse: empty document".to_string())
    } else {
        Ok(roots.into_iter().next().unwrap())
    }
}

// ---------------------------------------------------------------------------
// DOM emitter helpers
// ---------------------------------------------------------------------------

const MAX_EMIT_DEPTH: usize = 256;

fn emit_xml(val: ElleValue) -> Result<String, String> {
    let mut output = Vec::new();
    let mut writer = Writer::new(&mut output);
    emit_element(&mut writer, val, 0)?;
    String::from_utf8(output).map_err(|e| format!("xml/emit: UTF-8 error: {}", e))
}

fn emit_element(
    writer: &mut Writer<&mut Vec<u8>>,
    val: ElleValue,
    depth: usize,
) -> Result<(), String> {
    let a = api();

    if depth > MAX_EMIT_DEPTH {
        return Err("xml/emit: document too deeply nested (max 256)".to_string());
    }

    // If it's a string, emit it as escaped text content
    if let Some(text) = a.get_string(val) {
        let escaped = quick_xml::escape::escape(text);
        writer
            .write_event(Event::Text(BytesText::from_escaped(escaped.as_ref())))
            .map_err(|e| format!("xml/emit: write error: {}", e))?;
        return Ok(());
    }

    // Must be an element struct with :tag, :attrs, :children
    if !a.check_struct(val) {
        return Err(format!(
            "xml/emit: expected struct, got {}",
            a.type_name(val)
        ));
    }

    let tag_val = a.get_struct_field(val, "tag");
    let tag = match a.get_string(tag_val) {
        Some(s) => s.to_string(),
        None => {
            return Err(format!(
                "xml/emit: field 'tag' must be a string, got {}",
                a.type_name(tag_val)
            ))
        }
    };

    let attrs_val = a.get_struct_field(val, "attrs");
    if !a.check_struct(attrs_val) && !a.check_nil(attrs_val) {
        return Err(format!(
            "xml/emit: field 'attrs' must be a struct, got {}",
            a.type_name(attrs_val)
        ));
    }

    let children_val = a.get_struct_field(val, "children");
    let children_len = match a.get_array_len(children_val) {
        Some(n) => n,
        None => {
            return Err(format!(
                "xml/emit: field 'children' must be an array, got {}",
                a.type_name(children_val)
            ))
        }
    };

    let mut start = BytesStart::new(tag.as_str());

    // Emit attributes by iterating struct entries
    if a.check_struct(attrs_val) {
        for (key, field_val) in a.struct_entries(attrs_val) {
            let val_str = match a.get_string(field_val) {
                Some(s) => s.to_string(),
                None => {
                    return Err(format!(
                        "xml/emit: attribute '{}' value must be a string, got {}",
                        key,
                        a.type_name(field_val)
                    ));
                }
            };
            start.push_attribute((key, val_str.as_str()));
        }
    }

    if children_len == 0 {
        writer
            .write_event(Event::Empty(start))
            .map_err(|e| format!("xml/emit: write error: {}", e))?;
    } else {
        writer
            .write_event(Event::Start(start))
            .map_err(|e| format!("xml/emit: write error: {}", e))?;
        for i in 0..children_len {
            emit_element(writer, a.get_array_item(children_val, i), depth + 1)?;
        }
        writer
            .write_event(Event::End(BytesEnd::new(tag.as_str())))
            .map_err(|e| format!("xml/emit: write error: {}", e))?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Streaming reader
// ---------------------------------------------------------------------------

/// Internal state for the streaming XML reader handle.
struct XmlReaderState {
    reader: RefCell<Reader<Cursor<Vec<u8>>>>,
    buf: RefCell<Vec<u8>>,
}

fn attrs_from_start_streaming(
    e: &BytesStart,
) -> Result<Vec<(String, ElleValue)>, ElleResult> {
    let a = api();
    let mut attrs = Vec::new();
    for attr_result in e.attributes() {
        match attr_result {
            Ok(attr) => {
                let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
                let val = match attr.unescape_value() {
                    Ok(v) => v.into_owned(),
                    Err(err) => {
                        return Err(a.err(
                            "xml-error",
                            &format!("xml/next-event: attribute decode: {}", err),
                        ));
                    }
                };
                attrs.push((key, a.string(&val)));
            }
            Err(e) => {
                return Err(a.err(
                    "xml-error",
                    &format!("xml/next-event: attribute error: {}", e),
                ));
            }
        }
    }
    Ok(attrs)
}

extern "C" fn prim_xml_reader_new(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let arg0 = unsafe { a.arg(args, nargs, 0) };
    let s = match a.get_string(arg0) {
        Some(s) => s.to_string(),
        None => {
            return a.err(
                "type-error",
                &format!(
                    "xml/reader-new: expected string, got {}",
                    a.type_name(arg0)
                ),
            );
        }
    };
    let cursor = Cursor::new(s.into_bytes());
    let mut reader = Reader::from_reader(cursor);
    reader.config_mut().trim_text(false);
    let state = XmlReaderState {
        reader: RefCell::new(reader),
        buf: RefCell::new(Vec::new()),
    };
    a.ok(a.external("xml-reader", state))
}

extern "C" fn prim_xml_next_event(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let arg0 = unsafe { a.arg(args, nargs, 0) };
    let state = match a.get_external::<XmlReaderState>(arg0, "xml-reader") {
        Some(s) => s,
        None => {
            return a.err(
                "type-error",
                &format!(
                    "xml/next-event: expected xml-reader, got {}",
                    a.type_name(arg0)
                ),
            );
        }
    };
    loop {
        enum OwnedEvent {
            Start {
                tag: String,
                attrs: Result<Vec<(String, ElleValue)>, ElleResult>,
            },
            End {
                tag: String,
            },
            Text(String),
            Eof,
            Skip,
            Error(String),
        }
        let owned = {
            let mut buf = state.buf.borrow_mut();
            buf.clear();
            let mut reader = state.reader.borrow_mut();
            match reader.read_event_into(&mut buf) {
                Err(e) => OwnedEvent::Error(format!("xml/next-event: {}", e)),
                Ok(Event::Start(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                    let attrs = attrs_from_start_streaming(e);
                    OwnedEvent::Start { tag, attrs }
                }
                Ok(Event::Empty(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                    let attrs = attrs_from_start_streaming(e);
                    OwnedEvent::Start { tag, attrs }
                }
                Ok(Event::End(ref e)) => OwnedEvent::End {
                    tag: String::from_utf8_lossy(e.name().as_ref()).into_owned(),
                },
                Ok(Event::Text(ref e)) => match e.unescape() {
                    Err(err) => {
                        OwnedEvent::Error(format!("xml/next-event: text decode: {}", err))
                    }
                    Ok(t) => {
                        let text = t.into_owned();
                        if text.trim().is_empty() {
                            OwnedEvent::Skip
                        } else {
                            OwnedEvent::Text(text)
                        }
                    }
                },
                Ok(Event::CData(ref e)) => match e.decode() {
                    Err(err) => {
                        OwnedEvent::Error(format!("xml/next-event: CDATA decode: {}", err))
                    }
                    Ok(t) => OwnedEvent::Text(t.into_owned()),
                },
                Ok(Event::Eof) => OwnedEvent::Eof,
                Ok(Event::Comment(_))
                | Ok(Event::PI(_))
                | Ok(Event::Decl(_))
                | Ok(Event::DocType(_)) => OwnedEvent::Skip,
            }
        };
        match owned {
            OwnedEvent::Error(msg) => {
                return a.err("xml-error", &msg);
            }
            OwnedEvent::Start { tag, attrs } => {
                let attrs = match attrs {
                    Ok(a_vec) => a_vec,
                    Err(err) => return err,
                };
                let attrs_kvs: Vec<(&str, ElleValue)> =
                    attrs.iter().map(|(k, v)| (k.as_str(), *v)).collect();
                let attrs_val = a.build_struct(&attrs_kvs);
                return a.ok(a.build_struct(&[
                    ("type", a.keyword("start")),
                    ("tag", a.string(&tag)),
                    ("attrs", attrs_val),
                ]));
            }
            OwnedEvent::End { tag } => {
                return a.ok(a.build_struct(&[
                    ("type", a.keyword("end")),
                    ("tag", a.string(&tag)),
                ]));
            }
            OwnedEvent::Text(text) => {
                return a.ok(a.build_struct(&[
                    ("type", a.keyword("text")),
                    ("content", a.string(&text)),
                ]));
            }
            OwnedEvent::Eof => {
                return a.ok(a.build_struct(&[("type", a.keyword("eof"))]));
            }
            OwnedEvent::Skip => continue,
        }
    }
}

extern "C" fn prim_xml_reader_close(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let arg0 = unsafe { a.arg(args, nargs, 0) };
    match a.get_external::<XmlReaderState>(arg0, "xml-reader") {
        Some(_) => a.ok(a.nil()),
        None => a.err(
            "type-error",
            &format!(
                "xml/reader-close: expected xml-reader, got {}",
                a.type_name(arg0)
            ),
        ),
    }
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

extern "C" fn prim_xml_parse(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let arg0 = unsafe { a.arg(args, nargs, 0) };
    let s = match a.get_string(arg0) {
        Some(s) => s.to_string(),
        None => {
            return a.err(
                "type-error",
                &format!("xml/parse: expected string, got {}", a.type_name(arg0)),
            );
        }
    };
    match parse_xml(&s) {
        Ok(val) => a.ok(val),
        Err(e) => a.err("xml-error", &e),
    }
}

extern "C" fn prim_xml_emit(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let arg0 = unsafe { a.arg(args, nargs, 0) };
    if !a.check_struct(arg0) {
        return a.err(
            "xml-error",
            &format!(
                "xml/emit: expected element struct, got {}",
                a.type_name(arg0)
            ),
        );
    }
    match emit_xml(arg0) {
        Ok(s) => a.ok(a.string(&s)),
        Err(e) => a.err("xml-error", &e),
    }
}

// ---------------------------------------------------------------------------
// Registration table
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::exact(
        "xml/parse",
        prim_xml_parse,
        SIG_ERROR,
        1,
        "Parse an XML string into a nested struct/array tree",
        "xml",
        r#"(xml/parse "<root><child>text</child></root>")"#,
    ),
    EllePrimDef::exact(
        "xml/emit",
        prim_xml_emit,
        SIG_ERROR,
        1,
        "Serialize an element struct tree to an XML string",
        "xml",
        r#"(xml/emit {:tag "root" :attrs {} :children []})"#,
    ),
    EllePrimDef::exact(
        "xml/reader-new",
        prim_xml_reader_new,
        SIG_ERROR,
        1,
        "Create a streaming XML reader from a string",
        "xml",
        r#"(xml/reader-new "<root/>")"#,
    ),
    EllePrimDef::exact(
        "xml/next-event",
        prim_xml_next_event,
        SIG_ERROR,
        1,
        "Read the next event from a streaming XML reader",
        "xml",
        "(xml/next-event reader)",
    ),
    EllePrimDef::exact(
        "xml/reader-close",
        prim_xml_reader_close,
        SIG_ERROR,
        1,
        "Close a streaming XML reader (validates type; reader is freed with the value)",
        "xml",
        "(xml/reader-close reader)",
    ),
];
