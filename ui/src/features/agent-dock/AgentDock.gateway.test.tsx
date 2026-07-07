// The agent dock, driven against a REAL spawned gateway (no fake — CLAUDE §9). Proves the dock's thin
// channel-client behavior end to end: session lifecycle (create-on-post, history restore, new session),
// the durable answer reconciling as Done, the MANDATORY capability-deny and workspace-isolation, and
// the channels-surface `dock.*` exclusion. Isolation comes from a unique workspace per test (the node
// derives the workspace from the token — the hard wall §7).
//
// jsdom has no EventSource, so the LIVE run-stream deltas (Working/Answering) are not exercised here —
// that folding is proven as a unit test over real `RunEvent` shapes (dockRunState.test.ts) and the
// stream's cap/isolation at the transport in Rust. Here we drive the DURABLE path: post → drain →
// `agent_result` renders (Done), which is the dock's message of record.

import { useEffect, useState } from "react";
import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { AgentDock } from "./AgentDock";
import { PageContextProvider, type PageContextSource } from "./PageContextProvider";
import { ChannelList } from "@/features/channel/ChannelList";
import { MessageList } from "@/features/channel/MessageList";
import { history, listChannels } from "@/lib/channel/channel.api";
import type { Item } from "@/lib/channel/channel.types";
import { useRealGateway, signInReal, signInWithCaps, drainAgentRuns } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `dock-${n++}`;

/** A fixed page-context source so the dock is router-free in tests (the provider's decision-3 seam).
 *  The surface is "telemetry" deliberately — NO built-in persona claims it (persona-session #5), so
 *  the dock's persona focus resolves to "no suggestion" and sends NO `persona` arg. That keeps this
 *  suite focused on the dock's run lifecycle (the message-of-record + controls + history paths),
 *  not on persona pinning — proven separately in DockPersonaChip.gateway.test.tsx. */
const ctx: PageContextSource = {
  capture: () => ({ surface: "telemetry", path: "/telemetry", search: {} }),
};

function fixedClock() {
  let t = 0;
  return () => ++t;
}

function renderDock(ws: string, principal: string) {
  return render(
    <PageContextProvider source={ctx}>
      <AgentDock
        ws={ws}
        principal={principal}
        width={384}
        onWidth={() => {}}
        onClose={() => {}}
        now={fixedClock()}
      />
    </PageContextProvider>,
  );
}

beforeAll(() => useRealGateway());

