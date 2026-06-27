//! [`Subject`] — what a grant is *for*: a user, a team, or a role. The grant store's `subject`
//! column (authz-grants scope: `grant(subject -> cap)` where `subject ∈ {user, team, role}`).
//!
//! Wire form is a single `kind:name` string (`user:ada`, `team:facilities`, `role:operator`) so a
//! grant record is flat and the store filters on it directly. One concept, three constructors.

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
}

impl Subject {
    /// The `kind:name` wire string (`user:ada`).
    pub fn as_key(&self) -> String {
        match self {
            Subject::User(n) => format!("user:{n}"),
            Subject::Team(n) => format!("team:{n}"),
            Subject::Role(n) => format!("role:{n}"),
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
}
