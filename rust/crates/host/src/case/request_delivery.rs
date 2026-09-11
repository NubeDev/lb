//! **Reconciling `delivery`** — what actually happened to the ask's email (case-plane scope,
//! resolved decision 7).
//!
//! `delivery` mirrors **the effect's outcome AND the provider kind**, and the second half is the
//! point. A dev node runs `LoggingEmailProvider`, which acknowledges every mail it drops: the
//! outbox row reads `delivered`, the relay pass reports a success, and nobody was told anything.
//! Reading only the effect status would make the drawer say "sent" about mail that went into a log
//! file (`outbox-delivered-is-not-email-sent.md` — the memory this decision is named after).
//!
//! So the answer is assembled from two records:
//!   - the **delivered ledger** row the email target writes per recipient, which now carries the
//!     provider's own [`Disposition`](crate::outbox::Disposition) — `sent` or `logged`;
//!   - the **effect**, for the case where nothing was delivered at all: dead-lettered ⇒ `failed`,
//!     anything else ⇒ still `queued`.
//!
//! Derived on read rather than pushed on delivery: the relay knows nothing about cases (rule 10 —
//! it must not), so a callback from the target into this plane would be exactly the special case
//! the `Target` seam exists to avoid.

use lb_cases::{CaseRequest, Delivery};
use lb_store::Store;

use super::error::CaseSvcError;
use crate::outbox::{delivery_disposition, EMAIL_TARGET};

/// Bring `request.delivery` up to date with what the outbox and the ledger say, persisting it when
/// it moved. Returns the (possibly unchanged) value.
///
/// Never regresses to `queued`: [`lb_cases::request_set_delivery`] refuses that, because "we no
/// longer know" cannot become true about a mail we already attempted.
pub(super) async fn reconcile_delivery(
    store: &Store,
    ws: &str,
    request: &mut CaseRequest,
) -> Result<Delivery, CaseSvcError> {
    if request.delivery != Delivery::Queued {
        return Ok(request.delivery);
    }
    let Some(effect_id) = request.effect_id.clone() else {
        return Ok(request.delivery);
    };
    let Some(recipient) = recipient_of(store, ws, request).await? else {
        return Ok(request.delivery);
    };

    let observed = match delivery_disposition(store, ws, EMAIL_TARGET, &effect_id, &recipient)
        .await
        .map_err(|e| CaseSvcError::Store(e.to_string()))?
    {
        Some(d) if d == "logged" => Delivery::Logged,
        // A ledger row with a disposition we do not recognise is treated as a real send: it was
        // written by a provider that acknowledged the message, and the only value that means
        // "nobody got it" is the one this node writes for its own logging provider.
        Some(_) => Delivery::Sent,
        None => {
            if is_dead_lettered(store, ws, &effect_id).await? {
                Delivery::Failed
            } else {
                Delivery::Queued
            }
        }
    };

    lb_cases::request_set_delivery(store, ws, request, observed).await?;
    Ok(request.delivery)
}

/// The address the ask went to — the party's, read back rather than stored on the request, so a
/// corrected contact does not silently rewrite the history of where a mail was sent (it only
/// affects what this reconcile can find, which is the honest failure: unknown).
async fn recipient_of(
    store: &Store,
    ws: &str,
    request: &CaseRequest,
) -> Result<Option<String>, CaseSvcError> {
    Ok(lb_cases::party_get(store, ws, &request.party_id)
        .await?
        .and_then(|p| p.contact.email))
}

/// Whether the effect was parked. A scan of the (normally tiny) dead-letter set rather than a
/// keyed read, because `lb_outbox` deliberately exposes its table only through its own verbs.
async fn is_dead_lettered(store: &Store, ws: &str, effect_id: &str) -> Result<bool, CaseSvcError> {
    let parked = lb_outbox::dead_lettered(store, ws)
        .await
        .map_err(|e| CaseSvcError::Store(e.to_string()))?;
    Ok(parked.iter().any(|e| e.id == effect_id))
}