describe("AgentDock (real gateway)", () => {
  it("first send CREATES the dock channel and the caption shows the captured context", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    renderDock(ws, "user:ada");

    // The context caption previews what the next message will carry.
    expect(await screen.findByText(/asking about:/i)).toBeInTheDocument();
    expect(screen.getByText(/telemetry/)).toBeInTheDocument();

    await user.type(screen.getByLabelText("ask the agent"), "why did throughput dip?");
    await user.click(screen.getByLabelText("send"));

    // The request lands in a NEW `dock.` channel (create-on-first-post) — its history is now non-empty.
    await waitFor(async () => {
      // The dock minted `dock.user-ada.<ulid>`; the request text is the goal.
      expect(await screen.findByText("why did throughput dip?")).toBeInTheDocument();
    });
  });

  it("drives the run to a durable answer (Done — the message of record)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    renderDock(ws, "user:ada");
    await screen.findByLabelText("ask the agent");

    await user.type(screen.getByLabelText("ask the agent"), "summarize this dashboard");
    await user.click(screen.getByLabelText("send"));
    await screen.findByText("summarize this dashboard");

    // Drive the real channel-agent reactor (the test gateway doesn't spawn the timer) → the durable
    // `agent_result` lands as the answer OF RECORD in the store. In jsdom there is no SSE to push it
    // back into the mounted dock, so we assert the DURABLE record directly (the same thing the dock
    // renders once the channel stream / a remount delivers it — proven separately by history-restore).
    await drainAgentRuns();
    const [dockCid] = await listDockChannels(ws, "user:ada");
    await waitFor(async () => {
      const items = await history(ws, dockCid);
      const answer = items.find((it) => it.id.startsWith("a:"));
      expect(answer, "the durable agent_result item landed").toBeTruthy();
      expect(answer!.body).toContain("agent_result");
    });
  });

  it("surfaces the pause + stop run controls while a run is in flight", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    renderDock(ws, "user:ada");
    await screen.findByLabelText("ask the agent");

    // Post an agent request — before the reactor drains, the run is pending/active, so the dock shows
    // the live run status with its pause + stop controls (the durable pause/resume path itself is
    // proven end to end against a real run job in the Rust route test — jsdom can't sustain a live run).
    await user.type(screen.getByLabelText("ask the agent"), "long running question");
    await user.click(screen.getByLabelText("send"));
    await screen.findByText("long running question");

    expect(await screen.findByLabelText("pause run")).toBeInTheDocument();
    expect(screen.getByLabelText("stop run")).toBeInTheDocument();
  });

  it("history restores after a remount (durable — never anywhere but SurrealDB)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const first = renderDock(ws, "user:ada");
    await screen.findByLabelText("ask the agent");
    await user.type(screen.getByLabelText("ask the agent"), "remember me");
    await user.click(screen.getByLabelText("send"));
    await screen.findByText("remember me");

    // Find the minted dock channel so the remount opens the SAME session.
    const rows = await listDockChannels(ws, "user:ada");
    expect(rows.length).toBe(1);

    first.unmount();
    // Remount reading the SAME session's durable history → the request restores from the store (it was
    // never anywhere but SurrealDB). Proves durability across a full unmount/remount.
    render(<RestoredHistory ws={ws} cid={rows[0]} author="user:ada" />);
    expect(await screen.findByText("remember me")).toBeInTheDocument();
  });

  it("new session mints a SECOND dock channel; the old stays reopenable", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    renderDock(ws, "user:ada");
    await screen.findByLabelText("ask the agent");

    await user.type(screen.getByLabelText("ask the agent"), "first session");
    await user.click(screen.getByLabelText("send"));
    await screen.findByText("first session");
    // Settle the first run so its pending state can't hold the composer busy when we switch sessions.
    await drainAgentRuns();

    // "New session" mints a fresh dock channel and makes it current; the composer is free again.
    await user.click(screen.getByLabelText("new session"));
    await waitFor(() =>
      expect(screen.getByLabelText("ask the agent")).not.toBeDisabled(),
    );
    await user.type(screen.getByLabelText("ask the agent"), "second session");
    await user.click(screen.getByLabelText("send"));
    await screen.findByText("second session", {}, { timeout: 3000 });

    // TWO distinct `dock-user-ada-*` channels now exist (the old one stays listable/reopenable).
    await waitFor(async () => {
      const rows = await listDockChannels(ws, "user:ada");
      expect(rows.length).toBe(2);
    });
  });

  it("an opened session is registered eagerly — reselectable WITHOUT ever posting", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // Just mount the dock — do NOT type or send anything.
    renderDock(ws, "user:ada");
    await screen.findByLabelText("ask the agent");

    // The freshly-opened, never-posted session is already registered (create-on-open, not
    // create-on-first-post) → it shows up in the durable channel list, so a reload/remount can reopen it.
    await waitFor(async () => {
      const rows = await listDockChannels(ws, "user:ada");
      expect(rows.length).toBe(1);
    });
  });

  it("the channels surface EXCLUDES dock.* sessions", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // Seed an ordinary channel + a dock session (via the dock's post path).
    renderDock(ws, "user:ada");
    await screen.findByLabelText("ask the agent");
    await user.type(screen.getByLabelText("ask the agent"), "dock only");
    await user.click(screen.getByLabelText("send"));
    await screen.findByText("dock only");

    // Confirm the dock session really exists (so its absence below is exclusion, not emptiness).
    const dockRows = await listDockChannels(ws, "user:ada");
    expect(dockRows.length).toBe(1);

    // The channels surface list must NOT include the dock session (the `dock-*` filter in useChannels).
    const cl = render(<ChannelList ws={ws} selected="general" onSelect={() => {}} />);
    await waitFor(() => {
      expect(within(cl.container).queryByText(/^dock-/)).toBeNull();
    });
  });

  // agent-context-basket: gather an item via its paperclip toggle, then ask — the posted `agent`
  // payload carries the ref (`context_items`) and the basket clears (consumed). Refs, not bodies: the
  // durable request stores only the id; the worker resolves + fences server-side (proven in Rust).
  it("basket refs ride the next ask's payload and the basket clears after send", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    renderDock(ws, "user:ada");
    await screen.findByLabelText("ask the agent");

    // TOOLS MODE mounts the shared channel CommandPalette against the dock session — seed a durable
    // note through its chat path (no run, so the composer never goes busy in SSE-less jsdom).
    await user.click(screen.getByLabelText("Tools mode"));
    await user.type(screen.getByLabelText("message"), "sales dipped 12% in June");
    await user.click(screen.getByLabelText("send"));
    await screen.findByText("sales dipped 12% in June");

    // Gather it: the row's paperclip toggle adds it to the basket (the chip row appears).
    await user.click((await screen.findAllByLabelText("Add to context"))[0]);
    expect(await screen.findByLabelText("context basket")).toBeInTheDocument();

    // Back to ASK mode — the next ask carries the ref.
    await user.click(screen.getByLabelText("Ask mode"));
    await user.type(screen.getByLabelText("ask the agent"), "why the dip?");
    await user.click(screen.getByLabelText("send"));
    await screen.findByText("why the dip?");

    const [dockCid] = await listDockChannels(ws, "user:ada");
    await waitFor(async () => {
      const items = await history(ws, dockCid);
      const ask = items.find((it) => it.body.includes("why the dip?"));
      expect(ask, "the second agent request landed").toBeTruthy();
      const payload = JSON.parse(ask!.body);
      expect(payload.kind).toBe("agent");
      const seeded = items.find((it) => it.body.includes("sales dipped"));
      expect(payload.context_items).toEqual([seeded!.id]);
    });

    // Consumed: the chip row is gone after the send.
    expect(screen.queryByLabelText("context basket")).toBeNull();
  });

  it("MANDATORY capability-deny: no bus:chan/*:pub → the post 403s and the dock shows an error", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    // Sub cap only (may read) but NOT pub — the create-on-post gate denies.
    await signInWithCaps("user:ada", ws, ["bus:chan/*:sub"]);
    renderDock(ws, "user:ada");
    await screen.findByLabelText("ask the agent");

    await user.type(screen.getByLabelText("ask the agent"), "denied please");
    await user.click(screen.getByLabelText("send"));

    // The post is refused — the dock surfaces a capability error (an alert), never a silent hang.
    expect(await screen.findByRole("alert")).toBeInTheDocument();
  });

  it("MANDATORY workspace-isolation: a ws-B token cannot read a ws-A dock channel's history", async () => {
    const user = userEvent.setup();
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    renderDock(wsA, "user:ada");
    await screen.findByLabelText("ask the agent");
    await user.type(screen.getByLabelText("ask the agent"), "ws-A secret");
    await user.click(screen.getByLabelText("send"));
    await screen.findByText("ws-A secret");
    const [dockCid] = await listDockChannels(wsA, "user:ada");

    // Switch to workspace B (a different token/wall) and try to read ws-A's dock channel history.
    const wsB = nextWs();
    await signInReal("user:bob", wsB);
    // The read is workspace-derived from the token → ws-B sees an EMPTY history for the ws-A id (the
    // wall: a ws-B principal can never read ws-A's rows; it does not error, it simply sees nothing).
    const leaked = await history(wsB, dockCid);
    expect(leaked.find((it) => it.body.includes("ws-A secret"))).toBeUndefined();
  });
});

/** List the user's own `dock.` channels in `ws` over the real `channel.list` read. */
async function listDockChannels(ws: string, principal: string): Promise<string[]> {
  const rows = await listChannels(ws);
  const prefix = `dock-${principal.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "")}-`;
  return rows.map((r) => r.id).filter((id) => id.startsWith(prefix));
}

/** Renders one session's durable history (for the remount-restores case) — a message list fed by a
 *  fetch-on-mount `channel.history` read of the known dock channel. */
function RestoredHistory({ ws, cid, author }: { ws: string; cid: string; author: string }) {
  const [items, setItems] = useState<Item[]>([]);
  useEffect(() => {
    void history(ws, cid).then(setItems);
  }, [ws, cid]);
  return <MessageList items={items} author={author} ws={ws} onEdit={() => {}} onDelete={() => {}} />;
}
