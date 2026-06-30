use std::collections::BTreeMap;

pub(crate) fn split_args(args: Vec<String>) -> (BTreeMap<String, String>, Vec<String>) {
    let mut vars = BTreeMap::new();
    let mut positional = Vec::new();

    for arg in args {
        if let Some((name, value)) = arg.split_once('=') {
            if is_var_name(name) {
                vars.insert(name.to_string(), value.to_string());
                continue;
            }
        }
        positional.push(arg);
    }

    (vars, positional)
}

pub(crate) fn join_input(positional: Vec<String>) -> Option<String> {
    if positional.is_empty() {
        None
    } else {
        Some(positional.join(" "))
    }
}

fn is_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|char| char == '_' || char.is_ascii_alphanumeric())
}
