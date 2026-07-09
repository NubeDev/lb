// The Python quick-start snippet (setup scope) — generates the copy-paste `lb_client` example the
// Ingest wizard's last step shows, prefilled with the workspace, series, and (if just minted) the API
// key the earlier steps produced. It mirrors `clients/python/example.py` + the `lb_client` five-method
// shape (Client → with_bearer → write_samples / latest_sample / call_mcp), so what the user copies is
// the SAME path the real client library exercises — not an invented API. Pure string-building, no
// React, no I/O: reusable anywhere a "here's how to push data" snippet is wanted (docs, other
// wizards). One responsibility per file (FILE-LAYOUT).

export interface SnippetInput {
  /** The gateway origin the user is on (e.g. http://127.0.0.1:8080). */
  url: string;
  /** The workspace slug. */
  ws: string;
  /** The series to write into. */
  series: string;
  /** The one-time API key bearer (`lbk_…`) if just minted; else a placeholder the user fills in. */
  apiKey?: string;
}

/** The `pip install` line for the client package. */
export const PIP_INSTALL = "pip install lb-client";

/** Build the runnable Python quick-start. When `apiKey` is absent, a clearly-marked placeholder is
 *  emitted so the snippet stays copyable and the user knows exactly what to substitute. */
export function pythonSnippet({ url, ws, series, apiKey }: SnippetInput): string {
  const key = apiKey || "lbk_REPLACE_WITH_YOUR_API_KEY";
  return `import time
from lb_client import Client, write_samples, latest_sample, call_mcp

# The gateway node + your API key (shown once when you created it).
client = Client("${url}", "placeholder").with_bearer("${key}")

# 1. Push one Sample into the "${series}" series in workspace "${ws}".
#    (producer is host-forced to the key's principal, so you omit it.)
written = write_samples(client, [
    {
        "series": "${series}",
        "ts": int(time.time() * 1000),
        "seq": 1,
        "payload": 61.4,
        "labels": {"host": "pi-7"},
    },
])
print(f"accepted={written['accepted']} committed={written['committed']}")

# 2. Read the newest value back — the round-trip.
print("latest:", latest_sample(client, "${series}"))

# 3. Every other verb is one MCP call away.
print("series:", call_mcp(client, "series.list", {}))`;
}
