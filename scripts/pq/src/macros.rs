use std::collections::BTreeMap;
use std::io::{self, IsTerminal};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::args::split_args;
use crate::library::Library;
use crate::output::OutputKind;
use crate::role::render_role_by_name;
use crate::template::render_template;

#[derive(Debug, Deserialize)]
pub(crate) struct MacroFile {
    pub(crate) desc: Option<String>,
    #[serde(default)]
    vars: Vec<MacroVar>,
    steps: Vec<MacroStep>,
    #[serde(default)]
    output: OutputKind,
}

#[derive(Debug, Deserialize)]
struct MacroVar {
    name: String,
    #[serde(default)]
    rest: bool,
    default: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MacroStep {
    tool: Option<String>,
    role: Option<String>,
    input: Option<String>,
}

#[derive(Debug)]
pub(crate) struct MacroOutput {
    pub(crate) text: String,
    pub(crate) destination: OutputKind,
}

pub(crate) fn run_macro(
    library: &Library,
    name: &str,
    mut vars: BTreeMap<String, String>,
) -> Result<MacroOutput> {
    let macro_file = library.load_macro(name)?;
    let mut last = String::new();

    for (index, step) in macro_file.steps.iter().enumerate() {
        match (&step.tool, &step.role) {
            (Some(_), Some(_)) => bail!(
                "macro {name} step {} cannot have both tool and role",
                index + 1
            ),
            (None, None) => bail!("macro {name} step {} must have tool or role", index + 1),
            (Some(tool), None) => {
                vars.insert("last".to_string(), last.clone());
                let command_line = render_template(tool, &vars)
                    .with_context(|| format!("failed to render tool step {}", index + 1))?;
                last = run_tool(&command_line)?;
            }
            (None, Some(role_name)) => {
                vars.insert("last".to_string(), last.clone());
                let input = if let Some(input) = &step.input {
                    render_template(input, &vars)
                        .with_context(|| format!("failed to render input for step {}", index + 1))?
                } else {
                    last.clone()
                };
                last = render_role_by_name(library, role_name, input, &vars)?;
            }
        }
    }

    Ok(MacroOutput {
        text: last,
        destination: macro_file.output,
    })
}

pub(crate) fn resolve_macro_vars(
    macro_file: &MacroFile,
    args: &[String],
) -> Result<BTreeMap<String, String>> {
    let (mut passed, mut positional) = split_args(args.to_vec());
    let mut resolved = BTreeMap::new();

    for var in &macro_file.vars {
        let value = if let Some(value) = passed.remove(&var.name) {
            value
        } else if var.rest {
            if positional.is_empty() {
                resolve_default_or_prompt(var, &resolved)?
            } else {
                let value = positional.join(" ");
                positional.clear();
                value
            }
        } else if !positional.is_empty() {
            positional.remove(0)
        } else {
            resolve_default_or_prompt(var, &resolved)?
        };
        resolved.insert(var.name.clone(), value);
    }

    resolved.extend(passed);

    if !positional.is_empty() {
        bail!("unexpected positional args: {}", positional.join(" "));
    }

    Ok(resolved)
}

fn resolve_default_or_prompt(
    var: &MacroVar,
    resolved: &BTreeMap<String, String>,
) -> Result<String> {
    if let Some(default) = &var.default {
        return render_template(default, resolved)
            .with_context(|| format!("failed to render default for {}", var.name));
    }

    if io::stdin().is_terminal() {
        return inquire::Text::new(&format!("{}:", var.name))
            .prompt()
            .with_context(|| format!("failed to prompt for {}", var.name));
    }

    bail!("missing required variable `{}`", var.name)
}

fn run_tool(command_line: &str) -> Result<String> {
    let output = shell_command(command_line)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("failed to run tool command `{command_line}`"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "tool command failed with status {}: `{}`\n{}",
            output.status,
            command_line,
            stderr.trim()
        );
    }

    String::from_utf8(output.stdout).context("tool command produced non-UTF-8 output")
}

fn shell_command(command_line: &str) -> Command {
    #[cfg(windows)]
    {
        let mut command = Command::new("cmd");
        command.arg("/C").arg(command_line);
        command
    }

    #[cfg(not(windows))]
    {
        let mut command = Command::new("sh");
        command.arg("-c").arg(command_line);
        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn resolves_rest_macro_var_from_positional_args() {
        let macro_file = MacroFile {
            desc: None,
            vars: vec![MacroVar {
                name: "idea".to_string(),
                rest: true,
                default: None,
            }],
            steps: vec![],
            output: OutputKind::Clipboard,
        };

        let vars = resolve_macro_vars(
            &macro_file,
            &["let".to_string(), "users".to_string(), "pin".to_string()],
        )
        .unwrap();

        assert_eq!(vars.get("idea").unwrap(), "let users pin");
    }

    #[test]
    fn runs_tool_then_role_macro() {
        let temp = fixture_library();
        fs::write(
            temp.path().join("roles").join("wrap.md"),
            "---\ndesc: Wrap input\n---\nwrapped: {{input}}",
        )
        .unwrap();
        fs::write(
            temp.path().join("macros").join("wrap.yaml"),
            "steps:\n  - tool: printf hello\n  - role: wrap\noutput: stdout\n",
        )
        .unwrap();
        let library = Library::from_root(temp.path().to_path_buf());

        let output = run_macro(&library, "wrap", BTreeMap::new()).unwrap();

        assert_eq!(output.text, "wrapped: hello");
        assert_eq!(output.destination, OutputKind::Stdout);
    }

    fn fixture_library() -> TempDir {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join("roles")).unwrap();
        fs::create_dir(temp.path().join("macros")).unwrap();
        temp
    }
}
