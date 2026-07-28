//! The ext-tool-descriptors slice, proven end to end against a **REAL supervised child**.
//!
//! The claim under test: an extension self-declares its tools' contracts in the `init` handshake,
//! the host folds them into the one MCP registry, and `tools.catalog` serves them — cap-gated and
//! workspace-walled exactly as before, with a child that declares nothing behaving bit-identically
//! to the day before this existed.
//!
//! No mocks (rule 9): a real `echo-sidecar` OS process, real embedded SurrealDB, the real install
//! path, the real registry, the real catalog verb. The reference sidecar declares a full contract
//! for `echo` and **nothing** for `whoami`, so one spawn exercises both the enriched path and the
//! `name_only` fallback that every already-published extension takes.

use std::path::PathBuf;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{install_native, register_remote_descriptors, tools_catalog, Node};
use lb_mcp::ToolDescriptor;
use lb_supervisor::OsLauncher;

const MANIFEST: &str = include_str!("../../../extensions/echo-sidecar/extension.toml");

/// Where the built reference sidecar lives (see `native_test.rs` — same contract, same build hint).
fn sidecar_dir() -> String {
    if let Ok(p) = std::env::var("ECHO_SIDECAR_BIN") {
        return PathBuf::from(p)
            .parent()
            .unwrap()
            .to_string_lossy()
            .into_owned();
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug");
    if !dir.join("echo-sidecar").exists() {
        panic!(
            "missing echo-sidecar at {} — run: (cd rust && cargo build -p echo-sidecar)",
            dir.join("echo-sidecar").display()
        );
    }
    dir.to_string_lossy().into_owned()
}

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// The caps an admin needs to install the sidecar AND read the catalog rows for its two tools.
fn installer(ws: &str) -> Principal {
    principal(
        ws,
        &[
            "mcp:native.install:call",
            "mcp:native.call:call",
            "mcp:tools.catalog:call",
            "mcp:echo-sidecar.echo:call",
            "mcp:echo-sidecar.whoami:call",
        ],
    )
}

async fn install(node: &Node, caller: &Principal, ws: &str) {
    install_native(
        node,
        &OsLauncher,
        caller,
        ws,
        MANIFEST,
        &sidecar_dir(),
        &[],
        1,
    )
    .await
    .expect("native sidecar installs + spawns");
}

fn row<'a>(catalog: &'a lb_host::ToolsCatalog, name: &str) -> Option<&'a ToolDescriptor> {
    catalog.tools.iter().find(|d| d.name == name)
}

/// The headline: a schema the CHILD generated reaches `tools.catalog` through the real install
/// path, qualified with the extension id, alongside its title, group and external-effect flag.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_declared_schema_reaches_the_catalog_from_a_real_child() {
    let ws = "desc-real-child";
    let node = Node::boot().await.unwrap();
    let admin = installer(ws);
    install(&node, &admin, ws).await;

    let catalog = tools_catalog(&node, &admin, ws).await.expect("catalog");

    let echo = row(&catalog, "echo-sidecar.echo").expect("the declared tool is rowed");
    assert_eq!(echo.title, "Echo");
    assert_eq!(echo.group, "probes");
    assert!(!echo.emits_external);

    let schema = echo
        .input_schema
        .as_ref()
        .expect("the child's declared schema survived the handshake → registry → catalog");
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["properties"]["message"]["type"], "string");
    assert_eq!(schema["required"][0], "message");
}

/// The other half of the same spawn: a tool the child did NOT declare still registers, schema-less
/// — the `name_only` fallback, which is what every extension built against an older SDK gets. If
/// this regressed, an undeclared tool would vanish from the catalog and its dispatch would break.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_undeclared_tool_falls_back_to_name_only() {
    let ws = "desc-fallback";
    let node = Node::boot().await.unwrap();
    let admin = installer(ws);
    install(&node, &admin, ws).await;

    let catalog = tools_catalog(&node, &admin, ws).await.expect("catalog");

    let whoami = row(&catalog, "echo-sidecar.whoami").expect("an undeclared tool still rows");
    assert!(
        whoami.input_schema.is_none(),
        "nothing was declared for it, so nothing may be invented: {whoami:?}"
    );
    assert!(!whoami.emits_external);
    // The catalog's own long-standing defaults still apply to a name-only row: an empty group falls
    // back to the extension id and an empty title to the qualified name. That is exactly what an
    // undeclared tool looked like before this slice — the fallback must reproduce it, not bypass it.
    assert_eq!(whoami.group, "echo-sidecar");
    assert_eq!(whoami.title, "echo-sidecar.whoami");
}

