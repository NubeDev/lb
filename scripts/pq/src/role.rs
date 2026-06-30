use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::library::Library;
use crate::template::render_template;

#[derive(Debug)]
pub(crate) struct Role {
    pub(crate) name: String,
    pub(crate) desc: Option<String>,
    body: String,
}

#[derive(Debug, Deserialize)]
struct RoleFrontmatter {
    desc: Option<String>,
}

pub(crate) fn parse_role(name: &str, path: PathBuf, raw: &str) -> Result<Role> {
    let (frontmatter, body) = split_frontmatter(raw);
    let meta = if let Some(frontmatter) = frontmatter {
        serde_yml::from_str::<RoleFrontmatter>(frontmatter)
            .with_context(|| format!("failed to parse frontmatter in {}", path.display()))?
    } else {
        RoleFrontmatter { desc: None }
    };

    Ok(Role {
        name: name.to_string(),
        desc: meta.desc,
        body: body.trim_start_matches('\n').to_string(),
    })
}

pub(crate) fn render_role_by_name(
    library: &Library,
    name: &str,
    input: String,
    vars: &BTreeMap<String, String>,
) -> Result<String> {
    let role = library.load_role(name)?;
    render_role(&role, input, vars)
}

fn render_role(role: &Role, input: String, vars: &BTreeMap<String, String>) -> Result<String> {
    let mut context = vars.clone();
    context.insert("input".to_string(), input.clone());

    let mut text = render_template(&role.body, &context)
        .with_context(|| format!("failed to render role {}", role.name))?;

    if !has_input_placeholder(&role.body) && !input.is_empty() {
        if text.ends_with('\n') {
            text.push('\n');
        } else {
            text.push_str("\n\n");
        }
        text.push_str(&input);
    }

    Ok(text)
}

fn split_frontmatter(raw: &str) -> (Option<&str>, &str) {
    let Some(rest) = raw.strip_prefix("---\n") else {
        return (None, raw);
    };
    let Some((frontmatter, body)) = rest.split_once("\n---") else {
        return (None, raw);
    };
    let body = body.strip_prefix('\n').unwrap_or(body);
    (Some(frontmatter), body)
}

fn has_input_placeholder(template: &str) -> bool {
    template.contains("{{input}}") || template.contains("{{ input }}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_role_and_appends_input_when_placeholder_is_absent() {
        let role = Role {
            name: "plain".to_string(),
            desc: None,
            body: "Explain this".to_string(),
        };

        let rendered = render_role(&role, "fn main() {}".to_string(), &BTreeMap::new()).unwrap();

        assert_eq!(rendered, "Explain this\n\nfn main() {}");
    }

    #[test]
    fn renders_role_variables() {
        let role = Role {
            name: "var".to_string(),
            desc: None,
            body: "Hello {{name}}: {{input}}".to_string(),
        };
        let vars = BTreeMap::from([("name".to_string(), "Ada".to_string())]);

        let rendered = render_role(&role, "ship it".to_string(), &vars).unwrap();

        assert_eq!(rendered, "Hello Ada: ship it");
    }
}
