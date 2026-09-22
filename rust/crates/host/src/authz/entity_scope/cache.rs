//! A short-lived cache of resolved entity scopes. A board fires dozens of federation reads per page
//! load, and each would otherwise re-resolve the caller's menu. Entries live [`TTL`], so a menu or
//! grant change reaches a live session within that window without a re-login.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::EntityScope;

pub(super) const TTL: Duration = Duration::from_secs(30);
const MAX_ENTRIES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Key(String);

impl Key {
    pub(super) fn new(ws: &str, owner: &str, table: &str, sources: &[String]) -> Self {
        Key(format!(
            "{ws}\u{1f}{owner}\u{1f}{table}\u{1f}{}",
            sources.join(",")
        ))
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
        m.insert(key, (Instant::now(), scope));
    }
}
