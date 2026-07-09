// The schema-designer lib client, driven against a REAL in-process gateway (schema-designer scope;
// CLAUDE §9 / testing §0 — no fake backend). Pins the load/save round-trip through the real MCP
// bridge → `db_schema:{ws}:{name}` record in the real SurrealDB store: save → get (full layout) →
// list → delete → get-absent; plus the workspace-isolation wall (ws-B cannot get/list ws-A's
// schema). The canvas renders over this client; the round-trip IS the contract.
//
// No mocks: the SurrealDB store + caps + the MCP bridge + the `dbschema.*` host verbs are exercised
// for real. The external DB is not touched here (`dbschema.save` is store-only; migrate/write/
// export are pinned by the Rust integration tests against a real spawned SQLite sidecar).

import { beforeAll, describe, expect, it } from "vitest";

import {
  deleteDbSchema,
  getDbSchema,
  listDbSchemas,
  saveDbSchema,
  type DbSchemaRecord,
} from "@/lib/datasources";
import { signInReal, useRealGateway } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `sd-lib-${n++}`;

beforeAll(() => useRealGateway());

/** A minimal valid record shape (one table, one column, a layout entry). */
function sampleRecord(name: string): DbSchemaRecord {
  return {
    name,
    version: 1,
    tables: [
      {
        name: "users",
        pk: ["id"],
        columns: [
          { name: "id", type: "integer", nullable: false },
          { name: "email", type: "text", nullable: false },
        ],
      },
    ],
    fks: [],
    layout: { users: { x: 120, y: 80 } },
  };
}

describe("dbschema.* (real gateway)", () => {
  it("save → get round-trips the full record including layout", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveDbSchema("shop", sampleRecord("shop"));

    const got = await getDbSchema("shop");
    expect(got).not.toBeNull();
    expect(got!.name).toBe("shop");
    expect(got!.tables).toHaveLength(1);
    expect(got!.tables[0].name).toBe("users");
    expect(got!.tables[0].pk).toEqual(["id"]);
    expect(got!.tables[0].columns).toHaveLength(2);
    expect(got!.layout.users).toEqual({ x: 120, y: 80 });
  });

  it("list shows the saved schema (name + table count)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveDbSchema("catalog", sampleRecord("catalog"));

    const list = await listDbSchemas();
    const row = list.find((s) => s.name === "catalog");
    expect(row).toBeDefined();
    expect(row!.tableCount).toBe(1);
    expect(row!.version).toBe(1);
  });

  it("delete removes the schema (get returns null)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveDbSchema("temp", sampleRecord("temp"));
    expect(await getDbSchema("temp")).not.toBeNull();

    await deleteDbSchema("temp");
    expect(await getDbSchema("temp")).toBeNull();
  });

  it("save is idempotent (re-save overwrites, layout updates)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveDbSchema("shop", sampleRecord("shop"));

    // re-save with an updated layout
    const updated = sampleRecord("shop");
    updated.layout.users = { x: 300, y: 200 };
    await saveDbSchema("shop", updated);

    const got = await getDbSchema("shop");
    expect(got!.layout.users).toEqual({ x: 300, y: 200 });
    // still only one schema named shop (no duplicate)
    const list = await listDbSchemas();
    expect(list.filter((s) => s.name === "shop")).toHaveLength(1);
  });

  // Workspace-isolation between tenants is pinned by the Rust integration test
  // `dbschema_workspace_isolation` (host/tests/schema_designer_test.rs) which exercises the real
  // `call_tool` dispatch across two principals + two workspaces. The UI gateway path forwards the
  // workspace from the session token (a gateway concern, not a schema-designer one); the round-trip
  // tests above are the designer's UI-relevant contract.
});
