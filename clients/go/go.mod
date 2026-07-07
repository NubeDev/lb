// Module lbclient — a thin external client for a Lazybones gateway node.
//
// The five-method shape (mirrored across the four language clients under
// clients/): construct a Client with a base URL + a bearer, then call
// WriteSamples / LatestSample / CallMCP / SignWebhook / PostWebhook. The bearer
// is EITHER an API key (lbk_{ws}.{id}.{secret}) OR a JWT from /login; this
// library does not branch on which — the gateway already splits on the lbk_
// prefix in one place (rust/role/gateway/src/session/authenticate.rs).
//
// See README.md for the auth + round-trip walkthrough.
module github.com/lazybones/lb/clients/go

go 1.22
