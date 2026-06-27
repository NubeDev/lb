# Undo — the reversible-command journal (undo / redo)

> **TODO (stub).** Not shipped yet. The ask lives in
> [`scope/undo/undo-scope.md`](../../scope/undo/undo-scope.md); this becomes the "as built" source of
> truth when the S10 slice ships.

Planned: a host-level reversible-command journal capturing a **before-image** at the `write_tx` store
seam, so a mutation's change and its undo data commit atomically. The load-bearing design is the line
it draws: **undo reverses only reversible *state* mutations; irreversible *motion* (outbox effects) is
never undone — it is *compensated*** by a declared compensating action. The host **derives** a tool's
class (reaches-the-outbox ⇒ irreversible; the max over a mixed action's parts), so undo can't silently
diverge from the world. Per-(workspace, actor) bounded stacks (opt-in per-surface for editor-style
extensions); workspace-walled, capability-gated (no escalation via undo), audited, and sync-safe via an
optimistic version check that **refuses** a stale undo rather than clobbering a concurrent write.
Collaborative/OT undo stays a per-extension CRDT concern (README §6.8). MCP: `undo`/`redo`/
`history.list`/`history.compensations`; the journal write is host-only.
</content>
