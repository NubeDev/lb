//! [`parse_message`] — RFC 822 octets → a [`ParsedMail`], via `mail-parser`.
//!
//! **Never fails, by design.** The mail-source scope's containment rule is that the raw message is
//! stored first and normalization is allowed to be imperfect; a parser that returns `Err` on a
//! malformed message would tempt every caller into dropping the mail instead. So the only failure
//! this can express is "these bytes are not a message at all", and even that returns a `ParsedMail`
//! carrying the bytes as its body rather than nothing — the caller still gets an item it can show a
//! human, and the original is still on disk to re-parse after a parser fix.
//!
//! Everything here is a pure function of the input bytes. No clock (the `Date` header is the
//! sender's, and is labelled untrusted), no network, no store.

use mail_parser::{Addr, Address, MessageParser, MimeHeaders};

use super::message::{MailAddress, MailAttachment, ParsedMail};

/// The upper bound on a body part we will carry in memory as a `String`. A body is a *summary*
/// surface here (an inbox item's text); the raw message is always the fidelity escape hatch, so
/// truncating a pathological 200 MB HTML body is strictly better than holding it twice.
pub const MAX_BODY_BYTES: usize = 1 << 20; // 1 MiB

/// Parse `raw` (RFC 822 octets) into the normalized shape.
///
/// Returns a `ParsedMail` unconditionally — see the module doc. When the bytes do not parse as a
/// message at all, the result has an empty subject, no addresses, and the raw bytes (lossily decoded)
/// as its text body, so the arrival is still visible rather than silently absent.
pub fn parse_message(raw: &[u8]) -> ParsedMail {
    let Some(msg) = MessageParser::default().parse(raw) else {
        return ParsedMail {
            text: Some(truncate(&String::from_utf8_lossy(raw))),
            ..Default::default()
        };
    };

    let attachments = msg
        .attachments()
        .map(|part| MailAttachment {
            filename: part
                .attachment_name()
                .unwrap_or_default()
                .trim()
                .to_string(),
            mime: part
                .content_type()
                .map(|ct| match ct.subtype() {
                    Some(sub) => format!("{}/{}", ct.ctype(), sub).to_ascii_lowercase(),
                    None => ct.ctype().to_ascii_lowercase(),
                })
                // The honest default when the sender declared nothing. NOT sniffed: guessing a type
                // from the bytes here would let a sender's content decide how a downstream decoder
                // treats it, which is a decision the *importing workspace's* config owns.
                .unwrap_or_else(|| "application/octet-stream".into()),
            bytes: part.contents().to_vec(),
        })
        .collect();

    let mut mail = ParsedMail {
        message_id: msg.message_id().map(strip_angles),
        in_reply_to: msg.in_reply_to().as_text().map(strip_angles),
        from: msg.from().and_then(first_address),
        to: msg.to().map(all_addresses).unwrap_or_default(),
        subject: msg.subject().unwrap_or_default().trim().to_string(),
        // `to_timestamp` is epoch SECONDS; the platform's `ts` vocabulary is milliseconds
        // everywhere (see the `Insight ts seconds-vs-millis` class of bug), so the conversion
        // happens HERE, once, rather than at each of the callers that would otherwise each guess.
        // A pre-epoch date is dropped rather than wrapped into a huge u64.
        date_ms: msg
            .date()
            .map(|d| d.to_timestamp())
            .filter(|s| *s > 0)
            .map(|s| s as u64 * 1000),
        text: msg.body_text(0).map(|t| truncate(&t)),
        html: msg.body_html(0).map(|t| truncate(&t)),
        attachments,
    };
    // The never-lose-the-mail floor. `mail-parser` is tolerant enough to return a `Message` for
    // almost any byte sequence — including one with no body part it can name — so the `else` above
    // catches far less than it looks like it does. A message that yielded no text, no HTML, and no
    // attachment carries its own octets as its body instead, so the arrival is still something a
    // human can look at. (Headers-only mail lands here too, which is the right answer: the headers
    // ARE what arrived.)
    if mail.text.is_none() && mail.html.is_none() && mail.attachments.is_empty() {
        mail.text = Some(truncate(&String::from_utf8_lossy(raw)));
    }
    mail
}

/// Strip the angle brackets a `Message-ID` / `In-Reply-To` header carries.
fn strip_angles(s: &str) -> String {
    s.trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim()
        .to_string()
}

/// The first mailbox of an address header (`From:` is a single mailbox in practice).
fn first_address(addr: &Address<'_>) -> Option<MailAddress> {
    addr.first().and_then(to_mail_address)
}

