# rust/extensions/ — reference copies, retained temporarily

> ⚠️ **These are no longer the authoritative home for product extensions.** Per the out-of-tree
> migration, product extensions now live in standalone repos built against the **published SDKs**
> (`lb-ext-sdk`, `@nube/ext-ui-sdk`) with **zero lb-repo access**:
>
> - **`NubeDev/lb-extensions`** (public) — the open-source extensions.
> - **`NubeIO/rubix-ai-extensions`** (private) — NubeIO / rubix-ai's product extensions.
>
> The extensions in this folder are **kept as-is temporarily** as the reference implementation +
> fallback while the migration is validated. They will be removed once downstream is proven (~a few
> weeks). Do not treat them as the source of truth.
>
> **Exception:** `federation/` stays **first-class core** (least separable) and is **not** migrating out.
>
> Authoritative posture + retention window: [`../../MIGRATION.md`](../../MIGRATION.md).
> Owning scope: [`../../docs/scope/extensions/ext-out-of-tree-scope.md`](../../docs/scope/extensions/ext-out-of-tree-scope.md).

## What's here (in-tree, reference)

`proof-panel` (migrated to `rubix-ai-extensions`), `fleet-monitor`, `hello` / `hello-v2` (test
fixtures), `echarts-panel`, `energy-dashboard`, `github-bridge`, `mqtt`, `ros`, `thecrew`,
`control-engine`, `echo-sidecar`, and **`federation`** (stays core). The `ds-hidden-*` / `ds-pick-*`
directories are untracked DesignSync scratch, not extensions.
