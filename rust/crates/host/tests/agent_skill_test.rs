//! agent-run scope Part 5 — MODEL-ACTIVATED SKILLS. The grant gates the set; the model picks within
//! it. These tests exercise the real store + bus + loop (rule 9, no mocks) with the deterministic
//! `MockProvider` scripting the model to propose `skill.activate` (the only stubbed external, the LLM
//! provider). They cover: the granted-skills catalog, the model activating a granted skill, the
//! activation surviving resume, and the two MANDATORY isolation/deny categories (testing-scope §2.1,
//! §2.2).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    grant_skill, invoke, list_granted_skills, put_skill, resume, AllowedTool, Invocation, Node,
};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider, ToolCall};

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

const INVOKE: &str = "mcp:agent.invoke:call";
const SKILL_R: &str = "store:skill/*:read";
const SKILL_W: &str = "store:skill/*:write";

/// A gateway whose model first activates `repo-conventions`, then stops with a final answer.
fn activate_then_stop(skill_id: &str) -> AiGateway<MockProvider> {
    AiGateway::new(MockProvider::new(vec![
        AiResponse::calls(
            "I'll load the conventions.",
            vec![ToolCall {
                id: "a1".into(),
                name: "skill.activate".into(),
                input: format!(r#"{{"id":"{skill_id}"}}"#),
            }],
            10,
        ),
        AiResponse::stop("done: followed the conventions", 5),
    ]))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_model_activates_a_granted_skill() {
    // The model sees a granted skill in the catalog and activates it mid-run. The activation is
    // recorded in the durable transcript (survives resume) and the body enters context; the run
    // completes.
    let ws = "skill-activate";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[INVOKE, SKILL_R, SKILL_W]);

    put_skill(
        &node.store,
        &caller,
        ws,
        "repo-conventions",
        "1",
        "repo coding conventions",
        "Always run the linter.",
        1,
    )
    .await
    .unwrap();
    grant_skill(&node.store, &caller, ws, "repo-conventions")
        .await
        .unwrap();

    let gw = activate_then_stop("repo-conventions");
    let answer = invoke(
        &node,
        &gw,
        &caller,
        &[SKILL_R.into()],
        ws,
        Invocation {
            job_id: "sess-1",
            goal: "follow the repo conventions",
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
    .expect("agent runs to completion");

    assert_eq!(answer, "done: followed the conventions");

    let job = lb_jobs::load(&node.store, ws, "sess-1")
        .await
        .unwrap()
        .expect("job persisted");
    assert_eq!(job.status, lb_jobs::JobStatus::Done);
    let activated = job.events().any(|e| {
        matches!(e, lb_jobs::TranscriptEvent::SkillActivated { id } if id == "repo-conventions")
    });
    assert!(activated, "SkillActivated recorded in the transcript");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn activation_survives_resume() {
    // A run activates a skill, then we reload the job from the store and rehydrate the loop: the
    // activated skill is in the rehydrated `active_skills` (the transcript carries SkillActivated).
    let ws = "skill-resume";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[INVOKE, SKILL_R, SKILL_W]);

    put_skill(
        &node.store,
        &caller,
        ws,
        "conv",
        "1",
        "conventions",
        "Be terse.",
        1,
    )
    .await
    .unwrap();
    grant_skill(&node.store, &caller, ws, "conv").await.unwrap();

    let gw = activate_then_stop("conv");
    invoke(
        &node,
        &gw,
        &caller,
        &[SKILL_R.into()],
        ws,
        Invocation {
            job_id: "sess-r",
            goal: "use the skill",
            skill: None,
            doc: None,
            tools: &[],
            ts: 1,
        },
    )
    .await
    .unwrap();

    // Reload from the store and rehydrate — the activation must be in the rehydrated state.
    let job = lb_jobs::load(&node.store, ws, "sess-r")
        .await
        .unwrap()
        .expect("job persisted");
    let events: Vec<&lb_jobs::TranscriptEvent> = job.events().collect();
    let state = lb_host::rehydrate("You are a workspace agent.", "use the skill", &events);
    assert!(
        state.active_skills.iter().any(|s| s == "conv"),
        "the activated skill is in the rehydrated active_skills after reload"
    );

    // And a real resume runs cleanly on the terminal job (idempotent — returns the answer so far).
    let answer = resume(&node, &gw, &caller, &[SKILL_R.into()], ws, "sess-r", &[], 2)
        .await
        .unwrap();
    assert_eq!(answer, "done: followed the conventions");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn activating_an_ungranted_skill_is_denied() {
    // MANDATORY capability/grant-deny (§2.1): the skill exists but was NOT granted to the workspace.
    // `skill.activate` is grant-gated via load_skill → the model gets a denied error result, the
    // activation is NEVER recorded, and no body enters context. The loop still completes.
    let ws = "skill-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[INVOKE, SKILL_R, SKILL_W]);

    // Put the skill but do NOT grant it.
    put_skill(
        &node.store,
        &caller,
        ws,
        "secret",
        "1",
        "secret skill",
        "TOP SECRET",
        1,
    )
    .await
    .unwrap();

    let gw = activate_then_stop("secret");
    let answer = invoke(
        &node,
        &gw,
        &caller,
        &[SKILL_R.into()],
        ws,
        Invocation {
            job_id: "sess-d",
            goal: "use the secret skill",
            skill: None,
            doc: None,
            tools: &[],
            ts: 1,
        },
    )
    .await
    .expect("the loop completes even though activation was denied");

    assert_eq!(answer, "done: followed the conventions");
    let job = lb_jobs::load(&node.store, ws, "sess-d")
        .await
        .unwrap()
        .unwrap();
    let activated = job
        .events()
        .any(|e| matches!(e, lb_jobs::TranscriptEvent::SkillActivated { .. }));
    assert!(!activated, "an ungranted skill is NEVER activated/recorded");
    let denied = job.events().any(|e| {
        matches!(e, lb_jobs::TranscriptEvent::ToolResult { id, err: Some(_), .. } if id == "a1")
    });
    assert!(
        denied,
        "the denied activation is fed back as an error result"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_catalog_lists_only_granted_skills_without_the_body() {
    // The granted-skills catalog: seed two skills, grant ONE → the catalog has exactly that one,
    // with title + description ONLY (no body field on the entry to leak).
    let ws = "skill-catalog";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[SKILL_R, SKILL_W]);

    put_skill(
        &node.store,
        &caller,
        ws,
        "granted",
        "1",
        "the granted one",
        "GRANTED BODY",
        1,
    )
    .await
    .unwrap();
    put_skill(
        &node.store,
        &caller,
        ws,
        "ungranted",
        "1",
        "the hidden one",
        "HIDDEN BODY",
        1,
    )
    .await
    .unwrap();
    grant_skill(&node.store, &caller, ws, "granted")
        .await
        .unwrap();

    let catalog = list_granted_skills(&node.store, &caller, ws)
        .await
        .expect("catalog reads");
    assert_eq!(catalog.len(), 1, "only the granted skill is listed");
    let entry = &catalog[0];
    assert_eq!(entry.id, "granted");
    assert_eq!(entry.title, "granted");
    assert_eq!(entry.description, "the granted one");
    // The entry type carries no body (title + description only) — proven structurally.
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_for_catalog_and_activation() {
    // MANDATORY workspace-isolation (§2.2): a ws-B catalog never lists a ws-A-only skill, and ws-B
    // cannot activate a ws-A skill (its grant lives in ws-A's namespace — invisible to ws-B).
    let node = Arc::new(Node::boot().await.unwrap());

    // ws-A: seed + grant a skill.
    let ws_a = "iso-ws-a";
    let ada = principal("user:ada", ws_a, &[INVOKE, SKILL_R, SKILL_W]);
    put_skill(
        &node.store,
        &ada,
        ws_a,
        "a-only",
        "1",
        "ws-A skill",
        "A BODY",
        1,
    )
    .await
    .unwrap();
    grant_skill(&node.store, &ada, ws_a, "a-only")
        .await
        .unwrap();

    // ws-B: its catalog is empty (it granted nothing), never showing ws-A's skill.
    let ws_b = "iso-ws-b";
    let bob = principal("user:bob", ws_b, &[INVOKE, SKILL_R, SKILL_W]);
    let cat_b = list_granted_skills(&node.store, &bob, ws_b).await.unwrap();
    assert!(
        cat_b.iter().all(|e| e.id != "a-only"),
        "ws-B catalog never lists a ws-A-only skill"
    );
    assert!(cat_b.is_empty(), "ws-B granted nothing");

    // ws-B tries to activate the ws-A skill → denied (no grant in ws-B), no activation recorded.
    let gw = activate_then_stop("a-only");
    invoke(
        &node,
        &gw,
        &bob,
        &[SKILL_R.into()],
        ws_b,
        Invocation {
            job_id: "iso-job",
            goal: "steal the ws-A skill",
            skill: None,
            doc: None,
            tools: &[],
            ts: 1,
        },
    )
    .await
    .unwrap();
    let job = lb_jobs::load(&node.store, ws_b, "iso-job")
        .await
        .unwrap()
        .unwrap();
    let activated = job
        .events()
        .any(|e| matches!(e, lb_jobs::TranscriptEvent::SkillActivated { .. }));
    assert!(!activated, "ws-B cannot activate a ws-A skill");
}
