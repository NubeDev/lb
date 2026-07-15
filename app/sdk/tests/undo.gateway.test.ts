// The undo verbs through the app seam against a REAL spawned node (undo-exposure scope, testing §0:
// no mocks, no fake journal). These drive the TRUE capture path — a real granted `assets.put_doc`
// over `mcp_call` is auto-captured by the host at dispatch, so the stack under test is one the
// product actually produced, not one a test seeded behind the verb.
//
// Covers: the undo → redo round-trip proven by reading the doc back; the typed refusals surfaced as
// DATA (`{ok:false, reason}`) rather than thrown; the mandatory per-verb capability deny; and
// workspace isolation of the journal.

import { describe, expect, it } from "vitest";

import { InvokeError, type UndoHistory, type UndoOutcome } from "../src/index";
import { realClient, signInWithCaps } from "./harness";

/** The caps a member needs to author a doc AND undo their own step (incl. the no-escalation cap). */
const UNDO_CAPS = [
  "mcp:undo:call",
  "mcp:redo:call",
  "mcp:history.list:call",
  "mcp:history.compensations:call",
  "mcp:assets.put_doc:call",
  "mcp:assets.get_doc:call",
  "store:doc/*:write",
  "store:doc/*:read",
];

/** Save a doc through the real gateway — the host auto-captures it as a reversible step. */
async function putDoc(client: ReturnType<typeof realClient>, id: string, title: string, ts = 1) {
  return client.invoke("mcp_call", {
    tool: "assets.put_doc",
    args: { id, title, content: title, content_type: "markdown", tags: [], ts },
  });
}

async function getDocTitle(client: ReturnType<typeof realClient>, id: string) {
  const doc = await client.invoke<{ title?: string }>("mcp_call", {
    tool: "assets.get_doc",
    args: { id },
  });
  return doc?.title;
}

describe("the undo journal through the app seam", () => {
  it("undoes_and_redoes_a_real_captured_step", async () => {
    const c = realClient();
    await signInWithCaps(c, "ada", "app-undo-a", UNDO_CAPS);

    // Two real saves → the second is the newest undoable step.
    await putDoc(c, "d1", "v1");
    await putDoc(c, "d1", "v2");
    expect(await getDocTitle(c, "d1")).toBe("v2");

    // Undo → back to the captured before-image.
    const undone = await c.invoke<UndoOutcome>("undo");
    expect(undone.ok).toBe(true);
    expect(await getDocTitle(c, "d1")).toBe("v1");

    // Redo → forward again.
    const redone = await c.invoke<UndoOutcome>("redo");
    expect(redone.ok).toBe(true);
    expect(await getDocTitle(c, "d1")).toBe("v2");
  });

  it("lists_the_captured_step_in_history", async () => {
    const c = realClient();
    await signInWithCaps(c, "ada", "app-undo-b", UNDO_CAPS);
    await putDoc(c, "d1", "v1");

    const history = await c.invoke<UndoHistory>("undo_history");
    expect(history.items.length).toBeGreaterThan(0);
    // The real capture path labels the row with the tool that produced it.
    expect(history.items[0].tool).toBe("assets.put_doc");
    expect(history.items[0].undoable).toBe(true);
  });

  // A refusal is a normal outcome the shell renders — it must NOT throw.
  it("returns_an_empty_stack_as_typed_data_not_an_error", async () => {
    const c = realClient();
    await signInWithCaps(c, "ada", "app-undo-c", UNDO_CAPS);

    const out = await c.invoke<UndoOutcome>("undo");
    expect(out.ok).toBe(false);
    expect(out.ok === false && out.reason).toBe("empty");
  });

  // MANDATORY capability-deny (testing §2.1) — one per verb this slice ships.
  it("denies_each_undo_verb_without_its_grant", async () => {
    for (const missing of [
      "mcp:undo:call",
      "mcp:redo:call",
      "mcp:history.list:call",
      "mcp:history.compensations:call",
    ]) {
      const c = realClient();
      const caps = UNDO_CAPS.filter((x) => x !== missing);
      await signInWithCaps(c, "mallory", "app-undo-deny", caps);

      const cmd = {
        "mcp:undo:call": ["undo", undefined],
        "mcp:redo:call": ["redo", undefined],
        "mcp:history.list:call": ["undo_history", undefined],
        "mcp:history.compensations:call": ["undo_compensations", { seq: 1 }],
      }[missing] as [string, Record<string, unknown> | undefined];

      const denied = await c.invoke(cmd[0], cmd[1]).catch((e: unknown) => e);
      expect(denied, `${cmd[0]} must be denied without ${missing}`).toBeInstanceOf(InvokeError);
      expect((denied as InvokeError).isDenied, `${cmd[0]} without ${missing}`).toBe(true);
    }
  });

  // The no-escalation rule: undo may not reach past the caps the caller already holds.
  it("denies_undo_without_the_original_tools_cap", async () => {
    const c = realClient();
    await signInWithCaps(c, "ada", "app-undo-esc", UNDO_CAPS);
    await putDoc(c, "d1", "v1");

    // Re-sign the SAME actor+workspace without the doc-write cap: their own step is now un-undoable.
    const weak = UNDO_CAPS.filter(
      (x) => x !== "mcp:assets.put_doc:call" && x !== "store:doc/*:write",
    );
    await signInWithCaps(c, "ada", "app-undo-esc", weak);

    const denied = await c.invoke("undo").catch((e: unknown) => e);
    expect(denied).toBeInstanceOf(InvokeError);
    expect((denied as InvokeError).isDenied).toBe(true);
  });

  // MANDATORY workspace isolation (testing §2.2): the journal never crosses the hard wall.
  it("walls_the_journal_per_workspace", async () => {
    const a = realClient();
    await signInWithCaps(a, "ada", "app-undo-ws-a", UNDO_CAPS);
    await putDoc(a, "d1", "v1");

    // A different workspace — same node, same actor name — sees nothing of ws-A's stack.
    const b = realClient();
    await signInWithCaps(b, "ada", "app-undo-ws-b", UNDO_CAPS);
    const history = await b.invoke<UndoHistory>("undo_history");
    expect(history.items).toHaveLength(0);

    // And a ws-B undo finds nothing — ws-A's doc is untouched.
    const out = await b.invoke<UndoOutcome>("undo");
    expect(out.ok).toBe(false);
    expect(await getDocTitle(a, "d1")).toBe("v1");
  });
});
