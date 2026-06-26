// The in-memory asset stand-in used when NOT in the Tauri shell (plain browser, tests). It
// mirrors the node's asset contract faithfully enough for the UI to behave identically here and
// against the real node (the verb names + shapes match the Rust commands one-to-one).
//
// Faithful to the membership gate: a `get_doc` resolves owner / shared-team-member /
// linked-channel just like `host::get_doc`, so the UI's denied path is exercised in tests. The
// principal is taken from `args.author` (the demo session identity — the real node derives it
// from the token; the gateway open question in STATUS.md).
//
// One file per concern (FILE-LAYOUT): the asset fake lives beside the channel fake, not inside
// it.

import type { Doc, Skill } from "@/lib/assets/assets.types";

interface StoredDoc extends Doc {
  owner: string;
  content: string;
}

// Workspace-scoped maps (key prefix = ws) — the wall, mirrored.
const docs = new Map<string, StoredDoc>(); // `${ws}/${id}`
const shares = new Map<string, Set<string>>(); // `${ws}/${id}` -> teams
const links = new Map<string, Set<string>>(); // `${ws}/${id}` -> channels
const members = new Map<string, Set<string>>(); // `${ws}/${team}` -> users
const skills = new Map<string, Skill>(); // `${ws}/${id}@${version}`
const grants = new Set<string>(); // `${ws}/${id}`
const subCaps = new Map<string, Set<string>>(); // `${ws}/${user}` -> channels they may sub

const k = (ws: string, x: string) => `${ws}/${x}`;

/** Test seam: declare which channels a user may `sub` (the channel-link read path) and team
 *  membership, so a Vitest can set up the gate without a real token. */
export function __seedMembership(
  ws: string,
  opts: { members?: Record<string, string[]>; subs?: Record<string, string[]> },
): void {
  for (const [team, users] of Object.entries(opts.members ?? {})) {
    members.set(k(ws, team), new Set(users));
  }
  for (const [user, channels] of Object.entries(opts.subs ?? {})) {
    subCaps.set(k(ws, user), new Set(channels));
  }
}

function canRead(ws: string, doc: StoredDoc, who: string): boolean {
  if (doc.owner === who) return true;
  const teams = shares.get(k(ws, doc.id)) ?? new Set();
  for (const t of teams) if ((members.get(k(ws, t)) ?? new Set()).has(who)) return true;
  const channels = links.get(k(ws, doc.id)) ?? new Set();
  const mySubs = subCaps.get(k(ws, who)) ?? new Set();
  for (const c of channels) if (mySubs.has(c)) return true;
  return false;
}

export function assetsFakeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> | null {
  switch (cmd) {
    case "assets_put_doc": {
      const { ws, id, title, content, author } = args as Record<string, string>;
      const doc: StoredDoc = { id, title, content, owner: author ?? "user:demo" };
      docs.set(k(ws, id), doc);
      return Promise.resolve({ id } as T);
    }
    case "assets_get_doc": {
      const { ws, id, author } = args as Record<string, string>;
      const doc = docs.get(k(ws, id));
      if (!doc) return Promise.reject(new Error("not found"));
      if (!canRead(ws, doc, author ?? "user:demo")) return Promise.reject(new Error("denied"));
      return Promise.resolve({ id: doc.id, title: doc.title, content: doc.content, owner: doc.owner } as T);
    }
    case "assets_list_docs": {
      const { ws, author } = args as Record<string, string>;
      const mine = [...docs.values()].filter(
        (d) => d.owner === (author ?? "user:demo") && docs.get(k(ws, d.id)) === d,
      );
      return Promise.resolve(mine.map((d) => ({ id: d.id, title: d.title })) as T);
    }
    case "assets_share_doc": {
      const { ws, id, team } = args as Record<string, string>;
      (shares.get(k(ws, id)) ?? shares.set(k(ws, id), new Set()).get(k(ws, id))!).add(team);
      return Promise.resolve(undefined as T);
    }
    case "assets_link_doc": {
      const { ws, id, channel } = args as Record<string, string>;
      (links.get(k(ws, id)) ?? links.set(k(ws, id), new Set()).get(k(ws, id))!).add(channel);
      return Promise.resolve(undefined as T);
    }
    case "assets_put_skill": {
      const { ws, id, version, body } = args as Record<string, string>;
      skills.set(`${ws}/${id}@${version}`, { id, version, body });
      return Promise.resolve({ id, version } as T);
    }
    case "assets_grant_skill": {
      const { ws, id } = args as Record<string, string>;
      grants.add(k(ws, id));
      return Promise.resolve(undefined as T);
    }
    case "assets_load_skill": {
      const { ws, id, version } = args as Record<string, string>;
      if (!grants.has(k(ws, id))) return Promise.reject(new Error("denied"));
      const all = [...skills.values()].filter((s) => s.id === id);
      const s = version ? all.find((x) => x.version === version) : all[all.length - 1];
      if (!s) return Promise.reject(new Error("not found"));
      return Promise.resolve(s as T);
    }
    default:
      return null; // not an asset command — let the caller try the channel fake
  }
}

/** Test helper: clear all asset fake state. */
export function __resetAssetsFake(): void {
  docs.clear();
  shares.clear();
  links.clear();
  members.clear();
  skills.clear();
  grants.clear();
  subCaps.clear();
}
