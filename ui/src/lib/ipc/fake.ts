// The in-memory node stand-in used when NOT in the Tauri shell (plain browser during S2, and
// tests). It mirrors the real node's channel contract: `channel_post` persists + returns the
// stored item; `channel_history` returns the channel's items oldest→newest. This is a
// temporary seam — at S3 the browser talks to a real node over SSE/HTTP and this is dropped.
//
// It is deliberately faithful (ordered, idempotent on id) so the UI behaves identically here
// and against the real node — the verb names and shapes match the Rust commands one-to-one.

import type { Item } from "@/lib/channel/channel.types";
import { assetsFakeInvoke } from "./assets.fake";
import { agentFakeInvoke } from "./agent.fake";
import { workflowFakeInvoke } from "./workflow.fake";
import { registryFakeInvoke } from "./registry.fake";
import { nativeFakeInvoke } from "./native.fake";
import { sessionFakeInvoke } from "./session.fake";
import { workspaceFakeInvoke } from "./workspace.fake";
import { channelRegistryFakeInvoke, registerOnPost } from "./channelRegistry.fake";
import { membersFakeInvoke } from "./members.fake";
import { inboxFakeInvoke } from "./inbox.fake";
import { outboxFakeInvoke } from "./outbox.fake";
import { adminFakeInvoke } from "./admin.fake";
import { extFakeInvoke } from "./ext.fake";

const store = new Map<string, Item[]>(); // key: `${ws}/${channel}`

function key(ws: string, channel: string): string {
  return `${ws}/${channel}`;
}

export function fakeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  // Agent (`agent_*`), workflow (`workflow_*`), registry (`registry_*`), and asset (`assets_*`)
  // commands are handled by their own fakes; each returns null for anything it doesn't own, so the
  // channel cases below still run.
  const agent = agentFakeInvoke<T>(cmd, args);
  if (agent !== null) return agent;
  const workflow = workflowFakeInvoke<T>(cmd, args);
  if (workflow !== null) return workflow;
  const registry = registryFakeInvoke<T>(cmd, args);
  if (registry !== null) return registry;
  const native = nativeFakeInvoke<T>(cmd, args);
  if (native !== null) return native;
  const asset = assetsFakeInvoke<T>(cmd, args);
  if (asset !== null) return asset;
  // The collaboration surfaces (session, workspace dir, channel registry, members, inbox, outbox).
  // Each returns null for anything it doesn't own, so the channel cases below still run.
  const session = sessionFakeInvoke<T>(cmd, args);
  if (session !== null) return Promise.resolve(session);
  const workspace = workspaceFakeInvoke<T>(cmd, args);
  if (workspace !== null) return Promise.resolve(workspace);
  const channelReg = channelRegistryFakeInvoke<T>(cmd, args);
  if (channelReg !== null) return Promise.resolve(channelReg);
  const members = membersFakeInvoke<T>(cmd, args);
  if (members !== null) return Promise.resolve(members);
  const admin = adminFakeInvoke<T>(cmd, args);
  if (admin !== null) return Promise.resolve(admin);
  const ext = extFakeInvoke<T>(cmd, args);
  if (ext !== null) return Promise.resolve(ext);
  const inbox = inboxFakeInvoke<T>(cmd, args);
  if (inbox !== null) return Promise.resolve(inbox);
  const outbox = outboxFakeInvoke<T>(cmd);
  if (outbox !== null) return Promise.resolve(outbox);
  switch (cmd) {
    case "channel_post": {
      const { ws, channel, item } = args as {
        ws: string;
        channel: string;
        item: Item;
      };
      const list = store.get(key(ws, channel)) ?? [];
      const stored: Item = { ...item, channel };
      const existing = list.findIndex((m) => m.id === stored.id);
      if (existing >= 0) list[existing] = stored;
      else list.push(stored);
      list.sort((a, b) => a.ts - b.ts);
      store.set(key(ws, channel), list);
      // Create-on-post: a posted channel becomes listable (mirrors the host's registry upsert).
      registerOnPost(ws, channel, stored.author);
      return Promise.resolve(stored as T);
    }
    case "channel_history": {
      const { ws, channel } = args as { ws: string; channel: string };
      const list = store.get(key(ws, channel)) ?? [];
      return Promise.resolve([...list] as T);
    }
    default:
      return Promise.reject(new Error(`unknown command: ${cmd}`));
  }
}

/** Test helper: clear the fake store between tests (the fake is module-global). */
export function __resetFake(): void {
  store.clear();
}
