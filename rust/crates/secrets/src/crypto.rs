//! The at-rest envelope (secrets-at-rest) — a secret's VALUE is sealed before it touches the store
//! and opened after it leaves, so a copied database file, a store-level browse, or a backup never
//! contains a credential in the clear. This closes the "honest scope" note that shipped with the
//! crate ("values are plaintext-in-store for now").
//!
//! Grafana is the prior art: its `secureJsonData` fields are AEAD-sealed with a boot-configured
//! `secret_key` and stored as an opaque envelope, while values written before the feature stay
//! readable. Same posture here:
//!
//!   * **Cipher**: XChaCha20-Poly1305 — same AEAD class as Grafana's AES-GCM, chosen from the
//!     RustCrypto family the tree already standardises on (sha2/hmac/argon2). The X variant's
//!     24-byte nonce is drawn fresh at random per write, which IS the whole nonce-management story
//!     (no counters, no per-key state to persist).
//!   * **Envelope**: `enc:v1:<base64(nonce ‖ ciphertext)>`. The prefix is the discriminator: a
//!     stored value without it is a legacy plaintext record and is returned as-is, then sealed the
//!     next time it is written — migration by attrition, no big-bang rewrite, no flag day.
//!   * **Key custody is the embedder's** (the BootConfig doctrine): the 32-byte master key arrives
//!     through [`install_master_key`] exactly once at boot, filled at the binary boundary — no code
//!     below the seam reads an env var. **No key installed ⇒ values stay plaintext**, exactly the
//!     shipped behaviour, so a bare embedder or a unit test is never broken by this module existing.
//!
//! One deliberate asymmetry: [`seal`] without a key degrades to plaintext (additive, fail-open on
//! write), but [`open`] on an `enc:v1:` envelope without the right key FAILS — returning ciphertext
//! as if it were the value would hand garbage to a connection pool, and silently "recovering" a
//! secret we cannot read is the one thing a secret store must never pretend to do.
//!
//! One responsibility: string in, string out, sealed or opened. No store, no gates, no policy —
//! those stay in `lib.rs`.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit, OsRng};
use chacha20poly1305::{AeadCore, XChaCha20Poly1305, XNonce};
use std::sync::OnceLock;

/// The envelope discriminator. Versioned so a future cipher change is `enc:v2:` beside this one,
/// not a migration of this one.
const PREFIX_V1: &str = "enc:v1:";

/// XChaCha20's nonce width — the first bytes of the decoded envelope body.
const NONCE_LEN: usize = 24;

/// The process-wide master key, installed once at boot. `OnceLock` and not a parameter because the
/// crate's whole public surface (`set`/`get`/`reclaim`/…) is called from a dozen host sites that
/// have no business carrying key material through their signatures — custody stays at the boot
/// boundary, everything below just seals and opens.
static MASTER_KEY: OnceLock<[u8; 32]> = OnceLock::new();

#[derive(thiserror::Error, Debug)]
pub enum CryptoError {
    /// An `enc:v1:` envelope arrived but no master key is installed — a node rebooted without the
    /// key that sealed its store.
    #[error("secret is sealed but no master key is installed")]
    NoKey,
    /// The envelope failed to open: wrong key, or a tampered/truncated record. Indistinguishable by
    /// design (AEAD), and the message never carries the payload.
    #[error("secret envelope failed to open (wrong key or corrupt record)")]
    Open,
}

/// Install the 32-byte master key, once, at boot (the embedder's custody — see the module doc).
/// Returns `false` when a key was already installed (the call is ignored); boot treats that as a
/// programming error worth a warning, not a panic.
pub fn install_master_key(key: [u8; 32]) -> bool {
    MASTER_KEY.set(key).is_ok()
}

/// Whether a master key is installed — lets boot log the node's at-rest posture honestly.
pub fn master_key_installed() -> bool {
    MASTER_KEY.get().is_some()
}

/// Seal `plain` for storage. With a key: the `enc:v1:` envelope. Without: `plain` unchanged (the
/// shipped plaintext behaviour, chosen by the embedder not installing a key).
pub(crate) fn seal(plain: &str) -> String {
    let Some(key) = MASTER_KEY.get() else {
        return plain.to_string();
    };
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    // Encryption failure is unreachable for XChaCha20-Poly1305 over in-memory bytes; expect() is
    // honest about that rather than inventing an error path no caller could act on.
    let ct = cipher
        .encrypt(&nonce, plain.as_bytes())
        .expect("XChaCha20-Poly1305 seal");
    let mut body = Vec::with_capacity(NONCE_LEN + ct.len());
    body.extend_from_slice(&nonce);
    body.extend_from_slice(&ct);
    format!("{PREFIX_V1}{}", B64.encode(body))
}

/// Open a stored value. An `enc:v1:` envelope decrypts (and fails loud when it cannot — see the
/// module doc); anything else is a legacy plaintext record and passes through unchanged.
pub(crate) fn open(stored: &str) -> Result<String, CryptoError> {
    let Some(b64) = stored.strip_prefix(PREFIX_V1) else {
        return Ok(stored.to_string());
    };
    let key = MASTER_KEY.get().ok_or(CryptoError::NoKey)?;
    let body = B64.decode(b64).map_err(|_| CryptoError::Open)?;
    if body.len() <= NONCE_LEN {
        return Err(CryptoError::Open);
    }
    let (nonce, ct) = body.split_at(NONCE_LEN);
    let cipher = XChaCha20Poly1305::new(key.into());
    let plain = cipher
        .decrypt(XNonce::from_slice(nonce), ct)
        .map_err(|_| CryptoError::Open)?;
    String::from_utf8(plain).map_err(|_| CryptoError::Open)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One key for the whole test binary — `OnceLock` is process-global, so every test shares it.
    fn key() -> [u8; 32] {
        let k = [7u8; 32];
        install_master_key(k);
        k
    }

    #[test]
    fn round_trips_through_the_envelope() {
        key();
        let sealed = seal("postgres://user:hunter2@db/prod");
        assert!(
            sealed.starts_with(PREFIX_V1),
            "sealed value must carry the envelope prefix"
        );
        assert!(
            !sealed.contains("hunter2"),
            "ciphertext must not contain the plaintext"
        );
        assert_eq!(open(&sealed).unwrap(), "postgres://user:hunter2@db/prod");
    }

    #[test]
    fn two_seals_of_the_same_value_differ() {
        key();
        // Fresh random nonce per write: equal plaintexts must not produce equal records, or the
        // store leaks "these two datasources share a password".
        assert_ne!(seal("same"), seal("same"));
    }

    #[test]
    fn legacy_plaintext_passes_through() {
        key();
        assert_eq!(open("a-pre-envelope-dsn").unwrap(), "a-pre-envelope-dsn");
    }

    #[test]
    fn a_tampered_envelope_fails_loud() {
        key();
        let sealed = seal("value");
        let mut broken = sealed.clone();
        // Flip the last base64 character — AEAD must reject, never return near-plaintext.
        let last = broken.pop().unwrap();
        broken.push(if last == 'A' { 'B' } else { 'A' });
        assert!(matches!(open(&broken), Err(CryptoError::Open)));
        assert!(matches!(
            open("enc:v1:not-base64!!"),
            Err(CryptoError::Open)
        ));
        assert!(matches!(open("enc:v1:AAAA"), Err(CryptoError::Open)));
    }

    #[test]
    fn install_is_once() {
        key();
        assert!(
            !install_master_key([9u8; 32]),
            "second install must be refused"
        );
    }
}
