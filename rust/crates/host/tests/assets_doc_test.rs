//! The S4 doc-sharing exit gate, headless: a doc private to a user can be **shared to a team**
//! and read by a team member, and **linked into a channel** and read by a channel `sub`-grantee
//! — and a **non-member is DENIED** (the mandatory capability/membership deny, testing §2.1).
//!
//! Each test uses a unique workspace id (the isolation wall, §7) and the multi-thread flavor —
//! though no Node/bus is booted here (the asset verbs are pure store), the convention is kept so
//! the suite is uniform and a future bus-touching assertion needs no attribute change.

use lb_assets::Skill;
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{add_member, get_doc, link_doc, put_doc, share_doc, AssetError};
use lb_store::Store;

/// A principal `sub` in workspace `ws` holding `caps`.
fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const READ: &str = "store:doc/*:read";
const WRITE: &str = "store:doc/*:write";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn owner_can_read_their_private_doc() {
    let ws = "ws-doc-owner";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[READ, WRITE]);

    put_doc(&store, &ada, ws, "scope-x", "Scope X", "draft", 1)
        .await
        .unwrap();

    let got = get_doc(&store, &ada, ws, "scope-x").await.unwrap();
    assert_eq!(got.content, "draft");
    assert_eq!(got.owner, "user:ada");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn shared_to_a_team_a_member_reads_a_non_member_is_denied() {
    let ws = "ws-doc-share";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[READ, WRITE]); // owner + admin (write `*`)
    let ben = principal("user:ben", ws, &[READ]); // team member, read only
    let cleo = principal("user:cleo", ws, &[READ]); // NOT in the team

    // Ada writes a private doc and shares it to team:engineering; Ben is a member.
    put_doc(&store, &ada, ws, "scope-x", "Scope X", "draft", 1)
        .await
        .unwrap();
    add_member(&store, &ada, ws, "team:engineering", "user:ben")
        .await
        .unwrap();
    share_doc(&store, &ada, ws, "scope-x", "team:engineering")
        .await
        .unwrap();

    // Ben (member) reads it.
    assert_eq!(
        get_doc(&store, &ben, ws, "scope-x").await.unwrap().content,
        "draft"
    );

    // Cleo (non-member, holds the read cap) is DENIED — the gate-3 membership deny.
    let err = get_doc(&store, &cleo, ws, "scope-x").await.unwrap_err();
    assert!(
        matches!(err, AssetError::Denied),
        "non-member must be denied"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn linked_into_a_channel_a_sub_grantee_reads() {
    let ws = "ws-doc-link";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[READ, WRITE]);
    // Dot is not in any team, but may `sub` the channel the doc is linked into.
    let dot = principal("user:dot", ws, &[READ, "bus:chan/eng-general:sub"]);

    put_doc(&store, &ada, ws, "scope-x", "Scope X", "draft", 1)
        .await
        .unwrap();
    link_doc(&store, &ada, ws, "scope-x", "eng-general")
        .await
        .unwrap();

    // Dot reads via the channel-link path (no team membership needed).
    assert_eq!(
        get_doc(&store, &dot, ws, "scope-x").await.unwrap().content,
        "draft"
    );

    // A principal who can't sub the channel and isn't a member is still denied.
    let eve = principal("user:eve", ws, &[READ]);
    assert!(matches!(
        get_doc(&store, &eve, ws, "scope-x").await.unwrap_err(),
        AssetError::Denied
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn without_the_read_cap_even_the_owner_is_denied() {
    // The capability gate (gate 2) is independent of ownership: no `store:doc/*:read` → denied
    // before membership is even consulted.
    let ws = "ws-doc-nocap";
    let store = Store::memory().await.unwrap();
    let writer = principal("user:ada", ws, &[WRITE]); // can write, cannot read
    put_doc(&store, &writer, ws, "scope-x", "T", "draft", 1)
        .await
        .unwrap();
    assert!(matches!(
        get_doc(&store, &writer, ws, "scope-x").await.unwrap_err(),
        AssetError::Denied
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_non_owner_cannot_share_someone_elses_doc() {
    // A wildcard write cap does not let you re-share another user's doc — sharing is an owner act.
    let ws = "ws-doc-share-owner";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[READ, WRITE]);
    let mallory = principal("user:mallory", ws, &[READ, WRITE]);

    put_doc(&store, &ada, ws, "scope-x", "T", "draft", 1)
        .await
        .unwrap();
    // Mallory holds write `*` but does not own scope-x.
    assert!(matches!(
        share_doc(&store, &mallory, ws, "scope-x", "team:evil")
            .await
            .unwrap_err(),
        AssetError::Denied
    ));
}

// A small touch of the skill type so the import is exercised in this gate-focused file; the
// full skill flow lives in assets_skill_test.rs.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn skill_model_round_trips() {
    let s = Skill::new("s", "1.0.0", "user:ada", "d", "body", 1);
    assert_eq!(s.skill_key, "s");
}
