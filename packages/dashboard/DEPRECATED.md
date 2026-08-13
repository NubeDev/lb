# `@nube/dashboard` is deprecated

**Use [`NubeDev/lb-ui-kit`](https://github.com/NubeDev/lb-ui-kit) (`@nube/dash-kit`) instead**, pinned
by a `kit-v*` git tag. See `../README.md` for why the substrate moved.

## Status: kept, not deleted — deliberately

Unlike `source-picker`/`insights`/`panel`/`nav-rail` (deleted in the same change), this package is
still here for two reasons:

1. **A real external consumer pins it.** `NubeIO/ems` carries
   `"@nube/dashboard": "github:NubeDev/lb#dashboard-v0.2.2&path:/packages/dashboard"` in **two** of its
   UIs. That pin is a tag, so it keeps resolving whatever happens on `master` — but deleting the
   directory would leave anyone reading this repo with no signpost at all.
2. **The kit has not stated its relationship to this code yet.** `@nube/dashboard` is a versioned grid
   core that carries **its own `timerange.ts`**, overlapping territory the kit now owns
   (`@nube/dash-kit` shipped `lib/timerange` in `kit-v0.2.0`). Whether the kit subsumes this grid,
   absorbs part of it, or ignores it is an open call that rubix-ai's scope requires be made **before
   Tier 2** (`docs/scope/ui/ext-ui-kit-scope.md`, risk 7). Deleting it now would silently decide that
   question by destroying the artifact.

## Do not add a consumer

New work takes the kit. If you need this grid, raise the Tier 2 question on
[rubix-ai#152](https://github.com/NubeIO/rubix-ai/issues/152) first — a second consumer here makes the
overlap permanent.
