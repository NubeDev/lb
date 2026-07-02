# Session — control-engine: break the scope into build slices

Status: done (docs-only session; no code).

## Ask

Review `rust/extensions/control-engine/docs/control-engine-scope.md` and break it up
with more detail. Key goal: settle how we take on `@nube/ce-wiresheet` — fork it or
branch it — given the three external pieces in play (`ce-wiresheet`, `ce-client-rust`,
a running CE), all checked out under `~/code/ce/`.

## What was done

- **Grounded the plan in the actual repos** (not the scope's assumptions):
  - `ce-wiresheet` remote is **`NubeIO/ce-wiresheet` — our own org**, so "fork vs
    branch" resolves to: an **upstream branch** (`lb-transport`) carrying only a
    generic `EngineTransport` seam, kept mergeable to `main`. Its transport is
    currently hardwired (`lib/rest.ts` module-level `BASE` + direct `fetch`;
    `lib/ws.ts` owns the socket/session/reconnect; `lib/wire.ts` binary decode —
    ~1.3k lines), so the seam is a real but contained refactor.
  - `ce-client-rust` (`rubix-ce`) already exposes the full `ControlEngine` trait
    (15 methods incl. `subscribe_cov`) and absorbs CE's REST quirks — consumed as a
    pinned git dep, no branch needed.
  - `ce-studio` ships a **prebuilt runnable engine** (`engine.tar.gz`, ce-rest on
    `:7979`) — upgraded the testing plan with an opt-in real-engine tier alongside
    `ce_fake.rs` (previously the scope assumed CE was unrunnable in tests).
- **Wrote the eight slice docs** in `rust/extensions/control-engine/docs/`
  (S1 seam → S2 vendor → S3 sidecar/local → S4 registry/routing → S5 write verbs →
  S6 `ce.watch` → S7 bridge transport + page → S8 harden/ship), each with
  deliverables, file map, design detail, mandatory tests, and an exit gate.
- **Updated the umbrella scope:** new "Build plan — eight slices" section with the
  dependency table and critical path; revised the vendoring decision (seam upstream,
  byte-identical snapshot vendored, `BridgeTransport` LB-side in the extension `ui/`);
  revised the matching risk; aligned the example CE port to `7979`.
- Key new design detail captured in S6: `ce.watch` frames carry three kinds
  (`cov`/`topology`/`schema`) because the wiresheet's WS is more than values — a gap
  in the original scope's COV framing.

## Decisions (with rejected alternatives)

- **Seam upstream, not in the vendored copy.** Rejected carving it inside
  `packages/ce-wiresheet` (permanent divergence, 3-way merges on every sync) and a
  hard GitHub fork (we own the repo; a branch is the same isolation for free).
- **Two-tier CE test backend**: `ce_fake.rs` in CI + env-gated real-engine suite via
  the ce-studio bundle. Rejected hand-writing a fake CE HTTP/WS server — the real
  engine is runnable.

## Next

Start S1 (`NubeIO/ce-wiresheet`, branch `lb-transport`) and S3 (the sidecar crate) —
they are independent and can run in parallel. Each implementing session takes its
slice doc + `docs/HOW-TO-CODE.md`.
