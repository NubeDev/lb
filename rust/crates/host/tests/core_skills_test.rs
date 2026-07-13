//! Core-skills scope — the two-tier catalog enforcement, headless (host verbs over a real
//! `mem://` store; rule 9 — no fakes). Covers the mandatory deny/isolation categories plus the
//! decided deprecate/un-hide + tier behavior.
//!
//! Mandatory (testing §2):
//!   - **deny**: `put_skill("core.*")` rejected even for an admin holding `store:skill/*:write`;
//!     `deprecate_skill` without the write cap denied; an **ungranted** core skill fails `load_skill`
//!     exactly like an ungranted user skill (no core bypass); a caller without `store:skill/*:read`
//!     gets an EMPTY catalog + denied loads.
//!   - **isolation**: ws-B granting `core.x` is invisible in ws-A; a core skill loads in ws-B only
//!     because ws-B granted it — the grant relation is workspace-scoped for the core tier too.
//!
//! Decided: deprecate hides from list/latest but a pinned load still resolves; a new version un-hides.

use std::sync::{Arc, Mutex};

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    deprecate_skill, grant_skill, invoke, list_granted_skills, load_skill, put_skill,
    resolve_default_core_skills, seed_core_skills, workspace_create, AllowedTool, AssetError,
    Invocation, Node, SkillTier, DEFAULT_CORE_SKILLS,
};
use lb_role_ai_gateway::{AiGateway, AiRequest, AiResponse, Provider};
use lb_store::Store;

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

// Core skill ids contain a `.` (`core.lb-cli`), and the caps grammar splits a resource on BOTH `/`
// and `.` — so a single `*` matches only one segment. A grant that must span dotted ids uses the
// recursive-tail `**` (auth-caps grammar). `store:skill/*` still covers a flat user id like
// `acme-runbook`; `store:skill/**` additionally covers `skill/core/lb-cli`.
const READ: &str = "store:skill/**:read";
const WRITE: &str = "store:skill/**:write";

// ── The `core.*` namespace is reserved: rejected even for an admin ───────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn put_skill_on_a_core_id_is_rejected_even_for_an_admin() {
    let ws = "ws-core-reserved";
    let store = Store::memory().await.unwrap();
    // A workspace admin holding the write cap on EVERY skill.
    let admin = principal("user:admin", ws, &[READ, WRITE]);

    // A `core.*` id is rejected regardless of caps — core skills change only by shipping a node build.
    let err = put_skill(
        &store,
        &admin,
        ws,
        "core.lb-cli",
        "9.9.9",
        "hijack",
        "malicious body",
        1,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, AssetError::Reserved), "got {err:?}");

    // A user-tier id under the same admin still works (the reservation is only the `core.` prefix).
    put_skill(&store, &admin, ws, "acme-runbook", "1.0.0", "d", "b", 1)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn deprecate_on_a_core_id_is_rejected_even_for_an_admin() {
    let ws = "ws-core-deprecate-reserved";
    let store = Store::memory().await.unwrap();
    let admin = principal("user:admin", ws, &[READ, WRITE]);
    let err = deprecate_skill(&store, &admin, ws, "core.query")
        .await
        .unwrap_err();
    assert!(matches!(err, AssetError::Reserved), "got {err:?}");
}

// ── deprecate requires the write cap ─────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn deprecate_without_the_write_cap_is_denied() {
    let ws = "ws-deprecate-nocap";
    let store = Store::memory().await.unwrap();
    let author = principal("user:ada", ws, &[WRITE]);
    let reader = principal("user:bob", ws, &[READ]); // read only

    put_skill(&store, &author, ws, "acme-runbook", "1.0.0", "d", "b", 1)
        .await
        .unwrap();

    let err = deprecate_skill(&store, &reader, ws, "acme-runbook")
        .await
        .unwrap_err();
    assert!(matches!(err, AssetError::Denied), "got {err:?}");
}

