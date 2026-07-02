// CodeMirror extensions backed by the in-browser TypeScript service (tsEnv):
// autocomplete, hover tooltips, and inline diagnostics. The engine `dts` arrives
// async, so handlers read it from a ref each time; until it's present they fall
// back to the lightweight symbol-based completion source.

import { autocompletion, type Completion, type CompletionContext, type CompletionResult, type CompletionSource } from "@codemirror/autocomplete";
import { hoverTooltip } from "@codemirror/view";
import { linter, type Diagnostic } from "@codemirror/lint";
import type { Extension } from "@codemirror/state";
import { getCompletions, getCompletionDetail, getQuickInfo, getDiagnostics, cmCompletionType, isReady } from "./tsEnv";
import { jsCompletionSource, type JsSymbol } from "./jsApi";

type Ref<T> = { current: T };

function tsCompletionSource(dtsRef: Ref<string>, symbolsRef: Ref<JsSymbol[]>): CompletionSource {
  const fallback = jsCompletionSource(symbolsRef);
  return (context: CompletionContext): CompletionResult | null => {
    const dts = dtsRef.current;
    if (!dts) return fallback(context);
    const code = context.state.doc.toString();
    const entries = getCompletions(dts, code, context.pos); // also kicks the lazy TS load
    if (!isReady()) return fallback(context); // TS chunk still loading → symbol source
    if (!entries.length) return null;
    const word = context.matchBefore(/[\w$]*$/);
    if (!context.explicit && word && word.from === word.to && !isAfterDot(context)) return null;
    const from = word ? word.from : context.pos;
    const options: Completion[] = entries.map((e) => ({
      label: e.name,
      type: cmCompletionType(e.kind),
      // Defer detail/doc to the highlighted item (one TS call each, not all).
      info: () => {
        const d = getCompletionDetail(dts, code, context.pos, e.name);
        if (!d) return null;
        const dom = document.createElement("div");
        dom.style.whiteSpace = "pre-wrap";
        const sig = document.createElement("div");
        sig.style.fontFamily = "monospace";
        sig.textContent = d.detail;
        dom.appendChild(sig);
        if (d.doc) {
          const doc = document.createElement("div");
          doc.style.marginTop = "4px";
          doc.style.opacity = "0.85";
          doc.textContent = d.doc;
          dom.appendChild(doc);
        }
        return dom;
      },
    }));
    return { from, options, validFor: /^[\w$]*$/ };
  };
}

function isAfterDot(context: CompletionContext): boolean {
  return !!context.matchBefore(/\.\s*[\w$]*$/);
}

function tsHover(dtsRef: Ref<string>): Extension {
  return hoverTooltip((view, pos) => {
    const dts = dtsRef.current;
    if (!dts) return null;
    const qi = getQuickInfo(dts, view.state.doc.toString(), pos);
    if (!qi || !qi.detail) return null;
    return {
      pos: qi.from,
      end: qi.to,
      create() {
        const dom = document.createElement("div");
        dom.style.padding = "4px 8px";
        dom.style.maxWidth = "480px";
        const sig = document.createElement("div");
        sig.style.fontFamily = "monospace";
        sig.style.whiteSpace = "pre-wrap";
        sig.textContent = qi.detail;
        dom.appendChild(sig);
        if (qi.doc) {
          const doc = document.createElement("div");
          doc.style.marginTop = "4px";
          doc.style.opacity = "0.85";
          doc.style.whiteSpace = "pre-wrap";
          doc.textContent = qi.doc;
          dom.appendChild(doc);
        }
        return { dom };
      },
    };
  });
}

function tsLinter(dtsRef: Ref<string>): Extension {
  return linter((view) => {
    const dts = dtsRef.current;
    if (!dts) return [];
    const len = view.state.doc.length;
    return getDiagnostics(dts, view.state.doc.toString())
      .map((d): Diagnostic => ({
        from: Math.max(0, Math.min(d.from, len)),
        to: Math.max(0, Math.min(d.to, len)),
        severity: d.severity,
        message: d.message,
      }))
      .filter((d) => d.to >= d.from);
  }, { delay: 400 });
}

/** Full TS-powered extension set for the script editor (autocomplete + hover +
 *  lint). `dtsRef` supplies the engine declarations once loaded; `symbolsRef`
 *  feeds the fallback completion source meanwhile. */
export function tsEditorExtensions(dtsRef: Ref<string>, symbolsRef: Ref<JsSymbol[]>): Extension[] {
  return [
    autocompletion({ override: [tsCompletionSource(dtsRef, symbolsRef)] }),
    tsHover(dtsRef),
    tsLinter(dtsRef),
  ];
}
