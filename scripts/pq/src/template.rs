use std::collections::BTreeMap;

use anyhow::{Context, Result};
use minijinja::{Environment, UndefinedBehavior};

pub(crate) fn render_template(
    template: &str,
    context: &BTreeMap<String, String>,
) -> Result<String> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.render_str(template, context)
        .context("template rendering failed")
}
