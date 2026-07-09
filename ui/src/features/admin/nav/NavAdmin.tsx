// The nav builder (nav scope) — an admin surface to compose a workspace's navigation menu: pick items
// from the three REAL sources (core surfaces, `dashboard.list`, `ext.list`), add a dynamic tag-group,
// order/group them, set visibility, share to a team, and set the workspace default. The nav is a
// LENS — it grants nothing; it only shapes the menu. Every write is a real `nav.*` verb re-checked
// server-side; the builder can never author a cap. Cap-gated for DISPLAY by `nav.save`; the gateway
// is the boundary. Markup + local edit state; the data + writes live in `useNavs`.

import { useState } from "react";
import { List, Plus } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { AppEmptyState } from "@/components/app/empty-state";
import { AdminToolbar } from "../AdminToolbar";
import { CAP, hasCap } from "@/lib/session";
import type { NavItem, Visibility } from "@/lib/nav";
import { NavItemsBuilder } from "./NavItemsBuilder";
import { useNavs } from "./useNavs";

interface Props {
  ws: string;
  caps: string[] | undefined;
}

export function NavAdmin({ ws, caps }: Props) {
  const { navs, sources, shares, reloadShares, error, loading, save, remove, share, unshare, setDefault } =
    useNavs(ws);
  const canWrite = hasCap(caps, CAP.navSave);

  // The nav currently being edited: its id/title/items + a dirty flag. `null` = the roster view.
  const [editId, setEditId] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [items, setItems] = useState<NavItem[]>([]);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  // Share form state.
  const [shareVis, setShareVis] = useState<Visibility>("private");
  const [shareTeam, setShareTeam] = useState("");
  const [filter, setFilter] = useState("");

  const visibleNavs = navs.filter(
    (n) =>
      n.title.toLowerCase().includes(filter.toLowerCase()) ||
      n.id.toLowerCase().includes(filter.toLowerCase()),
  );

  const startNew = () => {
    setEditId("");
    setTitle("");
    setItems([]);
    setMsg(null);
  };

  const startEdit = async (id: string) => {
    setBusy(true);
    setMsg(null);
    try {
      const { getNav } = await import("@/lib/nav");
      const n = await getNav(id);
      setEditId(n.id);
      setTitle(n.title);
      setItems(n.items ?? []);
      setShareVis(n.visibility);
      await reloadShares(n.id);
    } catch (e) {
      setMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  const doSave = async () => {
    if (!editId && !title) {
      setMsg("A nav needs an id/title");
      return;
    }
    setBusy(true);
    setMsg(null);
    try {
      // The id is a slug from the title on create (lowercased, dashed); keep the id on update.
      const id = editId || title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
      await save(id, title, items);
      setEditId(id);
      setMsg("Saved.");
    } catch (e) {
      setMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  // Set the nav's overall audience tier (private / workspace) — the "Just me" / "Everyone" toggle.
  // Team sharing is `addTeam` (which force-sets the team tier itself); this handles the two non-team
  // audiences. Self-applying: one click writes it, no separate Save.
  const applyVis = async (vis: Visibility) => {
    if (!editId) return;
    setBusy(true);
    setMsg(null);
    try {
      await share(editId, vis, undefined);
      setShareVis(vis);
      setMsg(vis === "workspace" ? "Shared with everyone." : "Set to private.");
    } catch (e) {
      setMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  // A team's human name from the loaded roster (falls back to the raw id). The share roster stores
  // ids; the picker + list show names so an admin never reads `team:ops`.
  const teamName = (team: string): string =>
    sources.teams.find((t) => t.team === team)?.name || team;

  const addTeam = async () => {
    if (!editId || !shareTeam) return;
    setBusy(true);
    setMsg(null);
    try {
      // Writes one S4 `nav -[share]-> team` edge; the underlying relate is multi-edge, so each add
      // accumulates. Forces the Team tier (a team share is meaningless at private/workspace).
      if (shareVis !== "team") {
        setShareVis("team");
        await share(editId, "team", shareTeam);
      } else {
        await share(editId, "team", shareTeam);
      }
      setMsg(`Shared to ${shareTeam}.`);
      setShareTeam("");
    } catch (e) {
      setMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  const doUnshare = async (team: string) => {
    if (!editId) return;
    setBusy(true);
    setMsg(null);
    try {
      await unshare(editId, team);
      setMsg(`Removed share to ${team}.`);
    } catch (e) {
      setMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  if (!canWrite) {
    return (
      <div className="p-4 text-sm text-muted" role="status">
        You need the nav authoring capability (<code>mcp:nav.save:call</code>) to build navs.
      </div>
    );
  }

  // The editor form — rendered inline below the (always-mounted) roster toolbar so the search bar
  // stays put while editing, mirroring the API Keys "New key" flow.
  const navEditor = (
    <>
      {/* The nav's title + items, wrapped in the same bordered panel card the API Keys "New key"
          form uses (labelled fields over inputs) so both create surfaces read as one system. */}
      <div className="space-y-3 rounded-md border border-border bg-panel px-3 py-3">
        {/* The shared menu composer — title + items list + add-item form. Reused verbatim by the
            onboarding wizard's Access step (one grammar, one source of truth). */}
        <NavItemsBuilder
          title={title}
          onTitleChange={setTitle}
          items={items}
          onItemsChange={setItems}
          sources={{ dashboards: sources.dashboards, extensions: sources.extensions }}
        />

      {/* Save the nav's items/title. Audience (who sees it) is the section below — no separate
          "apply visibility" step, so a nav can't be Saved-but-invisible by accident. */}
      <div className="flex flex-wrap items-center gap-2">
        <Button size="sm" onClick={() => void doSave()} disabled={busy} aria-label="Save nav">
          Save
        </Button>
      </div>
      </div>

      {/* ── Who sees this nav ── the ONE audience control. The old model had three moving parts
          (Save, a visibility <Select>, an "Apply visibility" button, AND a separate team-add), so
          an admin could build a nav, walk away, and leave it `private` = invisible to everyone.
          Now audience is explicit and self-applying: "Just me" (private, default), one or more
          teams (auto-switches to the Team tier + writes the edge in one click), or "Everyone"
          (workspace). No tier dropdown to forget. A member reaches a team-shared nav by being in
          that team (Teams admin). */}
      <section
        className="flex flex-col gap-2 rounded-md border border-border bg-panel px-3 py-3"
        data-testid="nav-shares"
      >
        <h4 className="text-xs font-semibold uppercase text-muted">Who sees this nav</h4>
        <div className="text-sm text-muted">
          {shareVis === "workspace"
            ? "Everyone in the workspace."
            : shares.length > 0
              ? `Members of ${shares.length === 1 ? "this team" : "these teams"}:`
              : "Only you (private). Add a team below, or share with everyone."}
        </div>
        {shareVis !== "workspace" && shares.length > 0 && (
          <ul className="flex flex-col gap-1">
            {shares.map((team) => (
              <li
                key={team}
                className="flex items-center justify-between rounded-md border border-border p-2"
              >
                <code className="text-sm">{teamName(team)}</code>
                <Button
                  size="sm"
                  variant="destructive"
                  onClick={() => void doUnshare(team)}
                  disabled={busy}
                  aria-label={`Remove share to ${team}`}
                >
                  Remove
                </Button>
              </li>
            ))}
          </ul>
        )}

        {/* Add a team — one click both switches the nav to the Team tier AND writes the
            `nav -[share]-> team` edge (see `addTeam`). The dropdown is fed by the real
            `teams.list` (no typing ids); if no teams exist yet, create one in the Teams admin.
            Hidden when the nav is already Workspace-wide (adding a team there is meaningless). */}
        {shareVis !== "workspace" && (
        <div className="flex flex-wrap items-center gap-2">
          <Select
            className="w-48"
            aria-label="Team to add"
            value={shareTeam}
            onChange={(e) => setShareTeam(e.target.value)}
          >
            <option value="">
              {sources.teams.length === 0 ? "No teams — create one in Teams" : "Pick a team"}
            </option>
            {sources.teams.map((t) => (
              <option key={t.team} value={t.team}>
                {t.name || t.team}
              </option>
            ))}
          </Select>
          <Button
            size="sm"
            variant="outline"
            onClick={() => void addTeam()}
            disabled={busy || !editId || !shareTeam}
            aria-label="Add team share"
          >
            Add team
          </Button>
        </div>
        )}

        {/* Workspace-wide toggle — the "everyone" audience, opposite the per-team shares. Setting
            Workspace makes every member see it (no team needed); "Just me" pulls it back to
            private. Both self-apply (`applyVis`), so audience is always one click, never a
            separate Save. */}
        <div className="flex flex-wrap items-center gap-2 pt-1">
          {shareVis === "workspace" ? (
            <Button
              size="sm"
              variant="outline"
              onClick={() => void applyVis("private")}
              disabled={busy || !editId}
              aria-label="Make private"
            >
              Make private (just me)
            </Button>
          ) : (
            <Button
              size="sm"
              variant="outline"
              onClick={() => void applyVis("workspace")}
              disabled={busy || !editId}
              aria-label="Share with everyone"
            >
              Share with everyone in workspace
            </Button>
          )}
        </div>
        <p className="text-xs text-muted">
          To share to a single user, put them in a team and add it here — a nav has no per-user
          audience (manage membership in the Teams admin).
        </p>
      </section>
    </>
  );

  return (
    <div className="flex h-full min-h-0 flex-col" data-testid="nav-admin">
      {error && (
        <div
          role="alert"
          className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-xs text-destructive"
        >
          {error}
        </div>
      )}

      <section className="flex min-h-0 flex-1 flex-col">
        <AdminToolbar
          search={navs.length ? filter : undefined}
          onSearch={navs.length ? setFilter : undefined}
          searchPlaceholder="Filter navs…"
          action={
            editId === null ? (
              <Button size="sm" onClick={startNew} aria-label="New nav">
                <Plus size={13} /> New nav
              </Button>
            ) : (
              <>
                {msg && <span className="text-xs text-muted">{msg}</span>}
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => setEditId(null)}
                  aria-label="Back"
                >
                  Cancel
                </Button>
              </>
            )
          }
        />
        <div className="min-h-0 flex-1 overflow-y-auto">
          {editId !== null && (
            // ── Editor (rendered inline below the toolbar, roster stays put) ──
            <div className="flex flex-col gap-3 p-4">
              {navEditor}
            </div>
          )}
          {editId === null && (
          <>
          {loading ? (
            <div className="text-sm text-muted">Loading…</div>
          ) : navs.length === 0 || visibleNavs.length === 0 ? (
            <AppEmptyState
              icon={List}
              title={
                navs.length === 0 ? "No navs yet." : "No navs match."
              }
              description={
                navs.length === 0
                  ? "Members see the built-in sidebar until you build one."
                  : "Clear the filter to see every nav."
              }
            />
          ) : (
            <Table aria-label="navs">
              <TableHeader sticky>
                <TableRow>
                  <TableHead>Title</TableHead>
                  <TableHead>Visibility</TableHead>
                  <TableHead>Id</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {visibleNavs.map((n) => (
                  <TableRow key={n.id}>
                    <TableCell className="font-medium text-fg">{n.title}</TableCell>
                    <TableCell>
                      <Badge variant="outline">{n.visibility}</Badge>
                    </TableCell>
                    <TableCell className="font-mono text-muted">{n.id}</TableCell>
                    <TableCell className="text-right">
                      <span className="flex justify-end gap-1">
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={() => void startEdit(n.id)}
                          aria-label={`Edit ${n.title}`}
                        >
                          Edit
                        </Button>
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={() => void setDefault(n.id)}
                          aria-label={`Set ${n.title} as workspace default`}
                        >
                          Make default
                        </Button>
                        <Button
                          size="sm"
                          variant="destructive"
                          onClick={() => void remove(n.id)}
                          aria-label={`Delete ${n.title}`}
                        >
                          Delete
                        </Button>
                      </span>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
          </>
          )}
        </div>
      </section>
    </div>
  );
}

