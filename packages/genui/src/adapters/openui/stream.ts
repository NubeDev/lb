// `createLangStream` — the STREAMING OpenUI-Lang → IR adapter (genui-scope "Streaming: re-parse per
// text-delta, forward-refs resolve as lines land"). Wraps `@openuidev/lang-core`'s `createStreamingParser`
// (its own incremental model) and lowers the latest `ElementNode` tree to IR on each push, so the
// authoring preview renders SOMETHING stable through every mid-stream state (partial nodes included —
// `elementToIr` is tolerant of a null/partial root). Authoring-stratum only; never on the render path.

import { createStreamingParser } from "@openuidev/lang-core";
import type { Catalog } from "../../catalog/defineCatalog";
import { buildLangLibrary, langRootName } from "../../catalog/library";
import type { IrSpec } from "../../ir/types";
import { elementToIr } from "./toIr";

export interface LangStream {
  /** Feed the next stream chunk (a `text-delta`), get the latest IR. */
  push: (chunk: string) => IrSpec;
  /** Set the full accumulated text (diffs internally). Use when the caller holds the whole buffer. */
  set: (fullText: string) => IrSpec;
  /** Latest IR without consuming new input. */
  current: () => IrSpec;
}

export function createLangStream(catalog: Catalog, surfaceId = "cell"): LangStream {
  const parser = createStreamingParser(buildLangLibrary(catalog), langRootName());
  const toIr = () => elementToIr(parser.getResult().root, surfaceId);
  return {
    push: (chunk: string) => {
      parser.push(chunk);
      return toIr();
    },
    set: (fullText: string) => {
      parser.set(fullText);
      return toIr();
    },
    current: toIr,
  };
}
