// The control-engine federated page: pick an appliance, then mount the vendored `CeEditor` wired to a
// `BridgeTransport` for that appliance. The browser never touches CE — the picker lists appliances via
// `control-engine.appliance.list`, the empty state offers an `control-engine.appliance.add` flow, and
// every canvas action rides `bridge.call` (host-re-checked against install-scope ∩ grant, so a read-only
// user's canvas is read-only). One responsibility: page composition; the transport lives in its own file.

import { useEffect, useMemo, useState } from "react";
import { CeEditor } from "@nube/ce-wiresheet";

import type { Bridge, MountCtx } from "./contract";
import { BridgeTransport } from "./bridge-transport";

interface Appliance {
  id: string;
  name: string;
  base: string;
}

// `ctx` (workspace, etc.) is still part of the mount contract but no longer read here —
// the host header owns the workspace label, so the page renders no title/workspace chrome.
export function Page({ bridge }: { ctx: MountCtx; bridge: Bridge }) {
  const [appliances, setAppliances] = useState<Appliance[] | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // List this workspace's appliances (workspace-walled by the host — a ws-B user sees only ws-B's).
  useEffect(() => {
    let cancelled = false;
    bridge
      .call<{ appliances: Appliance[] }>("control-engine.appliance.list")
      .then((r) => {
        if (cancelled) return;
        setAppliances(r.appliances ?? []);
        setSelected((prev) => prev ?? r.appliances?.[0]?.id ?? null);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e instanceof Error ? e.message : e));
      });
    return () => {
      cancelled = true;
    };
  }, [bridge]);

  // One transport per selected appliance — recreated on switch so its request/stream target the new CE.
  const transport = useMemo(
    () => (selected ? new BridgeTransport(bridge, selected) : null),
    [bridge, selected],
  );

  return (
    <div className="ce-page flex h-full flex-col bg-bg text-fg" data-control-engine-page>
      {/* The HOST header owns the title + workspace (this page renders INTO the host's content
          region), so we render NO header of our own — a second "Control Engine · workspace" bar
          clashed with the host's (see the theme/header/nodes/codec handover, issue #2). The only
          chrome this page still needs is the appliance PICKER, and only when there's a real choice
          (≥2 appliances); a single appliance has nothing to pick. The connected/disconnected badge
          lives inside CeEditor (its toolbar `ConnectionStatus`), so it survives dropping this bar. */}
      {appliances && appliances.length > 1 && (
        <div className="flex items-center border-b border-border px-4 py-1.5">
          <label className="ml-auto flex items-center gap-2 text-xs text-muted">
            Appliance
            <select
              className="rounded border border-border bg-panel px-2 py-1 text-fg"
              value={selected ?? ""}
              onChange={(e) => setSelected(e.target.value)}
              aria-label="appliance"
            >
              {appliances.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}

      <div className="min-h-0 flex-1">
        {error ? (
          <StateMsg tone="error">{error}</StateMsg>
        ) : appliances == null ? (
          <StateMsg>Loading appliances…</StateMsg>
        ) : appliances.length === 0 ? (
          <EmptyState bridge={bridge} onAdded={(id) => setSelected(id)} />
        ) : transport && selected ? (
          <CeEditor base={applianceBase(appliances, selected)} transport={transport} />
        ) : (
          <StateMsg>Select an appliance to open its wiresheet.</StateMsg>
        )}
      </div>
    </div>
  );
}

function applianceBase(appliances: Appliance[], id: string): string {
  // `base` is cosmetic when a transport is injected (`setRestTransport` routes every request), but the
  // editor still reads it for display — hand it the selected appliance's origin.
  return appliances.find((a) => a.id === id)?.base ?? "";
}

/** Empty state: no appliance registered → a small add flow calling `control-engine.appliance.add`. The
 *  verb is admin-capped, so a non-admin's submit surfaces the host's DENIED error (absent-not-broken). */
function EmptyState({ bridge, onAdded }: { bridge: Bridge; onAdded: (id: string) => void }) {
  const [id, setId] = useState("");
  const [base, setBase] = useState("http://127.0.0.1:7979");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!id.trim()) return;
    setBusy(true);
    setErr(null);
    try {
      await bridge.call("control-engine.appliance.add", {
        id: id.trim(),
        name: id.trim(),
        mode: "local",
        base: base.trim(),
      });
      onAdded(id.trim());
    } catch (e2) {
      setErr(String(e2 instanceof Error ? e2.message : e2));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex h-full items-center justify-center p-6">
      <form
        onSubmit={submit}
        className="w-80 rounded-lg border border-border bg-panel p-4"
        aria-label="add appliance"
      >
        <h2 className="text-sm font-semibold">No control engines yet</h2>
        <p className="mt-1 text-xs text-muted">Register a CE this workspace can drive.</p>
        <input
          className="mt-3 w-full rounded border border-border bg-bg px-2 py-1 text-sm"
          placeholder="appliance id"
          value={id}
          onChange={(e) => setId(e.target.value)}
          aria-label="appliance id"
        />
        <input
          className="mt-2 w-full rounded border border-border bg-bg px-2 py-1 text-sm"
          placeholder="http://127.0.0.1:7979"
          value={base}
          onChange={(e) => setBase(e.target.value)}
          aria-label="appliance base"
        />
        {err && <p className="mt-2 text-xs text-red-400">{err}</p>}
        <button
          type="submit"
          disabled={busy}
          className="mt-3 w-full rounded bg-accent px-2 py-1 text-sm font-medium text-black disabled:opacity-50"
        >
          {busy ? "Adding…" : "Add appliance"}
        </button>
      </form>
    </div>
  );
}

function StateMsg({ children, tone }: { children: React.ReactNode; tone?: "error" }) {
  return (
    <div className="flex h-full items-center justify-center p-6">
      <span className={`text-sm ${tone === "error" ? "text-red-400" : "text-muted"}`}>{children}</span>
    </div>
  );
}
