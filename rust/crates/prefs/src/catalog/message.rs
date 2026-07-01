//! The hand-written **ICU MessageFormat 1 (MF1) subset** parser + renderer (i18n-catalogs scope:
//! "a hand-written MF1 subset parser (~200 lines, no crate)"). The subset is the *pinned closed
//! grammar* — argument, `plural` (`one`/`other` + exact `=0`/`=1`), `select` (arbitrary keywords +
//! mandatory `other`), the three typed placeholders (`date`/`number`/`quantity,<dim>`), one level of
//! nesting, the `#` count token, and `'{'`/`'}'` literal escapes. Anything outside is a **parse
//! error** — caught by the build-time catalog-lint test, never a silent mis-parse.
//!
//! Why hand-written: no Rust crate matches `intl-messageformat`'s MF1 dialect (icu4x is MF2-leaning,
//! fluent is FTL). Owning the closed subset means the Rust host and the TS client parse **only
//! constructs both implement** — the host==client guarantee the whole one-source design rests on
//! (asserted by the placeholder-parity + intl-messageformat cross-check tests).
//!
//! Pure: no I/O, no clock, no store. `parse` → AST; `render(ast, args, resolved)` → String. The
//! selection (plural category, select keyword) uses [`super::plural`]; placeholders dispatch to
//! [`super::interpolate`] (which routes to the shipped `format::*`). Both the count `#` and a plural
//! arm's number render via `format::number`, so a message and a direct `format.*` call agree.

use std::str::CharIndices;

use serde_json::Value;

use crate::axis::Dimension;
use crate::prefs::ResolvedPrefs;

use super::interpolate::{render_count, render_placeholder};
use super::plural::{category, PluralCategory};

/// A parsed MF1 message — a flat sequence of nodes. Rendering walks it in order.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    nodes: Vec<Node>,
}

/// One node of a parsed message.
#[derive(Debug, Clone, PartialEq)]
enum Node {
    /// Literal text (escapes already resolved).
    Text(String),
    /// The `#` count token — the enclosing plural's number, rendered via `format::number`.
    Count,
    /// A typed/bare placeholder dispatched to `format::*`.
    Simple(Placeholder),
    /// `{arg, plural, =0{…} one{…} other{…}}` — exact-value arms first, then categories.
    Plural { arg: String, arms: Vec<PluralArm> },
    /// `{arg, select, keyword{…} other{…}}` — arbitrary keywords, `other` mandatory.
    Select { arg: String, arms: Vec<SelectArm> },
}

/// A simple (non-branching) placeholder — the leaves that route to `format::*`.
#[derive(Debug, Clone, PartialEq)]
pub enum Placeholder {
    /// `{name}` — bare argument, stringified.
    Arg(String),
    /// `{name, date}` → `format::datetime`.
    Date(String),
    /// `{name, number}` → `format::number`.
    Number(String),
    /// `{name, quantity, <dim>}` → `format::quantity` from the dimension's canonical unit.
    Quantity(String, Dimension),
}

#[derive(Debug, Clone, PartialEq)]
enum PluralSelector {
    /// An exact-value arm `=0` / `=1` — matched before category arms.
    Exact(i64),
    /// A category keyword arm `one` / `other`.
    Category(PluralCategory),
}

#[derive(Debug, Clone, PartialEq)]
struct PluralArm {
    selector: PluralSelector,
    body: Message,
}

#[derive(Debug, Clone, PartialEq)]
struct SelectArm {
    keyword: String,
    body: Message,
}

/// A parse error — the authored message is outside the pinned subset. Surfaced by the catalog-lint
/// test at build time so an out-of-subset message never reaches render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MF1 parse error: {}", self.0)
    }
}
impl std::error::Error for ParseError {}

/// Parse an MF1 message from `src`. Rejects anything outside the pinned subset with a [`ParseError`]
/// (the catalog-lint contract). `nested` bounds nesting to one level: a `plural`/`select` arm may
/// contain placeholders and ONE nested `plural`/`select`, no deeper.
pub fn parse(src: &str) -> Result<Message, ParseError> {
    let mut p = Parser::new(src);
    let msg = p.parse_message(false, 0)?;
    if p.peek().is_some() {
        return Err(ParseError(format!("unexpected `}}` at byte {}", p.pos)));
    }
    Ok(msg)
}

