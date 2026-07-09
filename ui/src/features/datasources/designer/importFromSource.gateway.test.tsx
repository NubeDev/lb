// Regression for the import-from-source hang (schema-designer scope). The canvas import effect was
// gated `if (!importSource || importing) return`, but the page sets `importSource` and
// `importing:true` in the same click — so the guard short-circuited the effect before it ran and
// `onImportDone` was never called: the "importing…" spinner hung forever (the exact symptom the
// user hit). This pins the contract that setting `importSource` runs the effect to completion and
// calls `onImportDone` (which clears the spinner), whether the catalog read finds tables or not.
//
// Against a REAL gateway (CLAUDE §9 — no fake backend): a real `datasource.add` registers the
// source; `discoverTables` runs the real `federation.discover` verb. No sidecar spawns in this env,
// so the catalog read fails (the data round-trip is owned by the rust `federation_sqlite_test`) —
// which pins BOTH halves of the fix: the effect's `finally` fires `onImportDone` (un-hangs the UI),
// and the failure is reported via `onImportError` rather than rejecting unhandled.

import { describe, expect, it, beforeAll } from "vitest";
import { render, waitFor, cleanup } from "@testing-library/react";

import { useRealGateway, signInReal } from "@/test/gateway-session";
import { addDatasource } from "@/lib/datasources";
import { SchemaDesignerCanvas } from "./SchemaDesignerCanvas";
import type { DbSchemaRecord } from "@/lib/datasources";

beforeAll(() => useRealGateway());

let n = 0;
const nextWs = () => `sd-import-${n++}`;

const emptyRecord = (name: string): DbSchemaRecord => ({
  name,
  version: 1,
  tables: [],
  fks: [],
  layout: {},
});

describe("SchemaDesignerCanvas import-from-source (real gateway)", () => {
  it("setting importSource runs the import effect and calls onImportDone (spinner clears)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await addDatasource({
      name: "demo-buildings",
      kind: "sqlite",
      endpoint: "127.0.0.1:0",
      dsn: "/tmp/lb-import-demo.db",
    });

    let done = false;
    let importError: string | null = null;
    render(
      <SchemaDesignerCanvas
        record={emptyRecord("shop")}
        onChange={() => {}}
        importSource="demo-buildings"
        importing={true}
        onImportDone={() => {
          done = true;
        }}
        onImportError={(msg) => {
          importError = msg;
        }}
      />,
    );

    // The effect ran (was NOT short-circuited by the `importing` guard) and reached its `finally`.
    await waitFor(() => expect(done).toBe(true), { timeout: 5000 });
    // No sidecar spawns here, so the catalog read fails — but the failure is REPORTED via
    // onImportError, never left to reject unhandled (the second half of the fix).
    expect(importError).not.toBeNull();
    cleanup();
  });
});
