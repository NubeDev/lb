# Frontend (public docs)

The UI-shell truth, promoted as features ship.

- **[i18n — en + es everywhere](./i18n.md)** — the one catalog mechanism (prefs MF1 engine +
  the `@nube/ext-ui-sdk` catalog seam) across invite email, pre-auth accept, push, and the
  minimal shell. Shipped 2026-07-11 (`node-v0.2.0`).
- **[Pre-auth branding — a branded sign-in screen on a first-ever visit](./public-branding.md)** —
  `GET /public/branding?ws=<ws>`, the one unauthenticated read of a workspace's `ui_branding` +
  `ui_theme`, so a browser that has never signed in paints the deployment's brand instead of the
  product default. Shipped 2026-09-03.
- **[Shell chrome — header style & top-nav mode](./shell-chrome-layout.md)** — two appearance axes
  (a breadcrumb header style + a top-menu nav mode) on the Layout tab, riding the `ui_theme` prefs
  blob. Shipped 2026-07-10.

Scope: [`docs/scope/frontend/`](../../../../docs/scope/frontend/README.md) — the UI-shell scopes,
including the theme customizer/appearance, workspace branding, the nav rail, and
`shell-chrome-layout-scope.md` (header-style + top-nav-mode layout choices).
