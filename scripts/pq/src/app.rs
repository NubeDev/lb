use std::env;

use anyhow::Result;

use crate::args::{join_input, split_args};
use crate::cli::{print_help, Cli, CommandLine};
use crate::editor::edit_item;
use crate::library::Library;
use crate::macros::{resolve_macro_vars, run_macro};
use crate::output::{emit, read_primary_input, OutputKind};
use crate::picker::{list_items, pick_and_run};
use crate::role::render_role_by_name;

pub(crate) fn run() -> Result<()> {
    let cli = Cli::parse(env::args().skip(1))?;

    if cli.help {
        print_help();
        return Ok(());
    }

    let library = Library::resolve(cli.dir.as_deref())?;

    match cli.command {
        CommandLine::Pick => pick_and_run(&library, cli.print),
        CommandLine::List => list_items(&library),
        CommandLine::Edit { name } => edit_item(&library, &name),
        CommandLine::Run { name, args } => {
            let vars = resolve_macro_vars(&library.load_macro(&name)?, &args)?;
            let output = run_macro(&library, &name, vars)?;
            emit(output.text, output.destination, cli.print)
        }
        CommandLine::Role { name, args } => {
            let (vars, positional) = split_args(args);
            let input = read_primary_input()?
                .or_else(|| join_input(positional))
                .unwrap_or_default();
            let text = render_role_by_name(&library, &name, input, &vars)?;
            emit(text, OutputKind::Clipboard, cli.print)
        }
    }
}
