# `lbclient` (Go)

A thin external client for a Lazybones gateway node — **authenticate → connect
→ round-trip a `Sample`**, plus the webhook third-party caller path and the
universal `POST /mcp/call` bridge. Deliberately small: the shape to extend,
not an SDK. **Zero runtime dependencies** — stdlib only (`net/http`,
`crypto/hmac`, `encoding/json`).

> Scope: [`docs/scope/clients/client-libraries-scope.md`](../../docs/scope/clients/client-libraries-scope.md).
> Wire reference: [`docs/skills/ingest-series/SKILL.md`](../../docs/skills/ingest-series/SKILL.md).

## Install

```bash
go get github.com/lazybones/lb/clients/go@main
```

Or vendor it: copy `*.go` + `go.mod` into your module and adjust the module
path. Requires Go 1.22+.

## Authenticate

The bearer is **either** an API key (`lbk_{ws}.{id}.{secret}`) **or** a JWT from
`/login`. The library doesn't care which — the gateway splits on the `lbk_`
prefix in one place. **Read the key from an env var; never hard-code it.**

```go
import "github.com/lazybones/lb/clients/go"

// long-lived producer (recommended): an API key minted via the admin console
client := lbclient.New("http://127.0.0.1:8080", os.Getenv("LB_KEY"))

// or dev/admin script: log in to get a 12h session token
client := lbclient.New("http://127.0.0.1:8080", "placeholder")
c, reply, err := client.Login(ctx, "ada", "acme")
if err != nil { /* ... */ }
client = c
```

## The round-trip

```go
written, err := client.WriteSamples(ctx, []lbclient.Sample{{
    Series:  "node.cpu_temp",
    TS:      uint64(time.Now().UnixMilli()),
    Seq:     1,
    Payload: 61.4,
    Labels:  map[string]any{"host": "pi-7"},
    // Producer is host-forced to the authenticated principal; leave it empty
}})
// written.Accepted == 1, written.Committed == 1   (the gateway drains staging
// on the same call)

latest, err := client.LatestSample(ctx, "node.cpu_temp")
// latest.Sample.Payload == 61.4
```

## The universal MCP bridge

Every other platform verb — `series.list`, `series.find`, `inbox.read`,
`channel.post`, … — is one `CallMCP` away without a library update:

```go
series, err := client.CallMCP(ctx, "series.list", map[string]any{"prefix": "node."})
```

## Webhook (the third-party caller path)

A service the admin has shared a webhook secret with signs the raw body and
POSTs to `/hooks/{ws}/{id}`. **Sign the exact bytes you POST.**

```go
body := []byte(`{"event":"furnace-on"}`)
sig := lbclient.SignWebhook([]byte(sharedSecret), body)
accepted, err := client.PostWebhook(
    ctx, "acme", "wh_x",
    map[string]string{"X-Signature": sig},
    body,
)
```

## Errors

`*APIError` carries the gateway's status + body verbatim. `IsDenied()` covers
the opaque `401|403|404` statuses the gateway returns for missing-cap /
cross-workspace / unknown-record (the contract never distinguishes them):

```go
var apiErr *lbclient.APIError
if errors.As(err, &apiErr) && apiErr.IsDenied() {
    // opaque deny — missing cap, cross-workspace, or unknown record
}
```

## Run the example

```bash
make cloud                              # terminal 1: boot 127.0.0.1:8080
cd clients/go
LB_URL=http://127.0.0.1:8080 LB_USER=ada LB_WORKSPACE=acme go run ./cmd/roundtrip
# or with an API key:
LB_KEY=lbk_acme.k7f3a.ABCDEF23 go run ./cmd/roundtrip
```

## Lay of the land

One verb per file, per the project's FILE-LAYOUT rule:

```
client.go             — Client + Login() + the shared HTTP plumbing + APIError
ingest.go             — WriteSamples() + LatestSample() + the Sample struct
mcp.go                — CallMCP() (universal bridge)
webhook.go            — SignWebhook() + PostWebhook()
cmd/roundtrip/main.go — main package: login → write → read demo
go.mod
```
