//! `lb-pack` — package a built wasm extension into a **signed artifact** for upload.
//!
//! The missing link in the dev flow: `build.sh` produces a `*.wasm`, the gateway's `POST /extensions`
//! (and the UI's UploadArtifact) want a *signed [`Artifact`] JSON*, and nothing in the tree bridged
//! the two outside test fixtures. This does exactly what those fixtures do, as a CLI:
//!   digest(manifest, wasm) → Ed25519-sign the digest → emit the `Artifact` JSON.
//! It reuses `lb_registry::digest` + `ed25519-dalek` verbatim — the SAME idiom the node verifies with
//! — so a packaged artifact verifies by construction (no cross-stack key-encoding seam).
//!
//! Usage:
//!   lb-pack <manifest.toml> <ext.wasm> <key-file> [--key-id <id>] [--out <artifact.json>]
//!   lb-pack pubkey <key-file> [--key-id <id>]      # print just `key_id=hexpubkey` (for LB_TRUSTED_PUBKEYS)
//!
//! The **key file** holds the publisher's 32-byte Ed25519 seed (hex). If it does not exist it is
//! GENERATED and written (dev convenience — a stable dev publisher identity across runs). The matching
//! public key is printed to stderr as `trusted-pubkey: key_id=hexpubkey`, ready to paste into
//! `LB_TRUSTED_PUBKEYS` (the gateway's trust allow-list). The artifact JSON goes to stdout (or `--out`).
//!
//! Trust is environment, never the artifact: signing here does NOT make a node trust this key — an
//! operator must add the printed pubkey to `LB_TRUSTED_PUBKEYS`. That is the whole point of the split.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use ed25519_dalek::{Signer, SigningKey};
use lb_registry::{digest, digest_hex, Artifact};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // `lb-pack pubkey <key-file> [--key-id <id>]` — print just the `key_id=hexpubkey` line (stdout),
    // generating the key if absent. This is what the Makefile feeds into `LB_TRUSTED_PUBKEYS` so the
    // node trusts the dev publisher, without re-packaging an artifact just to learn the public key.
    if args.first().map(String::as_str) == Some("pubkey") {
        return print_pubkey(&args[1..]);
    }

    let parsed = parse_args(&args)?;

    let manifest_toml = fs::read_to_string(&parsed.manifest)
        .with_context(|| format!("read manifest {}", parsed.manifest))?;
    let wasm = fs::read(&parsed.wasm).with_context(|| format!("read wasm {}", parsed.wasm))?;

    let (sk, generated) = load_or_create_key(&parsed.key_file)?;
    let pub_hex = hex(&sk.verifying_key().to_bytes());

    let (ext_id, version) = manifest_id_version(&manifest_toml)?;
    let d = digest(&manifest_toml, &wasm);
    let artifact = Artifact {
        ext_id,
        version,
        manifest_toml,
        wasm,
        digest_hex: digest_hex(&d),
        publisher_key_id: parsed.key_id.clone(),
        signature: sk.sign(&d).to_bytes().to_vec(),
    };

    let json = serde_json::to_string_pretty(&artifact)?;
    match &parsed.out {
        Some(path) => {
            ensure_parent(path)?;
            fs::write(path, &json).with_context(|| format!("write artifact {path}"))?;
            eprintln!("wrote artifact: {path}");
        }
        None => println!("{json}"),
    }

    if generated {
        eprintln!("generated new dev publisher key: {}", parsed.key_file);
    }
    // The line an operator pastes into LB_TRUSTED_PUBKEYS so the gateway will accept this artifact.
    eprintln!("trusted-pubkey: {}={}", parsed.key_id, pub_hex);
    Ok(())
}

