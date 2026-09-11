//! `policy_set` → `policy_list` → `match_policy` over a **real** embedded store (`mem://`) — no
//! mocks, no fixtures (CLAUDE §4). Every policy here is written through the verb under test and read
//! back through the others, which is the only way to catch the class of bug the pure tests cannot:
//! a persistence shape mismatch.
//!
//! This file exists because of one. `policy_list` tolerates a row that will not decode (one bad
//! record must not blank the settings page), and `lb_store::scan` returns the whole `{ data, rev }`
//! record while `read`/`list` return the inner value already unwrapped. Decoding the wrapper
//! produced `None` for every row, the tolerance swallowed it, and the list came back **empty and
//! green** — no error anywhere. Only a write-then-read against the real store shows it.

use lb_cases::{
    match_policy, policy_list, policy_set, Calendar, DayHours, PolicyMatch, ServicePolicy,
};
use lb_store::Store;

fn policy(id: &str, m: PolicyMatch) -> ServicePolicy {
    ServicePolicy {
        id: id.into(),
        name: format!("{id} contract"),
        r#match: m,
        respond_h: 4,
        resolve_h: 24,
        calendar: Calendar::Always,
        party_window_h: 48,
        hold_down_days: 14,
    }
}

fn pinned(site: Option<&str>, category: Option<&str>, severity: Option<&str>) -> PolicyMatch {
    PolicyMatch {
        site: site.map(str::to_string),
        category: category.map(str::to_string),
        severity: severity.map(str::to_string),
    }
}

/// **The regression this file was written for.** A policy written through `policy_set` comes back
/// out of `policy_list` intact — id, name, hours, hold-down and calendar. If the envelope is not
/// unwrapped, the list is empty and everything downstream silently resolves to "no SLA".
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_policy_round_trips_through_a_real_store() {
    let store = Store::memory().await.unwrap();
    let mut p = policy("default", PolicyMatch::default());
    p.calendar = Calendar::Business {
        tz: "Australia/Brisbane".into(),
        hours: [DayHours::new(9 * 60, 17 * 60); 7],
        holidays: vec!["2026-12-25".into()],
    };
    policy_set(&store, "nube", &p).await.expect("set");

    let got = policy_list(&store, "nube").await.expect("list");
    assert_eq!(
        got.len(),
        1,
        "the row came back — the envelope is unwrapped"
    );
    assert_eq!(got[0], p, "and it round-tripped byte for byte");
}

/// `set` is an upsert keyed by id: a second write replaces the row rather than adding a second.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_upserts_by_id() {
    let store = Store::memory().await.unwrap();
    policy_set(&store, "nube", &policy("c", PolicyMatch::default()))
        .await
        .unwrap();
    let mut edited = policy("c", PolicyMatch::default());
    edited.respond_h = 1;
    edited.name = "renegotiated".into();
    policy_set(&store, "nube", &edited).await.unwrap();

    let got = policy_list(&store, "nube").await.unwrap();
    assert_eq!(got.len(), 1, "one row, not two");
    assert_eq!(got[0].respond_h, 1);
    assert_eq!(got[0].name, "renegotiated");
}

/// The workspace wall is structural: ws-B's policies are invisible to ws-A even when the two share
/// a policy id, and a write to the shared id in one workspace does not touch the other's row.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_workspace_sees_only_its_own_policies() {
    let store = Store::memory().await.unwrap();
    policy_set(&store, "ws-a", &policy("default", PolicyMatch::default()))
        .await
        .unwrap();
    policy_set(
        &store,
        "ws-a",
        &policy("extra", pinned(Some("s1"), None, None)),
    )
    .await
    .unwrap();
    // A deliberately DIFFERENT respond_h, so "ws-A's row was not overwritten" is a real assertion
    // and not two identical values agreeing by accident. (1, not 99: the write door rightly refuses
    // a respond_h past the resolve_h — it caught this fixture when it was 99.)
    let mut b = policy("default", PolicyMatch::default());
    b.respond_h = 1;
    policy_set(&store, "ws-b", &b).await.unwrap();

    let a_rows = policy_list(&store, "ws-a").await.unwrap();
    assert_eq!(
        a_rows.len(),
        2,
        "ws-A sees its two rows and nothing of ws-B"
    );
    let a_default = a_rows.iter().find(|p| p.id == "default").unwrap();
    assert_eq!(a_default.respond_h, 4, "ws-B's write did not reach ws-A");

    let b_rows = policy_list(&store, "ws-b").await.unwrap();
    assert_eq!(b_rows.len(), 1);
    assert_eq!(b_rows[0].respond_h, 1);
}

