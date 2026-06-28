//! `evaluate` — the **pure** policy evaluator (agent-run scope Part 2). Tool name + parsed args +
//! policy → an [`Effect`]. No store, no async, no I/O — unit-testable in isolation (and so the loop
//! pays nothing to consult it).
//!
//! **Precedence is fixed, not list-order: Deny beats Allow beats Ask.** Every rule that matches
//! (name glob, and the optional shallow arg equality) contributes its effect; the strongest wins.
//! Rationale: a Deny is a hard stop a later Allow must never silently override, and an Ask (a human
//! gate) is the weakest — if any rule already Allows the call outright, there is nothing to ask. This
//! also makes the policy order-insensitive, so an admin appending a rule can never accidentally
//! weaken an existing Deny by placement.
//!
//! **Default (no rule matches) is Allow** — the policy only *adds* gating over the existing behavior
//! (every workspace without a policy, and every tool no rule names, runs exactly as before). The
//! Allow it returns is still subject to `caps::check` downstream (defense in depth).
//!
//! Rejected alternative: first-match-wins (like a firewall ruleset). It is more familiar but couples
//! correctness to author ordering — exactly the footgun a small, safety-oriented policy should avoid.
//! With three effects and a clear safety ordering, max-by-precedence is both simpler and safer.

use serde_json::Value;

use super::glob::matches;
use super::model::{Effect, Policy, Rule};

/// Evaluate `policy` for a proposed call to `tool` with already-parsed `args`. Returns the effect of
/// the highest-precedence matching rule (Deny > Allow > Ask), or [`Effect::Allow`] if none match.
pub fn evaluate(policy: &Policy, tool: &str, args: &Value) -> Effect {
    let mut chosen: Option<Effect> = None;
    for rule in &policy.rules {
        if rule_matches(rule, tool, args) {
            chosen = Some(stronger(chosen, rule.effect));
            // A Deny is the strongest possible outcome — short-circuit once one matches.
            if chosen == Some(Effect::Deny) {
                return Effect::Deny;
            }
        }
    }
    chosen.unwrap_or(Effect::Allow)
}

/// Whether `rule` applies to this call: the tool-name glob matches AND (if present) the shallow arg
/// equality holds. A missing `arg` matches on the name alone.
fn rule_matches(rule: &Rule, tool: &str, args: &Value) -> bool {
    if !matches(&rule.tool, tool) {
        return false;
    }
    match &rule.arg {
        None => true,
        Some(m) => arg_equals(args, &m.path, &m.equals),
    }
}

/// Shallow top-level arg equality: the value at the top-level key `path` must stringify to `equals`.
/// A JSON string compares by its inner text (no quotes); any other scalar/value by its JSON form, so
/// `{"path":"n","equals":"5"}` matches `{"n":5}`. A missing key never matches.
fn arg_equals(args: &Value, path: &str, equals: &str) -> bool {
    match args.get(path) {
        Some(Value::String(s)) => s == equals,
        Some(v) => v.to_string() == equals,
        None => false,
    }
}

/// Combine the running choice with a newly-matched effect under the fixed precedence
/// (Deny > Allow > Ask). The first match seeds the choice.
fn stronger(current: Option<Effect>, next: Effect) -> Effect {
    match current {
        None => next,
        Some(cur) => {
            if rank(next) >= rank(cur) {
                next
            } else {
                cur
            }
        }
    }
}

/// The precedence rank: higher wins. Deny (2) > Allow (1) > Ask (0).
fn rank(e: Effect) -> u8 {
    match e {
        Effect::Deny => 2,
        Effect::Allow => 1,
        Effect::Ask => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::policy::model::{ArgMatch, Rule};
    use serde_json::json;

    fn rule(tool: &str, effect: Effect) -> Rule {
        Rule {
            tool: tool.into(),
            arg: None,
            effect,
        }
    }

    #[test]
    fn default_is_allow_when_no_rule_matches() {
        let policy = Policy {
            rules: vec![rule("shell.*", Effect::Deny)],
        };
        assert_eq!(evaluate(&policy, "hello.echo", &json!({})), Effect::Allow);
    }

    #[test]
    fn empty_policy_allows_everything() {
        assert_eq!(
            evaluate(&Policy::default(), "anything.at.all", &json!({})),
            Effect::Allow
        );
    }

    #[test]
    fn glob_matches_on_tool_name() {
        let policy = Policy {
            rules: vec![rule("shell.*", Effect::Ask)],
        };
        assert_eq!(evaluate(&policy, "shell.run", &json!({})), Effect::Ask);
        assert_eq!(evaluate(&policy, "shellx.run", &json!({})), Effect::Allow);
    }

    #[test]
    fn deny_beats_allow_beats_ask_regardless_of_order() {
        // All three match `shell.run`; the order is Ask, Allow, Deny and reversed — both → Deny.
        let policy = Policy {
            rules: vec![
                rule("shell.run", Effect::Ask),
                rule("shell.*", Effect::Allow),
                rule("*", Effect::Deny),
            ],
        };
        assert_eq!(evaluate(&policy, "shell.run", &json!({})), Effect::Deny);

        let reordered = Policy {
            rules: vec![
                rule("*", Effect::Deny),
                rule("shell.*", Effect::Allow),
                rule("shell.run", Effect::Ask),
            ],
        };
        assert_eq!(evaluate(&reordered, "shell.run", &json!({})), Effect::Deny);
    }

    #[test]
    fn allow_beats_ask() {
        let policy = Policy {
            rules: vec![
                rule("shell.run", Effect::Ask),
                rule("shell.run", Effect::Allow),
            ],
        };
        assert_eq!(evaluate(&policy, "shell.run", &json!({})), Effect::Allow);
    }

    #[test]
    fn shallow_arg_path_equality_match() {
        let policy = Policy {
            rules: vec![Rule {
                tool: "shell.run".into(),
                arg: Some(ArgMatch {
                    path: "cmd".into(),
                    equals: "rm -rf".into(),
                }),
                effect: Effect::Deny,
            }],
        };
        // Matches only when the shallow key equals the value.
        assert_eq!(
            evaluate(&policy, "shell.run", &json!({"cmd": "rm -rf"})),
            Effect::Deny
        );
        assert_eq!(
            evaluate(&policy, "shell.run", &json!({"cmd": "ls"})),
            Effect::Allow
        );
        // A missing key never matches the arg rule → falls through to default-allow.
        assert_eq!(evaluate(&policy, "shell.run", &json!({})), Effect::Allow);
    }

    #[test]
    fn arg_equality_compares_non_string_scalars_by_json() {
        let policy = Policy {
            rules: vec![Rule {
                tool: "x.y".into(),
                arg: Some(ArgMatch {
                    path: "n".into(),
                    equals: "5".into(),
                }),
                effect: Effect::Deny,
            }],
        };
        assert_eq!(evaluate(&policy, "x.y", &json!({"n": 5})), Effect::Deny);
    }
}
