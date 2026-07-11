// Minimal gateway client — login + ext.list + mcp_call. One invoke seam; HTTP only (no Tauri).
// A product host extends this with its own verbs; the shell only needs auth + discovery + mount.

export function gatewayUrl(): string {
  return (import.meta as any).env?.VITE_GATEWAY_URL || "http://127.0.0.1:8080";
}

export function sessionToken(): string | null {
  try {
    const s = JSON.parse(localStorage.getItem("lb.session") || "null");
    return s?.token || null;
  } catch {
    return null;
  }
}

export function authHeaders(): Record<string, string> {
  const t = sessionToken();
  return t ? { Authorization: `Bearer ${t}` } : {};
}

async function postJson(path: string, body: unknown): Promise<any> {
  const r = await fetch(`${gatewayUrl()}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json", ...authHeaders() },
    body: JSON.stringify(body),
  });
  if (r.status === 401) {
    localStorage.removeItem("lb.session");
  }
  if (!r.ok) throw new Error(`${r.status} ${r.statusText}`);
  return r.json();
}

async function getJson(path: string): Promise<any> {
  const r = await fetch(`${gatewayUrl()}${path}`, { headers: authHeaders() });
  if (!r.ok) throw new Error(`${r.status} ${r.statusText}`);
  return r.json();
}

// The verbs the shell needs: login, ext_list, mcp_call.
export async function login(user: string, workspace: string, secret = ""): Promise<Session> {
  return postJson("/login", { user, workspace, secret });
}

export async function listExtensions(): Promise<ExtRow[]> {
  return getJson("/extensions");
}

export async function mcpCall(tool: string, args: unknown): Promise<any> {
  return postJson("/mcp/call", { tool, args });
}

export interface Session {
  token: string;
  principal: string;
  workspace: string;
  caps?: string[];
}

export interface ExtRow {
  ext: string;
  version?: string;
  tier?: string;
  enabled?: boolean;
  running?: boolean;
  ui?: { entry: string; label?: string; icon?: string };
}

export async function acceptInvite(
  workspace: string,
  token: string,
  secret: string,
  currentSecret?: string,
): Promise<Session> {
  return postJson("/public/invite/accept", {
    workspace,
    token,
    secret,
    current_secret: currentSecret,
  });
}