/// `pubkey` subcommand: `<key-file> [--key-id <id>]` → print `key_id=hexpubkey` on stdout.
fn print_pubkey(args: &[String]) -> Result<()> {
    let mut key_file = None;
    let mut key_id = "dev-publisher".to_string();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--key-id" => {
                key_id = args
                    .get(i + 1)
                    .cloned()
                    .ok_or_else(|| anyhow!("--key-id needs a value"))?;
                i += 2;
            }
            other if other.starts_with("--") => bail!("unknown flag {other}"),
            other => {
                key_file = Some(other.to_string());
                i += 1;
            }
        }
    }
    let key_file =
        key_file.ok_or_else(|| anyhow!("usage: lb-pack pubkey <key-file> [--key-id <id>]"))?;
    let (sk, _) = load_or_create_key(&key_file)?;
    println!("{}={}", key_id, hex(&sk.verifying_key().to_bytes()));
    Ok(())
}

struct ParsedArgs {
    manifest: String,
    wasm: String,
    key_file: String,
    key_id: String,
    out: Option<String>,
}

fn parse_args(args: &[String]) -> Result<ParsedArgs> {
    let mut positional = Vec::new();
    let mut key_id = "dev-publisher".to_string();
    let mut out = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--key-id" => {
                key_id = args
                    .get(i + 1)
                    .cloned()
                    .ok_or_else(|| anyhow!("--key-id needs a value"))?;
                i += 2;
            }
            "--out" => {
                out = Some(
                    args.get(i + 1)
                        .cloned()
                        .ok_or_else(|| anyhow!("--out needs a value"))?,
                );
                i += 2;
            }
            other if other.starts_with("--") => bail!("unknown flag {other}"),
            other => {
                positional.push(other.to_string());
                i += 1;
            }
        }
    }
    if positional.len() != 3 {
        bail!(
            "usage: lb-pack <manifest.toml> <ext.wasm> <key-file> [--key-id <id>] [--out <file>]"
        );
    }
    Ok(ParsedArgs {
        manifest: positional[0].clone(),
        wasm: positional[1].clone(),
        key_file: positional[2].clone(),
        key_id,
        out,
    })
}

/// Load the 32-byte Ed25519 seed (hex) from `path`, or generate + persist one if absent. Returns the
/// signing key and whether it was freshly generated (so the caller can tell the operator).
fn load_or_create_key(path: &str) -> Result<(SigningKey, bool)> {
    if Path::new(path).exists() {
        let hexed = fs::read_to_string(path).with_context(|| format!("read key {path}"))?;
        let bytes = decode_hex(hexed.trim())?;
        let seed: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow!("key file {path} must be a 32-byte (64 hex char) seed"))?;
        Ok((SigningKey::from_bytes(&seed), false))
    } else {
        let sk = SigningKey::generate(&mut rand_core::OsRng);
        ensure_parent(path)?;
        fs::write(path, hex(&sk.to_bytes())).with_context(|| format!("write key {path}"))?;
        Ok((sk, true))
    }
}

/// Create the parent directory of `path` if it has one (so `--out`/the key file can name a dir that
/// does not exist yet — the dev `.lazybones/...` layout is created on demand).
fn ensure_parent(path: &str) -> Result<()> {
    if let Some(parent) = Path::new(path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).with_context(|| format!("create dir {parent:?}"))?;
    }
    Ok(())
}

/// Pull `id`/`version` out of the manifest's `[extension]` table so the artifact carries them. A tiny
/// line scan (not a full toml dep): the manifest is the loader's source of truth; we only need two
/// fields for the artifact metadata (the node re-parses the full manifest on install).
fn manifest_id_version(toml: &str) -> Result<(String, String)> {
    let id = scan_field(toml, "id").ok_or_else(|| anyhow!("manifest missing [extension] id"))?;
    let version = scan_field(toml, "version")
        .ok_or_else(|| anyhow!("manifest missing [extension] version"))?;
    Ok((id, version))
}

/// Find `key = "value"` and return the unquoted value. First match wins (the `[extension]` block is
/// first in our manifests). Good enough for the two fields we read here.
fn scan_field(toml: &str, key: &str) -> Option<String> {
    for line in toml.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                return Some(v.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn decode_hex(s: &str) -> Result<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        bail!("hex length must be even");
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(Into::into))
        .collect()
}
