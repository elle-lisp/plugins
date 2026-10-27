//! Elle markdown plugin — CommonMark rendered to HTML via `pulldown-cmark`.

use elle_plugin::{ElleCtx, EllePrimDef, ElleResult, ElleValue, SIG_ERROR};
use pulldown_cmark::{html, Options, Parser};

elle_plugin::define_plugin!("markdown/", &PRIMITIVES);

const HTML: &str = "markdown/html";

/// Each option key, and the `pulldown-cmark` extension it turns on.
const EXTENSIONS: &[(&str, Options)] = &[
    ("tables", Options::ENABLE_TABLES),
    ("strikethrough", Options::ENABLE_STRIKETHROUGH),
    ("tasklists", Options::ENABLE_TASKLISTS),
    ("footnotes", Options::ENABLE_FOOTNOTES),
];

/// The extensions an options struct turns on. A key that names no
/// extension is an error rather than ignored, so a misspelled key never
/// renders the text without the extension it asked for.
fn options(ctx: *mut ElleCtx, v: ElleValue) -> Result<Options, ElleResult> {
    let a = api();
    if !a.check_struct(v) {
        return Err(a.err(
            ctx,
            "type-error",
            &format!(
                "{}: expected a struct of options, got {}",
                HTML,
                a.type_name(v)
            ),
        ));
    }
    let mut out = Options::empty();
    for (key, value) in a.struct_entries(ctx, v) {
        let Some(&(_, flag)) = EXTENSIONS.iter().find(|(k, _)| *k == key) else {
            return Err(a.err(
                ctx,
                "value-error",
                &format!("{}: no extension is named :{}", HTML, key),
            ));
        };
        match a.get_bool(value) {
            Some(on) => out.set(flag, on),
            None => {
                return Err(a.err(
                    ctx,
                    "type-error",
                    &format!(
                        "{}: :{} expects a boolean, got {}",
                        HTML,
                        key,
                        a.type_name(value)
                    ),
                ))
            }
        }
    }
    Ok(out)
}

extern "C" fn prim_html(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let args = unsafe { a.args(args, nargs) };
    if args.is_empty() || args.len() > 2 {
        return a.err(
            ctx,
            "arity-error",
            &format!("{}: expected 1 or 2 arguments, got {}", HTML, args.len()),
        );
    }
    let Some(text) = a.get_string(args[0]) else {
        return a.err(
            ctx,
            "type-error",
            &format!("{}: expected string, got {}", HTML, a.type_name(args[0])),
        );
    };
    let opts = match args.get(1) {
        None => Options::empty(),
        Some(&v) => match options(ctx, v) {
            Ok(o) => o,
            Err(e) => return e,
        },
    };
    let mut out = String::with_capacity(text.len() + text.len() / 2);
    html::push_html(&mut out, Parser::new_ext(text, opts));
    a.ok(a.string(ctx, &out))
}

static PRIMITIVES: &[EllePrimDef] = &[EllePrimDef::range(
    HTML,
    prim_html,
    SIG_ERROR,
    1,
    2,
    "Render a markdown string to HTML. The optional struct turns on extensions by keyword: :tables, :strikethrough, :tasklists and :footnotes, each true or false.",
    "markdown",
    r#"(markdown/html "a ~~b~~" {:strikethrough true})"#,
)];
