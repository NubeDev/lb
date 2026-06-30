//! The switching action. One responsibility: make a named provider the active one
//! and push its `env` block into Claude Code's `settings.json`.
use crate::config::{Config, Provider};
use crate::output;
use crate::settings;
use anyhow::{bail, Result};
use colored::Colorize;

/// Switch the active provider to `name` and write it into settings.json.
/// Returns the path of settings.json that was written.
pub fn run(cfg: &mut Config, name: &str, quiet: bool) -> Result<()> {
    let provider: Provider = match cfg.get(name) {
        Some(p) => p.clone(),
        None => {
            bail!("unknown provider '{name}'. Known: {}", provider_list(cfg));
        }
    };

    let was = cfg.active.clone();
    let path = settings::write_env(&provider.env)?;
    cfg.active = Some(name.to_string());
    cfg.save()?;

    if !quiet {
        match &was {
            Some(prev) if prev != name => output::success(format!(
                "switched {prev} → {name} and applied to {}",
                path.display()
            )),
            _ => output::success(format!(
                "'{name}' is already active — reapplied to {}",
                path.display()
            )),
        }
        print_provider(name, &provider);
    }
    Ok(())
}

/// Re-apply whichever provider is marked active (or nothing if none).
pub fn apply_active(cfg: &Config) -> Result<()> {
    let name = match &cfg.active {
        Some(n) => n.clone(),
        None => bail!("no active provider set; run `claude-switch use <provider>` first"),
    };
    let provider = cfg
        .get(&name)
        .ok_or_else(|| anyhow::anyhow!("active provider '{name}' is missing from the config"))?;
    let path = settings::write_env(&provider.env)?;
    output::success(format!("reapplied '{name}' to {}", path.display()));
    Ok(())
}

/// Render a provider's summary (base url + env block, secrets masked).
pub fn print_provider(name: &str, p: &Provider) {
    output::header(format!("provider: {name}"));
    if let Some(d) = &p.description {
        output::kv("description", d);
    }
    if let Some(u) = &p.base_url {
        output::kv("base url", u);
    }
    println!("{}:", "  env".dimmed());
    for (k, v) in &p.env {
        println!("{}", output::env_line(k, v));
    }
}

/// A comma-joined list of provider names, for error messages.
pub fn provider_list(cfg: &Config) -> String {
    let names: Vec<&str> = cfg.providers.keys().map(String::as_str).collect();
    names.join(", ")
}
