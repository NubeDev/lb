// The nav builder (nav scope) — an admin surface to compose a workspace's navigation menu: pick items
// from the three REAL sources (core surfaces, `dashboard.list`, `ext.list`), add a dynamic tag-group,
// order/group them, set visibility, share to a team, and set the workspace default. The nav is a
// LENS — it grants nothing; it only shapes the menu. Every write is a real `nav.*` verb re-checked
// server-side; the builder can never author a cap. Cap-gated for DISPLAY by `nav.save`; the gateway
// is the boundary. Markup + local edit state; the data + writes live in `useNavs`.

import { useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { CAP, hasCap } from "@/lib/session";
import type { NavItem, Visibility } from "@/lib/nav";
import { useNavs } from "./useNavs";

/** The core surfaces a nav may link to (the static core set — surfaces are core, not extensions).
 *  Kept aligned with the shell's `SURFACES`; a nav entry stores the opaque key as data. */
const CORE_SURFACES = [
  "channels",
  "dashboards",
  "rules",
  "flows",
  "datasources",
  "reminders",
  "ingest",
  "data",
  "system",
  "telemetry",
  "inbox",
  "outbox",
  "admin",
  "extensions",
  "studio",
  "settings",
];

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

  // Item-add form state.
  const [addKind, setAddKind] = useState<NavItem["kind"]>("surface");
  const [addSurface, setAddSurface] = useState(CORE_SURFACES[0]);
  const [addDashboard, setAddDashboard] = useState("");
  const [addExt, setAddExt] = useState("");
  const [addFacetKey, setAddFacetKey] = useState("");
  const [addLabel, setAddLabel] = useState("");
  // reusable-pages: template-group binds ONE dashboard's parameter (`addVar`) per facet value; a plain
  // `dashboard` entry can carry a pinned binding (`addPinned`, `name=value, …`) as a named instance.
  const [addVar, setAddVar] = useState("");
  const [addPinned, setAddPinned] = useState("");

  // Share form state.
  const [shareVis, setShareVis] = useState<Visibility>("private");
  const [shareTeam, setShareTeam] = useState("");

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

  // Parse a `name=value, name2=value2` pinned-binding string into a `vars` record (reusable-pages).
  const parsePinned = (raw: string): Record<string, string> => {
    const out: Record<string, string> = {};
    for (const pair of raw.split(",")) {
      const [k, ...rest] = pair.split("=");
      const name = k.trim();
      const value = rest.join("=").trim();
      if (name && value) out[name] = value;
    }
    return out;
  };

  const addItem = () => {
    let it: NavItem | null = null;
    if (addKind === "surface") it = { kind: "surface", surface: addSurface, label: addLabel };
    else if (addKind === "dashboard" && addDashboard) {
      // A plain dashboard entry, optionally carrying a pinned binding (a named page instance).
      const vars = parsePinned(addPinned);
      it = { kind: "dashboard", dashboard: addDashboard, label: addLabel };
      if (Object.keys(vars).length) it.vars = vars;
    } else if (addKind === "ext" && addExt) it = { kind: "ext", ext: addExt, label: addLabel };
    else if (addKind === "tag-group" && addFacetKey)
      it = { kind: "tag-group", label: addLabel || addFacetKey, facets: [{ key: addFacetKey }] };
    else if (addKind === "template-group" && addDashboard && addVar && addFacetKey)
      // reusable-pages: ONE dashboard fanned out per distinct value of `addFacetKey`, bound to `addVar`.
      it = {
        kind: "template-group",
        label: addLabel || addFacetKey,
        dashboard: addDashboard,
        var: addVar,
        facets: [{ key: addFacetKey }],
      };
    else if (addKind === "group") it = { kind: "group", label: addLabel || "Group", items: [] };
    if (it) {
      setItems((cur) => [...cur, it!]);
      setAddLabel("");
      setAddFacetKey("");
      setAddVar("");
      setAddPinned("");
    }
  };

  const move = (i: number, delta: number) => {
    setItems((cur) => {
      const next = [...cur];
      const j = i + delta;
      if (j < 0 || j >= next.length) return cur;
      [next[i], next[j]] = [next[j], next[i]];
      return next;
    });
  };

  const removeItem = (i: number) => setItems((cur) => cur.filter((_, k) => k !== i));

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

  const doShare = async () => {
    if (!editId) return;
    setBusy(true);
    setMsg(null);
    try {
      await share(editId, shareVis, shareVis === "team" ? shareTeam : undefined);
      setMsg(`Visibility set to ${shareVis}.`);
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

  const dashOptions = useMemo(
    () => sources.dashboards.map((d) => ({ id: `dashboard:${d.id}`, label: d.title })),
    [sources.dashboards],
  );
  const extOptions = useMemo(
    () => sources.extensions.map((e) => ({ id: e.ext, label: e.ui?.label ?? e.ext })),
    [sources.extensions],
  );

  if (!canWrite) {
    return (
      <div className="p-4 text-sm text-muted" role="status">
        You need the nav authoring capability (<code>mcp:nav.save:call</code>) to build navs.
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 p-4" data-testid="nav-admin">
      {error && (
        <div className="text-sm text-destructive" role="alert">
          {error}
        </div>
      )}

      {editId === null ? (
        // ── Roster ──
        <section className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-semibold">Navigation menus</h3>
            <Button size="sm" onClick={startNew} aria-label="New nav">
              New nav
            </Button>
          </div>
          {loading ? (
            <div className="text-sm text-muted">Loading…</div>
          ) : navs.length === 0 ? (
            <div className="text-sm text-muted">
              No navs yet. Members see the built-in sidebar until you build one.
            </div>
          ) : (
            <ul className="flex flex-col gap-2">
              {navs.map((n) => (
                <li
                  key={n.id}
                  className="flex items-center justify-between rounded-md border border-border p-2"
                >
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium">{n.title}</span>
                    <Badge variant="outline">{n.visibility}</Badge>
                    <code className="text-xs text-muted">{n.id}</code>
                  </div>
                  <div className="flex gap-2">
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => void startEdit(n.id)}
                      aria-label={`Edit ${n.title}`}
                    >
                      Edit
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
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
                  </div>
                </li>
              ))}
            </ul>
          )}
        </section>
      ) : (
        // ── Editor ──
        <section className="flex min-h-0 flex-1 flex-col gap-3">
          <div className="flex items-center justify-between">
            <Button size="sm" variant="ghost" onClick={() => setEditId(null)} aria-label="Back">
              ← Back
            </Button>
            {msg && <span className="text-xs text-muted">{msg}</span>}
          </div>

          <Input
            aria-label="Nav title"
            placeholder="Menu title (e.g. Operations)"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
          />

          {/* The ordered items list */}
          <ol className="flex flex-col gap-2" data-testid="nav-items">
            {items.map((it, i) => (
              <li
                key={`${it.kind}-${i}`}
                className="flex items-center justify-between rounded-md border border-border p-2"
              >
                <div className="flex items-center gap-2">
                  <Badge variant="outline">{it.kind}</Badge>
                  <span className="text-sm">
                    {it.label ||
                      it.surface ||
                      it.dashboard ||
                      it.ext ||
                      it.facets?.map((f) => f.key).join(",")}
                  </span>
                </div>
                <div className="flex gap-1">
                  <Button size="sm" variant="ghost" onClick={() => move(i, -1)} aria-label="Move up">
                    ↑
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => move(i, 1)} aria-label="Move down">
                    ↓
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => removeItem(i)}
                    aria-label="Remove item"
                  >
                    ✕
                  </Button>
                </div>
              </li>
            ))}
            {items.length === 0 && (
              <li className="text-sm text-muted">No items yet — add one below.</li>
            )}
          </ol>

          {/* Add-item form */}
          <div className="flex flex-wrap items-center gap-2 rounded-md border border-dashed border-border p-2">
            <Select
              className="w-36"
              aria-label="Item kind"
              value={addKind}
              onChange={(e) => setAddKind(e.target.value as NavItem["kind"])}
            >
              <option value="surface">Surface</option>
              <option value="dashboard">Dashboard</option>
              <option value="ext">Extension</option>
              {/* Two dynamism mechanisms, named side by side (reusable-pages scope, "one mental model"):
                  tag-group = MANY dashboards (by tag); template-group = ONE dashboard, many bindings. */}
              <option value="tag-group">Dashboards by tag</option>
              <option value="template-group">One dashboard per ⟨value⟩</option>
              <option value="group">Group</option>
            </Select>

            {addKind === "surface" && (
              <Select
                className="w-40"
                aria-label="Surface"
                value={addSurface}
                onChange={(e) => setAddSurface(e.target.value)}
              >
                {CORE_SURFACES.map((s) => (
                  <option key={s} value={s}>
                    {s}
                  </option>
                ))}
              </Select>
            )}
            {addKind === "dashboard" && (
              <>
                <Select
                  className="w-48"
                  aria-label="Dashboard"
                  value={addDashboard}
                  onChange={(e) => setAddDashboard(e.target.value)}
                >
                  <option value="">Pick a dashboard</option>
                  {dashOptions.map((d) => (
                    <option key={d.id} value={d.id}>
                      {d.label}
                    </option>
                  ))}
                </Select>
                {/* reusable-pages: an optional pinned binding → a curated named instance in the menu. */}
                <Input
                  aria-label="Pinned vars"
                  placeholder="Pin vars (site=plant-2)"
                  className="w-44"
                  value={addPinned}
                  onChange={(e) => setAddPinned(e.target.value)}
                />
              </>
            )}
            {addKind === "template-group" && (
              // reusable-pages: pick the TEMPLATE dashboard, the parameter it binds, and the facet key
              // whose distinct values fan it out (one menu page per value, no dashboard copy).
              <>
                <Select
                  className="w-48"
                  aria-label="Template dashboard"
                  value={addDashboard}
                  onChange={(e) => setAddDashboard(e.target.value)}
                >
                  <option value="">Pick a template</option>
                  {dashOptions.map((d) => (
                    <option key={d.id} value={d.id}>
                      {d.label}
                    </option>
                  ))}
                </Select>
                <Input
                  aria-label="Template parameter"
                  placeholder="Parameter (e.g. site)"
                  className="w-36"
                  value={addVar}
                  onChange={(e) => setAddVar(e.target.value)}
                />
                <Input
                  aria-label="Fan-out facet key"
                  placeholder="Facet key (e.g. site)"
                  className="w-36"
                  value={addFacetKey}
                  onChange={(e) => setAddFacetKey(e.target.value)}
                />
              </>
            )}
            {addKind === "ext" && (
              <Select
                className="w-48"
                aria-label="Extension"
                value={addExt}
                onChange={(e) => setAddExt(e.target.value)}
              >
                <option value="">Pick an extension</option>
                {extOptions.map((e) => (
                  <option key={e.id} value={e.id}>
                    {e.label}
                  </option>
                ))}
              </Select>
            )}
            {addKind === "tag-group" && (
              <Input
                aria-label="Facet key"
                placeholder="Facet key (e.g. site)"
                className="w-40"
                value={addFacetKey}
                onChange={(e) => setAddFacetKey(e.target.value)}
              />
            )}
            <Input
              aria-label="Item label"
              placeholder="Label (optional)"
              className="w-40"
              value={addLabel}
              onChange={(e) => setAddLabel(e.target.value)}
            />
            <Button size="sm" variant="outline" onClick={addItem} aria-label="Add item">
              Add item
            </Button>
          </div>

          {/* Save + share */}
          <div className="flex flex-wrap items-center gap-2">
            <Button size="sm" onClick={() => void doSave()} disabled={busy} aria-label="Save nav">
              Save
            </Button>
            <div className="flex items-center gap-2">
              <Select
                className="w-32"
                aria-label="Visibility"
                value={shareVis}
                onChange={(e) => setShareVis(e.target.value as Visibility)}
              >
                <option value="private">Private</option>
                <option value="team">Team</option>
                <option value="workspace">Workspace</option>
              </Select>
              {shareVis === "team" && (
                <Input
                  aria-label="Team"
                  placeholder="team:ops"
                  className="w-32"
                  value={shareTeam}
                  onChange={(e) => setShareTeam(e.target.value)}
                />
              )}
              <Button
                size="sm"
                variant="outline"
                onClick={() => void doShare()}
                disabled={busy || !editId}
                aria-label="Apply visibility"
              >
                Apply visibility
              </Button>
            </div>
          </div>

          {/* The live share roster — every team this nav is currently shared to, each removable.
              A nav has no per-user share by design; a user reaches it by being a member of one of
              these teams (managed in the Teams admin via teams.add_member/remove_member). */}
          <section className="flex flex-col gap-2" data-testid="nav-shares">
            <h4 className="text-xs font-semibold uppercase text-muted">Shared to teams</h4>
            {shares.length === 0 ? (
              <div className="text-sm text-muted">
                {shareVis === "team"
                  ? "Not shared to any team yet — add one above."
                  : "Team shares apply only at the Team visibility tier."}
              </div>
            ) : (
              <ul className="flex flex-col gap-1">
                {shares.map((team) => (
                  <li
                    key={team}
                    className="flex items-center justify-between rounded-md border border-border p-2"
                  >
                    <code className="text-sm">{team}</code>
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
          </section>
        </section>
      )}
    </div>
  );
}