// ── an ungranted CORE skill is denied exactly like an ungranted user skill (no core bypass) ──────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_ungranted_core_skill_is_denied_like_a_user_skill() {
    let ws = "ws-core-ungranted";
    let store = Store::memory().await.unwrap();
    seed_core_skills(&store, "0.1.0", 1).await.unwrap();
    let agent = principal("key:agent", ws, &[READ]); // holds the read cap, no grant

    // Present on the node, read cap held — but NOT granted → denied (the headline deny).
    assert!(matches!(
        load_skill(&store, &agent, ws, "core.lb-cli", None)
            .await
            .unwrap_err(),
        AssetError::Denied
    ));

    // Grant it → now it loads from the reserved namespace, through the SAME grant gate.
    let admin = principal("user:admin", ws, &[WRITE]);
    grant_skill(&store, &admin, ws, "core.lb-cli")
        .await
        .unwrap();
    let s = load_skill(&store, &agent, ws, "core.lb-cli", None)
        .await
        .unwrap();
    assert_eq!(s.id, "core.lb-cli");
    assert!(!s.body.is_empty(), "the seeded core body loads");
}

// ── empty catalog + denied loads when the caller lacks the read cap ──────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn without_read_cap_the_catalog_is_empty_and_loads_deny() {
    let ws = "ws-core-nocap-catalog";
    let store = Store::memory().await.unwrap();
    seed_core_skills(&store, "0.1.0", 1).await.unwrap();
    let admin = principal("user:admin", ws, &[WRITE]);
    grant_skill(&store, &admin, ws, "core.lb-cli")
        .await
        .unwrap();

    let nobody = principal("key:nobody", ws, &[]); // no read cap at all

    // Catalog: no read cap → denied → the caller sees an EMPTY catalog (list_granted_skills refuses).
    assert!(matches!(
        list_granted_skills(&store, &nobody, ws).await.unwrap_err(),
        AssetError::Denied
    ));
    // And any load denies too — the agent is exactly as smart as the caller is allowed.
    assert!(matches!(
        load_skill(&store, &nobody, ws, "core.lb-cli", None)
            .await
            .unwrap_err(),
        AssetError::Denied
    ));
}

// ── the granted catalog carries tier + description (list_skills additive rows) ───────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_catalog_carries_core_and_user_tiers() {
    let ws = "ws-core-catalog-tier";
    let store = Store::memory().await.unwrap();
    seed_core_skills(&store, "0.1.0", 1).await.unwrap();
    let admin = principal("user:admin", ws, &[READ, WRITE]);

    // A user skill + a core skill, both granted.
    put_skill(
        &store,
        &admin,
        ws,
        "acme-runbook",
        "1.0.0",
        "the runbook",
        "b",
        1,
    )
    .await
    .unwrap();
    grant_skill(&store, &admin, ws, "acme-runbook")
        .await
        .unwrap();
    grant_skill(&store, &admin, ws, "core.lb-cli")
        .await
        .unwrap();

    let catalog = list_granted_skills(&store, &admin, ws).await.unwrap();
    let core = catalog.iter().find(|e| e.id == "core.lb-cli").unwrap();
    assert_eq!(core.tier, SkillTier::Core);
    assert!(!core.description.is_empty());
    assert_eq!(core.latest, "0.1.0");
    let user = catalog.iter().find(|e| e.id == "acme-runbook").unwrap();
    assert_eq!(user.tier, SkillTier::User);
    assert_eq!(user.description, "the runbook");
}

// ── deprecate: hidden from list/latest, pinned load still works, a new version un-hides ──────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn deprecate_hides_from_catalog_and_latest_but_pinned_still_loads_and_republish_unhides() {
    let ws = "ws-deprecate";
    let store = Store::memory().await.unwrap();
    let author = principal("user:ada", ws, &[READ, WRITE]);

    put_skill(&store, &author, ws, "acme-runbook", "1.0.0", "d", "v1", 1)
        .await
        .unwrap();
    grant_skill(&store, &author, ws, "acme-runbook")
        .await
        .unwrap();

    // Baseline: in the catalog, latest loads.
    assert!(list_granted_skills(&store, &author, ws)
        .await
        .unwrap()
        .iter()
        .any(|e| e.id == "acme-runbook"));
    assert_eq!(
        load_skill(&store, &author, ws, "acme-runbook", None)
            .await
            .unwrap()
            .body,
        "v1"
    );

    // Deprecate → gone from the catalog and from LATEST resolution…
    deprecate_skill(&store, &author, ws, "acme-runbook")
        .await
        .unwrap();
    assert!(!list_granted_skills(&store, &author, ws)
        .await
        .unwrap()
        .iter()
        .any(|e| e.id == "acme-runbook"));
    assert!(matches!(
        load_skill(&store, &author, ws, "acme-runbook", None)
            .await
            .unwrap_err(),
        AssetError::NotFound
    ));
    // …but a PINNED load still resolves (rollback / audit preserved).
    assert_eq!(
        load_skill(&store, &author, ws, "acme-runbook", Some("1.0.0"))
            .await
            .unwrap()
            .body,
        "v1"
    );

    // Re-publishing a NEW version un-hides the id (deprecate is a state, not a tombstone).
    put_skill(&store, &author, ws, "acme-runbook", "1.1.0", "d", "v2", 2)
        .await
        .unwrap();
    assert!(list_granted_skills(&store, &author, ws)
        .await
        .unwrap()
        .iter()
        .any(|e| e.id == "acme-runbook"));
    assert_eq!(
        load_skill(&store, &author, ws, "acme-runbook", None)
            .await
            .unwrap()
            .body,
        "v2"
    );
}

