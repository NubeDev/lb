//! The job record and the on-disk store shape.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// The whole `jobs.yaml`: just a list of jobs.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub jobs: Vec<Job>,
}

/// One scheduled auto-commit-and-push job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    /// Stable, unique id (used in the systemd unit names `gj-<id>.{service,timer}`).
    pub id: String,
    /// Absolute path to the git repo whose working tree is committed + pushed.
    pub repo: String,
    /// Branch to push (`git push origin <branch>`). `None` ⇒ the repo's current branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// How often to fire, as a human duration (`10m`, `1h`, `30s`, `2d`). Drives the
    /// systemd timer's `OnUnitActiveSec`/`OnBootSec`.
    #[serde(default = "default_every")]
    pub every: String,
    /// The commit message template. `{n}` = staged file count, `{t}` = UTC timestamp.
    #[serde(default = "default_message")]
    pub message: String,
    /// Whether the timer is active. `gj disable` flips this and stops the timer.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Job {
    /// `OnBootSec`/`OnUnitActiveSec` clause lines for the `[Timer]` section.
    pub fn timer_clause(&self) -> Result<String> {
        let secs = every_seconds(&self.every)?;
        Ok(format!("OnBootSec={secs}s\nOnUnitActiveSec={secs}s\n"))
    }

    pub fn branch_display(&self) -> &str {
        self.branch.as_deref().unwrap_or("(current)")
    }

    pub fn state_display(&self) -> &str {
        if self.enabled {
            "enabled"
        } else {
            "disabled"
        }
    }
}

pub fn default_every() -> String {
    "10m".to_string()
}

pub fn default_message() -> String {
    "chore(autocommit): {n} files @ {t}".to_string()
}

fn default_true() -> bool {
    true
}

/// Parse a human duration (`30s`, `10m`, `1h`, `2d`) into seconds.
pub fn every_seconds(spec: &str) -> Result<u64> {
    let spec = spec.trim();
    let (num, unit) = spec.split_at(
        spec.find(|c: char| !c.is_ascii_digit())
            .unwrap_or(spec.len()),
    );
    let value: u64 = num
        .parse()
        .map_err(|_| anyhow::anyhow!("bad --every '{spec}' (use e.g. 30s, 10m, 1h, 2d)"))?;
    let mult = match unit {
        "s" | "" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86_400,
        other => bail!("bad --every unit '{other}' (use s, m, h, or d)"),
    };
    if value == 0 {
        bail!("--every must be > 0");
    }
    Ok(value * mult)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_durations() {
        assert_eq!(every_seconds("30s").unwrap(), 30);
        assert_eq!(every_seconds("10").unwrap(), 10); // bare number = seconds
        assert_eq!(every_seconds("10m").unwrap(), 600);
        assert_eq!(every_seconds("1h").unwrap(), 3600);
        assert_eq!(every_seconds("2d").unwrap(), 172_800);
    }

    #[test]
    fn rejects_bad_durations() {
        assert!(every_seconds("0m").is_err());
        assert!(every_seconds("10x").is_err());
        assert!(every_seconds("abc").is_err());
    }

    #[test]
    fn timer_clause_uses_seconds() {
        let job = Job {
            id: "x".into(),
            repo: "/r".into(),
            branch: None,
            every: "10m".into(),
            message: default_message(),
            enabled: true,
        };
        let clause = job.timer_clause().unwrap();
        assert!(clause.contains("OnUnitActiveSec=600s"));
        assert!(clause.contains("OnBootSec=600s"));
    }
}
