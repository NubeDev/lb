// The Settings → Sidebar tab (hide-and-pins scope) — the admin's one subtractive sidebar-curation
// lever. Lists every rail source honestly from the SAME data the rail renders (the shared
// `SURFACE_GROUPS` + live `ext.list` pages — never a copy), with a hide switch per row; saving
// replaces the one workspace `nav_hidden` record (`nav.hidden.set`, full-set LWW). Hiding is
// DECLUTTER, never authz: a hidden page a member is permitted to reach still loads by deep link,
// and the gateway re-checks every verb on click regardless. Hide beats pin.
//
// Member view: a non-admin (no `mcp:nav.save:call`) sees the current hidden-set read-only — the
// gateway re-checks the cap server-side regardless.

import { useCallback, useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { getNavHidden, setNavHidden } from "@/lib/nav";
import { CAP, hasCap } from "@/lib/session";
import { SURFACE_GROUPS } from "@/features/shell";
import { SURFACE_DEF } from "@/features/shell/surfaceDefs";
import { useExtensionPages } from "@/features/ext-host/useExtensionPages";
import { Field, FieldGroup } from "./Field";

interface Props {
  ws: string;
  caps: string[] | undefined;
}

export function SidebarTab({ ws, caps }: Props) {
  const canEdit = hasCap(caps, CAP.navSave);
  const { pages } = useExtensionPages(ws);

  const [hidden, setHidden] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [dirty, setDirty] = useState(false);
  const [status, setStatus] = useState<"idle" | "saving" | "saved" | "error">("idle");

  const load = useCallback(() => {
    setLoading(true);
    void getNavHidden()
      .then((h) => {
        setHidden(new Set(h.hidden));
        setDirty(false);
        setLoading(false);
      })
      .catch(() => setLoading(false)); // read-deny/offline → an empty set, controls stay disabled
  }, []);
  useEffect(load, [load]);

  const toggle = (ref: string, hide: boolean) => {
    setHidden((prev) => {
      const next = new Set(prev);
      if (hide) next.add(ref);
      else next.delete(ref);
      return next;
    });
    setDirty(true);
    setStatus("idle");
  };

  const save = () => {
    setStatus("saving");
    void setNavHidden([...hidden])
      .then((h) => {
        setHidden(new Set(h.hidden));
        setDirty(false);
        setStatus("saved");
      })
      .catch(() => setStatus("error"));
  };

  const row = (ref: string, label: string, help?: string) => (
    <Field key={ref} label={label} help={help} htmlFor={`hide-${ref}`}>
      <div className="flex items-center gap-2">
        <Switch
          id={`hide-${ref}`}
          checked={hidden.has(ref)}
          disabled={!canEdit || loading}
          onCheckedChange={(v) => toggle(ref, v)}
          aria-label={`Hide ${label}`}
        />
        <span className="text-xs text-muted">{hidden.has(ref) ? "Hidden" : "Shown"}</span>
      </div>
    </Field>
  );

  return (
    <div className="mx-auto w-full max-w-2xl px-4 py-6">
      <p className="mb-4 text-xs leading-relaxed text-muted">
        Hide entries from the sidebar for everyone in this workspace. Hiding only declutters the
        rail — it never revokes access: a member with permission can still reach a hidden page by
        its link, and pinned favorites of a hidden page are hidden too.
        {!canEdit && " You can view the workspace's hidden entries, but only an admin can change them."}
      </p>

      {SURFACE_GROUPS.map((grp) => (
        <FieldGroup key={grp.label} title={grp.label}>
          {grp.items.map((key) => row(key, SURFACE_DEF[key].label))}
        </FieldGroup>
      ))}

      {pages.length > 0 && (
        <FieldGroup title="Extensions">
          {pages.map((p) => row(`ext:${p.ext}`, p.ui.label || p.ext))}
        </FieldGroup>
      )}

      {canEdit && (
        <div className="mt-4 flex items-center gap-3">
          <Button size="sm" onClick={save} disabled={!dirty || status === "saving"}>
            {status === "saving" ? "Saving…" : "Save"}
          </Button>
          {status === "saved" && !dirty && <span className="text-xs text-muted">Saved.</span>}
          {status === "error" && (
            <span className="text-xs text-destructive">Save failed — try again.</span>
          )}
        </div>
      )}
    </div>
  );
}