// ── default grant set applied at workspace creation (config, revocable) ──────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn creating_a_workspace_applies_the_default_core_skill_grants() {
    let node = Node::boot().await.unwrap();
    // The creator must hold workspace.create in ITS OWN token workspace; it creates "acme".
    let creator = principal("user:ada", "acme", &["mcp:workspace.create:call"]);
    seed_core_skills(&node.store, "0.1.0", 1).await.unwrap();

    workspace_create(&node.store, &creator, "acme", "Acme", 1)
        .await
        .unwrap();

    // The creator (bootstrapped as admin, so it can read/load skills) sees the default set granted.
    let admin = principal("user:ada", "acme", &[READ]);
    let catalog = list_granted_skills(&node.store, &admin, "acme")
        .await
        .unwrap();
    for expected in DEFAULT_CORE_SKILLS {
        assert!(
            catalog.iter().any(|e| &e.id == expected),
            "default grant {expected} missing from the fresh workspace catalog"
        );
        // Each default is a loadable core skill (grant + read cap + seeded body).
        let s = load_skill(&node.store, &admin, "acme", expected, None)
            .await
            .unwrap();
        assert_eq!(&s.id, expected);
    }
    // A write-driving skill is NOT in the default set (read-only defaults, scope decision).
    assert!(!catalog.iter().any(|e| e.id == "core.secrets"));
}

#[test]
fn the_default_set_is_the_persona_grounding_set_with_an_env_override() {
    // The compiled-in defaults are the persona-grounding set — every grounding skill the built-in
    // persona catalog (`personas/personas.toml`) pins, so a fresh workspace's built-in personas all
    // start (a pinned skill is fail-closed at run assembly). This set GROWS as personas are added,
    // so we assert invariants + canonical members rather than a brittle whole-array equality (the
    // old `== &["core.lb-cli","core.query","core.store-read"]` rotted the moment the catalog grew).
    assert!(
        !DEFAULT_CORE_SKILLS.is_empty(),
        "the default grant set is non-empty (persona grounding)"
    );
    assert!(
        DEFAULT_CORE_SKILLS.iter().all(|id| id.starts_with("core.")),
        "every default id is a well-formed core id"
    );
    // Canonical members every built-in persona pins — spot-check the ones this test historically
    // guarded (read-only defaults) PLUS the per-persona grounding skills added since.
    for canonical in [
        "core.lb-cli",
        "core.query",
        "core.store-read",
        "core.panels",
        "core.rules",
        "core.insights",
    ] {
        assert!(
            DEFAULT_CORE_SKILLS.contains(&canonical),
            "canonical default {canonical} missing (the persona-grounding set rotted)"
        );
    }
    // A write-driving skill is NOT in the default set (read-only defaults, scope decision).
    assert!(!DEFAULT_CORE_SKILLS.contains(&"core.secrets"));
    // The env-style override parses a comma list; empty ⇒ none (a workspace with no default grants).
    assert_eq!(
        resolve_default_core_skills(Some("core.query, core.tags")),
        vec!["core.query".to_string(), "core.tags".to_string()]
    );
    assert!(resolve_default_core_skills(Some("")).is_empty());
    assert_eq!(
        resolve_default_core_skills(None).len(),
        DEFAULT_CORE_SKILLS.len()
    );
}

// ── real in-house run: the granted catalog is injected into context, and tracks grant/revoke ─────

