//! `lb-pack` — package a built extension into the signed `Artifact` accepted by the gateway, as
//! either wire shape: JSON (the original, default — `wasm` inline as a decimal-int array, fine for a
//! small wasm module) or `.zip` (`--format zip` — the binary as a real archive member instead, for a
//! native sidecar binary large enough that JSON's ~4-8x inflation would blow past upload/memory
//! limits; see `lb_registry::zip`). Default stays JSON so nothing depending on today's output shape
//! breaks; `--format zip` is opt-in.
//!
//! The signing implementation lives in `lb-devkit`; this binary is only argument parsing, file I/O,
//! and operator-facing output. That keeps the SDK path and CLI path on one crypto idiom regardless of
//! which wire shape gets written — both formats carry the exact same signed `Artifact`.

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use lb_devkit::{load_or_create_key, publisher_trust_line, sign_artifact};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Json,
    Zip,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("pubkey") {
        return print_pubkey(&args[1..]);
    }

    let parsed = parse_args(&args)?;
    let manifest_toml = fs::read_to_string(&parsed.manifest)
        .with_context(|| format!("read manifest {}", parsed.manifest))?;
    let wasm = fs::read(&parsed.wasm).with_context(|| format!("read wasm {}", parsed.wasm))?;
    let loaded = load_or_create_key(&parsed.key_file)?;
    let artifact = sign_artifact(
        manifest_toml,
        wasm,
        parsed.key_id.clone(),
        &loaded.signing_key,
    )?;

    let bytes: Vec<u8> = match parsed.format {
        Format::Json => serde_json::to_string_pretty(&artifact)?.into_bytes(),
        Format::Zip => lb_registry::artifact_to_zip(&artifact)?,
    };

    match &parsed.out {
        Some(path) => {
            ensure_parent(path)?;
            fs::write(path, &bytes).with_context(|| format!("write artifact {path}"))?;
            eprintln!("wrote artifact: {path}");
        }
        None if parsed.format == Format::Zip => {
            std::io::stdout()
                .write_all(&bytes)
                .context("write artifact to stdout")?;
        }
        None => println!("{}", String::from_utf8_lossy(&bytes)),
    }

    if loaded.generated {
        eprintln!("generated new dev publisher key: {}", parsed.key_file);
    }
    eprintln!(
        "trusted-pubkey: {}",
        publisher_trust_line(&parsed.key_id, &loaded.signing_key)
    );
    Ok(())
}

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
    let loaded = load_or_create_key(&key_file)?;
    println!("{}", publisher_trust_line(&key_id, &loaded.signing_key));
    Ok(())
}

struct ParsedArgs {
    manifest: String,
    wasm: String,
    key_file: String,
    key_id: String,
    out: Option<String>,
    format: Format,
}

fn parse_args(args: &[String]) -> Result<ParsedArgs> {
    let mut positional = Vec::new();
    let mut key_id = "dev-publisher".to_string();
    let mut out = None;
    let mut format = Format::Json;
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
            "--format" => {
                let value = args
                    .get(i + 1)
                    .cloned()
                    .ok_or_else(|| anyhow!("--format needs a value (json|zip)"))?;
                format = match value.as_str() {
                    "json" => Format::Json,
                    "zip" => Format::Zip,
                    other => bail!("unknown --format {other} (expected json|zip)"),
                };
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
            "usage: lb-pack <manifest.toml> <ext.wasm> <key-file> [--key-id <id>] [--out <file>] \
             [--format json|zip]"
        );
    }
    Ok(ParsedArgs {
        manifest: positional[0].clone(),
        wasm: positional[1].clone(),
        key_file: positional[2].clone(),
        key_id,
        out,
        format,
    })
}

fn ensure_parent(path: &str) -> Result<()> {
    if let Some(parent) = Path::new(path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;
    }
    Ok(())
}
