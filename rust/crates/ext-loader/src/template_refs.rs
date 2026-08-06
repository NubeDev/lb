//! Reference-name extraction for the ONE template grammar the platform agrees on
//! (nav-context-builtins scope; `widget-config-vars-scope.md` owns the grammar itself).
//!
//! The host **expands nothing** — there is no templating engine in Rust and this module is not one.
//! It only answers "which names does this string reference?", so `validate_nav` (and the nav-builder
//! write path) can refuse a template naming something the item could never bind. Interpolation stays
//! client-side, exactly as `dashboard-variables-advanced-scope.md:95` pins it.
//!
//! The grammar, verbatim from the shipped `parse.ts`:
//!   - `$name`, `${name}` and `[[name]]`
//!   - a name is `[A-Za-z_][\w.]*` (dotted, because `${__user.login}` needed it)
//!   - an optional `:formathint` suffix inside the braced/bracketed forms (`${site:raw}`)
//!   - a `__`-prefixed name is a **built-in** (`isBuiltinName`), supplied by the client's `VarScope`
//!     rather than by the item — so it is bindable everywhere, by classification not by a list. A
//!     closed allow-list here would reject a built-in the client already resolves the moment the
//!     namespace grows, which is the opposite of what the frozen grammar promises.
//!
//! There is deliberately **no escape sequence** (`$$` is deferred to the scope that owns the engine),
//! so a literal `$` in shipped text reads as a reference here. That is precisely why `label` warns
//! rather than rejects — see `validate_nav`.

/// Every reference name in `template`, in first-seen order (duplicates collapsed).
pub fn reference_names(template: &str) -> Vec<String> {
    let chars: Vec<char> = template.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let hit = if chars[i] == '$' {
            if chars.get(i + 1) == Some(&'{') {
                delimited(&chars, i + 2, &['}'])
            } else {
                bare(&chars, i + 1)
            }
        } else if chars[i] == '[' && chars.get(i + 1) == Some(&'[') {
            delimited(&chars, i + 2, &[']', ']'])
        } else {
            None
        };
        match hit {
            Some((name, next)) => {
                if !out.iter().any(|n| n == &name) {
                    out.push(name);
                }
                i = next;
            }
            None => i += 1,
        }
    }
    out
}

/// Is `name` a **built-in** — supplied by the client's `VarScope`, not bound by the item? Classified
/// by the `__` prefix, exactly as `parse.ts:isBuiltinName` does.
pub fn is_builtin(name: &str) -> bool {
    name.starts_with("__")
}

/// The first reference in `template` that the item cannot bind: not a built-in and not one of
/// `bindable` (its own `vars` keys plus, for a `template-group`, the declared `var`). `None` when
/// every reference resolves — the accept case.
pub fn first_unbindable(template: &str, bindable: &[&str]) -> Option<String> {
    reference_names(template)
        .into_iter()
        .find(|n| !is_builtin(n) && !bindable.iter().any(|b| b == n))
}

/// The first `__nav.*` reference in `template` (the namespace name itself, or any member). Used to
/// refuse a **self-referential** nav label: `__nav.label` is computed FROM the resolved label, so a
/// label that reads it is a cycle (nav-context-builtins scope, "Risks").
pub fn first_nav_builtin(template: &str) -> Option<String> {
    reference_names(template)
        .into_iter()
        .find(|n| n == "__nav" || n.starts_with("__nav."))
}

/// Read a name at `start` — `[A-Za-z_][\w.]*` with any trailing `.` trimmed (a sentence-ending dot
/// after a bare `$ref` belongs to the prose, not the name). Returns the name + the index after it.
fn bare(chars: &[char], start: usize) -> Option<(String, usize)> {
    let first = *chars.get(start)?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let mut end = start + 1;
    while end < chars.len()
        && (chars[end].is_alphanumeric() || chars[end] == '_' || chars[end] == '.')
    {
        end += 1;
    }
    let mut name: String = chars[start..end].iter().collect();
    while name.ends_with('.') {
        name.pop();
    }
    Some((name, end))
}

/// Read a `${name}` / `${name:hint}` / `[[name]]` / `[[name:hint]]` body starting at `start` (just
/// past the opener), requiring `close`. Returns the name + the index after the closer; `None` when
/// the form is unterminated or the body is not a name (then it is literal text, not a reference).
fn delimited(chars: &[char], start: usize, close: &[char]) -> Option<(String, usize)> {
    let (name, mut i) = bare(chars, start)?;
    if chars.get(i) == Some(&':') {
        // A `:formathint` — skipped whole; the hint never names a variable.
        while i < chars.len() && !closes_at(chars, i, close) {
            i += 1;
        }
    }
    if closes_at(chars, i, close) {
        Some((name, i + close.len()))
    } else {
        None
    }
}

fn closes_at(chars: &[char], i: usize, close: &[char]) -> bool {
    close
        .iter()
        .enumerate()
        .all(|(k, c)| chars.get(i + k) == Some(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_all_three_forms_with_hints() {
        assert_eq!(reference_names("$device"), vec!["device"]);
        assert_eq!(reference_names("${device}"), vec!["device"]);
        assert_eq!(reference_names("[[device]]"), vec!["device"]);
        assert_eq!(reference_names("${device:raw}"), vec!["device"]);
        assert_eq!(reference_names("[[device:csv]]"), vec!["device"]);
    }

    #[test]
    fn dotted_names_and_builtins() {
        assert_eq!(
            reference_names("${__nav.parent.label} · ${__nav.label}"),
            vec!["__nav.parent.label", "__nav.label"]
        );
        assert!(is_builtin("__page.ext"));
        assert!(!is_builtin("device"));
        assert_eq!(first_nav_builtin("${__page.title}"), None);
        assert_eq!(
            first_nav_builtin("${__nav.parent.label}").as_deref(),
            Some("__nav.parent.label")
        );
    }

    #[test]
    fn trailing_dot_is_prose_not_name() {
        assert_eq!(reference_names("Opened $device."), vec!["device"]);
    }

    #[test]
    fn non_references_are_literal() {
        // No name after the `$`, an unterminated brace, and a digit-led name are all literal text.
        assert!(reference_names("100% $ ").is_empty());
        assert!(reference_names("${unterminated").is_empty());
        assert!(reference_names("$1000").is_empty());
        assert!(reference_names("[[ nope ]]").is_empty());
    }

    #[test]
    fn a_literal_dollar_word_reads_as_a_reference() {
        // The retroactivity hazard, pinned: there is no escape in the grammar, so `Cost $USD` names
        // `USD`. This is WHY `label` warns instead of rejecting.
        assert_eq!(reference_names("Cost $USD"), vec!["USD"]);
    }

    #[test]
    fn first_unbindable_finds_the_offender_and_accepts_bound_names() {
        assert_eq!(
            first_unbindable("${network} / ${device}", &["network"]).as_deref(),
            Some("device")
        );
        assert_eq!(
            first_unbindable("${network} — ${__nav.label}", &["network"]),
            None
        );
        assert_eq!(first_unbindable("plain text", &[]), None);
    }

    #[test]
    fn duplicates_collapse_in_first_seen_order() {
        assert_eq!(
            reference_names("$a ${b} $a [[c]] ${b}"),
            vec!["a", "b", "c"]
        );
    }
}
