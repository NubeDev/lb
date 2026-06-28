use lb_devkit::{load_or_create_key, publisher_trust_line, sign_artifact};
use lb_registry::{verify_artifact, PublisherKey, TrustedKeys};

fn temp_case(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("lb-devkit-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp case dir");
    dir
}

fn manifest(id: &str, version: &str) -> String {
    format!(
        r#"[extension]
id = "{id}"
version = "{version}"
tier = "wasm"
"#
    )
}

fn trusted_from_line(line: &str) -> TrustedKeys {
    let (key_id, hexed) = line.split_once('=').expect("trust line");
    let bytes: Vec<u8> = (0..hexed.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hexed[i..i + 2], 16).expect("hex byte"))
        .collect();
    let key: [u8; 32] = bytes.try_into().expect("32-byte public key");
    TrustedKeys::from([(key_id.to_string(), PublisherKey::from_bytes(&key).unwrap())])
}

#[test]
fn signs_artifact_the_registry_verifies() {
    let dir = temp_case("signs");
    let key_path = dir.join("dev-publisher.key");
    let loaded = load_or_create_key(&key_path).expect("key generated");
    assert!(loaded.generated);

    let artifact = sign_artifact(
        manifest("devkit-proof", "0.1.0"),
        b"\0asm-devkit-proof".to_vec(),
        "dev-publisher",
        &loaded.signing_key,
    )
    .expect("artifact signed");
    let trust = trusted_from_line(&publisher_trust_line("dev-publisher", &loaded.signing_key));
    let verified = verify_artifact(artifact.clone(), &trust).expect("registry verifies artifact");

    assert_eq!(verified.artifact().ext_id, "devkit-proof");
    assert_eq!(artifact.version, "0.1.0");
}

#[test]
fn reuses_existing_publisher_seed() {
    let dir = temp_case("reuses");
    let key_path = dir.join("dev-publisher.key");
    let first = load_or_create_key(&key_path).expect("key generated");
    let first_line = publisher_trust_line("dev-publisher", &first.signing_key);

    let second = load_or_create_key(&key_path).expect("key loaded");
    let second_line = publisher_trust_line("dev-publisher", &second.signing_key);

    assert!(!second.generated);
    assert_eq!(first_line, second_line);
}
