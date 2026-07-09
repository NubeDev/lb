// The thin client for the Rust seam (Axum `POST /convert`). No state, no auth —
// the standalone tool owns no token / workspace (grafana-conv scope Stage 0).

import type { ConvertResponse } from "../types";

export interface ConvertResult {
  ok: true;
  data: ConvertResponse;
}

export interface ConvertFailure {
  ok: false;
  status: number;
  message: string;
}

/** Send a parsed Grafana document to the seam; return `{dashboard, report}`. */
export async function convert(grafana: unknown): Promise<ConvertResult | ConvertFailure> {
  let resp: Response;
  try {
    resp = await fetch("/convert", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ grafana }),
    });
  } catch (e) {
    return {
      ok: false,
      status: 0,
      message: `cannot reach the converter seam — is the Rust server running? (${fmtErr(e)})`,
    };
  }

  if (!resp.ok) {
    const body = await resp.json().catch(() => ({ error: resp.statusText }));
    return { ok: false, status: resp.status, message: body.error ?? resp.statusText };
  }
  const data = (await resp.json()) as ConvertResponse;
  return { ok: true, data };
}

function fmtErr(e: unknown): string {
  if (e instanceof Error) return e.message;
  return String(e);
}
