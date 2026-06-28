//! The static **ACP adapter facts** the `system.acp` verb returns — protocol version, handled methods,
//! advertised capabilities, error codes, and the auth/notes. ACP (Agent Client Protocol, agent-run
//! Part 4) is a per-stdio-session adapter that lets Zed/Cursor drive the central agent; it is NOT a
//! polled network server, so there is no live health to report — only *reachable capability info*.
//!
//! The host owns this truth (rather than the `role/acp` binary) so the UI/gateway can surface it
//! without importing a role binary. It mirrors `role/acp/src/session.rs` (`initialize`'s handshake +
//! the `handle` method arms + `rpc::codes`); if that adapter changes, this is the one place to update,
//! kept honest by the shape the page reads.

use super::model::{AcpInfo, Metric};

/// Build the ACP adapter's static capability/protocol facts. Pure (no I/O) — derived from constants
/// that mirror the acp role's `initialize` handshake and handled methods.
pub(crate) fn acp_info() -> AcpInfo {
    AcpInfo {
        protocol_version: 1,
        methods: [
            "initialize",
            "session/new",
            "session/prompt",
            "session/cancel",
            "session/load",
            "session/resume",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        capabilities: vec![
            Metric::new("loadSession", "true"),
            Metric::new("prompt.image", "false"),
            Metric::new("prompt.audio", "false"),
            Metric::new("mcp.http", "false"),
            Metric::new("mcp.sse", "false"),
        ],
        error_codes: vec![
            Metric::new("-32001 unauthenticated", "missing/forged/expired session token"),
            Metric::new("-32002 denied", "the workspace-first capability gate refused"),
            Metric::new(
                "-32010 unsupported client servers",
                "client-provided mcpServers/cwd are rejected (would need a net:* grant)",
            ),
        ],
        notes: vec![
            "Authentication is the trusted-session path: the adapter verifies a real lb_auth token \
             with the node key and binds the session to exactly the token's workspace (§7). A forged \
             call is denied like any other."
                .to_string(),
            "Client-provided mcpServers/cwd are rejected cleanly (not silently dropped) — bridging \
             client-side tools needs a net:* grant that is a future scope."
                .to_string(),
            "It is a stdio adapter driven per editor session, not a persistent network server — this \
             page reports its capabilities, not a live connection count."
                .to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_protocol_and_methods() {
        let info = acp_info();
        assert_eq!(info.protocol_version, 1);
        // The five session/* methods the driver handles, plus initialize.
        for m in [
            "initialize",
            "session/new",
            "session/prompt",
            "session/cancel",
            "session/load",
        ] {
            assert!(info.methods.iter().any(|x| x == m), "missing method {m}");
        }
        assert!(!info.capabilities.is_empty());
        assert!(!info.error_codes.is_empty());
        assert!(!info.notes.is_empty());
    }
}
