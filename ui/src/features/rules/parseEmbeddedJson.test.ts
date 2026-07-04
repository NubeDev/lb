// Unit tests for the embedded-JSON deep-parse (rules-editor-ux). The load-bearing property: it turns a
// JSON-string field (a channel row's `body`) into a real nested object AND leaves faithful scalars
// untouched — a `"42"`/`"true"`/plain-message string must survive verbatim (over-parsing would silently
// change the data the author sees). Pure function — no gateway.

import { describe, expect, it } from "vitest";

import { parseEmbeddedJson } from "./parseEmbeddedJson";

describe("parseEmbeddedJson", () => {
  it("parses a channel row's JSON-string body into a real nested object", () => {
    const row = {
      author: "system:agent-worker",
      body: '{"kind":"agent_result","goal":"What is 2+2?","answer":"2+2 equals 4."}',
      channel: "abc",
    };
    const out = parseEmbeddedJson(row) as Record<string, unknown>;
    expect(out.body).toEqual({
      kind: "agent_result",
      goal: "What is 2+2?",
      answer: "2+2 equals 4.",
    });
    // Sibling scalar fields are untouched.
    expect(out.author).toBe("system:agent-worker");
    expect(out.channel).toBe("abc");
  });

  it("recurses through arrays and into nested parsed payloads", () => {
    const result = {
      output: {
        kind: "scalar",
        value: [{ body: '{"inner":"[1,2,3]"}' }],
      },
    };
    const out = parseEmbeddedJson(result) as any;
    // The array element's body parsed, and the string-array INSIDE it parsed too (payload-in-payload).
    expect(out.output.value[0].body).toEqual({ inner: [1, 2, 3] });
  });

  it("does NOT re-type faithful scalar strings", () => {
    // A bare number/bool/quoted-word string is faithful data — parsing it would change its meaning.
    expect(parseEmbeddedJson({ a: "42", b: "true", c: "hello" })).toEqual({
      a: "42",
      b: "true",
      c: "hello",
    });
  });

  it("leaves a plain message that merely starts with a brace as a string", () => {
    // Looks container-ish but is not valid JSON — stays a faithful string, never throws.
    const msg = "{not really json";
    expect(parseEmbeddedJson({ body: msg })).toEqual({ body: msg });
  });

  it("passes through primitives, null, and already-structured values", () => {
    expect(parseEmbeddedJson(null)).toBeNull();
    expect(parseEmbeddedJson(7)).toBe(7);
    expect(parseEmbeddedJson(true)).toBe(true);
    expect(parseEmbeddedJson({ n: 1, arr: [true, null] })).toEqual({ n: 1, arr: [true, null] });
  });
});
