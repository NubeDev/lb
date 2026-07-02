# GenUI — agent-authored generative UI (public)

Status: **TODO — nothing shipped yet.**

The ask lives at [`docs/scope/genui/genui-scope.md`](../../scope/genui/genui-scope.md): a reusable
`@nube/genui` package (a versioned A2UI-shaped IR + a catalog renderer, with the OpenUI-Lang
authoring adapter in v1) and the `view:"genui"` dashboard widget as its first tenant, authored by
the workspace agent via the `genui-widget` skill — emissions parse once at accept, the typed IR is
what persists.

When slices ship, promote the durable facts here (package API, cell shape, skill grant, trust
tier) per `docs/ABOUT-DOCS.md`.
