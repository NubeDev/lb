//! Shared real-infra fixture for the `report.export` **media-envelope** suites, `#[path]`-included
//! by `report_export_media_test.rs` (the round trip), `report_export_media_parity_test.rs` (the two
//! doors), `report_export_media_bundle_test.rs` (the malformed-bundle refusals) and
//! `report_export_media_caps_test.rs` (the mandatory deny + isolation categories).
//!
//! Nothing here is a mock (rule 9): a bundle is genuinely uploaded through
//! `media.upload_begin`/`chunk_write`/`commit`, a PDF genuinely comes back out of `media.read`, and
//! the boards are saved through the shipped `dashboard.save` path against a real store.

#![allow(dead_code)] // each including suite uses a subset

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    dashboard_save_meta, media_chunk_put, media_read, media_upload_begin, media_upload_commit,
    Cell, PageMeta,
};
use lb_store::Store;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// A principal `sub` in workspace `ws` holding `caps`.
pub fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

pub const D_GET: &str = "mcp:dashboard.get:call";
pub const D_SAVE: &str = "mcp:dashboard.save:call";
pub const EXPORT: &str = "mcp:report.export:call";
/// ⚠ The **GATE**, not the tool. `gate_tool_for` aliases all three upload phases onto this one cap,
/// and no `mcp:media.upload_begin:call` exists in any role bundle — requesting the literal phase
/// name is the shipped-but-unusable trap `tool_gate.rs` records four times.
pub const M_UPLOAD: &str = "mcp:media.upload:call";
pub const M_READ: &str = "mcp:media.read:call";

/// Everything the round trip needs. `store:media/**:read` is the per-ITEM gate `media_serve` checks
/// behind `mcp:media.read:call` — a grant that reaches no item is a grant that reaches nothing, which
/// `builtin_roles.rs` already has its own test for.
pub const ALL: &[&str] = &[
    D_GET,
    D_SAVE,
    EXPORT,
    M_UPLOAD,
    M_READ,
    "store:media/**:read",
];

/// Upload a JSON document through the REAL three-phase media path and return its id.
pub async fn upload_json(store: &Store, p: &Principal, ws: &str, doc: &Value) -> String {
    let bytes = serde_json::to_vec(doc).unwrap();
    let checksum = {
        let mut h = Sha256::new();
        h.update(&bytes);
        h.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let begun = media_upload_begin(
        store,
        p,
        ws,
        "application/json",
        bytes.len() as u64,
        &checksum,
        Some("test"),
        1,
    )
    .await
    .expect("begin ok");
    let id = begun["id"].as_str().unwrap().to_string();
    for (n, chunk) in bytes.chunks(lb_host::CHUNK_SIZE as usize).enumerate() {
        media_chunk_put(store, p, ws, &id, n as u32, chunk)
            .await
            .expect("chunk ok");
    }
    media_upload_commit(store, p, ws, &id, 1)
        .await
        .expect("commit ok");
    id
}

/// Walk a media item down through `media.read`, exactly as the kit does — slices until `eof`.
pub async fn read_media(store: &Store, p: &Principal, ws: &str, id: &str) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut offset = 0u64;
    for _ in 0..64 {
        let slice = media_read(store, p, ws, id, None, offset, None)
            .await
            .expect("read ok");
        let bytes = BASE64
            .decode(slice["bytes"].as_str().unwrap_or_default())
            .expect("valid base64");
        out.extend_from_slice(&bytes);
        if slice["eof"].as_bool().unwrap_or(false) {
            return out;
        }
        let len = slice["len"].as_u64().unwrap_or(0);
        assert!(len > 0, "an unmoving cursor would loop forever");
        offset += len;
    }
    panic!("media.read did not terminate within 64 slices");
}

/// A grid cell for a report-kind dashboard.
pub fn report_cell(i: &str, x: u32, y: u32, w: u32, h: u32, title: &str) -> Cell {
    Cell {
        i: i.into(),
        x,
        y,
        w,
        h,
        title: title.into(),
        view: "stat".into(),
        ..Cell::default()
    }
}

pub async fn save_report_dashboard(
    store: &Store,
    p: &Principal,
    ws: &str,
    id: &str,
    title: &str,
    cells: Vec<Cell>,
) {
    dashboard_save_meta(
        store,
        p,
        ws,
        id,
        title,
        PageMeta {
            kind: Some("report".into()),
            ..PageMeta::default()
        },
        cells,
        vec![],
        1,
    )
    .await
    .expect("dashboard save ok");
}

/// A real 1x1 PNG — what the browser posts as a panel capture.
pub fn one_px_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xfc,
        0xcf, 0xc0, 0x50, 0x0f, 0x00, 0x04, 0x85, 0x01, 0x80, 0x84, 0xa9, 0x8c, 0x21, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ]
}
