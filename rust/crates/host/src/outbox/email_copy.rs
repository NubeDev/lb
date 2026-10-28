//! `sent_copy` — the subject and plain-text body an email effect will go out as, for a record the
//! sender keeps (a case request's Email record, Insights review 2026-10). Rendered by the SAME
//! `content_for` the email target delivers with, from the same payload, so the record cannot drift
//! from the mail. The one difference is the link: the record says "(reply link)", because the raw
//! token it carries must never be stored.
//!
//! One responsibility: an email effect to the copy a record keeps.

use lb_outbox::Effect;

use super::email_content::content_for;
use super::email_payload::EmailPayload;

/// `effect`'s subject and text body, with the reply link (`…/r/{token}`) replaced by "(reply link)".
/// `None` for a payload the target itself would refuse.
pub fn sent_copy(effect: &Effect, token: &str) -> Option<(String, String)> {
    let (payload, _) = EmailPayload::parse(&effect.payload).ok()?;
    let content = content_for(&effect.action, &payload, None);
    let body = content.text.replace(&format!("/r/{token}"), "(reply link)");
    Some((content.subject, body))
}
