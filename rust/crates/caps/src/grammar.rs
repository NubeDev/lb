//! The capability grammar: `<surface>:<resource>:<action>` with `*` (one segment) and `**`
//! (recursive tail) wildcards in the resource path (auth-caps scope).
//!
//! The risky invariant (testing §2, property/fuzz): `*` never matches across a `/`, and `**`
//! only ever matches a *trailing* run of segments. [`segments_match`] is the one place that
//! invariant lives.

use thiserror::Error;

use crate::request::{Action, Request, Surface};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("capability must have exactly three colon-delimited parts")]
    Shape,
    #[error("unknown surface")]
    Surface,
    #[error("unknown action")]
    Action,
}

/// A parsed, held capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    pub surface: Surface,
    /// Resource pattern; may contain `*` / `**` segments.
    pub resource: String,
    pub action: Action,
}

impl Capability {
    /// Parse `surface:resource:action`. The resource keeps its raw pattern (wildcards intact).
    pub fn parse(s: &str) -> Result<Self, ParseError> {
        let mut parts = s.splitn(3, ':');
        let surface = parts.next().ok_or(ParseError::Shape)?;
        let resource = parts.next().ok_or(ParseError::Shape)?;
        let action = parts.next().ok_or(ParseError::Shape)?;
        if action.contains(':') || resource.is_empty() {
            return Err(ParseError::Shape);
        }
        Ok(Self {
            surface: Surface::parse(surface).ok_or(ParseError::Surface)?,
            resource: resource.to_string(),
            action: Action::parse(action).ok_or(ParseError::Action)?,
        })
    }

    /// Does this held capability grant `req`? Surface must equal, action must be `*` or equal,
    /// and the resource pattern must match the request resource. (Workspace is NOT here — it
    /// is the precondition checked first in `check`.)
    pub fn grants(&self, req: &Request) -> bool {
        self.surface == req.surface
            && (self.action == Action::Any || self.action == req.action)
            && segments_match(&self.resource, &req.resource)
    }
}

/// Parse any of `caps`' strings and test each against `req`; true if at least one grants it.
/// Unparseable capability strings are ignored (deny-by-default): a malformed grant grants
/// nothing.
pub fn matches(held: &[String], req: &Request) -> bool {
    held.iter()
        .filter_map(|s| Capability::parse(s).ok())
        .any(|cap| cap.grants(req))
}

/// Segment-wise wildcard match. `*` matches exactly one segment; `**` matches zero or more
/// trailing segments and must be the last pattern segment. This is the fuzzed invariant.
///
/// Segments are delimited by `/` OR `.`: the mcp surface names resources `<ext>.<tool>`
/// (so `mcp:hello.*:call` matches `hello.echo`), while store/bus/secret use `/`. Both
/// delimiters split into segments so a single `*` matches exactly one segment under either
/// (auth-caps scope: the resource path meaning is surface-specific, but the wildcard rule is
/// uniform).
fn segments_match(pattern: &str, resource: &str) -> bool {
    let pat: Vec<&str> = pattern.split(['/', '.']).collect();
    let res: Vec<&str> = resource.split(['/', '.']).collect();
    walk(&pat, &res)
}

fn walk(pat: &[&str], res: &[&str]) -> bool {
    match pat.first() {
        // pattern exhausted: match iff resource is also exhausted
        None => res.is_empty(),
        Some(&"**") => {
            // `**` must be terminal; it then matches the entire remaining resource tail
            // (including empty). Anything after `**` is a malformed pattern → no match.
            pat.len() == 1
        }
        Some(&"*") => {
            // consume exactly one resource segment
            match res.split_first() {
                Some((_, rest)) => walk(&pat[1..], rest),
                None => false,
            }
        }
        Some(&literal) => match res.split_first() {
            Some((&head, rest)) if head == literal => walk(&pat[1..], rest),
            _ => false,
        },
    }
}
