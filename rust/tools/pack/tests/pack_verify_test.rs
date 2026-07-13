//! The pack toolchain's published promise, proven end to end with the REAL binary and the REAL
//! verifier (`pack-toolchain-publish` scope; no mocks per testing-scope §0):
//!
//! 1. the `lb-pack` binary packages a real built fixture wasm (`hello-v2`) into an artifact that
//!    `lb_registry::verify_artifact` — the node's own trust gate — ACCEPTS against the trust line
//!    the tool printed;
//! 2. a key NOT in the trusted set is REJECTED (publishing the packager must not weaken the gate);
//! 3. a byte flipped after signing fails verification (the digest binds the payload);
//! 4. same inputs + same key → byte-identical artifact (deterministic, CI-cacheable).
//!
//! Each test drives the actual installed-binary code path via `CARGO_BIN_EXE_lb-pack`.

use std::path::{Path, PathBuf};
use std::process::Command;

use lb_registry::{verify_artifact, Artifact, PublisherKey, TrustedKeys};

/// The real built inputs for `hello-v2` (same fixture files `sign_test.rs` uses).
const MANIFEST_PATH: &str = "../../extensions/hello-v2/extension.toml";
const WASM_PATH: &str = "../../extensions/hello-v2/target/wasm32-wasip2/release/hello_v2_ext.wasm";

/// Run the real `lb-pack` binary and return the packaged artifact plus the stderr-printed trust line.
fn run_pack(dir: &Path, key_id: &str, key_file: &str) -> (Artifact, String) {
    let out_path = dir.join("artifact.json");
    let output = Command::new(env!("CARGO_BIN_EXE_lb-pack"))
        .args([
            MANIFEST_PATH,
            WASM_PATH,
            dir.join(key_file).to_str().unwrap(),
            "--key-id",
            key_id,
            "--out",
            out_path.to_str().unwrap(),
        ])
        .output()
        .expect("run lb-pack");
    assert!(
        output.status.success(),
        "lb-pack failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let trust_line = stderr
        .lines()
        .find_map(|l| l.strip_prefix("trusted-pubkey: "))
        .expect("lb-pack prints the trust line")
        .to_string();
    let json = std::fs::read_to_string(&out_path).expect("read artifact json");
    (
        serde_json::from_str(&json).expect("artifact json parses"),
        trust_line,
    )
}

/// Build the node-side trusted set from the exact `key_id=hexpubkey` line the tool printed —
/// the same string an operator pastes into `LB_TRUSTED_PUBKEYS`.
fn trusted_from_line(line: &str) -> TrustedKeys {
    let (key_id, hex) = line.split_once('=').expect("key_id=hex");
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    let mut trusted = TrustedKeys::new();
    trusted.insert(
        key_id.to_string(),
        PublisherKey::from_bytes(&bytes.try_into().unwrap()).unwrap(),
    );
    trusted
}

#[test]
fn packed_artifact_verifies_against_the_printed_trust_line() {
    let tmp = tempfile::tempdir().unwrap();
    let (artifact, trust_line) = run_pack(tmp.path(), "pack-test", "dev.key");
    assert_eq!(artifact.ext_id, "hello");
    let trusted = trusted_from_line(&trust_line);
    verify_artifact(artifact, &trusted).expect("the node's own verifier accepts lb-pack's output");
}

#[test]
fn an_untrusted_key_is_rejected_at_verify() {
    let tmp = tempfile::tempdir().unwrap();
    // Sign with one key, but trust a DIFFERENT one — the node-side gate must reject.
    let (artifact, _) = run_pack(tmp.path(), "pack-test", "untrusted.key");
    let (_, other_trust_line) = run_pack(tmp.path(), "pack-test", "other.key");
    let trusted = trusted_from_line(&other_trust_line);
    assert!(
        verify_artifact(artifact, &trusted).is_err(),
        "an artifact from a key outside LB_TRUSTED_PUBKEYS must be rejected"
    );
}

#[test]
fn a_tampered_wasm_fails_verification() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut artifact, trust_line) = run_pack(tmp.path(), "pack-test", "dev.key");
    artifact.wasm[0] ^= 0xff;
    assert!(
        verify_artifact(artifact, &trusted_from_line(&trust_line)).is_err(),
        "a byte flipped after signing must fail the digest check"
    );
}

#[test]
fn packing_is_deterministic_for_same_inputs_and_key() {
    let tmp = tempfile::tempdir().unwrap();
    run_pack(tmp.path(), "pack-test", "dev.key");
    let first = std::fs::read(tmp.path().join("artifact.json")).unwrap();
    run_pack(tmp.path(), "pack-test", "dev.key");
    let second = std::fs::read(tmp.path().join("artifact.json")).unwrap();
    assert_eq!(
        first, second,
        "same wasm + same key must produce identical artifact bytes"
    );
}

/// The publishable-chain check — the test that would have caught this gap (it fails on the old
/// `publish = false` flags): every lb crate in `lb-pack`'s dependency closure must be publishable,
/// or `cargo install --git …lb lb-pack` has nothing to install.
#[test]
fn the_pack_toolchain_dependency_chain_is_publishable() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .arg("--manifest-path")
        .arg(&manifest)
        .output()
        .expect("cargo metadata");
    assert!(output.status.success());
    let meta: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    // The closure of lb-pack: itself, lb-devkit, lb-registry. `publish: []` is `publish = false`.
    for name in ["lb-pack", "lb-devkit", "lb-registry"] {
        let pkg = meta["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == name)
            .unwrap_or_else(|| panic!("{name} in workspace metadata"));
        assert_ne!(
            pkg["publish"],
            serde_json::json!([]),
            "{name} is publish = false — the pack toolchain un-published itself"
        );
    }
}
