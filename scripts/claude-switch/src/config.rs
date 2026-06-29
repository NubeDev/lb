//! YAML-backed configuration: the set of named providers and which one is active.
//!
//! One responsibility: load/save the `~/.config/claude-switch/config.yaml` file and
//! the in-memory model of a "provider" (a base URL + an ordered `env` block that gets
//! written into Claude Code's `settings.json`).
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// The whole persisted state: the active provider name + the provider catalog.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// Name of the provider currently applied (or will be applied) to Claude Code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
    /// Provider definitions, keyed by short name (`claude`, `glm`, …).
    #[serde(default)]
    pub providers: BTreeMap<String, Provider>,
}

/// A single switchable server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    /// Human-readable note shown by `list`/`show`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Convenience: the base URL. Mirrored into `env` when present (kept separate so the
    /// UI can display it prominently without scraping the env map).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// The exact `env` block written into Claude Code's `settings.json`.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl Config {
    /// Resolve `~/.config/claude-switch/config.yaml`, honouring `$XDG_CONFIG_HOME`.
    pub fn path() -> Result<PathBuf> {
        let base = dirs::config_dir()
            .context("could not determine the user config directory ($XDG_CONFIG_HOME)")?;
        Ok(base.join("claude-switch").join("config.yaml"))
    }

    /// Load the config, creating it with sensible defaults on first run.
    pub fn load_or_init() -> Result<Self> {
        let path = Self::path()?;
        if path.exists() {
            Self::load_from(&path)
        } else {
            let cfg = Self::defaults();
            cfg.save_to(&path)
                .with_context(|| format!("failed to write initial config to {}", path.display()))?;
            Ok(cfg)
        }
    }

    fn load_from(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read config at {}", path.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        serde_yaml::from_str(&raw)
            .with_context(|| format!("failed to parse YAML config at {}", path.display()))
    }

    /// Persist the config, creating the parent directory first.
    pub fn save(&self) -> Result<()> {
        Self::save_to(self, &Self::path()?)
    }

    fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let yaml = serde_yaml::to_string(self).context("failed to serialize config to YAML")?;
        fs::write(path, yaml)
            .with_context(|| format!("failed to write config to {}", path.display()))
    }

    /// Seed the two providers documented for this tool: official Claude and the z.ai GLM
    /// coding plan. Tokens are left as placeholders the user must fill in via `add`/`edit`.
    fn defaults() -> Self {
        let mut glm_env = BTreeMap::new();
        glm_env.insert(
            "ANTHROPIC_BASE_URL".into(),
            "https://api.z.ai/api/anthropic".into(),
        );
        glm_env.insert(
            "ANTHROPIC_AUTH_TOKEN".into(),
            "REPLACE_WITH_YOUR_ZAI_API_KEY".into(),
        );
        glm_env.insert("API_TIMEOUT_MS".into(), "3000000".into());
        glm_env.insert("CLAUDE_CODE_AUTO_COMPACT_WINDOW".into(), "1000000".into());
        glm_env.insert("ANTHROPIC_DEFAULT_HAIKU_MODEL".into(), "glm-4.7".into());
        glm_env.insert(
            "ANTHROPIC_DEFAULT_SONNET_MODEL".into(),
            "glm-5.2[1m]".into(),
        );
        glm_env.insert("ANTHROPIC_DEFAULT_OPUS_MODEL".into(), "glm-5.2[1m]".into());

        let mut claude_env = BTreeMap::new();
        claude_env.insert(
            "ANTHROPIC_AUTH_TOKEN".into(),
            "REPLACE_WITH_YOUR_ANTHROPIC_API_KEY".into(),
        );
        claude_env.insert(
            "ANTHROPIC_BASE_URL".into(),
            "https://api.anthropic.com".into(),
        );

        let mut providers = BTreeMap::new();
        providers.insert(
            "glm".into(),
            Provider {
                description: Some("Z.AI GLM coding plan (GLM-5.2, 1M context)".into()),
                base_url: Some("https://api.z.ai/api/anthropic".into()),
                env: glm_env,
            },
        );
        providers.insert(
            "claude".into(),
            Provider {
                description: Some("Official Anthropic Claude API".into()),
                base_url: Some("https://api.anthropic.com".into()),
                env: claude_env,
            },
        );

        Self {
            active: Some("glm".into()),
            providers,
        }
    }

    /// Look up a provider by name.
    pub fn get(&self, name: &str) -> Option<&Provider> {
        self.providers.get(name)
    }
}
