// The Add-datasource form (rules-workbench scope, Phase 3) — name / kind / endpoint / dsn fields. This
// is the ONLY place a DSN exists client-side (held in local state until submit, then forwarded to the
// host and forgotten). It surfaces the IMPLIED grants the registration carries (`net:tls:host:port:
// connect` + `secret:federation/{name}:get`) derived live from the form — DISPLAY ONLY ("this is what
// you're approving"); the real approval is the host install-grant record. One responsibility, one file.
//
// Kind is a SELECT over the kinds the sidecar's `source/mod.rs::connect` accepts — listed here as
// DATA (per-kind DSN semantics + placeholder), never branched on by the core. For `sqlite` the DSN is
// a database FILE PATH resolved on the node running the federation sidecar (not the browser), and the
// endpoint is the `127.0.0.1:0` convention (a file has no network endpoint) — prefilled, editable.

import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { AddDatasource } from "@/lib/datasources";
import { impliedGrants } from "./impliedGrants";

/** The source kinds the federation sidecar accepts — data, not branches (sqlite-datasource-demo scope). */
const KINDS = [
  { kind: "postgres", dsnHint: "dsn (host=… port=… user=… password=… dbname=…)", endpointHint: "endpoint (host:port)", localEndpoint: null },
  { kind: "timescale", dsnHint: "dsn (host=… port=… user=… password=… dbname=…)", endpointHint: "endpoint (host:port)", localEndpoint: null },
  { kind: "sqlite", dsnHint: "db file path on the node (e.g. /var/lib/lb/demo/buildings.db)", endpointHint: "endpoint (local — no network)", localEndpoint: "127.0.0.1:0" },
] as const;

interface Props {
  onAdd: (input: AddDatasource) => void;
}

export function AddDatasourceForm({ onAdd }: Props) {
  const [name, setName] = useState("");
  const [kind, setKind] = useState<string>(KINDS[0].kind);
  const [endpoint, setEndpoint] = useState("");
  const [dsn, setDsn] = useState("");

  const meta = KINDS.find((k) => k.kind === kind) ?? KINDS[0];
  const grants = impliedGrants({ name, endpoint });

  return (
    <form
      aria-label="add datasource"
      className="space-y-2 border-b border-border px-3 py-3"
      onSubmit={(e) => {
        e.preventDefault();
        if (name.trim() && endpoint.trim() && dsn.trim()) {
          onAdd({ name: name.trim(), kind, endpoint: endpoint.trim(), dsn });
          setName("");
          setEndpoint("");
          setDsn("");
        }
      }}
    >
      <div className="grid grid-cols-2 gap-2">
        <Input
          aria-label="datasource name"
          placeholder="name (e.g. timescale)"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <select
          aria-label="datasource kind"
          className="h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm"
          value={kind}
          onChange={(e) => {
            const next = e.target.value;
            setKind(next);
            const nextMeta = KINDS.find((k) => k.kind === next);
            // A local-file kind has no network endpoint; prefill its convention (still editable).
            if (nextMeta?.localEndpoint) setEndpoint(nextMeta.localEndpoint);
            else if (meta.localEndpoint && endpoint === meta.localEndpoint) setEndpoint("");
          }}
        >
          {KINDS.map((k) => (
            <option key={k.kind} value={k.kind}>
              {k.kind}
            </option>
          ))}
        </select>
        <Input
          aria-label="datasource endpoint"
          placeholder={meta.endpointHint}
          value={endpoint}
          onChange={(e) => setEndpoint(e.target.value)}
        />
        <Input
          aria-label="datasource dsn"
          type="password"
          placeholder={meta.dsnHint}
          value={dsn}
          onChange={(e) => setDsn(e.target.value)}
        />
      </div>

      {kind === "sqlite" && (
        <p className="text-xs text-muted">
          The path is resolved on the node running the federation sidecar — not your browser.
        </p>
      )}

      {/* The implied grants — display only. The real approval is the host install-grant record. */}
      <div aria-label="implied grants" className="text-xs text-muted">
        <span className="text-muted/80">Registering this implies approving:</span>
        {grants.length === 0 ? (
          <span className="ml-1 italic">fill in name + endpoint to preview</span>
        ) : (
          <ul className="ml-2 mt-0.5 space-y-0.5">
            {grants.map((g) => (
              <li key={g} className="font-mono text-accent">
                {g}
              </li>
            ))}
          </ul>
        )}
      </div>

      <Button aria-label="submit datasource" size="sm" type="submit">
        Add datasource
      </Button>
    </form>
  );
}
