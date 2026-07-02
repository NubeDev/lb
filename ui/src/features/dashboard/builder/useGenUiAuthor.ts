// The "AI widget" authoring hook (genui-scope "Authoring path"). Choreography:
//   prompt → `agent.invoke` (caller's principal, `caller ∩ agent`) under `skill:core.genui-widget`
//        → open the RunEvent stream (`openRunStream`) → feed each `text-delta` into the Lang stream
//          (`createLangStream`) so the preview IR fills in progressively
//        → the durable `AgentResult.answer` is the full emission (the source of truth when the SSE
//          stream is unavailable — jsdom/Tauri have no `EventSource`, so `openRunStream` returns null)
//        → `accept()` runs parse → normalize → validate → size-check ONCE, loudly (`acceptLang`),
//          returning the typed IR to persist (never raw Lang) + the warnings to show.
//
// The stream is a PREVIEW convenience; correctness rides the durable answer, so the flow works headless.

import { useCallback, useRef, useState } from "react";
import { createLangStream, acceptLang, type AcceptResult } from "@nube/genui/authoring";
import { nubeCatalog, type IrSpec } from "@nube/genui";

import { invokeAgent } from "@/lib/agent/agent.api";
import { openRunStream } from "@/lib/channel/run.stream";

/** The core skill the agent activates to author a genui widget. */
export const GENUI_SKILL = "core.genui-widget";

export interface GenUiAuthorState {
  running: boolean;
  /** The live-preview IR while streaming (or the final parse). Null before the first emission. */
  preview: IrSpec | null;
  /** The raw accumulated emission (kept as `meta.raw` for refine turns). */
  raw: string;
  error: string | null;
}

/** Mint a job id without pulling a uuid dep — a timestamp-free random-ish token is fine (the host owns
 *  the real durable id; this only correlates the stream). Uses crypto when present. */
function mintJobId(): string {
  const c = (globalThis as { crypto?: Crypto }).crypto;
  if (c?.randomUUID) return `genui-${c.randomUUID()}`;
  let s = "genui-";
  for (let i = 0; i < 16; i++) s += Math.floor(Math.random() * 16).toString(16);
  return s;
}

export function useGenUiAuthor(ws: string, surfaceId = "cell") {
  const [state, setState] = useState<GenUiAuthorState>({
    running: false,
    preview: null,
    raw: "",
    error: null,
  });
  const rawRef = useRef("");

  /** Run one authoring turn for `prompt`. Streams a live preview; resolves when the durable answer lands. */
  const run = useCallback(
    async (prompt: string, author?: string, caps?: string[]) => {
      const jobId = mintJobId();
      const stream = createLangStream(nubeCatalog, surfaceId);
      rawRef.current = "";
      setState({ running: true, preview: null, raw: "", error: null });

      // Live preview: feed each text-delta into the Lang stream (best-effort; null in jsdom/Tauri).
      const rs = openRunStream(jobId, (event) => {
        if (event.type === "text-delta") {
          rawRef.current += event.text;
          const ir = stream.push(event.text);
          setState((s) => ({ ...s, preview: ir, raw: rawRef.current }));
        }
      });

      try {
        const result = await invokeAgent(ws, jobId, prompt, { skill: GENUI_SKILL, author, caps });
        // The durable answer is the authoritative full emission — re-parse it whole so the preview is
        // correct even when the SSE stream never ran (headless) or lagged.
        const full = result.answer ?? rawRef.current;
        rawRef.current = full;
        const finalStream = createLangStream(nubeCatalog, surfaceId);
        const ir = finalStream.set(full);
        setState({ running: false, preview: ir, raw: full, error: null });
        return { ir, raw: full };
      } catch (e) {
        const error = e instanceof Error ? e.message : String(e);
        setState((s) => ({ ...s, running: false, error }));
        return null;
      } finally {
        rs?.close();
      }
    },
    [ws, surfaceId],
  );

  /** Run the loud accept step on the current raw emission. Returns the typed IR + warnings, or a loud
   *  rejection with the stated message — the SAME check the host re-runs on save. */
  const accept = useCallback((): AcceptResult => {
    return acceptLang(rawRef.current, { catalog: nubeCatalog, surfaceId });
  }, [surfaceId]);

  return { state, run, accept };
}
