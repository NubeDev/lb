//! `nav.ext_boards.get` / `nav.ext_boards.set` — the host-authored ext nav boards record
//! (host-authored-ext-nav-boards scope). Mirrors `nav.hidden.*` exactly: ONE workspace record,
//! get-list + full-set LWW write, absent record == empty map.
//!
//! **Which cap each rides, and why they differ.** The READ is member-level and rides
//! `mcp:nav.resolve:call`, like `nav.hidden.get`: every member's rail must be able to see the rows,
//! or a board an admin placed would be invisible to the very people it was placed for. The WRITE is
//! authoring and rides `mcp:nav.save:call`, like `nav.hidden.set` / `nav.set_default` — no separate
//! cap is minted for one more curation record.
//!
//! ⚠️ Both verbs MUST be aliased in `tool_call::gate_tool_for`. A verb riding an existing cap is
//! gated on its own namesake by default, and `mcp:nav.ext_boards.set:call` exists in no role bundle
//! — without the alias the outer gate answers `denied` for every caller, admins included, while
//! every direct-call test stays green (`call_*_tool` tests never cross that gate).
//!
//! **The rows grant nothing.** A row is a lens: it contributes no reach cap of its own, it follows
//! the section's reach, and the board's own viewer gate remains the authority (scope, "Reach").
//! Rule 10: slot keys and dashboard refs are opaque — no extension is named or branched on here.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_nav;
use super::error::NavError;
use super::ext_boards_model::{
    ExtBoardRow, ExtNavBoards, MAX_EXT_BOARD_ID_LEN, MAX_EXT_BOARD_LABEL_LEN, MAX_EXT_BOARD_ROWS,
    MAX_EXT_BOARD_SLOTS, MAX_EXT_BOARD_TOTAL, MAX_EXT_BOARD_VARS,
};
use super::model::{MAX_ICON_COLOR_LEN, MAX_ICON_LEN};
use super::store::{read_ext_boards, write_ext_boards};

/// The slot-ref prefix. This is the REF GRAMMAR, not an extension name — `ext:` is the same opaque
/// namespace `NavItem`/`nav_hidden` already use for "an extension section". Validating it turns a
/// typo'd key (which would bind rows to a slot nothing renders) into a loud `BadInput`.
const SLOT_PREFIX: &str = "ext:";

/// Read the workspace's host-authored ext board rows. Absent → an empty map. Member-level: this is
/// part of resolving one's own menu, so it rides `mcp:nav.resolve:call`.
pub async fn ext_nav_boards_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<ExtNavBoards, NavError> {
    authorize_nav(principal, ws, "nav.resolve")?;
    Ok(read_ext_boards(store, ws).await?.unwrap_or_default())
}

/// Replace the workspace's host-authored ext board rows (full-set LWW on the one `[ws]` record; an
/// empty map clears it), at logical time `now`. Bounded — an over-cap or malformed input is
/// `BadInput`, never a silent truncation. Admin write, rides `mcp:nav.save:call`.
pub async fn ext_nav_boards_set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    slots: std::collections::BTreeMap<String, Vec<ExtBoardRow>>,
    now: u64,
) -> Result<ExtNavBoards, NavError> {
    authorize_nav(principal, ws, "nav.save")?;
    validate(&slots)?;
    let record = ExtNavBoards {
        slots,
        updated_ts: now,
    };
    write_ext_boards(store, ws, &record).await?;
    Ok(record)
}

/// The bounds + shape checks for a full-set write. Kept whole in one place so "what is a legal
/// record" has a single owner and the error text names the offending key.
fn validate(slots: &std::collections::BTreeMap<String, Vec<ExtBoardRow>>) -> Result<(), NavError> {
    if slots.len() > MAX_EXT_BOARD_SLOTS {
        return Err(NavError::BadInput(format!(
            "ext boards name {} slots, exceeds cap {MAX_EXT_BOARD_SLOTS}",
            slots.len()
        )));
    }
    let total: usize = slots.values().map(|r| r.len()).sum();
    if total > MAX_EXT_BOARD_TOTAL {
        return Err(NavError::BadInput(format!(
            "ext boards hold {total} rows, exceeds cap {MAX_EXT_BOARD_TOTAL}"
        )));
    }
    for (slot, rows) in slots {
        let slot = slot.trim();
        if slot.is_empty() {
            return Err(NavError::BadInput("slot ref must be non-empty".into()));
        }
        // The grammar, not an identity: `ext:<id>` | `ext:<id>/<navid>`. A key outside it would
        // bind rows to a slot no renderer looks at — silent data loss dressed as a save.
        if !slot.starts_with(SLOT_PREFIX) || slot.len() == SLOT_PREFIX.len() {
            return Err(NavError::BadInput(format!(
                "slot ref {slot:?} must be {SLOT_PREFIX}<id> or {SLOT_PREFIX}<id>/<navid>"
            )));
        }
        if rows.len() > MAX_EXT_BOARD_ROWS {
            return Err(NavError::BadInput(format!(
                "slot {slot:?} holds {} rows, exceeds cap {MAX_EXT_BOARD_ROWS}",
                rows.len()
            )));
        }
        let mut seen = std::collections::BTreeSet::new();
        for row in rows {
            validate_row(slot, row)?;
            if !seen.insert(row.id.trim()) {
                return Err(NavError::BadInput(format!(
                    "slot {slot:?} row id {:?} appears more than once",
                    row.id
                )));
            }
        }
    }
    Ok(())
}

/// One row's shape. `id` is a ref SEGMENT, so it may not carry the `/` the ref grammar separates on.
fn validate_row(slot: &str, row: &ExtBoardRow) -> Result<(), NavError> {
    let id = row.id.trim();
    if id.is_empty() {
        return Err(NavError::BadInput(format!(
            "slot {slot:?} has a row with an empty id"
        )));
    }
    if id.len() > MAX_EXT_BOARD_ID_LEN {
        return Err(NavError::BadInput(format!(
            "row id {id:?} exceeds {MAX_EXT_BOARD_ID_LEN} chars"
        )));
    }
    if id.contains('/') {
        return Err(NavError::BadInput(format!(
            "row id {id:?} may not contain '/' — it is one ref segment"
        )));
    }
    if row.dashboard.trim().is_empty() {
        return Err(NavError::BadInput(format!(
            "row {id:?} must name a dashboard ref"
        )));
    }
    if row.label.len() > MAX_EXT_BOARD_LABEL_LEN {
        return Err(NavError::BadInput(format!(
            "row {id:?} label exceeds {MAX_EXT_BOARD_LABEL_LEN} chars"
        )));
    }
    if row.icon.len() > MAX_ICON_LEN {
        return Err(NavError::BadInput(format!(
            "row {id:?} icon exceeds {MAX_ICON_LEN} chars"
        )));
    }
    if row.icon_color.len() > MAX_ICON_COLOR_LEN {
        return Err(NavError::BadInput(format!(
            "row {id:?} iconColor exceeds {MAX_ICON_COLOR_LEN} chars"
        )));
    }
    if row.vars.len() > MAX_EXT_BOARD_VARS {
        return Err(NavError::BadInput(format!(
            "row {id:?} binds {} vars, exceeds cap {MAX_EXT_BOARD_VARS}",
            row.vars.len()
        )));
    }
    Ok(())
}
