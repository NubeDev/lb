/**
 * `POST /mcp/call {tool, args}` — the universal host-mediated bridge. Every
 * platform verb that isn't wrapped by name in this library is reachable from
 * here without a library update (see `docs/skills/ingest-series/SKILL.md` for
 * the verb table). Re-checks the workspace + `mcp:<tool>:call` capability.
 */

import type { Client } from "./client.js";

/** Call `tool` with `args` over the bridge. `args` may be any JSON-serializable
 * value (pass `null` for a no-arg tool). Returns the tool's raw JSON output. */
export async function callMcp(
  client: Client,
  tool: string,
  args: unknown = {},
): Promise<unknown> {
  return client.requestJson<unknown>("POST", "/mcp/call", { tool, args });
}
