//! `thecrew` — the graphics-canvas extension's Tier-1 WASM component. It serves ZERO tools.
//!
//! This is proof-panel minus the tools (thecrew-extension-scope.md §Intent). The manifest
//! loader (`lb-ext-loader`) has exactly two tiers (`wasm` | `native`) and the registry's
//! publish path requires component bytes (`lb-registry` `Artifact.wasm`, verify-before-store);
//! there is no UI-only tier, and adding one would be core surface — rejected per the
//! zero-core-additions posture. So this component exists only to satisfy the world, publish,
//! and load. All real behavior lives in `ui/` (the federated `[ui]` page + `[[widget]]` cell),
//! reaching data through the host-mediated bridge under the viewer's grant.
//!
//! Because it serves no tools, `call` answers every name with an explicit error — never a
//! silent success. The extension is stateless by construction (§3.4): there is nothing to
//! keep, so a hot-reload swap loses nothing.

// The `generate!` call is emitted by `build.rs` into `$OUT_DIR/wit_gen.rs`, reading the WIT from the
// standalone `lb-sdk` crate (the authoritative owner) — see the build script. Generated against the
// SAME WIT the host uses, so the ABI cannot drift.
include!(concat!(env!("OUT_DIR"), "/wit_gen.rs"));

struct TheCrew;

impl exports::lazybones::ext::tool::Guest for TheCrew {
    fn call(
        name: String,
        _input_json: String,
    ) -> Result<String, exports::lazybones::ext::tool::ToolError> {
        use exports::lazybones::ext::tool::ToolError;
        // Zero tools: this extension is UI-only. Any tool name is an explicit error, never a
        // silent success — the whole point is that nothing is served here.
        lazybones::ext::host::log(&format!("thecrew: no tools served; rejecting {name}"));
        Err(ToolError::Failed(format!(
            "thecrew serves no tools (UI-only extension): {name}"
        )))
    }
}

export!(TheCrew);

#[cfg(test)]
mod tests {
    /// The dispatch under test, decoupled from the generated WIT `Guest::call` (only callable
    /// from a wasm host). Identical shape: every tool name is an explicit error.
    fn dispatch(name: &str) -> Result<String, String> {
        Err(format!(
            "thecrew serves no tools (UI-only extension): {name}"
        ))
    }

    #[test]
    fn any_tool_is_an_explicit_error_never_a_silent_success() {
        let err = dispatch("scene.draw").expect_err("a UI-only ext must serve no tools");
        assert!(err.contains("serves no tools"), "got {err}");
    }
}
