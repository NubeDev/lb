# Session — panel-editor parity review (2026-07-03)

**Ask.** The user tried to actually set up a panel through the editor (Query / Transform / Panel
options / Field / Overrides) and found it "very lacking … like it's been vibe coded with no
consideration this is for a real person to set up" — e.g. **Organize fields is a raw JSON textarea**.
The stated goal was 100% Grafana feature parity. Asked for a review + plan (no code this session).

**What was done.**

- Reviewed every editor tab directly (`ui/src/features/dashboard/editor/tabs/*`), the per-view option
  editors (`tabs/options/*`), the fieldconfig library (`fieldconfig/*`), the transform registry, and
  the backend `rust/crates/viz` transformer set; cross-checked against a full Grafana 10/11 panel-edit
  feature checklist.
- **Finding (the headline):** the architecture is right and mostly built — 11 transforms real in Rust,
  value mappings/color modes real in the render path, one `cell ↔ editorState` round-trip — but the
  **editing surface is a stub**: 7 of 11 transforms (incl. `organize`) edit via raw JSON; overrides
  take free-typed dotted property ids; value mappings and color schemes have **no editor at all**
  despite being applied at render; per-viz options cover ~20% of Grafana's surface; the Query tab is
  single-target despite the `targets[]` model. Each phase shipped the minimum UI to prove its
  contract, and the in-code "Phase-2 follow-up" notes were never scheduled. Tests pin round-trip
  fidelity, not usability — everything green, experience unusable.
- **Deliverable:** [`../../scope/frontend/dashboard/viz/editor-parity-scope.md`](../../scope/frontend/dashboard/viz/editor-parity-scope.md)
  — the tab-by-tab gap table + the Phase-3.5 plan (primitives → option registry → typed transform
  editors → overrides pickers → per-viz parity → multi-target queries), with usability exit gates as
  tests ("build any supported panel without ever typing JSON or a remembered field name").
- Inserted **Phase 3.5** into the viz umbrella phasing
  ([`viz/README.md`](../../scope/frontend/dashboard/viz/README.md)).

**No code changed; no tests run** (review/plan session). Nothing broke → no debugging entry.

**Next.** Execute Phase 3.5 per the scope's sequencing; step 1 is the shadcn Select/Textarea/
Checkbox/color/FieldNamePicker primitives that 10+ in-code suppressions already wait on.
