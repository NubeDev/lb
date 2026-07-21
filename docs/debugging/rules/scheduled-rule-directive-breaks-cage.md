# A scheduled rule's `#[schedule(...)]` directive broke its own run — `#` is reserved in rhai

- **Area:** rules
- **Symptom:** A rule with a `#[schedule("every 15 minutes")]` directive saved fine, compiled to the
  right cron, and built its managed `cron → rule` flow — but when the flow cron reactor fired it, the
  managed flow settled `partialFailure` and **raised no insight**. The `rule` node's `rules.eval`
  errored with `bad input: Syntax error: '#' is a reserved symbol (line 1, position 1)`.
- **Status:** resolved
- **Date:** 2026-07-21

## What was observed

The end-to-end firing test (`scheduled_rule_fires_through_the_flow_cron_reactor_and_dedups`) fired
exactly one run (`pass.fired == 1`, so the reactor + managed flow were correct), but `count_insights`
stayed at 0. Dumping the run:

```
"steps":[
  {"id":"rule","outcome":"err","error":"bad input: Syntax error: '#' is a reserved symbol (line 1, position 1)"},
  {"id":"trigger","outcome":"ok","output":{"payload":{"cron_ts":900}}}
]
```

The trigger fired the rule node, the rule node dispatched `rules.eval {rule_id: "cooler"}`, which loaded
the **stored body** and handed it to the rhai cage — including the directive line as line 1.

## Root cause

The `#[schedule(...)]` directive is stored **verbatim in the rule body** (the body is the source of
truth for the schedule — the syncer re-parses it on every save). But `#` is a reserved symbol in rhai
(it prefixes object-map literals like `#{ ... }`), so `#[schedule(...)]` at the top of the body is a
compile error the moment the body enters the cage. Slice 1 correctly parsed the directive at **save**
without executing the body, so the bug was invisible until slice 4 actually **ran** the rule through the
managed flow. The directive is authoring metadata that must never reach the interpreter.

## Fix

Strip the directive line(s) from the body at the single cage-compile chokepoint, keeping the stored
record untouched:

- `lb_rules::schedule::strip_directive(body) -> Cow<str>` — removes every line whose first non-space
  chars are `#[schedule` (the same anchor `extract_schedule` uses, so a `#[schedule` appearing inside
  rule logic is not a directive and is left alone). Zero-copy `Cow::Borrowed` on the common
  (unscheduled) path — it only allocates when a directive is actually removed.
- `crates/rules/src/engine.rs` — the one `engine.eval_with_scope(&mut scope, &rule.body)` call now
  compiles `strip_directive(&rule.body)`. This is the single entry both `rules.run` and `rules.eval`
  ride, so the directive is invisible to the cage everywhere a rule executes, with the stored body (and
  thus the schedule source of truth) unchanged.

## Regression

- `crates/rules/src/schedule.rs::directive_line_is_stripped_before_the_cage` — unit proof: the directive
  line is gone from the stripped body, the rule logic is preserved, and the no-directive path stays a
  zero-copy borrow.
- `crates/host/tests/scheduled_rules_test.rs::scheduled_rule_fires_through_the_flow_cron_reactor_and_dedups`
  — the end-to-end proof that failed-before/passes-after: the managed flow fires, the rule runs, one
  insight lands, and a second tick dedups (no second record).

## Lesson

A save-time-only feature (parse, don't execute) hides run-time bugs until something actually runs the
artifact. When authoring sugar is stored inside an executable body, the strip must live at the
interpreter boundary, not just the parser — and the end-to-end firing test is what caught it.
