# minimal-shell's `@nube/ext-ui-sdk` link: path pointed inside the lb repo — first runtime import failed

- **Date:** 2026-07-11
- **Area:** frontend (minimal-shell)
- **Status:** resolved
- **Session:** [../../sessions/release/updates-to-core-release-session.md](../../sessions/release/updates-to-core-release-session.md)

## Symptom

Adding the first **runtime** import from `@nube/ext-ui-sdk` (the i18n seam) broke every
minimal-shell test at transform time:

```
Error: Failed to resolve import "@nube/ext-ui-sdk" from "src/i18n.tsx". Does the file exist?
```

## Root cause

`packages/minimal-shell/package.json` declared `"@nube/ext-ui-sdk": "link:../../lb-ext-ui-sdk"` —
relative to the package that resolves to `<lb repo>/lb-ext-ui-sdk`, which does not exist; the SDK
is a **sibling of the lb repo** (`../../../lb-ext-ui-sdk`). The bug was invisible until now
because the shell only imported SDK **types** (erased at build time); pnpm created a dangling
symlink and nothing ever followed it.

## Fix

Corrected the link to `link:../../../lb-ext-ui-sdk` + `pnpm install`. The i18n tests
(`src/i18n.test.tsx`) now exercise the runtime import, so a future dangling link fails the suite.

## Lesson

A `link:` dep consumed only for types is an unverified claim — the first runtime import is the
real test. Same family as the "fake backends look done" rule: type-only wiring can look wired.
