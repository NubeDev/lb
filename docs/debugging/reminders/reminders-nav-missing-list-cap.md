# Reminders missing from the sidebar — member role lacked the concrete `reminder.list` cap

Date: 2026-07-08 · Area: reminders / auth-caps · Status: **resolved**

## Symptom

The Reminders page loaded fine by deep link (`/reminders`) and every `reminder.*` verb worked, but
the **Reminders entry never appeared in the sidebar** for a normal dev-login / member session. Rules
and Flows (its siblings in the Automation group) showed correctly.

## Root cause

The sidebar rail is cap-gated. In `ui/src/features/routing/allowed.ts`:

```ts
if (hasCap(caps, CAP.reminderList)) allowed.push("reminders");  // CAP.reminderList = "mcp:reminder.list:call"
```

`hasCap` (`ui/src/lib/session/admin-caps.ts`) is an **exact membership test**
(`caps.includes(cap)`) — it does **not** expand wildcards. The member token carries the broad
`mcp:*.list:call` wildcard, which authorizes the *verb* server-side, but the frontend gate needs the
**concrete** `mcp:reminder.list:call` string present in the token's cap array.

The built-in member role (`rust/crates/host/src/authz/builtin_roles.rs`, the base of `dev_claims`)
spells out `mcp:rules.list:call`, `mcp:flows.list:call`, `mcp:datasource.list:call`,
`mcp:insight.list:call` explicitly — but for reminders it listed only `mcp:reminder.fire:call`, not
`mcp:reminder.list:call`. So the wildcard let the verb run, yet the rail filtered Reminders out.

## Fix

Added `"mcp:reminder.list:call"` to the member role next to `reminder.fire`. Requires a **node
rebuild + restart** (Rust doesn't hot-reload) and a **re-login** to re-mint the token with the cap.

## Lesson

A sidebar/nav gate keyed on a concrete cap needs that **exact** cap in the token — the
`mcp:*.list:call` wildcard authorizes the verb but is invisible to the frontend's literal `hasCap`.
Any new core surface with a nav gate must add its concrete `*.list` cap to the member role the way
`rules`/`flows`/`datasources` do; the wildcard is not enough for the rail to show it.