/// Render a parsed message against `args` (a flat JSON object) and the recipient's `resolved` prefs.
/// Never fails — a bad placeholder yields `[<arg>]` (the failure contract lives in `interpolate`).
pub fn render(msg: &Message, args: &Value, resolved: &ResolvedPrefs) -> String {
    render_message(msg, args, resolved, None)
}

/// Render with an active plural `count` in scope (so `#` and category selection see the number).
fn render_message(
    msg: &Message,
    args: &Value,
    resolved: &ResolvedPrefs,
    count: Option<i64>,
) -> String {
    let mut out = String::new();
    for node in &msg.nodes {
        match node {
            Node::Text(t) => out.push_str(t),
            Node::Count => match count {
                Some(n) => out.push_str(&render_count(n, resolved)),
                None => out.push('#'), // a stray `#` outside a plural is a literal (MF1 behavior).
            },
            Node::Simple(ph) => out.push_str(&render_placeholder(ph, args, resolved)),
            Node::Plural { arg, arms } => {
                let n = arg_i64(args, arg);
                let arm = select_plural(arms, &resolved.language, n);
                out.push_str(&render_message(arm, args, resolved, Some(n)));
            }
            Node::Select { arg, arms } => {
                let kw = arg_str(args, arg);
                let arm = select_select(arms, &kw);
                out.push_str(&render_message(arm, args, resolved, count));
            }
        }
    }
    out
}

/// The plural count for `arg` — a missing/non-integer value falls to 0 (→ the `other` arm), never a
/// panic (the never-blank contract extends to selection).
fn arg_i64(args: &Value, arg: &str) -> i64 {
    args.as_object()
        .and_then(|m| m.get(arg))
        .and_then(Value::as_i64)
        .unwrap_or(0)
}

/// The select keyword for `arg` — a missing/non-string value is the empty string (→ the `other`
/// arm), never a panic.
fn arg_str(args: &Value, arg: &str) -> String {
    args.as_object()
        .and_then(|m| m.get(arg))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// Pick the plural arm for `n`: exact-value arms first (`=0`/`=1`), then the CLDR category, then the
/// mandatory `other` (guaranteed present by the parser).
fn select_plural<'a>(arms: &'a [PluralArm], lang: &str, n: i64) -> &'a Message {
    for arm in arms {
        if let PluralSelector::Exact(v) = arm.selector {
            if v == n {
                return &arm.body;
            }
        }
    }
    let cat = category(lang, n);
    for arm in arms {
        if arm.selector == PluralSelector::Category(cat) {
            return &arm.body;
        }
    }
    // `other` is mandatory (parser-enforced); fall to it.
    other_plural(arms)
}

fn other_plural(arms: &[PluralArm]) -> &Message {
    arms.iter()
        .find(|a| a.selector == PluralSelector::Category(PluralCategory::Other))
        .map(|a| &a.body)
        .expect("parser guarantees a plural `other` arm")
}

/// Pick the select arm for `kw`, falling to the mandatory `other`.
fn select_select<'a>(arms: &'a [SelectArm], kw: &str) -> &'a Message {
    arms.iter()
        .find(|a| a.keyword == kw)
        .or_else(|| arms.iter().find(|a| a.keyword == "other"))
        .map(|a| &a.body)
        .expect("parser guarantees a select `other` arm")
}

// --- the parser (one concern: MF1 subset text → AST, rejecting out-of-subset) ---

