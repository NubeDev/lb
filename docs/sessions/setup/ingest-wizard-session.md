# Session — Setup wizards: ingest wizard (+ reusable wizard framework)

Area: `ui/src/features/admin/setup`, `ui/src/features/admin/wizard`
Scope refs: `docs/scope/ingest/ingest-scope.md`, `docs/scope/auth-caps/api-keys-scope.md`

## The ask

The Setup tab (Access console) should host **more than one wizard**. Add an **Ingest wizard** that
walks a user from nothing to "an external program is pushing data in", ending with a **copy-paste
Python example**. The hard requirement across all these wizards: **reusable code — reuse the existing
editors/hooks, don't duplicate them.**

(Earlier in the same line of work: the Setup tab was turned into an **icon-card picker** — `SetupHub`
+ `WizardPicker` + `catalog.ts` — and an **Appearance wizard** was added; both sit on the same
framework below.)

## What shipped

**Reusable framework** — `features/admin/wizard/StepFlow.tsx`:
- `StepFlow` — generic multi-step frame (Stepper rail + scrolling body + Back/Continue/Finish footer).
  Owns *only* navigation state; each step supplies `render(ctx)` and can drive `ctx.next()/back()`.
  Domain-free (no caps, no gateway) so it drops into any feature. Reuses the existing `setup/Stepper`.
- `StepShell` — shared icon-badged step header, exported for every step to use.

**Ingest wizard** — `features/admin/setup/IngestWizard.tsx`, three steps, **pure orchestration**:
1. **Series** — reuses the ingest `CreateSeriesWizard` modal (the recursive schema builder) +
   `lib/ingest` `listSeries`/`saveSchema`. No schema UI re-implemented.
2. **API key** — reuses `admin/useApiKeys` (`create` → one-time `newSecret`), minting an
   `apikey-write` / `kind:api` key. Secret shown once via the shared `CopyButton`.
3. **Python** — `pythonSnippet.ts` (pure string builder) generates the runnable `lb_client` example,
   **prefilled** with the gateway URL, workspace, series, and (if just minted) the key. It mirrors
   `clients/python/example.py` + the `lb_client` five-method shape, so the copied code is the real
   path, not an invented API. `pip install lb-client` + `example.py` blocks, each with copy.

**Wiring** — `catalog.ts` gains the `ingest` entry (Database icon); `SetupHub.tsx` renders
`IngestWizard` when its card is picked. Adding a wizard is now: one catalog entry + one hub branch +
a `FlowStep[]`.

## Reuse ledger (the point of the task)

| Piece | Reused from | New code? |
|---|---|---|
| Stepping/footer/rail | `StepFlow` + `setup/Stepper` | framework only |
| Series + schema builder | `features/ingest/CreateSeriesWizard`, `lib/ingest` | none |
| API key mint + one-time secret | `admin/useApiKeys`, `lib/admin/apikeys.api` | none |
| Copy affordance | `components/ui/copy-button` | none |
| Python snippet | mirrors `clients/python/example.py` / `lb_client` | `pythonSnippet.ts` (string builder) |

No new backend verb, no duplicated editor.

## Caps / gating (display convenience; gateway re-checks)

- Series step's "Build a typed schema" needs `CAP.ingestWrite` (`mcp:ingest.write:call`); without it
  the user may still name an existing series to write into.
- API-key step needs `CAP.apikeyManage` (`mcp:apikey.manage:call`); without it the step explains to
  get a key from an admin and paste it into the snippet (which keeps its `lbk_REPLACE_…` placeholder).

## Bug found + fixed: the series wasn't actually being created

First cut only carried the typed series *name* forward — step 1's "Continue" persisted nothing, so
the API key got minted but no series existed. Fixed: step 1 now PERSISTS the series before you can
advance (`canAdvance` gates on a `created` flag). Two reuse paths: a **"Create series"** button that
writes a minimal one-field schema via `saveSchema`, or the ingest **`CreateSeriesWizard`** modal for a
full typed schema. A green "Series created — ready to receive samples" confirmation shows the state;
the name input locks once created.

**What "created" means here (important):** there is no separate create-series verb. A series is
defined by its **schema record** (`__schema.<name>`, written through `ingest.write`); a producer's
first sample fills in the data. So `series.list` (and `listRealSeries`) do NOT return the raw name
until real data arrives — a schema-only series is "ready" but empty. This is the SAME create path the
ingest explorer's own `CreateSeriesWizard` uses (`useIngest.create` = `saveSchema` + select). The
wizard's copy reflects this ("ready to receive samples").

## Tests

- `IngestWizard.gateway.test.tsx` — drives the wizard against a **real** in-process gateway
  (`useRealGateway` + `signInReal`): asserts Continue is blocked until create, that the schema
  persists (read back via `loadSchema` over the gateway), that a real `lbk_…` key mints, and that the
  Python snippet is prefilled with the workspace + series + minted key (and no placeholder). **Green.**
- Typecheck green across the workspace (`npx tsc --noEmit`).
