# Audit — the immutable, workspace-walled audit ledger

> **TODO (stub).** Not shipped yet. The ask lives in
> [`scope/audit/audit-scope.md`](../../scope/audit/audit-scope.md); this becomes the "as built"
> source of truth when the S10 slice ships.

Planned: one canonical `AuditEntry` appended at the host dispatch/cap chokepoint for **every** mediated
action — **allow and deny** — so the security record is complete by construction (a guest cannot act
un-audited). Append-only and **tamper-evident** via a per-(ws, node) hash chain; references/digests,
never secrets or raw payloads. As durable as the action: a mutating allow's entry rides the same
`write_tx` as its change; a deny is a standalone chained append. Workspace-walled (+ a reserved
`_lb_audit_system` ledger for cross-tenant admin). Read-only MCP surface (`audit.query`/`get`/`verify`,
gated by `mcp:audit.read`; reads are themselves audited); append is host-only. Generalizes the AI
gateway's per-model-call audit (README §6.14) into one ledger.
</content>