struct Parser<'a> {
    chars: std::iter::Peekable<CharIndices<'a>>,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Parser {
            chars: src.char_indices().peekable(),
            pos: 0,
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().map(|&(_, c)| c)
    }

    fn bump(&mut self) -> Option<char> {
        match self.chars.next() {
            Some((i, c)) => {
                self.pos = i + c.len_utf8();
                Some(c)
            }
            None => None,
        }
    }

    /// Parse text + placeholders until a top-level `}` (if `in_arm`) or EOF. `depth` bounds nesting
    /// to one level (an arm body may hold ONE nested plural/select; a body at depth 1 rejects a
    /// further branching placeholder).
    fn parse_message(&mut self, in_arm: bool, depth: u32) -> Result<Message, ParseError> {
        let mut nodes = Vec::new();
        let mut text = String::new();
        loop {
            match self.peek() {
                None => break,
                Some('}') if in_arm => break,
                Some('}') => break, // top-level stray `}` — caught by the caller.
                Some('\'') => {
                    // Escape: `'{'`, `'}'`, or `''` → a literal quote. Only these are supported.
                    self.bump();
                    match self.bump() {
                        Some(c @ ('{' | '}' | '\'')) => {
                            text.push(c);
                            // A trailing closing quote is optional in MF1 for a single escaped char;
                            // accept an immediate `'` as the terminator if present.
                            if self.peek() == Some('\'') {
                                self.bump();
                            }
                        }
                        other => {
                            return Err(ParseError(format!(
                            "unsupported quote escape (only '{{', '}}', '' allowed), got {other:?}"
                        )))
                        }
                    }
                }
                Some('#') => {
                    self.bump();
                    flush(&mut nodes, &mut text);
                    nodes.push(Node::Count);
                }
                Some('{') => {
                    flush(&mut nodes, &mut text);
                    let node = self.parse_placeholder(depth)?;
                    nodes.push(node);
                }
                Some(c) => {
                    self.bump();
                    text.push(c);
                }
            }
        }
        flush(&mut nodes, &mut text);
        Ok(Message { nodes })
    }

    /// Parse a `{ … }` placeholder. `depth` is the current nesting level.
    fn parse_placeholder(&mut self, depth: u32) -> Result<Node, ParseError> {
        self.expect('{')?;
        self.skip_ws();
        let arg = self.parse_ident()?;
        self.skip_ws();
        match self.peek() {
            Some('}') => {
                self.bump();
                Ok(Node::Simple(Placeholder::Arg(arg)))
            }
            Some(',') => {
                self.bump();
                self.skip_ws();
                let fmt = self.parse_ident()?;
                self.skip_ws();
                self.parse_fmt(arg, &fmt, depth)
            }
            other => Err(ParseError(format!(
                "expected `,` or `}}` after arg `{arg}`, got {other:?}"
            ))),
        }
    }

    fn parse_fmt(&mut self, arg: String, fmt: &str, depth: u32) -> Result<Node, ParseError> {
        match fmt {
            "date" => {
                self.expect('}')?;
                Ok(Node::Simple(Placeholder::Date(arg)))
            }
            "number" => {
                self.expect('}')?;
                Ok(Node::Simple(Placeholder::Number(arg)))
            }
            "quantity" => {
                self.expect(',')?;
                self.skip_ws();
                let dim_token = self.parse_ident()?;
                self.skip_ws();
                self.expect('}')?;
                let dim = parse_dimension(&dim_token)?;
                Ok(Node::Simple(Placeholder::Quantity(arg, dim)))
            }
            "plural" => {
                reject_deep_nest(depth, "plural")?;
                self.expect(',')?;
                let arms = self.parse_plural_body(depth)?;
                self.expect('}')?;
                ensure_plural_other(&arms)?;
                Ok(Node::Plural { arg, arms })
            }
            "select" => {
                reject_deep_nest(depth, "select")?;
                self.expect(',')?;
                let arms = self.parse_select_body(depth)?;
                self.expect('}')?;
                ensure_select_other(&arms)?;
                Ok(Node::Select { arg, arms })
            }
            other => Err(ParseError(format!(
                "unsupported format `{other}` (subset: date, number, quantity, plural, select)"
            ))),
        }
    }

    fn parse_plural_body(&mut self, depth: u32) -> Result<Vec<PluralArm>, ParseError> {
        let mut arms = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some('}') | None => break,
                _ => {}
            }
            let selector = self.parse_plural_selector()?;
            self.skip_ws();
            let body = self.parse_arm_body(depth)?;
            arms.push(PluralArm { selector, body });
        }
        Ok(arms)
    }

    fn parse_plural_selector(&mut self) -> Result<PluralSelector, ParseError> {
        if self.peek() == Some('=') {
            self.bump();
            let digits = self.parse_ident_raw(|c| c.is_ascii_digit())?;
            let v: i64 = digits
                .parse()
                .map_err(|_| ParseError(format!("bad exact plural value `={digits}`")))?;
            return Ok(PluralSelector::Exact(v));
        }
        let kw = self.parse_ident()?;
        match kw.as_str() {
            "one" => Ok(PluralSelector::Category(PluralCategory::One)),
            "other" => Ok(PluralSelector::Category(PluralCategory::Other)),
            other => Err(ParseError(format!(
                "unsupported plural category `{other}` (subset: one, other, =N)"
            ))),
        }
    }

    fn parse_select_body(&mut self, depth: u32) -> Result<Vec<SelectArm>, ParseError> {
        let mut arms = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some('}') | None => break,
                _ => {}
            }
            let keyword = self.parse_ident()?;
            self.skip_ws();
            let body = self.parse_arm_body(depth)?;
            arms.push(SelectArm { keyword, body });
        }
        Ok(arms)
    }

    /// Parse one `{ … }` arm body at `depth + 1`. The depth increment is what lets
    /// [`reject_deep_nest`] refuse a plural/select nested more than one level deep (the subset's "one
    /// level of nesting" rule) — a branching placeholder at depth ≥ 2 is a lint error.
    fn parse_arm_body(&mut self, depth: u32) -> Result<Message, ParseError> {
        self.expect('{')?;
        let body = self.parse_message(true, depth + 1)?;
        self.expect('}')?;
        Ok(body)
    }

    fn parse_ident(&mut self) -> Result<String, ParseError> {
        self.parse_ident_raw(|c| c.is_alphanumeric() || c == '_' || c == '.')
    }

    fn parse_ident_raw(&mut self, ok: impl Fn(char) -> bool) -> Result<String, ParseError> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if ok(c) {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        if s.is_empty() {
            return Err(ParseError(format!(
                "expected identifier at byte {}",
                self.pos
            )));
        }
        Ok(s)
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.bump();
            } else {
                break;
            }
        }
    }

    fn expect(&mut self, c: char) -> Result<(), ParseError> {
        match self.bump() {
            Some(got) if got == c => Ok(()),
            other => Err(ParseError(format!(
                "expected `{c}` at byte {}, got {other:?}",
                self.pos
            ))),
        }
    }
}

