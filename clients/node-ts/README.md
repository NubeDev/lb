# `@lazybones/client-node` (TypeScript / Node.js)

A thin external client for a Lazybones gateway node — **authenticate → connect
→ round-trip a `Sample`**, plus the webhook third-party caller path and the
universal `POST /mcp/call` bridge. Deliberately small: the shape to extend,
not an SDK.

> Scope: [`docs/scope/clients/client-libraries-scope.md`](../../docs/scope/clients/client-libraries-scope.md).
> Wire reference: [`docs/skills/ingest-series/SKILL.md`](../../docs/skills/ingest-series/SKILL.md).
> This package is **not** a member of the root `pnpm-workspace.yaml` — install
> it standalone.

## Install

```bash
cd clients/node-ts && pnpm install     # or npm install / yarn install
pnpm build                             # emits ./dist (tsc)
```

Or vendor it: copy `src/` into your project (no runtime deps; Node 18+ for the
global `fetch`, and `node:crypto` for HMAC).

## Authenticate

The bearer is **either** an API key (`lbk_{ws}.{id}.{secret}`) **or** a JWT from
`/login`. The library doesn't care which — the gateway splits on the `lbk_`
prefix in one place. **Read the key from an env var; never hard-code it.**

```ts
import { Client } from "@lazybones/client-node";

// long-lived producer (recommended): an API key minted via the admin console
const client = new Client("http://127.0.0.1:8080", process.env.LB_KEY!);

// or dev/admin script: log in to get a 12h session token
let client = new Client("http://127.0.0.1:8080", "placeholder");
const { client: authed, reply } = await client.login("ada", "acme");
client = authed;
```

## The round-trip

```ts
import { writeSamples, latestSample } from "@lazybones/client-node";

const written = await writeSamples(client, [
  {
    series: "node.cpu_temp",
    ts: Date.now(),
    seq: 1,
    payload: 61.4,
    labels: { host: "pi-7" },
    // producer is host-forced to the authenticated principal; omit it
  },
]);
// { accepted: 1, committed: 1 }   (the gateway drains staging on the same call)

const latest = await latestSample(client, "node.cpu_temp");
// latest.sample.payload === 61.4
```

## The universal MCP bridge

Every other platform verb — `series.list`, `series.find`, `inbox.read`,
`channel.post`, … — is one `callMcp` away without a library update:

```ts
import { callMcp } from "@lazybones/client-node";

const series = await callMcp(client, "series.list", { prefix: "node." });
```

## Webhook (the third-party caller path)

A service the admin has shared a webhook secret with signs the raw body and
POSTs to `/hooks/{ws}/{id}`. **Sign the exact bytes you POST.**

```ts
import { signWebhook, postWebhook } from "@lazybones/client-node";

const body = Buffer.from(JSON.stringify({ event: "furnace-on" }));
const sig = signWebhook(sharedSecret, body);
const accepted = await postWebhook(
  client, "acme", "wh_x",
  { "X-Signature": sig },
  body,
);
```

## Errors

`ApiError` carries the gateway's status + body verbatim. `isDenied()` covers
the opaque `401|403|404` statuses the gateway returns for missing-cap /
cross-workspace / unknown-record (the contract never distinguishes them):

```ts
import { ApiError, writeSamples } from "@lazybones/client-node";

try {
  await writeSamples(client, samples);
} catch (e) {
  if (e instanceof ApiError && e.isDenied()) {
    // opaque deny — missing cap, cross-workspace, or unknown record
  } else throw e;
}
```

## Run the example

```bash
make cloud                              # terminal 1: boot 127.0.0.1:8080
cd clients/node-ts && pnpm install
LB_URL=http://127.0.0.1:8080 LB_USER=ada LB_WORKSPACE=acme pnpm example
# or with an API key:
LB_KEY=lbk_acme.k7f3a.ABCDEF23 pnpm example
```

## Lay of the land

One verb per file, per the project's FILE-LAYOUT rule:

```
src/
  index.ts      — barrel re-export
  client.ts     — Client + login() + the shared fetch plumbing + ApiError
  ingest.ts     — writeSamples() + latestSample() + the Sample type
  mcp.ts        — callMcp() (universal bridge)
  webhook.ts    — signWebhook() + postWebhook()
example.ts      — login → write → read demo
```
