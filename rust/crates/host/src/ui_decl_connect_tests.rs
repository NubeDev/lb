//! Tests for [`super::project_connect`] — the `[connect]` projection `ext.list` carries to the
//! Datasources page. Split from `ui_decl_tests.rs` (FILE-LAYOUT 400-line cap); a `#[path]` child of
//! `ui_decl`, so it reaches the `pub(crate)` surface.
//!
//! The regression these pin: the grant check compared the manifest's BARE tool name
//! (`mcp:connection.create:call`) against grants that are always QUALIFIED
//! (`mcp:uart.connection.create:call`), so no native extension's connect kind ever projected and the
//! Datasources page silently never offered it.

use super::*;
use lb_ext_loader::Manifest;

fn manifest(probe: bool) -> Manifest {
    let probe_line = if probe {
        "probe_tool = \"connection.probe\""
    } else {
        ""
    };
    Manifest::parse(&format!(
        r#"
[extension]
id = "uart"
version = "0.1.0"
[runtime]
tier = "wasm"
world = "lazybones:ext/extension@0.1.0"
placement = "either"
[connect]
kind = "uart"
label = "UART (Rubix OS)"
create_tool = "connection.create"
list_tool = "connection.list"
delete_tool = "connection.delete"
{probe_line}
[[tools]]
name = "connection.create"
[[tools]]
name = "connection.list"
[[tools]]
name = "connection.delete"
[[tools]]
name = "connection.probe"
[visibility]
class = "public"
"#
    ))
    .expect("manifest parses")
}

fn grants(verbs: &[&str]) -> Vec<String> {
    verbs.iter().map(|v| format!("mcp:uart.{v}:call")).collect()
}

#[test]
fn projects_when_every_qualified_cap_is_granted() {
    let c = project_connect(
        &manifest(true),
        &grants(&[
            "connection.create",
            "connection.list",
            "connection.delete",
            "connection.probe",
        ]),
    )
    .expect("a fully granted connect kind projects");
    assert_eq!(c.kind, "uart");
    assert_eq!(c.label, "UART (Rubix OS)");
}

/// The page calls these names verbatim and looks `create_tool` up in `tools.catalog`, where every
/// extension tool is `<ext>.<tool>` — so the projection hands over the callable, qualified name.
#[test]
fn emits_the_qualified_callable_tool_names() {
    let c = project_connect(
        &manifest(true),
        &grants(&[
            "connection.create",
            "connection.list",
            "connection.delete",
            "connection.probe",
        ]),
    )
    .unwrap();
    assert_eq!(c.create_tool, "uart.connection.create");
    assert_eq!(c.list_tool, "uart.connection.list");
    assert_eq!(c.delete_tool, "uart.connection.delete");
    assert_eq!(c.probe_tool.as_deref(), Some("uart.connection.probe"));
}

/// All-or-nothing still holds: one missing verb withholds the whole kind.
#[test]
fn withholds_the_kind_when_one_verb_is_not_granted() {
    let partial = grants(&["connection.create", "connection.list", "connection.probe"]); // no delete
    assert!(project_connect(&manifest(true), &partial).is_none());
}

/// A declared optional verb (probe) is required once declared; an undeclared one is not.
#[test]
fn an_optional_verb_is_required_only_when_declared() {
    let no_probe = grants(&["connection.create", "connection.list", "connection.delete"]);
    assert!(project_connect(&manifest(true), &no_probe).is_none());
    assert!(project_connect(&manifest(false), &no_probe).is_some());
}

/// A BARE grant is not the gating cap and must not satisfy the check — otherwise an extension could
/// surface a connect kind on a grant that authorizes nothing callable.
#[test]
fn a_bare_grant_does_not_count() {
    let bare: Vec<String> = [
        "connection.create",
        "connection.list",
        "connection.delete",
        "connection.probe",
    ]
    .iter()
    .map(|v| format!("mcp:{v}:call"))
    .collect();
    assert!(project_connect(&manifest(true), &bare).is_none());
}

#[test]
fn no_connect_block_projects_nothing() {
    let m = Manifest::parse(
        r#"
[extension]
id = "plain"
version = "0.1.0"
[runtime]
tier = "wasm"
world = "lazybones:ext/extension@0.1.0"
placement = "either"
[visibility]
class = "public"
"#,
    )
    .unwrap();
    assert!(project_connect(&m, &[]).is_none());
}
