// The datasource wire types (rules-workbench scope, Phase 3). The roster summary the gateway returns
// carries NO DSN — only the kind, endpoint, and a redacted secret ref. The DSN exists ONLY in the Add
// REQUEST type (`AddDatasource`), supplied on submit and forwarded to the host (which writes it to the
// secret store); it is never present on any RESPONSE type, by design (§6.7 / the redaction rule).

/** One registered source — what `datasource.list` returns. Never a DSN. */
export interface DatasourceSummary {
  name: string;
  kind: string;
  endpoint: string;
  /** The secret store reference (e.g. `federation/timescale`) — the ref, never the value. */
  secretRef: string;
}

/** The Add submit — the ONLY place a DSN exists client-side. */
export interface AddDatasource {
  name: string;
  kind: string;
  endpoint: string;
  /** The connection string. Write-only to the host; never read back. */
  dsn: string;
}

/** A connectivity probe result — green on `ok`, red with the error message otherwise. */
export interface ProbeResult {
  ok: boolean;
  /** The error message when the probe is red (a sidecar fault / refused endpoint / missing source). */
  error?: string;
}
