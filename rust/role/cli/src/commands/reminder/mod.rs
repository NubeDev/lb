//! `lb reminder …` — the reference family of the common resource grammar
//! (`core/resource-verbs-scope.md`), typed sugar over the shipped `reminder.*` MCP verbs. One verb per
//! file (FILE-LAYOUT §3): [`ls`] → `reminder.list`, [`show`] → `reminder.get`, [`create`] →
//! `reminder.create`, [`update`] → `reminder.update`, [`rm`] → `reminder.delete`. Every command funnels
//! through the SAME transport as `lb call` — no new client path, no typed REST route, zero new verbs.
//!
//! Two family-shared pieces live here (not a `utils` grab-bag — they are the family's own vocabulary):
//! [`now_ts`], the wall-clock logical timestamp every write verb requires, and [`derive_id`], the
//! body→id slug that gives `create` a friendly no-`--id` UX while still supplying the client-chosen id
//! the verb expects.

pub mod create;
pub mod ls;
pub mod rm;
pub mod show;
pub mod update;

use std::time::{SystemTime, UNIX_EPOCH};

/// The logical timestamp a write verb (`create`/`update`/`delete`) requires — seconds since the epoch
/// from the wall clock. The server computes `next_attempt_ts` from it; a client always passes "now".
pub fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Derive a friendly, reasonably-unique id from a reminder body when the operator gives no `--id`.
/// A kebab slug of the first few words plus a short suffix from the timestamp keeps ids readable
/// (`standup-time-3af`) without a random dependency, and collisions are the caller's to resolve with
/// an explicit `--id` (the verb upserts on id, so a clash would overwrite — the suffix avoids the
/// common case). The suffix is base-36 of the low bits of `now`, so two creates a second apart differ.
pub fn derive_id(body: &str, now: u64) -> String {
    let mut slug = String::new();
    for ch in body.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
        // Cap the slug so the id stays short and readable.
        if slug.trim_end_matches('-').len() >= 24 {
            break;
        }
    }
    let stem = slug.trim_matches('-');
    let stem = if stem.is_empty() { "reminder" } else { stem };
    format!("{stem}-{}", base36(now))
}

/// Lowercase base-36 of `n` — a compact, url-safe suffix (no external dependency).
fn base36(mut n: u64) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".to_string();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).expect("base36 digits are ascii")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_id_slugs_the_body_and_appends_a_suffix() {
        let id = derive_id("Standup time!", 100);
        assert!(id.starts_with("standup-time-"), "{id}");
        // The suffix disambiguates two creates at different instants.
        assert_ne!(
            derive_id("Standup time!", 100),
            derive_id("Standup time!", 200)
        );
    }

    #[test]
    fn derive_id_falls_back_when_the_body_has_no_word_chars() {
        let id = derive_id("!!!", 5);
        assert!(id.starts_with("reminder-"), "{id}");
    }

    #[test]
    fn base36_is_compact_and_lowercase() {
        assert_eq!(base36(0), "0");
        assert_eq!(base36(35), "z");
        assert_eq!(base36(36), "10");
    }
}
