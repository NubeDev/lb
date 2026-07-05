/**
 * `@lazybones/client-node` — a thin external client for a Lazybones gateway
 * node. The five-method shape (mirrored across the four language clients under
 * `clients/`): construct a `Client` with a base URL + a bearer, then call
 * `writeSamples` / `latestSample` / `callMcp` / `signWebhook` / `postWebhook`.
 *
 * The bearer is EITHER an API key (`lbk_{ws}.{id}.{secret}`) OR a JWT from
 * `/login`; this library does not branch on which — the gateway already splits
 * on the `lbk_` prefix in one place (`session/authenticate.rs`).
 *
 * See `README.md` for the auth + round-trip walkthrough.
 */

export { Client, ApiError } from "./client.js";
export type { LoginReply } from "./client.js";
export { writeSamples, latestSample } from "./ingest.js";
export type { Sample, WriteSamplesReply, LatestSampleReply } from "./ingest.js";
export { callMcp } from "./mcp.js";
export { signWebhook, postWebhook } from "./webhook.js";
export type { WebhookAccepted } from "./webhook.js";