/// Every mailbox of an address header, flattened out of any groups.
fn all_addresses(addr: &Address<'_>) -> Vec<MailAddress> {
    match addr {
        Address::List(list) => list.iter().filter_map(to_mail_address).collect(),
        Address::Group(groups) => groups
            .iter()
            .flat_map(|g| g.addresses.iter())
            .filter_map(to_mail_address)
            .collect(),
    }
}

/// One `mail-parser` mailbox → our own. An entry with no addr-spec (a bare group name, a malformed
/// header) is dropped: an address with no address is not one.
fn to_mail_address(addr: &Addr<'_>) -> Option<MailAddress> {
    let address = addr.address()?.trim();
    (!address.is_empty())
        .then(|| MailAddress::new(addr.name().map(|n| n.trim().to_string()), address))
}

/// Bound a body part at [`MAX_BODY_BYTES`], on a char boundary.
fn truncate(s: &str) -> String {
    if s.len() <= MAX_BODY_BYTES {
        return s.to_string();
    }
    let mut end = MAX_BODY_BYTES;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A multipart message with a plain body and one CSV attachment — the shape the whole
    /// mail→ingest path is built around.
    const WITH_CSV: &[u8] = b"From: Meter Data <data@example.com>\r\n\
To: alerts@nube-io.com\r\n\
Subject: NEM12 for July\r\n\
Message-ID: <abc-123@example.com>\r\n\
Date: Tue, 25 Aug 2026 08:10:00 +1000\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/mixed; boundary=\"BB\"\r\n\
\r\n\
--BB\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
Attached is the interval data.\r\n\
--BB\r\n\
Content-Type: text/csv; name=\"meter.csv\"\r\n\
Content-Disposition: attachment; filename=\"meter.csv\"\r\n\
\r\n\
100,NEM12,202608250810,TCAUSTM\r\n\
--BB--\r\n";

    #[test]
    fn a_multipart_message_yields_body_and_attachment() {
        let mail = parse_message(WITH_CSV);
        assert_eq!(mail.subject, "NEM12 for July");
        assert_eq!(mail.message_id.as_deref(), Some("abc-123@example.com"));
        assert_eq!(mail.from_address(), "data@example.com");
        assert_eq!(
            mail.from.as_ref().unwrap().name.as_deref(),
            Some("Meter Data")
        );
        assert_eq!(mail.to.len(), 1);
        assert_eq!(mail.to[0].address, "alerts@nube-io.com");
        assert!(mail.body().contains("interval data"), "{}", mail.body());
        assert_eq!(mail.attachments.len(), 1);
        assert_eq!(mail.attachments[0].filename, "meter.csv");
        assert_eq!(mail.attachments[0].mime, "text/csv");
        assert_eq!(mail.attachments[0].extension(), "csv");
        assert!(mail.attachments[0].bytes.starts_with(b"100,NEM12"));
    }

    #[test]
    fn the_date_header_is_milliseconds_not_seconds() {
        // 2026-08-25T08:10:00+10:00 == 2026-08-24T22:10:00Z == 1_787_609_400s.
        let mail = parse_message(WITH_CSV);
        assert_eq!(mail.date_ms, Some(1_787_609_400_000));
    }

    #[test]
    fn an_address_is_lower_cased_so_an_allowlist_is_a_plain_comparison() {
        let raw = b"From: SHOUTY <Data@EXAMPLE.Com>\r\nSubject: hi\r\n\r\nbody\r\n";
        let mail = parse_message(raw);
        assert_eq!(mail.from_address(), "data@example.com");
        assert_eq!(mail.from.as_ref().unwrap().domain(), "example.com");
    }

    #[test]
    fn a_message_with_no_message_id_still_parses() {
        let raw = b"From: a@b.com\r\nSubject: no id\r\n\r\nbody text\r\n";
        let mail = parse_message(raw);
        assert_eq!(
            mail.message_id, None,
            "the ledger's hash fallback exists for this"
        );
        assert_eq!(mail.subject, "no id");
        assert_eq!(mail.body().trim(), "body text");
    }

    #[test]
    fn garbage_bytes_never_lose_the_mail() {
        let mail = parse_message(b"\x00\x01 not a message at all");
        assert!(
            mail.body().contains("not a message"),
            "the bytes survive as a body"
        );
    }

    #[test]
    fn an_html_only_body_is_still_a_body() {
        let raw = b"From: a@b.com\r\nSubject: html\r\nContent-Type: text/html\r\n\r\n<p>hi</p>\r\n";
        let mail = parse_message(raw);
        assert!(mail.html.is_some());
        assert!(mail.body().contains("hi"), "{}", mail.body());
    }

    #[test]
    fn an_eight_bit_subject_is_decoded() {
        let raw = b"From: a@b.com\r\nSubject: =?utf-8?B?w6FjbWU=?=\r\n\r\nbody\r\n";
        assert_eq!(parse_message(raw).subject, "ácme");
    }
}