/// The one-level-nesting guard. A top-level branching placeholder is at depth 0; its arm bodies
/// parse at depth 1, so a nested plural/select is at depth 1 (allowed — one level). A body inside
/// *that* is at depth 2, so a plural/select at depth ≥ 2 is deeper than one nest → a lint error.
fn reject_deep_nest(depth: u32, kind: &str) -> Result<(), ParseError> {
    if depth >= 2 {
        Err(ParseError(format!(
            "`{kind}` nested deeper than one level (subset allows one nest)"
        )))
    } else {
        Ok(())
    }
}

fn parse_dimension(token: &str) -> Result<Dimension, ParseError> {
    Dimension::ALL
        .iter()
        .copied()
        .find(|d| d.as_str() == token)
        .ok_or_else(|| ParseError(format!("unknown quantity dimension `{token}`")))
}

fn flush(nodes: &mut Vec<Node>, text: &mut String) {
    if !text.is_empty() {
        nodes.push(Node::Text(std::mem::take(text)));
    }
}

fn ensure_plural_other(arms: &[PluralArm]) -> Result<(), ParseError> {
    if arms
        .iter()
        .any(|a| a.selector == PluralSelector::Category(PluralCategory::Other))
    {
        Ok(())
    } else {
        Err(ParseError("plural requires an `other` arm".into()))
    }
}

fn ensure_select_other(arms: &[SelectArm]) -> Result<(), ParseError> {
    if arms.iter().any(|a| a.keyword == "other") {
        Ok(())
    } else {
        Err(ParseError("select requires an `other` arm".into()))
    }
}
