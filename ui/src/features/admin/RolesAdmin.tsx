// Roles administration (admin-console redesign) — the REAL role editor the old UI never had. Left: a
// table of the workspace's roles with the count of caps each bundles. Right: the selected (or new)
// role's caps as a CHECKLIST, so you build a role by ticking capabilities instead of typing
// `role:<name>` strings. The candidate caps are the admin's OWN session caps (∪ caps already in any
// role) — which is exactly the no-widening set the server enforces, so the UI can't offer something
// the gateway will reject. Save calls `roles.define` (define replaces, so this is create AND edit).
//
// Built on shadcn primitives (access-console consistency): the shared `Table` (sticky header), the
// shared `AdminToolbar` (search + "New role"), `Button`/`Input`/`Checkbox` — no raw `<table>`/
// `<button>`/`<input>` and no local page header (the `AdminView` tab strip owns it). Tokens only —
// destructive actions use the `Button` `destructive` variant, never `red-…` literals. The two-region
// body stacks on phone-width (`flex-col md:flex-row`, no fixed `w-1/2`), matching PeopleAdmin.

import { useEffect, useMemo, useState } from "react";
import { KeyRound, Plus, ChevronDown, ChevronRight } from "lucide-react";

import { AppEmptyState } from "@/components/app/empty-state";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { BUILTIN_ROLES } from "@/lib/admin/roles.api";
import { hasCap } from "@/lib/session";
import { CAP } from "@/lib/session/admin-caps";
import { ConfirmDestructive } from "@/features/confirm";
import { AdminToolbar } from "./AdminToolbar";
import { groupCaps } from "./roles/groupCaps";
import { useRoles } from "./useRoles";

interface Props {
  /** The workspace is shown in the parent `AdminView`'s header; kept on the prop for API compat. */
  ws?: string;
  /** The admin's session caps — gates roles.delete (`mcp:roles.manage:call`). */
  caps: string[] | undefined;
}

