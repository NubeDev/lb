//! Where diskwarden keeps its policy file, and the default roots to watch.
//!
//! XDG: `~/.config/diskwarden/policy.toml`. One file, hand-editable, and also what
//! the tray writes back when you flip a toggle.

use std::path::PathBuf;

use anyhow::Context;
use diskwarden_reclaim::Policy;

/// `~/.config/diskwarden/policy.toml`
pub fn policy_file() -> anyhow::Result<PathBuf> {
    let dir = dirs::config_dir().context("no XDG config dir (is HOME set?)")?;
    Ok(dir.join("diskwarden").join("policy.toml"))
}

/// Load the policy, or fall back to defaults if there's no file yet.
///
/// A file that exists but is malformed is a hard error, never a silent fallback to
/// defaults: if you typo `auto_cleen = true`, you must hear about it rather than
/// have the tool quietly run with settings you didn't choose.
pub fn load_policy() -> anyhow::Result<Policy> {
    let path = policy_file()?;
    if !path.exists() {
        let mut p = Policy::default();
        p.general.roots = default_roots();
        return Ok(p);
    }
    let src =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut p = Policy::parse(&src).with_context(|| format!("parsing {}", path.display()))?;
    if p.general.roots.is_empty() {
        p.general.roots = default_roots();
    }
    Ok(p)
}

/// Write the policy back. Atomic: write a temp file beside the target, then rename,
/// so a crash mid-write can't leave you with a truncated policy that fails to parse
/// (and, worse, would then be a hard error on next start).
pub fn save_policy(policy: &Policy) -> anyhow::Result<()> {
    let path = policy_file()?;
    let dir = path.parent().context("policy path has no parent")?;
    std::fs::create_dir_all(dir)?;

    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, policy.to_toml()?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Watch `~/code` by default — where the build artifacts actually are.
fn default_roots() -> Vec<PathBuf> {
    dirs::home_dir()
        .map(|h| vec![h.join("code")])
        .unwrap_or_default()
}
