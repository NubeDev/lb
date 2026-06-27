# shadcn/ui Migration

Goal: Lazybones UI should use shadcn/ui primitives for reusable controls, with app-specific compositions layered on top. Feature files should contain behavior and layout, not bespoke button/input CSS.

## Baseline

- Keep generated or hand-vendored primitives in `src/components/ui`.
- Keep product-specific compositions in `src/components/app`.
- Use `cn`, `buttonVariants`, and primitive variants instead of string-building repeated control classes.
- Prefer shadcn tokens: `background`, `foreground`, `card`, `border`, `input`, `ring`, `primary`, `secondary`, `muted-foreground`, `destructive`, and `sidebar-*`.
- Keep the legacy aliases `bg`, `fg`, `panel`, `muted`, and `accent` temporarily for migrated screens that still use them.

## Migration Order

1. Shell and sidebar.
2. Shared page header, workspace badge, alerts, form fields, and buttons.
3. Simple CRUD surfaces: Members, Inbox, Outbox.
4. Channel composer/list and workspace switcher.
5. Dashboard, Ingest, Data, Admin.

## Definition Of Done Per Surface

- Imports shadcn primitives from `@/components/ui/*` for controls.
- Imports app compositions from `@/components/app/*` for recurring shell/page patterns.
- Does not use `control-field`, `soft-button`, `danger-button`, `page-header`, `page-header-icon`, `scope-pill`, or `state-alert`.
- Keeps existing accessibility labels and test-visible text unless there is a test update with the change.
- Has been checked in expanded and collapsed sidebar states.