export function RolesAdmin({ caps }: Props) {
  const { roles, error, define, remove } = useRoles();
  const [selected, setSelected] = useState<string | null>(null);
  const [draftName, setDraftName] = useState("");
  const [draftCaps, setDraftCaps] = useState<Set<string>>(new Set());
  const [pendingDelete, setPendingDelete] = useState<string | null>(null);
  const [deleteResult, setDeleteResult] = useState<string | null>(null);
  const [roleFilter, setRoleFilter] = useState("");
  const [capFilter, setCapFilter] = useState("");
  // The editor is a RESPONSE, not an always-on form: it opens only after you pick a role (edit) or
  // click "New role" (create). Until then the right pane is a placeholder — so "New role" visibly
  // opens the form instead of appearing to do nothing.
  const [creating, setCreating] = useState(false);
  // Which cap groups the user has explicitly expanded. Groups also auto-open when they hold a
  // checked cap or match the filter (see `isOpen`), so this only tracks manual toggles.
  const [openGroups, setOpenGroups] = useState<Set<string>>(new Set());

  const canDelete = hasCap(caps, CAP.rolesManage);

  const selRole = roles.find((r) => r.name === selected) ?? null;

  // When the selection changes, seed the draft from that role (edit) or clear it (new).
  useEffect(() => {
    setDraftName(selRole?.name ?? "");
    setDraftCaps(new Set(selRole?.caps ?? []));
  }, [selRole]);

  // The caps the admin may bundle: their own session caps, plus any cap already used by a role so an
  // existing role stays editable even if it lists a cap the current admin lacks (it just can't be
  // re-added if removed — the server would reject widening).
  const candidates = useMemo(
    () =>
      Array.from(
        new Set([
          ...(caps ?? []).filter((c) => c.startsWith("mcp:")),
          ...roles.flatMap((r) => r.caps),
        ]),
      ).sort(),
    [caps, roles],
  );

  const visibleRoles = roles.filter((r) => r.name.toLowerCase().includes(roleFilter.toLowerCase()));

  // The caps bucketed by extension (agent, dashboard, …) — the whole set, so per-group counts are
  // filter-independent. The filter only narrows the rendered ROWS (below), matching the old
  // substring-on-full-cap semantics.
  const groups = useMemo(() => groupCaps(candidates), [candidates]);
  const q = capFilter.trim().toLowerCase();
  const anyMatch = q === "" || candidates.some((c) => c.toLowerCase().includes(q));

  /** A group is open when the user expanded it, OR it holds a checked cap, OR (with an active
   *  filter) it has a matching row — so selecting a role reveals its caps and filtering reveals hits. */
  function isOpen(group: string, caps: { cap: string }[]): boolean {
    if (openGroups.has(group)) return true;
    if (caps.some((e) => draftCaps.has(e.cap))) return true;
    if (q !== "" && caps.some((e) => e.cap.toLowerCase().includes(q))) return true;
    return false;
  }

  function startNew() {
    setSelected(null);
    setDraftName("");
    setDraftCaps(new Set());
    setCreating(true);
  }
  function toggle(cap: string) {
    setDraftCaps((prev) => {
      const next = new Set(prev);
      next.has(cap) ? next.delete(cap) : next.add(cap);
      return next;
    });
  }
  function toggleGroup(group: string) {
    setOpenGroups((prev) => {
      const next = new Set(prev);
      next.has(group) ? next.delete(group) : next.add(group);
      return next;
    });
  }
  function setGroupAll(caps: { cap: string }[], on: boolean) {
    setDraftCaps((prev) => {
      const next = new Set(prev);
      for (const { cap } of caps) (on ? next.add(cap) : next.delete(cap));
      return next;
    });
  }
  async function save() {
    const name = draftName.trim();
    if (!name) return;
    await define(name, [...draftCaps]);
    setSelected(name);
    setCreating(false);
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {error && (
        <div
          role="alert"
          className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-xs text-destructive"
        >
          {error}
        </div>
      )}
      {deleteResult && (
        <p
          role="status"
          className="border-b border-border bg-panel px-4 py-2 text-xs text-muted"
        >
          {deleteResult}
        </p>
      )}

      <div className="flex min-h-0 flex-1 flex-col md:flex-row">
        {/* Left: the roster of roles. Stacks above the editor on a phone. */}
        <div className="flex min-w-0 flex-1 flex-col border-b border-border md:max-w-[32rem] md:border-b-0 md:border-r">
          <AdminToolbar
            search={roleFilter}
            onSearch={setRoleFilter}
            searchPlaceholder="Filter roles…"
            action={
              <Button size="sm" aria-label="new role" onClick={startNew}>
                <Plus size={13} /> New role
              </Button>
            }
          />
          <div className="min-h-0 flex-1 overflow-y-auto">
            {visibleRoles.length === 0 ? (
              <AppEmptyState
                icon={KeyRound}
                title={roleFilter ? "No roles match." : "No roles yet."}
                description={
                  roleFilter
                    ? "Clear the filter to see every role."
                    : "Create one to bundle capabilities."
                }
              />
            ) : (
              <Table>
                <TableHeader sticky>
                  <TableRow>
                    <TableHead>Role</TableHead>
                    <TableHead>Capabilities</TableHead>
                    <TableHead className="text-right">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {visibleRoles.map((r) => {
                    const immutable = BUILTIN_ROLES.has(r.name);
                    return (
                      <TableRow
                        key={r.name}
                        aria-label={`select role ${r.name}`}
                        aria-selected={selected === r.name}
                        data-state={selected === r.name ? "selected" : undefined}
                        className="cursor-pointer"
                        onClick={() => {
                          setSelected(r.name);
                          setCreating(false);
                        }}
                      >
                        <TableCell className="font-medium text-fg">
                          {r.name}
                          {immutable && (
                            <span className="ml-1.5 text-[0.6875rem] text-muted">(built-in)</span>
                          )}
                        </TableCell>
                        <TableCell className="text-muted">{r.caps.length}</TableCell>
                        <TableCell className="text-right">
                          {canDelete && !immutable && (
                            <Button
                              type="button"
                              variant="destructive"
                              size="sm"
                              aria-label={`delete role ${r.name}`}
                              onClick={(e) => {
                                e.stopPropagation();
                                setPendingDelete(r.name);
                              }}
                            >
                              Delete
                            </Button>
                          )}
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            )}
          </div>
        </div>

        {/* Right: the editor — opens only for a selected role (edit) or after "New role" (create);
            otherwise a placeholder, so "New role" has a visible effect. */}
        <div className="min-w-0 flex-1 overflow-y-auto px-4 py-4">
          {!selRole && !creating ? (
            <AppEmptyState
              icon={KeyRound}
              title="No role selected."
              description="Select a role to edit it, or click “New role” to create one."
            />
          ) : (
          <div className="space-y-4">
            {/* Sticky header keeps the title + Save pinned while the (long) cap tree scrolls under
                it. `-mx-4 -mt-4 px-4` cancels the pane's padding so the bar spans full width. */}
            <div className="sticky top-0 z-10 -mx-4 -mt-4 flex items-center gap-2 border-b border-border bg-panel px-4 py-3">
              <h2 className="text-sm font-semibold text-fg">
                {selRole ? `Edit role: ${selRole.name}` : "New role"}
              </h2>
              <div className="ml-auto flex items-center gap-2">
                <Button
                  size="sm"
                  aria-label="save role"
                  disabled={!draftName.trim()}
                  onClick={() => void save()}
                >
                  {selRole ? "Save changes" : "Create role"}
                </Button>
                {creating && (
                  <Button
                    variant="ghost"
                    size="sm"
                    aria-label="cancel new role"
                    onClick={() => setCreating(false)}
                  >
                    Cancel
                  </Button>
                )}
              </div>
            </div>
            <div>
              <label
                htmlFor="role-name"
                className="mb-1 block text-xs font-medium uppercase tracking-wide text-muted"
              >
                Role name
              </label>
              <Input
                id="role-name"
                aria-label="role name"
                className="h-8"
                placeholder="e.g. operator"
                value={draftName}
                disabled={selRole !== null}
                onChange={(e) => setDraftName(e.target.value)}
              />
            </div>

            <div>
              <h3 className="mb-2 text-xs font-medium uppercase tracking-wide text-muted">
                Capabilities ({draftCaps.size})
              </h3>
              {candidates.length === 0 ? (
                <p className="text-xs text-muted">
                  You hold no capabilities to bundle (no-widening).
                </p>
              ) : (
                <>
                  <Input
                    aria-label="filter capabilities"
                    className="mb-2 h-8"
                    placeholder="Filter capabilities…"
                    value={capFilter}
                    onChange={(e) => setCapFilter(e.target.value)}
                  />
                  {!anyMatch ? (
                    <p className="text-xs text-muted">No capabilities match the filter.</p>
                  ) : (
                    <div className="space-y-1">
                      {groups.map(({ group, caps: groupCapsList }) => {
                        const rows = q
                          ? groupCapsList.filter((e) => e.cap.toLowerCase().includes(q))
                          : groupCapsList;
                        if (rows.length === 0) return null; // group has no filter hit — hide it
                        const checked = groupCapsList.filter((e) => draftCaps.has(e.cap)).length;
                        const total = groupCapsList.length;
                        const open = isOpen(group, groupCapsList);
                        const allOn = checked === total;
                        return (
                          <Collapsible key={group} open={open}>
                            <div className="flex items-center gap-1">
                              <CollapsibleTrigger
                                aria-label={`toggle group ${group}`}
                                onClick={() => toggleGroup(group)}
                                className="flex flex-1 items-center gap-1.5 rounded-md px-1 py-1 text-xs hover:bg-bg/60"
                              >
                                {open ? (
                                  <ChevronDown size={13} className="text-muted" />
                                ) : (
                                  <ChevronRight size={13} className="text-muted" />
                                )}
                                <span className="font-medium text-fg">{group}</span>
                                <span className="text-muted">
                                  {checked}/{total}
                                </span>
                              </CollapsibleTrigger>
                              <Button
                                variant="ghost"
                                size="sm"
                                className="h-6 px-1.5 text-[0.6875rem]"
                                aria-label={allOn ? `deselect all ${group}` : `select all ${group}`}
                                onClick={() => setGroupAll(groupCapsList, !allOn)}
                              >
                                {allOn ? "None" : "All"}
                              </Button>
                            </div>
                            <CollapsibleContent forceMount className="data-[state=closed]:hidden">
                              <ul className="ml-5 space-y-1 border-l border-border pl-3">
                                {rows.map(({ cap, label }) => (
                                  <li key={cap} className="flex items-center gap-2 text-xs">
                                    <Checkbox
                                      id={`cap-${cap}`}
                                      aria-label={`include ${cap}`}
                                      checked={draftCaps.has(cap)}
                                      onChange={() => toggle(cap)}
                                    />
                                    <label
                                      htmlFor={`cap-${cap}`}
                                      title={cap}
                                      className="cursor-pointer font-mono"
                                    >
                                      {label}
                                    </label>
                                  </li>
                                ))}
                              </ul>
                            </CollapsibleContent>
                          </Collapsible>
                        );
                      })}
                    </div>
                  )}
                </>
              )}
            </div>
          </div>
          )}
        </div>
      </div>

      {pendingDelete && (
        <ConfirmDestructive
          title={`Delete role ${pendingDelete}`}
          consequence={`Deletes the role and un-assigns it from every subject holding role:${pendingDelete} (one transaction, idempotent). Built-in roles are not deletable. Not reversible — re-create the role and re-assign to restore.`}
          reversible={false}
          escalation="none"
          confirmLabel="Delete role"
          onConfirm={async () => {
            const name = pendingDelete;
            setPendingDelete(null);
            const affected = await remove(name);
            setDeleteResult(
              `Deleted role ${name} — un-assigned from ${affected} subject${affected === 1 ? "" : "s"}.`,
            );
            if (selected === name) setSelected(null);
          }}
          onCancel={() => setPendingDelete(null)}
        />
      )}
    </div>
  );
}
