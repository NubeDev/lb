//! [`Subject`] — what a grant is *for*: a user, a team, a role, or an API key. The grant store's
//! `subject` column (authz-grants scope: `grant(subject -> cap)` where
//! `subject ∈ {user, team, role, key}`; the `key:` prefix was reserved by auth-caps-scope and is
//! fulfilled by the api-keys scope).
//!
//! Wire form is a single `kind:name` string (`user:ada`, `team:facilities`, `role:operator`,
//! `key:k7f3a`) so a grant record is flat and the store filters on it directly. One concept, four
//! constructors.

use serde::{Deserialize, Serialize};

/// The thing a grant targets. Serialized as its `kind:name` string so a `grant` row carries one
/// `subject` column the store can equality-filter (list a subject's grants in one query).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    /// A single principal — `user:ada`.
    User(String),
    /// A team — `team:facilities`. A user inherits a team's grants via the `member` edge.
    Team(String),
    /// A role — `role:operator`. A cap bundle; assigning a role to a user/team is itself a grant.
    Role(String),
    /// An API key — `key:k7f3a` (api-keys scope). A machine principal whose caps resolve from its
    /// direct grants + roles (NO team-membership edge — keys join no teams in v1).
    Key(String),
}

impl Subject {
    /// The `kind:name` wire string (`user:ada`).
    pub fn as_key(&self) -> String {
        match self {
            Subject::User(n) => format!("user:{n}"),
            Subject::Team(n) => format!("team:{n}"),
            Subject::Role(n) => format!("role:{n}"),
            Subject::Key(n) => format!("key:{n}"),
        }
    }

    /// Parse a `kind:name` wire string back into a `Subject`. Unknown kinds → `None` (deny by
    /// default — an unparseable subject grants nothing, like a malformed cap).
    pub fn parse(s: &str) -> Option<Self> {
        let (kind, name) = s.split_once(':')?;
        if name.is_empty() {
            return None;
        }
        match kind {
            "user" => Some(Subject::User(name.to_string())),
            "team" => Some(Subject::Team(name.to_string())),
            "role" => Some(Subject::Role(name.to_string())),
            "key" => Some(Subject::Key(name.to_string())),
            _ => None,
        }
    }
}

impl Serialize for Subject {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.as_key())
    }
}

impl<'de> Deserialize<'de> for Subject {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Subject::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("bad subject: {s}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_each_kind() {
        for s in [
            Subject::User("ada".into()),
            Subject::Team("facilities".into()),
            Subject::Role("operator".into()),
            Subject::Key("k7f3a".into()),
        ] {
            assert_eq!(Subject::parse(&s.as_key()), Some(s));
        }
    }

    #[test]
    fn rejects_unknown_kind_and_empty_name() {
        assert_eq!(Subject::parse("org:acme"), None);
        assert_eq!(Subject::parse("user:"), None);
        assert_eq!(Subject::parse("nope"), None);
    }

    #[test]
    fn key_subject_has_the_reserved_prefix() {
        // A stored `key:…` grant MUST deserialize back to `Subject::Key` — a missed `"key"` arm in
        // `Subject::parse` would silently resolve every key to no caps (deny everything). Pinned.
        assert_eq!(
            Subject::parse("key:k7f3a"),
            Some(Subject::Key("k7f3a".into()))
        );
        assert_eq!(Subject::Key("k7f3a".into()).as_key(), "key:k7f3a");
    }
}
