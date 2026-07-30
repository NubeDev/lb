//! [`MailMessage`] — the message to submit, and its rendering to RFC 5322 bytes.
//!
//! Two things here are load-bearing beyond "make a MIME blob":
//!
//! 1. **The plain-text alternative is not optional.** A single-part `text/html` mail scores badly with
//!    every spam filter and is unreadable in a text client. So an HTML body is always sent as
//!    `multipart/alternative` with a text part — supplied by the caller (the translated catalog key) or
//!    generated here by stripping tags when the catalog has no text half. The generated fallback is a
//!    fallback, not the plan: translators should see both keys.
//! 2. **`Message-ID` is injectable, and the caller injects the outbox idempotency key.** Email has no
//!    collapse key, so the at-least-once outbox can genuinely put the same message on the wire twice
//!    (accepted-but-ack-lost). A stable `Message-ID` across retries is what lets the *receiving* side
//!    collapse the duplicate — most MTAs and every major webmail dedup on it. This is a mitigation,
//!    not exactly-once: an MTA is free to ignore it. The delivered-marker ledger on the target side is
//!    the other half, and the crash-between-accept-and-marker window remains real.
//!
//! `Date` comes from the wall clock at render time. That is a deliberate exception to "no wall-clock in
//! a crate" (testing §3): a message without a `Date` header is non-conformant and gets filed as spam,
//! and the value must be the real submission time, not an injected logical tick. Nothing in the
//! platform reads it back, so it stays out of every assertion — tests pin `message_id`, never `Date`.

use mail_builder::MessageBuilder;

use crate::error::{MailError, MailResult};

/// One outbound message. Addresses are already validated/normalized by the caller — this struct is
/// the transport's input, not a user-facing form.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MailMessage {
    /// The display name for the `From` header (may be empty).
    pub from_name: String,
    /// The `From` address — the identity the recipient sees.
    pub from_addr: String,
    /// The recipient address.
    pub to: String,
    /// An optional `Reply-To`.
    pub reply_to: Option<String>,
    /// The subject. Non-ASCII is encoded by the builder (RFC 2047).
    pub subject: String,
    /// The plain-text body. Empty + an HTML body ⇒ a text part is generated from the HTML.
    pub text: String,
    /// The optional HTML body. When set the message is `multipart/alternative`.
    pub html: Option<String>,
    /// An optional stable `Message-ID` (WITHOUT angle brackets) — the retry-dedup handle described
    /// above. `None` ⇒ the builder generates a random one.
    pub message_id: Option<String>,
    /// Optional attachments as `(content_type, filename, bytes)`.
    pub attachments: Vec<(String, String, Vec<u8>)>,
}

impl MailMessage {
    /// A minimal text message.
    pub fn new(
        from_addr: impl Into<String>,
        to: impl Into<String>,
        subject: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            from_addr: from_addr.into(),
            to: to.into(),
            subject: subject.into(),
            text: text.into(),
            ..Default::default()
        }
    }

    /// Render to RFC 5322 bytes (headers + body), ready for `DATA`.
    ///
    /// Fails **permanently** on an absent `From`/`To`: a message with no envelope is a construction
    /// bug or a bad effect payload, and no number of retries will grow an address.
    pub fn to_rfc5322(&self) -> MailResult<Vec<u8>> {
        if self.from_addr.trim().is_empty() {
            return Err(MailError::Permanent("mail: message has no From".into()));
        }
        if self.to.trim().is_empty() {
            return Err(MailError::Permanent(
                "mail: message has no recipient".into(),
            ));
        }

        let mut builder = MessageBuilder::new()
            .from((self.from_name.as_str(), self.from_addr.as_str()))
            .to(self.to.as_str())
            .subject(self.subject.as_str());

        if let Some(reply_to) = self.reply_to.as_deref().filter(|r| !r.trim().is_empty()) {
            builder = builder.reply_to(reply_to);
        }
        if let Some(id) = self.message_id.as_deref().filter(|i| !i.trim().is_empty()) {
            builder = builder.message_id(id);
        }

        match self.html.as_deref().filter(|h| !h.trim().is_empty()) {
            // HTML present ⇒ multipart/alternative with a text part, always (see the module note).
            Some(html) => {
                builder = builder
                    .text_body(self.text_or_generated(html))
                    .html_body(html);
            }
            None => builder = builder.text_body(self.text.as_str()),
        }

        for (content_type, filename, bytes) in &self.attachments {
            builder =
                builder.attachment(content_type.as_str(), filename.as_str(), bytes.as_slice());
        }

        builder
            .write_to_vec()
            // The only failure mode of an in-memory write is OOM — permanent, not a retry.
            .map_err(|e| MailError::Permanent(format!("mail: render message: {e}")))
    }

    /// The text part to pair with `html`: the caller's text when it has one, else generated from the
    /// HTML.
    fn text_or_generated(&self, html: &str) -> String {
        if !self.text.trim().is_empty() {
            return self.text.clone();
        }
        text_from_html(html)
    }
}