/// **Capability-deny** (mandatory category). The menu is the permission model: a subject without
/// `mcp:echo-sidecar.echo:call` gets no row — no schema, no name, and no error that would confirm
/// the tool exists. Declaring a schema must not widen what an uncapped caller can see.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_catalog_hides_a_declared_row_from_an_uncapped_subject() {
    let ws = "desc-deny";
    let node = Node::boot().await.unwrap();
    install(&node, &installer(ws), ws).await;

    // Holds the catalog verb itself, but not the extension's tools.
    let viewer = principal(ws, &["mcp:tools.catalog:call"]);
    let catalog = tools_catalog(&node, &viewer, ws)
        .await
        .expect("the catalog verb itself is granted");

    assert!(
        row(&catalog, "echo-sidecar.echo").is_none(),
        "an ungranted tool must be ABSENT, not schema-stripped"
    );
    assert!(row(&catalog, "echo-sidecar.whoami").is_none());
    assert!(
        !catalog
            .tools
            .iter()
            .any(|d| d.name.contains("echo-sidecar")),
        "nothing about the extension may leak: {:?}",
        catalog.tools.iter().map(|d| &d.name).collect::<Vec<_>>()
    );
}

/// **Workspace isolation** (mandatory category).
///
/// The MCP registry is deliberately **node-global**: one `SidecarDispatch` entry per extension id
/// serves every workspace's child, and the wall is structural — the `SidecarMap` is keyed by
/// `(ws, ext_id)`, so a call resolves the caller's OWN child or none. A neighbour workspace holding
/// the same caps therefore sees the row (shipped behaviour, unchanged by this slice: descriptors
/// ride the same node-global entry names already did) but **cannot reach ws-a's process**.
///
/// This test pins the wall where it actually is. Asserting catalog-invisibility instead would pin a
/// property the design never promised — and would have passed for the wrong reason before schemas
/// existed, since a name-only row leaks the same fact a schema'd one does.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_declared_tool_does_not_reach_another_workspaces_child() {
    let node = Node::boot().await.unwrap();
    let admin_a = installer("ws-a");
    install(&node, &admin_a, "ws-a").await;

    // Same caps, different workspace. The extension is installed (and spawned) under ws-a only.
    let admin_b = principal(
        "ws-b",
        &[
            "mcp:tools.catalog:call",
            "mcp:native.call:call",
            "mcp:echo-sidecar.echo:call",
        ],
    );

    // ws-a's own call reaches ws-a's child.
    lb_host::call_sidecar(
        &node,
        &OsLauncher,
        &admin_a,
        "ws-a",
        "echo-sidecar",
        "echo",
        r#""hi""#,
        1,
    )
    .await
    .expect("the owner reaches its own child");

    // ws-b's identical call finds no child of its own and is refused — it never crosses into ws-a's.
    let crossed = lb_host::call_sidecar(
        &node,
        &OsLauncher,
        &admin_b,
        "ws-b",
        "echo-sidecar",
        "echo",
        r#""hi""#,
        1,
    )
    .await;
    assert!(
        crossed.is_err(),
        "a neighbour workspace must not reach ws-a's supervised child: {crossed:?}"
    );
}

/// **Routed descriptor carriage.** Symmetric nodes means the same catalog either side of the bus:
/// an extension hosted on another node must serve schemas here too. Before this slice the routed
/// registration took bare names, so descriptors were dropped at the node boundary by construction.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_remote_registration_carries_its_descriptors() {
    let ws = "desc-routed";
    let node = Node::boot().await.unwrap();
    let admin = principal(
        ws,
        &["mcp:tools.catalog:call", "mcp:remote-ext.point.write:call"],
    );

    register_remote_descriptors(
        &node,
        "remote-ext",
        lb_bus::NodeId::new("node-b").expect("valid node id"),
        vec![ToolDescriptor {
            name: "point.write".into(),
            title: "Write point".into(),
            group: "points".into(),
            input_schema: Some(serde_json::json!({
                "type": "object",
                "properties": { "point": { "type": "string" } },
                "required": ["point"],
            })),
            emits_external: true,
            result: None,
        }],
    );

    let catalog = tools_catalog(&node, &admin, ws).await.expect("catalog");
    let write = row(&catalog, "remote-ext.point.write").expect("the routed row is served here");
    assert_eq!(write.group, "points");
    assert!(
        write.emits_external,
        "the external-effect flag drives undo's irreversible class and must survive the hop"
    );
    assert_eq!(
        write.input_schema.as_ref().expect("schema crossed the hop")["properties"]["point"]["type"],
        "string"
    );
}