/// A `Provider` that captures the first turn's messages, then stops. Real store + real loop drive
/// it (rule 9 — the model provider is the ONE sanctioned external); capturing what the model *saw*
/// is how we assert the catalog injection without a fake backend.
struct CapturingProvider {
    seen: Arc<Mutex<Vec<String>>>,
}

impl Provider for CapturingProvider {
    async fn complete(
        &self,
        req: &AiRequest,
    ) -> Result<AiResponse, lb_role_ai_gateway::ProviderFault> {
        let joined = req
            .messages
            .iter()
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n");
        *self.seen.lock().unwrap() = vec![joined];
        Ok(AiResponse::stop("done", 1))
    }
}

async fn run_and_capture_context(
    node: &Arc<Node>,
    caller: &Principal,
    ws: &str,
    job: &str,
) -> String {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let gw = AiGateway::new(CapturingProvider { seen: seen.clone() });
    invoke(
        node,
        &gw,
        caller,
        &[READ.into()],
        ws,
        Invocation {
            job_id: job,
            goal: "hi",
            skill: None,
            doc: None,
            tools: &[AllowedTool {
                name: "skill.activate".into(),
                description: "activate a granted skill".into(),
                input_schema: None,
            }],
            ts: 1,
        },
    )
    .await
    .expect("run completes");
    let captured = seen.lock().unwrap().first().cloned().unwrap_or_default();
    captured
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_real_run_injects_exactly_the_granted_catalog_and_tracks_changes() {
    let ws = "ws-real-catalog";
    let node = Arc::new(Node::boot().await.unwrap());
    seed_core_skills(&node.store, "0.1.0", 1).await.unwrap();
    // The caller may invoke + read skills + (for setup) write/grant.
    let caller = principal("user:ada", ws, &["mcp:agent.invoke:call", READ, WRITE]);

    // No grants yet → the injected context carries NO skill catalog line.
    let ctx0 = run_and_capture_context(&node, &caller, ws, "job-0").await;
    assert!(
        !ctx0.contains("core.lb-cli"),
        "an ungranted catalog is not injected"
    );

    // Grant a core skill → the next run's context lists exactly it.
    grant_skill(&node.store, &caller, ws, "core.lb-cli")
        .await
        .unwrap();
    let ctx1 = run_and_capture_context(&node, &caller, ws, "job-1").await;
    assert!(ctx1.contains("core.lb-cli"), "granted core skill appears");
    assert!(!ctx1.contains("core.query"), "an ungranted one does NOT");

    // Grant one more → the catalog grows.
    grant_skill(&node.store, &caller, ws, "core.query")
        .await
        .unwrap();
    let ctx2 = run_and_capture_context(&node, &caller, ws, "job-2").await;
    assert!(
        ctx2.contains("core.lb-cli") && ctx2.contains("core.query"),
        "catalog grew"
    );

    // Revoke → the catalog shrinks.
    lb_host::revoke_skill(&node.store, &caller, ws, "core.lb-cli")
        .await
        .unwrap();
    let ctx3 = run_and_capture_context(&node, &caller, ws, "job-3").await;
    assert!(
        !ctx3.contains("core.lb-cli"),
        "revoked skill drops from the catalog"
    );
    assert!(ctx3.contains("core.query"), "the still-granted one remains");
}

// ── workspace isolation: a core grant in ws-B never leaks into ws-A ──────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_core_grant_in_one_workspace_is_invisible_in_another() {
    let store = Store::memory().await.unwrap();
    seed_core_skills(&store, "0.1.0", 1).await.unwrap();

    let admin_b = principal("user:admin", "ws-b", &[WRITE]);
    grant_skill(&store, &admin_b, "ws-b", "core.lb-cli")
        .await
        .unwrap();

    // ws-B granted it → loads there.
    let agent_b = principal("key:agent", "ws-b", &[READ]);
    assert!(load_skill(&store, &agent_b, "ws-b", "core.lb-cli", None)
        .await
        .is_ok());

    // ws-A never granted it → denied there, even though the core record is node-shared.
    let agent_a = principal("key:agent", "ws-a", &[READ]);
    assert!(matches!(
        load_skill(&store, &agent_a, "ws-a", "core.lb-cli", None)
            .await
            .unwrap_err(),
        AssetError::Denied
    ));
    // And ws-A's catalog does not carry ws-B's grant.
    assert!(list_granted_skills(&store, &agent_a, "ws-a")
        .await
        .unwrap()
        .is_empty());
}
