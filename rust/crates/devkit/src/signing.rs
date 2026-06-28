use ed25519_dalek::{Signer, SigningKey};
use lb_registry::{digest, digest_hex, Artifact};

use crate::manifest_id::manifest_id_version;

/// Build the same signed `Artifact` that `lb-registry::verify_artifact` verifies: SHA-256 digest over
/// `(manifest_toml, wasm)`, then Ed25519 over that digest. All SDK/CLI/server-side publish paths must
/// call this instead of reimplementing signing.
pub fn sign_artifact(
    manifest_toml: String,
    wasm: Vec<u8>,
    key_id: impl Into<String>,
    signing_key: &SigningKey,
) -> anyhow::Result<Artifact> {
    let key_id = key_id.into();
    let (ext_id, version) = manifest_id_version(&manifest_toml)?;
    // `lb_registry::digest` owns the framing. Calling it here keeps packaging and verification on one
    // digest idiom and prevents a second, subtly incompatible artifact format.
    let d = digest(&manifest_toml, &wasm);
    Ok(Artifact {
        ext_id,
        version,
        manifest_toml,
        wasm,
        digest_hex: digest_hex(&d),
        publisher_key_id: key_id,
        signature: signing_key.sign(&d).to_bytes().to_vec(),
    })
}
