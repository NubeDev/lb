# An apostrophe (or quoted placeholder) in a catalog message silently falls back to the key literal

- **Date:** 2026-07-11
- **Area:** prefs (catalog engine)
- **Status:** resolved
- **Session:** [../../sessions/release/updates-to-core-release-session.md](../../sessions/release/updates-to-core-release-session.md)

## Symptom

The new `invite.email.subject`/`invite.email.body` catalog messages rendered as their **key
literals** (`invite.email.subject`) instead of the authored text — in both locales. Test output:

```
assertion `left == right` failed
  left: "invite.email.subject"
 right: "You're invited to join acme"
```

## Root cause

MF1 (ICU MessageFormat 1) treats `'` as the **escape/quoting character**. `You're` starts a quote
run, and `'{workspace}'` means *the literal text `{workspace}`*, not a placeholder. The pinned
MF1-subset parser rejects the malformed message, and `catalog::render`'s never-blank chain then
falls through to the last resort — the key literal. Nothing errors; the message just degrades.

## Fix

Author catalog messages without bare apostrophes or quoted braces (reworded to "You are invited…"
and dropped the quotes around `{workspace}`), then re-ran `gen-prefs-catalog` so the client twin
matches. Regression coverage: `crates/host/tests/invite_i18n_test.rs` asserts the **rendered
text** (not just delivery), so a future message that fails to parse fails the suite.

## Lesson

When adding catalog keys, assert the rendered output in a test — the engine's never-blank fallback
means an authoring error looks like "it delivered fine" while users see raw keys. (The
`lint_catalog` gate catches out-of-subset constructs only when a test runs it over the new keys.)
