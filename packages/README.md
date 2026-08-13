# `packages/` — the shared frontend libraries

## The substrate moved

The reusable dashboard substrate that used to live here — the source picker, insights, the panel and
the nav rail — is now **[`NubeDev/lb-ui-kit`](https://github.com/NubeDev/lb-ui-kit)**, published as
**`@nube/dash-kit`** and pinned by a `kit-v*` git tag:

```jsonc
{ "dependencies": { "@nube/dash-kit": "github:NubeDev/lb-ui-kit#kit-v0.3.0" } }
```

**Why it moved.** These packages were written to be reused — `@nube/source-picker`'s own description
says the host injects how to reach the node "so it works from the shell gateway **OR an extension
bridge**" — but they lived as `workspace:*` members with no publish channel. So the one thing they
were built for could not happen: an extension author had no way to depend on them. The copies that
kept being changed were the *product* ones, and lb's have been dormant since `ui/` was retired
(commit `678503f`). Keeping a second, diverging home here would make "one substrate" mean three
(lb's, the kit's, and whatever `file:` path someone points at an lb checkout).

Rationale and the rejected alternatives: rubix-ai `docs/scope/ui/ext-ui-kit-scope.md` (§3, risk 7) and
[rubix-ai#152](https://github.com/NubeIO/rubix-ai/issues/152).

## 🚫 Do not `file:` into an lb checkout

If you find yourself writing something like

```jsonc
"@nube/source-picker": "file:../../../../lb/packages/source-picker"
```

stop — that is the exact anti-pattern this move exists to end. It breaks the extensions repo's
load-bearing **zero lb-repo access** rule and only resolves on one developer's machine. Use the kit's
git-tag pin instead.

## What is left here, and why

| Package | Status |
|---|---|
| `dashboard` | **Deprecated** — see its `DEPRECATED.md`. Kept because a real external consumer still pins it, and because the kit must state its relationship to this grid core before absorbing that territory. |
| `genui` | **Deprecated** — see its `DEPRECATED.md`. The kit's Tier 2; not absorbed yet, so it is signposted rather than deleted. |
| `ce-wiresheet`, `minimal-shell`, `thecrew` | **Live.** These are lb's own demos/test beds, not shared substrate. Untouched. |