/// Strip HTML to a readable plain-text approximation — the *fallback* text part.
///
/// Deliberately small and dependency-free: drop tags, keep the anchor `href` (an invite email whose
/// text part loses the accept link is useless), turn block ends into newlines, and unescape the five
/// XML entities. Not a general HTML-to-text renderer, and not a sanitizer — the input is our own
/// catalog-rendered template, never user HTML.
pub fn text_from_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut chars = html.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '<' {
            out.push(c);
            continue;
        }
        // Collect the tag body up to '>'.
        let mut tag = String::new();
        for tc in chars.by_ref() {
            if tc == '>' {
                break;
            }
            tag.push(tc);
        }
        let lower = tag.trim().to_ascii_lowercase();
        if lower.starts_with("br") || lower.starts_with("/p") || lower.starts_with("/div") {
            out.push('\n');
        }
        if lower.starts_with('a') {
            if let Some(href) = attr_value(&tag, "href") {
                // Keep the destination inline: "text <https://…>" survives the strip.
                out.push_str(" <");
                out.push_str(&href);
                out.push_str("> ");
            }
        }
    }
    let out = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    // Collapse the whitespace the tag-strip left behind, keeping paragraph breaks.
    out.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Pull `name="value"` (or `name='value'`) out of a tag body.
fn attr_value(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let at = lower.find(&format!("{name}="))? + name.len() + 1;
    let rest = &tag[at..];
    let quote = rest.chars().next()?;
    if quote == '"' || quote == '\'' {
        let end = rest[1..].find(quote)? + 1;
        Some(rest[1..end].to_string())
    } else {
        Some(
            rest.split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_body_always_carries_a_text_alternative() {
        let msg = MailMessage {
            from_addr: "reports@acme.com".into(),
            to: "sam@example.com".into(),
            subject: "Invited".into(),
            html: Some("<p>Join <a href=\"https://acme/accept?t=1\">now</a></p>".into()),
            ..Default::default()
        };
        let bytes = String::from_utf8(msg.to_rfc5322().unwrap()).unwrap();
        assert!(bytes.contains("multipart/alternative"), "{bytes}");
        assert!(bytes.contains("text/plain"), "{bytes}");
        // The generated text part must keep the accept link — a text-only reader must still be able
        // to accept the invite.
        assert!(bytes.contains("https://acme/accept?t=1"), "{bytes}");
    }

    #[test]
    fn message_id_is_injectable_for_retry_dedup() {
        let msg = MailMessage {
            message_id: Some("invite-hash1@lazybones".into()),
            ..MailMessage::new("a@b.c", "d@e.f", "s", "t")
        };
        let bytes = String::from_utf8(msg.to_rfc5322().unwrap()).unwrap();
        assert!(
            bytes.contains("Message-ID: <invite-hash1@lazybones>"),
            "{bytes}"
        );
    }

    #[test]
    fn no_recipient_is_permanent_not_retryable() {
        let msg = MailMessage::new("a@b.c", "", "s", "t");
        assert!(msg.to_rfc5322().unwrap_err().is_permanent());
    }

    #[test]
    fn text_from_html_keeps_links_and_drops_tags() {
        let text =
            text_from_html("<div><b>Hi</b> &amp; welcome<br/><a href='http://x/y'>go</a></div>");
        assert!(
            !text.contains('<') || text.contains("<http://x/y>"),
            "{text}"
        );
        assert!(text.contains("Hi & welcome"), "{text}");
        assert!(text.contains("http://x/y"), "{text}");
    }
}
