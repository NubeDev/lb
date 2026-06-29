// The Add-datasource form (rules-workbench scope, Phase 3) — name / kind / endpoint / dsn fields. This
// is the ONLY place a DSN exists client-side (held in local state until submit, then forwarded to the
// host and forgotten). It surfaces the IMPLIED grants the registration carries (`net:tls:host:port:
// connect` + `secret:federation/{name}:get`) derived live from the form — DISPLAY ONLY ("this is what
// you're approving"); the real approval is the host install-grant record. One responsibility, one file.

import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { AddDatasource } from "@/lib/datasources";
import { impliedGrants } from "./impliedGrants";

interface Props {
  onAdd: (input: AddDatasource) => void;
}

export function AddDatasourceForm({ onAdd }: Props) {
  const [name, setName] = useState("");
  const [kind, setKind] = useState("postgres");
  const [endpoint, setEndpoint] = useState("");
  const [dsn, setDsn] = useState("");

  const grants = impliedGrants({ name, endpoint });

  return (
    <form
      aria-label="add datasource"
      className="space-y-2 border-b border-border px-3 py-3"
      onSubmit={(e) => {
        e.preventDefault();
        if (name.trim() && endpoint.trim() && dsn.trim()) {
          onAdd({ name: name.trim(), kind: kind.trim(), endpoint: endpoint.trim(), dsn });
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
        <Input
          aria-label="datasource kind"
          placeholder="kind (e.g. postgres)"
          value={kind}
          onChange={(e) => setKind(e.target.value)}
        />
        <Input
          aria-label="datasource endpoint"
          placeholder="endpoint (host:port)"
          value={endpoint}
          onChange={(e) => setEndpoint(e.target.value)}
        />
        <Input
          aria-label="datasource dsn"
          type="password"
          placeholder="dsn (connection string)"
          value={dsn}
          onChange={(e) => setDsn(e.target.value)}
        />
      </div>

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