/// The full ladder, resolved through the store the reactor will use: default → category → site
/// override, with the wrong-site policy never chosen.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn match_policy_resolves_the_ladder_against_the_store() {
    let store = Store::memory().await.unwrap();
    for p in [
        policy("default", PolicyMatch::default()),
        policy("dq", pinned(None, Some("data_quality"), None)),
        policy("s1-dq", pinned(Some("s1"), Some("data_quality"), None)),
        policy("s9-dq", pinned(Some("s9"), Some("data_quality"), None)),
    ] {
        policy_set(&store, "nube", &p).await.unwrap();
    }
    async fn pick(
        store: &Store,
        site: Option<&str>,
        cat: Option<&str>,
        sev: Option<&str>,
    ) -> Option<String> {
        match_policy(store, "nube", site, cat, sev)
            .await
            .unwrap()
            .map(|p| p.id)
    }

    assert_eq!(
        pick(&store, Some("s1"), Some("data_quality"), None)
            .await
            .as_deref(),
        Some("s1-dq")
    );
    assert_eq!(
        pick(&store, Some("s2"), Some("data_quality"), None)
            .await
            .as_deref(),
        Some("dq")
    );
    assert_eq!(
        pick(&store, Some("s1"), Some("other"), None)
            .await
            .as_deref(),
        Some("default")
    );
    // s9's policy is more specific than the default and its category fits — and it is still never
    // chosen for s1, because a pinned axis that contradicts the query disqualifies outright.
    assert_eq!(
        pick(&store, Some("s1"), None, None).await.as_deref(),
        Some("default")
    );
}

/// A workspace with no policy at all resolves to `None`, not to an invented default — the reactor
/// must be able to tell "no SLA applies here" from "the 24x7 default applies".
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_empty_workspace_resolves_to_none() {
    let store = Store::memory().await.unwrap();
    assert!(policy_list(&store, "nube").await.unwrap().is_empty());
    assert!(match_policy(&store, "nube", Some("s1"), None, None)
        .await
        .unwrap()
        .is_none());
}

/// The write door refuses what the clock could not use, and refuses it *before* writing — a
/// rejected policy leaves no row behind to be picked up later.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_rejected_policy_writes_nothing() {
    let store = Store::memory().await.unwrap();

    let mut backwards = policy("backwards", PolicyMatch::default());
    backwards.respond_h = 24;
    backwards.resolve_h = 4;
    assert!(policy_set(&store, "nube", &backwards).await.is_err());

    let mut bad_tz = policy("bad-tz", PolicyMatch::default());
    bad_tz.calendar = Calendar::Business {
        tz: "Mars/Olympus".into(),
        hours: [DayHours::new(9 * 60, 17 * 60); 7],
        holidays: vec![],
    };
    assert!(policy_set(&store, "nube", &bad_tz).await.is_err());

    let mut never_open = policy("never", PolicyMatch::default());
    never_open.calendar = Calendar::Business {
        tz: "Australia/Brisbane".into(),
        hours: [DayHours::CLOSED; 7],
        holidays: vec![],
    };
    assert!(policy_set(&store, "nube", &never_open).await.is_err());

    let mut blank_id = policy("  ", PolicyMatch::default());
    blank_id.id = "  ".into();
    assert!(policy_set(&store, "nube", &blank_id).await.is_err());

    assert!(
        policy_list(&store, "nube").await.unwrap().is_empty(),
        "not one reject was stored"
    );
}

/// The list order IS the resolution ladder, asserted against rows the store handed back in its own
/// id order — so the sort is doing the work, not the store.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_list_is_ordered_most_specific_first() {
    let store = Store::memory().await.unwrap();
    for p in [
        policy("aaa-default", PolicyMatch::default()),
        policy("zzz-three", pinned(Some("s1"), Some("c"), Some("critical"))),
        policy("mmm-one", pinned(Some("s1"), None, None)),
        policy("bbb-one", pinned(Some("s2"), None, None)),
    ] {
        policy_set(&store, "nube", &p).await.unwrap();
    }
    let ids: Vec<String> = policy_list(&store, "nube")
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(ids, ["zzz-three", "bbb-one", "mmm-one", "aaa-default"]);
}
