use std::io::{self, IsTerminal, Read, Write};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Copy, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OutputKind {
    Clipboard,
    Stdout,
}

impl Default for OutputKind {
    fn default() -> Self {
        Self::Clipboard
    }
}

pub(crate) fn read_primary_input() -> Result<Option<String>> {
    if io::stdin().is_terminal() {
        return Ok(None);
    }

    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("failed to read stdin")?;

    if input.is_empty() {
        Ok(None)
    } else {
        Ok(Some(input))
    }
}

pub(crate) fn emit(text: String, destination: OutputKind, force_stdout: bool) -> Result<()> {
    if force_stdout || destination == OutputKind::Stdout {
        print!("{text}");
        io::stdout().flush().context("failed to flush stdout")?;
        return Ok(());
    }

    let mut clipboard = arboard::Clipboard::new()
        .context("clipboard unavailable; rerun with -p/--print to write stdout")?;
    clipboard
        .set_text(text)
        .context("failed to write final output to clipboard")?;
    Ok(())
}
