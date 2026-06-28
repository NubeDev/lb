use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use ed25519_dalek::SigningKey;

use crate::hex::{decode, encode};

/// A publisher seed loaded from disk, plus whether the dev convenience path generated it this call.
/// Keeping generation visible lets CLI/UI callers warn the operator when a new trust identity exists.
pub struct LoadedPublisherKey {
    pub signing_key: SigningKey,
    pub generated: bool,
}

pub fn load_or_create_key(path: impl AsRef<Path>) -> Result<LoadedPublisherKey> {
    let path = path.as_ref();
    if path.exists() {
        // Existing keys are 32-byte Ed25519 seeds, not private-key PEMs. This mirrors the shipped
        // `lb-pack` custody format so the SDK path can reuse current `.lazybones/keys` state.
        let hexed =
            fs::read_to_string(path).with_context(|| format!("read key {}", path.display()))?;
        let bytes = decode(hexed.trim())?;
        let seed: [u8; 32] = bytes.try_into().map_err(|_| {
            anyhow!(
                "key file {} must be a 32-byte (64 hex char) seed",
                path.display()
            )
        })?;
        Ok(LoadedPublisherKey {
            signing_key: SigningKey::from_bytes(&seed),
            generated: false,
        })
    } else {
        // Dev ergonomics: first use creates a stable local publisher identity. Trust remains external
        // to the artifact; a node accepts this key only if its environment allow-lists the public key.
        let signing_key = SigningKey::generate(&mut rand_core::OsRng);
        ensure_parent(path)?;
        fs::write(path, encode(&signing_key.to_bytes()))
            .with_context(|| format!("write key {}", path.display()))?;
        Ok(LoadedPublisherKey {
            signing_key,
            generated: true,
        })
    }
}

pub fn publisher_trust_line(key_id: &str, signing_key: &SigningKey) -> String {
    // This is intentionally the exact `key_id=hexpubkey` string consumed by `LB_TRUSTED_PUBKEYS`.
    format!(
        "{key_id}={}",
        encode(&signing_key.verifying_key().to_bytes())
    )
}

pub(crate) fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;
    }
    Ok(())
}
