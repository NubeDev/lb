use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};

#[derive(Debug)]
pub(crate) struct Cli {
    pub(crate) dir: Option<PathBuf>,
    pub(crate) print: bool,
    pub(crate) help: bool,
    pub(crate) command: CommandLine,
}

#[derive(Debug)]
pub(crate) enum CommandLine {
    Pick,
    List,
    Edit { name: String },
    Run { name: String, args: Vec<String> },
    Role { name: String, args: Vec<String> },
}

impl Cli {
    pub(crate) fn parse<I>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = String>,
    {
        let mut dir = None;
        let mut print = false;
        let mut help = false;
        let mut positional = Vec::new();
        let mut iter = args.into_iter().peekable();
        let mut literal_args = false;

        while let Some(arg) = iter.next() {
            if literal_args {
                positional.push(arg);
                continue;
            }

            if arg == "--" {
                literal_args = true;
            } else if arg == "-p" || arg == "--print" {
                print = true;
            } else if arg == "-h" || arg == "--help" {
                help = true;
            } else if arg == "--dir" {
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow!("--dir requires a path argument"))?;
                dir = Some(PathBuf::from(value));
            } else if let Some(value) = arg.strip_prefix("--dir=") {
                dir = Some(PathBuf::from(value));
            } else {
                positional.push(arg);
            }
        }

        let command = match positional.as_slice() {
            [] => CommandLine::Pick,
            [cmd] if cmd == "ls" => CommandLine::List,
            [cmd, name, rest @ ..] if cmd == "edit" => {
                if !rest.is_empty() {
                    bail!("pq edit accepts exactly one role or macro name");
                }
                CommandLine::Edit { name: name.clone() }
            }
            [cmd] if cmd == "edit" => bail!("pq edit requires a role or macro name"),
            [cmd, name, rest @ ..] if cmd == "run" => CommandLine::Run {
                name: name.clone(),
                args: rest.to_vec(),
            },
            [cmd] if cmd == "run" => bail!("pq run requires a macro name"),
            [name, rest @ ..] => CommandLine::Role {
                name: name.clone(),
                args: rest.to_vec(),
            },
        };

        Ok(Self {
            dir,
            print,
            help,
            command,
        })
    }
}

pub(crate) fn print_help() {
    println!(
        "\
pq - reusable prompt and workflow CLI

Usage:
  pq [--dir PATH] [-p|--print] [role] [input|name=value...]
  pq [--dir PATH] [-p|--print] run <macro> [input|name=value...]
  pq [--dir PATH] ls
  pq [--dir PATH] edit <role-or-macro>

Options:
  --dir PATH      Prompt library directory
  -p, --print     Print final output to stdout instead of clipboard
  -h, --help      Show this help
"
    );
}
