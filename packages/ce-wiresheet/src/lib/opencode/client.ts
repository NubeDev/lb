// Thin browser-side wrapper around the official opencode SDK (@opencode-ai/sdk).
// The wiresheet talks DIRECTLY from the browser to a locally-running
// `opencode serve` — the same "Model A" direct-to-service pattern the editor
// uses for the Control Engine (see lib/rest.ts). opencode in turn is wired to
// the engine's MCP server (the `nubeio-mcp` extension at :7980/mcp), so the
// agent can read and edit THIS wiresheet via its 17 tools.
//
//   browser (AgentChatPanel) ──HTTP/SSE──► opencode serve ──MCP──► engine :7980
//
// Launch the server with CORS allowed for the dev origin:
//   pnpm opencode          # → opencode serve --port 4096 --cors http://localhost:5193
//
// This module is framework-agnostic; the React panel (ui/AgentChatPanel.tsx)
// drives it. All SDK calls return { data, error }; we surface plain values and
// throw on transport failure.

// Import the client subpath, NOT the package root: the root entry pulls in
// `process.js` (node:child_process) for the server-spawn helpers, which break a
// browser bundle. We only need the HTTP client.
import { createOpencodeClient } from "@opencode-ai/sdk/client";

export type OpencodeClient = ReturnType<typeof createOpencodeClient>;

/** Default location of a local `opencode serve`. Overridable in the panel
 *  (persisted to localStorage) so users can point at a different port/host. */
export const DEFAULT_OPENCODE_URL = "http://localhost:4096";

/** Default engine MCP endpoint (the nubeio-mcp extension's `url` output). */
export const DEFAULT_ENGINE_MCP_URL = "http://127.0.0.1:7980/mcp";

const OPENCODE_URL_KEY = "ce-ui.opencodeUrl";

export function loadOpencodeUrl(): string {
  try {
    return window.localStorage.getItem(OPENCODE_URL_KEY) || DEFAULT_OPENCODE_URL;
  } catch {
    return DEFAULT_OPENCODE_URL;
  }
}

export function saveOpencodeUrl(url: string): void {
  try {
    window.localStorage.setItem(OPENCODE_URL_KEY, url.replace(/\/+$/, ""));
  } catch {
    /* ignore */
  }
}

/** A model the agent can run as, flattened from `config.providers()`. */
export interface ModelOption {
  providerID: string;
  modelID: string;
  label: string;
}

export interface ModelSelection {
  providerID: string;
  modelID: string;
}

/** Connect to a running opencode server and sanity-check it answers. */
export async function connect(baseUrl: string): Promise<OpencodeClient> {
  const client = createOpencodeClient({ baseUrl: baseUrl.replace(/\/+$/, "") });
  // A cheap call that proves the server is reachable AND has a usable provider.
  const res = await client.config.providers();
  if (res.error) throw new Error(describeError(res.error));
  return client;
}

/** Flatten `config.providers()` into a pickable, sorted model list, plus the
 *  server's default model (used to preselect). */
export async function listModels(
  client: OpencodeClient,
): Promise<{ models: ModelOption[]; defaultModel: ModelSelection | null }> {
  const res = await client.config.providers();
  if (res.error) throw new Error(describeError(res.error));
  const data = res.data as ProvidersResponse | undefined;
  const models: ModelOption[] = [];
  for (const p of data?.providers ?? []) {
    const provLabel = p.name || p.id;
    for (const m of Object.values(p.models ?? {})) {
      models.push({
        providerID: p.id,
        modelID: m.id,
        label: `${provLabel} · ${m.name || m.id}`,
      });
    }
  }
  models.sort((a, b) => a.label.localeCompare(b.label));

  // `default` maps providerID → preferred modelID.
  let defaultModel: ModelSelection | null = null;
  const def = data?.default ?? {};
  const firstProv = Object.keys(def)[0];
  if (firstProv && def[firstProv]) {
    defaultModel = { providerID: firstProv, modelID: def[firstProv] };
  } else if (models[0]) {
    defaultModel = { providerID: models[0].providerID, modelID: models[0].modelID };
  }
  return { models, defaultModel };
}

/** Best-effort: ensure opencode knows about the engine MCP server so the agent
 *  can drive the wiresheet. opencode normally already has this in its global
 *  config (~/.config/opencode/opencode.json → mcp.ce-engine); this just makes
 *  the wiring explicit and self-healing. Failures (already-exists, etc.) are
 *  swallowed — the global config remains the source of truth. */
export async function ensureEngineMcp(
  client: OpencodeClient,
  url = DEFAULT_ENGINE_MCP_URL,
): Promise<void> {
  try {
    await client.mcp.add({
      body: { name: "ce-engine", config: { type: "remote", url, enabled: true } },
    } as never);
  } catch {
    /* already configured / not supported — fine */
  }
}

/** Create a fresh chat session bound to the chosen model. */
export async function createSession(client: OpencodeClient): Promise<string> {
  const res = await client.session.create({ body: {} } as never);
  if (res.error) throw new Error(describeError(res.error));
  const id = (res.data as { id?: string } | undefined)?.id;
  if (!id) throw new Error("opencode: session create returned no id");
  return id;
}

/** Send a user turn. Streaming output arrives via the event subscription. */
export async function sendPrompt(
  client: OpencodeClient,
  sessionID: string,
  model: ModelSelection,
  text: string,
): Promise<void> {
  const res = await client.session.prompt({
    path: { id: sessionID },
    body: { model: { providerID: model.providerID, modelID: model.modelID }, parts: [{ type: "text", text }] },
  } as never);
  if (res.error) throw new Error(describeError(res.error));
}

/** Reply to a permission request the agent raised (e.g. before a write tool). */
export async function replyPermission(
  client: OpencodeClient,
  sessionID: string,
  permissionID: string,
  response: "once" | "always" | "reject",
): Promise<void> {
  try {
    await client.postSessionIdPermissionsPermissionId({
      path: { id: sessionID, permissionID },
      body: { response },
    } as never);
  } catch {
    /* the request may have expired; ignore */
  }
}

/** Interrupt the current turn. */
export async function abort(client: OpencodeClient, sessionID: string): Promise<void> {
  try {
    await client.session.abort({ path: { id: sessionID } } as never);
  } catch {
    /* ignore */
  }
}

/**
 * Subscribe to the server's event stream and invoke `onEvent` for each event.
 * Returns a disposer that stops the loop. The stream is global (all sessions);
 * the caller filters by sessionID.
 */
export function subscribeEvents(
  client: OpencodeClient,
  onEvent: (ev: OpencodeEvent) => void,
  onError?: (err: unknown) => void,
): () => void {
  let stopped = false;
  (async () => {
    try {
      const sub = await client.event.subscribe();
      const stream = (sub as { stream?: AsyncIterable<OpencodeEvent> }).stream;
      if (!stream) throw new Error("opencode: event stream unavailable");
      for await (const ev of stream) {
        if (stopped) break;
        onEvent(ev);
      }
    } catch (err) {
      if (!stopped) onError?.(err);
    }
  })();
  return () => {
    stopped = true;
  };
}

// --- minimal local shapes for the bits of the SDK payloads we read ----------

export interface OpencodeEvent {
  type: string;
  properties?: Record<string, unknown>;
}

interface ProvidersResponse {
  providers?: Array<{
    id: string;
    name?: string;
    models?: Record<string, { id: string; name?: string }>;
  }>;
  default?: Record<string, string>;
}

function describeError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err && typeof err === "object") {
    const o = err as { data?: { message?: string }; message?: string };
    return o.data?.message || o.message || JSON.stringify(err);
  }
  return String(err);
}
