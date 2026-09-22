//! A short-lived cache of resolved entity scopes. A board fires dozens of federation reads per page
//! load, and each would otherwise re-resolve the caller's menus. Entries live [`TTL`] and are
//! dropped for the whole workspace on any access-changing write (`invalidate_ws`), so a menu or
//! grant change reaches a live session at once, and a session that saw no write within [`TTL`].

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::EntityScope;

pub(super) const TTL: Duration = Duration::from_secs(30);
const MAX_ENTRIES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Key {
    ws: String,
    rest: String,
}

impl Key {
    pub(super) fn new(ws: &str, owner: &str, table: &str, sources: &[String]) -> Self {
        let mut sources: Vec<&str> = sources.iter().map(String::as_str).collect();
        sources.sort_unstable();
        sources.dedup();
        Key {
            ws: ws.to_string(),
            rest: format!("{owner}\u{1f}{table}\u{1f}{}", sources.join(",")),
        }
    }
}

fn map() -> &'static Mutex<HashMap<Key, (Instant, EntityScope)>> {
    static MAP: OnceLock<Mutex<HashMap<Key, (Instant, EntityScope)>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn get(key: &Key) -> Option<EntityScope> {
    let m = map().lock().ok()?;
    m.get(key)
        .filter(|(at, _)| at.elapsed() < TTL)
        .map(|(_, scope)| scope.clone())
}

pub(super) fn put(key: Key, scope: EntityScope) {
    if let Ok(mut m) = map().lock() {
        if m.len() >= MAX_ENTRIES {
            m.retain(|_, (at, _)| at.elapsed() < TTL);
        }
        if m.len() >= MAX_ENTRIES {
            // Still full of live entries: a read that is not cached is only slower, never wrong.
            return;
        }
        m.insert(key, (Instant::now(), scope));
    }
}

pub(super) fn invalidate_ws(ws: &str) {
    if let Ok(mut m) = map().lock() {
        m.retain(|k, _| k.ws != ws);
    }
}
