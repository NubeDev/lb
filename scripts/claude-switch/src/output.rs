//! Terminal output helpers. One responsibility: print consistently styled,
//! color-coded messages. No business logic lives here.
use colored::*;

/// A 2-space gutter used to align key/value pairs.
const GUTTER: &str = "  ";

pub fn info(msg: impl AsRef<str>) {
    println!("{} {}", "›".cyan().bold(), msg.as_ref());
}

pub fn success(msg: impl AsRef<str>) {
    println!("{} {}", "✓".green().bold(), msg.as_ref());
}

pub fn warn(msg: impl AsRef<str>) {
    eprintln!("{} {}", "!".yellow().bold(), msg.as_ref());
}

pub fn error(msg: impl AsRef<str>) {
    eprintln!("{} {}", "✗".red().bold(), msg.as_ref());
}

pub fn header(title: impl AsRef<str>) {
    println!();
    println!("{}", title.as_ref().bold().underline());
}

/// Print a labelled value, e.g. `  base url   https://...`.
pub fn kv(key: impl AsRef<str>, value: impl AsRef<str>) {
    println!("{GUTTER}{:<12} {}", key.as_ref().dimmed(), value.as_ref());
}

/// Mask a secret value for display: show only the last 4 characters.
pub fn mask(secret: &str) -> String {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return "(empty)".to_string();
    }
    if trimmed.len() <= 4 {
        return "••••".to_string();
    }
    let tail = &trimmed[trimmed.len() - 4..];
    format!("••••{tail}")
}

/// A single line of an `env` entry, with known token keys masked.
pub fn env_line(key: &str, value: &str) -> String {
    let is_secret = key.contains("TOKEN") || key.contains("KEY") || key.contains("SECRET");
    let shown = if is_secret {
        mask(value)
    } else {
        value.to_string()
    };
    format!("{GUTTER}  {} = {}", key.cyan(), shown)
}
