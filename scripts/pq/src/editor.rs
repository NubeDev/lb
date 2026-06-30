use std::env;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::library::Library;

pub(crate) fn edit_item(library: &Library, name: &str) -> Result<()> {
    let role_path = library.role_path(name);
    let macro_path = library.macro_path(name);
    let path = if role_path.is_file() {
        role_path
    } else if macro_path.is_file() {
        macro_path
    } else {
        bail!("no role or macro named `{name}`");
    };

    let editor = env::var("EDITOR")
        .or_else(|_| env::var("VISUAL"))
        .unwrap_or_else(|_| default_editor().to_string());

    let status = editor_command(&editor, &path)
        .status()
        .with_context(|| format!("failed to start editor `{editor}`"))?;

    if !status.success() {
        bail!("editor exited with status {status}");
    }

    Ok(())
}

fn default_editor() -> &'static str {
    if cfg!(windows) {
        "notepad"
    } else {
        "vi"
    }
}

fn editor_command(editor: &str, path: &Path) -> Command {
    #[cfg(windows)]
    {
        let mut command = Command::new("cmd");
        command
            .arg("/C")
            .arg(format!("\"{}\" \"{}\"", editor, path.display()));
        command
    }

    #[cfg(not(windows))]
    {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("exec \"$EDITOR\" \"$1\"")
            .arg("sh")
            .arg(path)
            .env("EDITOR", editor);
        command
    }
}
