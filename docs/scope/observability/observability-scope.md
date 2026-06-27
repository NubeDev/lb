# Observability scope — structured logs, distributed traces, metrics

Status: scope (the ask). Promotes to `public/observability/` once shipped. Stage: **S10 —
cross-cutting retrofit** (`../../STAGES.md`). The capture chokepoint (the host tool-dispatch +
capability check, README §6.5/§6.6) already ships on every node from S1; this was the operational
half that was never scoped. `key-stack.md` row "Observability/audit" already flags it: *"Needs a
dedicated coding scope; key for trust and debugging."*

> Read with: README §6.5 (MCP/dispatch — the capture point), §6.6 (capabilities — the decision
> we instrument), §6.7 (secrets — the redaction rule), §3.3 (state vs motion — telemetry is
> **neither** store-state nor must-deliver motion), `../audit/audit-scope.md` and
> `../undo/undo-scope.md` (the two **sibling** consumers of the same chokepoint — see "The shared
> seam" below), `../debugging/debugging-scope.md` (the post-hoc history system telemetry feeds).

The platform moves work across nodes — an edge UI invokes a tool that **routes over Zenoh** to the
hub (§6.5), which spawns a **job** (§6.9), which enqueues an **outbox effect** (§6.10) delivered by
a **relay** on a third tick. Today, when that chain fails, there is **no correlated operational
signal** to follow it: no trace that spans the edge→hub hop, no structured event with a stable
request id, no metric for tool-call latency or deny rate. You can *enforce* the system but you
cannot *see* it. This scope makes every node **emit** well-structured, correlated telemetry — logs,
traces, and metrics — with a correlation id that survives the routed hop and the job/outbox handoff.

## The shared seam (read once; the other two scopes reference it)

Three concerns were missed together because they are **three durable projections of one already-
existing event**: the host mediates *every* action at one chokepoint (§6.5 dispatch + §6.6 cap
check). Bolting any of them on per-extension would violate capability-first / one-chokepoint. So:

| Concern | This scope | Lifecycle | Walled? | Capture point |
|---|---|---|---|---|
| **Observability** | here | sampled, ephemeral, operator-facing | node/operator (not the tenant wall) | tool-dispatch span |
| **Audit** | `../audit/` | durable, immutable, complete | workspace-walled | dispatch decision (allow+deny) |
| **Undo** | `../undo/` | bounded, reversible, per-actor | workspace-walled | store-write before-image (`write_tx`) |

They **share the capture point, not a store** — different retention, immutability, and visibility.
None owns the others. This scope owns the *operational* projection only.

## Goals

- **One telemetry vocabulary, emitted everywhere.** Adopt the Rust `tracing` ecosystem
  (`tracing` + `tracing-subscriber`) as the single span/event API used by `host`, `caps`, the
  store/bus seams, jobs, the outbox relay, and the role crates. Structured fields, not
  `println!`/ad-hoc strings (FILE-LAYOUT bans the `utils`-style log dumping ground too).
- **A correlation id that survives every hop.** A `trace_id` (W3C `traceparent` shape) is minted
  at each **ingress** (gateway HTTP, webhook, Tauri command, a job tick, a relay pass) and
  **propagated**: into the routed MCP call over Zenoh (carried in the query attachment), into the
  job record, and onto each outbox effect. One id ties edge click → hub tool → job → PR.
- **The three pillars, scoped to what the platform should own:**
  - **Traces** — spans around each mediated tool call, each store transaction, each bus
    publish/route, each job step, each relay delivery; parent/child across the routed hop.
  - **Logs** — structured events (`level`, `ws`, `actor`, `tool`, `trace_id`, message) emitted
    inside spans; one event schema, no free-text-only logs for security-relevant paths.
  - **Metrics** — counters/histograms for the things you alert on: tool-call latency + outcome,
    **capability-deny count**, bus route latency, job duration/outcome, outbox attempts/dead-letters,
    sync lag.
- **Secret-safe by construction.** No secret value, token, or credential ever enters a span field,
  log line, or metric label (§6.7). Enforced by a `Secret<T>` newtype whose `Debug`/`Display`
  render `***`, and a params-digest discipline shared with audit (hash/summary, never raw payload).
- **Symmetric emission, config-selected sink.** Every node emits identically; **config** chooses
  the sink (stderr/rolling file on an `appliance`/`workstation`; an OTLP exporter to a collector on
  the `hub`). No `if cloud` — the sink is a subscriber layer wired by the `node` binary's entry
  layer, not a core-crate branch.

## Non-goals

- **We do not build a dashboards/observability UI, log store, or query engine.** The platform's job
  is to **emit** clean OpenTelemetry-shaped signal; collection and visualization are external and
  best-of-breed (OTLP → Tempo/Jaeger for traces, Loki for logs, Prometheus/Grafana for metrics).
  Building an in-core telemetry store would (a) reinvent a second datastore for non-state data
  (against rule #2's spirit) and (b) duplicate mature tools. The `debugging/` history system
  (post-hoc, human-curated) stays the in-repo narrative; this is the machine signal that feeds it.
- **Not the audit log.** Telemetry is **sampled and may drop**, has operational retention, and is
  operator-visible across workspaces. It is explicitly **not** a compliance record — that is
  `../audit/`. A deny shows up in *both* (a metric/span here; an immutable ledger entry there);
  they are not redundant, they are different guarantees.
- **No business/product analytics, no per-user behavioral tracking.** Metric labels are bounded
  dimensions (`ws`, `role`, `tool`, `outcome`), never high-cardinality per-user/per-record ids
  (that path is the cardinality-explosion risk tags already named).
- **No change to the capability grammar.** Emission is host-internal; reading telemetry through a
  surface (if added) is gated, but instrumentation itself is not a new grant.

## Intent / approach

**Instrument the chokepoint once, not every extension.** The host already wraps every tool call in
a dispatch function that performs the `caps::check`. That function gets a `#[tracing::instrument]`
span carrying `ws`, `actor`, `tool`, and `trace_id`; the cap decision is recorded as a span field
(`decision = allow|deny`) and a metric. Because *every* tool — host service or WASM guest — is
dispatched here, a guest extension is observed **without cooperating** (and cannot opt out of being
observed), exactly as it is capability-checked here. This is the single highest-leverage span in
the system; everything else (store tx, bus route, job step, relay delivery) is a child span.

**Propagation is the hard part and the reason this is a *platform* concern.** A `tracing` span on
the edge does not automatically become a parent of a span on the hub — the call crosses a Zenoh
queryable. So the routed-MCP layer must **inject** the current `traceparent` into the query
attachment on the calling node and **extract**/re-parent it on the serving node (the same place the
caller's workspace+caps will eventually ride on the bus — the S5/S6 "token-on-the-bus" open item).
The job record persists its originating `trace_id`; each outbox `Effect` carries it; the relay
opens a child span when it delivers. Without this, cross-node work is a set of disconnected local
traces — which is precisely the gap that made this a "missed floor."

**Why `tracing` + OpenTelemetry and not a bespoke logger.** `tracing` is the de-facto Rust standard,
zero-cost when a level is disabled (static callsites), and has a first-class `tracing-opentelemetry`
bridge to OTLP. We get traces, structured logs (events-within-spans), and a metrics bridge from one
API. **Rejected:** `log` + string formatting (no spans, no structure, no propagation); a custom
event bus on Zenoh as the *primary* telemetry path (telemetry is operator signal, not workspace
motion — and making the bus the telemetry backbone couples observability to the thing you most need
to observe when the bus itself is sick). A live **tail** over a reserved `_lb/telemetry/**` subject
is a fine *optional* convenience for a dev UI — fire-and-forget motion, never the source of truth.

**Redaction is a type, not a guideline.** Secret material is wrapped in `Secret<T>` (the secrets
surface returns it; §6.7); its `Debug` is `***`, so it is *impossible* to accidentally format into
a span/log. Tool params are recorded as a **digest + a redacted shape summary** (the same helper
audit uses), never the raw value. A test asserts a known secret never appears in captured output.

## How it fits the core

- **Tenancy / isolation:** every span/event/metric carries `ws` as a **field/label** for filtering
  — but the raw operator sink is **node-level operator data**, not tenant-walled state (an operator
  debugging a node sees across workspaces, by design). The wall reappears at any *workspace-facing*
  read surface: a workspace admin querying their telemetry (if such a surface is added) is gated and
  filtered to their `ws` — they never see another tenant's signal. The distinction (operator sink
  vs. tenant-facing view) is the careful part and is called out as an open question.
- **Capabilities:** instrumentation needs **no** grant (it is the host observing itself). A
  workspace-facing read surface (`telemetry.tail`/`metrics.read`), if built, is gated like any tool;
  deny is opaque. The thing we most instrument *is* the cap decision.
- **Placement:** *either* — identical emission on every node; the sink/exporter is config wired in
  the entry layer (`node` binary), never a core branch. An offline `appliance` writes to a local
  rolling file and ships nothing until it can reach a collector.
- **MCP surface:** writes — **N/A** (telemetry is emitted via `tracing` macros, not a tool call;
  there is no `telemetry.write` to forge). Reads — *optional, deferred*: a gated `metrics.read`
  (snapshot) and a `telemetry.tail` (live feed over the bus, §6.13 SSE on the gateway). The
  recommendation is to **export to a collector first** and add in-product reads only if a real
  caller needs them — don't reflexively ship CRUD (SCOPE-WRITTING §6.1).
- **Data (SurrealDB):** **none by default.** Telemetry is neither durable state nor must-deliver —
  keeping it out of the store protects rule #2/#3 and avoids bloating the one datastore with
  high-volume non-state. (Contrast: audit *is* state and *does* live in SurrealDB.)
- **Bus (Zenoh):** the **fire-and-forget** class only, and only for the optional live tail. The
  *propagation* of `traceparent` rides the existing routed-MCP query attachment — it does not add a
  message class. Telemetry must never go through the **outbox** (it is explicitly allowed to drop).
- **Sync / authority:** N/A — operator signal is not synced workspace data. Each node's sink is
  independent; correlation across nodes is by `trace_id`, not by replication.
- **Secrets:** the central rule — secret material is `Secret<T>` (redacted `Debug`), params are
  digested. The secrets surface itself is instrumented (a span for "secret requested by X for Y")
  **without** the value.

## Example flow (one click, one trace, four nodes-worth of hops)

1. A `browser` POSTs `agent.invoke` to the `hub` gateway. The entry layer **mints** a `trace_id`
   and opens the root span `{ws, actor, tool: agent.invoke, trace_id}`.
2. The hub dispatch span records `decision=allow` (a child of the root) and a `tool_calls_total{tool,
   outcome=allow}` counter increments; the latency histogram starts.
3. The agent loop routes a granted tool to an **edge** node over Zenoh. The routed-MCP layer injects
   the `traceparent`; the edge re-parents its dispatch span under the same `trace_id` — **one trace
   now spans hub→edge**, the hop that was previously invisible.
4. The agent enqueues an outbox `Effect` (open a PR). The effect carries `trace_id`. Minutes later
   the **relay** delivers it and opens a child span — the PR is attributable to the original click.
5. A secret is fetched for the GitHub target: a span `secret.request{by, scope}` is emitted; the
   token is `Secret<String>` and renders `***` — it is in no span, log, or metric.
6. An operator opens Tempo, searches the `trace_id`, and sees the whole edge→hub→job→relay→GitHub
   chain on one timeline. A Grafana panel shows the `capability_deny_total` that spiked at step 2 of
   a *different*, failed attempt.

## Testing plan

Mandatory categories from `../testing/testing-scope.md` — applied to the *emission contract*:

- **Capability-deny (§2.1):** assert a **denied** tool call produces (a) a span with
  `decision=deny` and (b) a `capability_deny_total` increment — observability must capture the deny,
  not just the allow. (The deny's *enforcement* is tested by caps; here we test it is *seen*.)
- **Workspace-isolation (§2.2):** assert every emitted span/event/metric for a ws-A call carries
  `ws=A` and never `ws=B`; and that the optional tenant-facing read surface (if built) filters to the
  caller's `ws` (a ws-B `metrics.read` returns no ws-A series) — the wall holds at the *view*, while
  the operator sink legitimately spans tenants.
- **Offline/sync (§2.3):** an offline `appliance` buffers to its local file sink and **loses no
  span to a dropped collector** within the buffer bound; on reconnect, export resumes (telemetry may
  drop *beyond* the bound by design — assert the documented bound, not exactly-once).
- **The redaction test (specified, not generic — the #1 risk):** plant a known secret value through
  the secrets surface and through a tool param, exercise the full dispatch→span→log→metric path with
  capture enabled, and assert the secret string appears in **zero** captured output. A test using a
  *different* value would pass under a leak, so the planted-value identity is required.
- **Propagation test:** a routed cross-node call produces a **single** trace whose hub and edge
  spans share one `trace_id` with correct parent/child — the headline new capability.
- Unit: the `Secret<T>` redaction; the params-digest helper; the `traceparent` inject/extract codec.

## Risks & hard problems

- **Secret leakage is the catastrophe, not a nuisance.** A single `tracing::info!(?secret)` in a
  role crate undoes the secrets design. Mitigation is *structural* (the `Secret<T>` newtype + the
  digest helper + the planted-value test in CI), not "remember not to log secrets." A FILE-LAYOUT/CI
  lint that flags `?`/`%` formatting of known secret types is a strong follow-up.
- **Trace propagation across Zenoh has no standard.** We are inventing the inject/extract over the
  query attachment. It must compose with the still-open **token-on-the-bus** work (the caller's
  workspace+caps also need to ride the routed call) — design them as **one attachment envelope**,
  not two bolt-ons, or they will fight.
- **Cardinality explosion in metric labels** — the same trap tags named. `ws`/`role`/`tool`/`outcome`
  are bounded; per-user, per-record, per-trace labels are not and will melt a Prometheus. Labels are
  an allow-list, enforced in the metric helpers.
- **Overhead under load.** Spans are cheap when sampled; unsampled per-call allocation on a hot
  ingest path is not free. Sampling is config (tail-based at the collector; head-based ratio on the
  node). The ingest hot path (§ingest) gets coarse metrics, not a span per sample.
- **The operator-sink vs. tenant-wall boundary is genuinely subtle.** Operator logs span tenants;
  any workspace-facing surface must hard-filter. Getting this wrong leaks tenant A's tool names/usage
  to tenant B through a "harmless" metrics endpoint. The wall lives at the read surface.

## Open questions

- **Guest (WASM) emission — touches the SDK/WIT boundary (flag loudly).** Host-side dispatch spans
  observe a guest *from outside* with no WIT change (the safe default, ships first). Letting a guest
  emit its *own* structured spans/events needs a host import (`log`/`emit-event`) on the stable
  plugin ABID — an **additive but forever** WIT change. Recommendation: ship host-observed first;
  add the guest import as its own scoped ABI change only when an extension author needs it.
- **Any in-product read surface at all?** Lean: **no** for v1 — export to a collector, keep the core
  an emitter. Revisit if the `hub` must show a workspace admin their own usage without standing up
  Grafana (then `metrics.read` + a tenant-filtered panel, gated).
- **Metrics backend:** the `metrics` crate facade vs. OpenTelemetry metrics directly. Lean: OTel
  metrics via `tracing-opentelemetry` to keep one exporter (OTLP) for all three pillars.
- **Sampling policy + defaults** per role/path (head ratio on edge, tail at the hub collector).
- **Retention/rotation** of the local file sink on a constrained `appliance` (size/age cap).
- **Correlation-id format:** confirm W3C `traceparent` (interop with OTel) vs. a bespoke id.

## Related

- README **§6.5** (the dispatch chokepoint we instrument), **§6.6** (the cap decision we record),
  **§6.7** (secrets/redaction), **§6.2** (message classes — telemetry is fire-and-forget),
  **§6.14** (the AI gateway's per-model-call audit — a *specialized* consumer of this discipline),
  **§3** (symmetric nodes, one datastore, state vs motion).
- `../audit/audit-scope.md`, `../undo/undo-scope.md` — the two sibling projections of the same
  chokepoint (see "The shared seam").
- `../debugging/debugging-scope.md` — the human, post-hoc history system this machine signal feeds.
- `key-stack.md` — the "Observability/audit" row (this resolves its "needs a dedicated scope" note).
- The open **token-on-the-bus** item (STATUS "fit-and-finish") — co-design the routed-call
  attachment envelope with `traceparent` propagation.
</content>
</invoke>
