use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, IsTerminal};

use anyhow::{bail, Context, Result};

use crate::library::{ItemSummary, Library};
use crate::macros::{resolve_macro_vars, run_macro};
use crate::output::{emit, OutputKind};
use crate::role::render_role_by_name;

pub(crate) fn list_items(library: &Library) -> Result<()> {
    for role in library.list_roles()? {
        println!("role  {:<18} {}", role.name, role.desc.unwrap_or_default());
    }
    for macro_item in library.list_macros()? {
        println!(
            "macro {:<18} {}",
            macro_item.name,
            macro_item.desc.unwrap_or_default()
        );
    }
    Ok(())
}

pub(crate) fn pick_and_run(library: &Library, force_stdout: bool) -> Result<()> {
    if !io::stdin().is_terminal() {
        bail!("interactive picker requires a TTY; pass a role or macro name instead");
    }

    let mut items = Vec::new();
    items.extend(library.list_roles()?.into_iter().map(PickItem::Role));
    items.extend(library.list_macros()?.into_iter().map(PickItem::Macro));

    if items.is_empty() {
        bail!("no roles or macros found in {}", library.root().display());
    }

    let selected = inquire::Select::new("Select a role or macro", items)
        .prompt()
        .context("selection failed")?;

    match selected {
        PickItem::Role(role) => {
            let text = render_role_by_name(library, &role.name, String::new(), &BTreeMap::new())?;
            emit(text, OutputKind::Clipboard, force_stdout)
        }
        PickItem::Macro(macro_item) => {
            let macro_file = library.load_macro(&macro_item.name)?;
            let vars = resolve_macro_vars(&macro_file, &[])?;
            let output = run_macro(library, &macro_item.name, vars)?;
            emit(output.text, output.destination, force_stdout)
        }
    }
}

#[derive(Debug)]
enum PickItem {
    Role(ItemSummary),
    Macro(ItemSummary),
}

impl fmt::Display for PickItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PickItem::Role(role) => write!(
                f,
                "role  {:<18} {}",
                role.name,
                role.desc.as_deref().unwrap_or_default()
            ),
            PickItem::Macro(macro_item) => write!(
                f,
                "macro {:<18} {}",
                macro_item.name,
                macro_item.desc.as_deref().unwrap_or_default()
            ),
        }
    }
}
