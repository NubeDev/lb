// Unit tests for the JSON payload builder's pure layer (widget-config-vars Slice 5): the target list
// (the platform sinks + installed extension write tools) and the template interpolation (the shared lib
// runs over the parsed JSON tree before send). No gateway — the send round-trip is the e2e gateway test.

import { describe, expect, it } from "vitest";

import { payloadTargets } from "./JsonPayloadField";
import { interpolateArgs, resolveBuiltins } from "@/lib/vars";
import type { ExtRow } from "@/lib/ext/ext.api";

const todoExt: ExtRow = {
  ext: "todo",
  version: "0.1.0",
  tier: "wasm",
  enabled: true,
  running: true,
  health: "ok",
  restart_count: 0,
  ui: { entry: "remoteEntry.js", label: "Todo", icon: "x", scope: ["todo.create", "todo.list"] },
  widgets: [],
};

describe("payloadTargets", () => {
  it("always offers the platform sinks bus.publish + ingest.write", () => {
    const tools = payloadTargets([]).map((t) => t.tool);
    expect(tools).toContain("bus.publish");
    expect(tools).toContain("ingest.write");
  });

  it("adds an installed extension's WRITE tools (todo.create), not its read tools (todo.list)", () => {
    const tools = payloadTargets([todoExt]).map((t) => t.tool);
    expect(tools).toContain("todo.create"); // write
    expect(tools).not.toContain("todo.list"); // read — not a payload target
  });
});

describe("payload template interpolation (the shared lib over the parsed JSON tree)", () => {
  it("the add-todo example: ${newTodo} + ${__workspace} resolve, type-preserved", () => {
    const scope = {
      values: { newTodo: "buy milk" },
      builtins: resolveBuiltins({ workspace: "acme" }),
    };
    const template = JSON.parse('{"text":"${newTodo}","ws":"${__workspace}"}');
    expect(interpolateArgs(template, scope)).toEqual({ text: "buy milk", ws: "acme" });
  });

  it("an unknown slot is left literal (a shared template never throws)", () => {
    const out = interpolateArgs(JSON.parse('{"x":"${nope}"}'), { values: {}, builtins: {} });
    expect(out).toEqual({ x: "${nope}" });
  });
});
