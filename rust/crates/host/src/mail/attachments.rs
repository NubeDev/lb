//! [`AttachmentPolicy`] — what a source does with a message's attachments.
//!
//! Its own file because it answers a different question from the rest of the source record: that one
//! is *where the mailbox is and how to reach it*, this one is *what the files inside are for*. They
//! are edited by different people at different times — the endpoint once at setup, the policy every
//! time a new kind of file starts arriving.

use serde::{Deserialize, Serialize};

/// What to do with a message's attachments.
///
/// The two switches are separate on purpose. `store_bytes` keeps the file (an audit trail, and the
/// thing a human clicks in the inbox); `ingest` turns it into series data. A workspace that only
/// wants the numbers can turn the first off, and one that receives PDFs it cannot decode still keeps
/// them with the second off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentPolicy {
    /// Keep each attachment as a workspace asset.
    #[serde(default = "yes")]
    pub store_bytes: bool,
    /// Decode matching attachments into series samples.
    #[serde(default = "yes")]
    pub ingest: bool,
    /// The decoder to run: `auto` (identify from the bytes) or a named format id. Opaque here —
    /// `lb_ingest::decode` owns the registry, and this service never branches on the value.
    #[serde(default = "default_format")]
    pub format: String,
    /// Only attachments with one of these (lower-case, dotless) extensions are decoded. Empty ⇒ try
    /// every attachment. A filter, not a security control: it saves work, it does not gate reach.
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Prefixed onto every series the decode produces.
    #[serde(default)]
    pub series_prefix: String,
    /// How far ahead of UTC the file's wall-clock timestamps are (`600` for NEM time).
    #[serde(default)]
    pub offset_minutes: i64,
    /// The per-file sample ceiling; `0` ⇒ the decoder default.
    #[serde(default)]
    pub max_samples: usize,
}

fn yes() -> bool {
    true
}

fn default_format() -> String {
    lb_ingest::AUTO.into()
}

impl Default for AttachmentPolicy {
    fn default() -> Self {
        Self {
            store_bytes: true,
            ingest: true,
            format: default_format(),
            extensions: Vec::new(),
            series_prefix: String::new(),
            offset_minutes: 0,
            max_samples: 0,
        }
    }
}

impl AttachmentPolicy {
    /// Should this attachment be handed to a decoder?
    pub fn decodes(&self, extension: &str) -> bool {
        if !self.ingest || self.format.trim().is_empty() {
            return false;
        }
        self.extensions.is_empty()
            || self.extensions.iter().any(|e| {
                e.trim()
                    .trim_start_matches('.')
                    .eq_ignore_ascii_case(extension)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extension_filter_is_case_and_dot_insensitive() {
        let policy = AttachmentPolicy {
            extensions: vec![".CSV".into()],
            ..Default::default()
        };
        assert!(policy.decodes("csv"));
        assert!(!policy.decodes("pdf"));

        let off = AttachmentPolicy {
            ingest: false,
            ..Default::default()
        };
        assert!(!off.decodes("csv"), "ingest off means no decode at all");
    }
}
