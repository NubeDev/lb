# weather — "weather fetch failed: error sending request for url (…open-meteo…)"

- Date: 2026-07-09
- Area: weather (host-native `weather.current` tool)
- Status: **resolved** — transient outbound-connectivity blip; no code change.

## Symptom

`POST /mcp/call {tool:"weather.current"}` on the live dev node returned:

```
extension error: weather fetch failed: error sending request for url (https://api.open-meteo.com/v1/forecast?...)
```

…while **every** `cargo test` passed, `cargo build`/`fmt` were clean, and a standalone `reqwest`
probe against the same host returned `200 OK`. The `{e}` Display truncates the source chain, so the
raw message was uninformative (it covers DNS + connect + TLS alike).

## Investigation (each hypothesis killed with evidence)

1. **Harness sandbox blocked egress** (the leading prior hypothesis) — *wrong*. The node's parent
   chain is `make dev` ← VSCode integrated terminal (pid 2229800), **not** a Claude background task.
   No seccomp/netns wrapper: `readlink /proc/<node>/ns/net` == the shell's netns.
2. **Env / DNS / TLS roots** — *not the cause*. A fresh process launched with the node's **exact**
   `/proc/<pid>/environ` (via `execve`) reached Open-Meteo and printed `OK status=200 OK`. The env
   carries `SSL_CERT_FILE=/usr/lib/ssl/cert.pem` (→ `/etc/ssl/certs/ca-certificates.crt`, resolves
   fine) and `SSL_CERT_DIR=/usr/lib/ssl/certs`.
3. **Stale binary** — *wrong*. `/proc/<pid>/exe` → `target/debug/node` (not `(deleted)`), built
   17:00, started 17:01; weather source last modified 16:14 → the code is compiled in.
4. **Re-run** — the identical live `mcp/call` **now succeeds**, returning real data:
   `{"location":"-27.47,153.02","temp_c":14.3,"wind_kph":5.3,"code":1,"observed_ts":"2026-07-09T10:00"}`.

## Root cause

A **transient network blip** (DNS or connect) against `api.open-meteo.com` at the moment of the
earlier test. Not code, caps, TLS roots, or sandbox.

## Note for the future

- The reqwest TLS backend in this workspace resolves to **`rustls-tls-native-roots`** (pulled in
  transitively via `polars-io` / `object_store`), **not** `rustls-tls-webpki-roots`. So the system
  trust store (`SSL_CERT_FILE`/`SSL_CERT_DIR`) *is* consulted at runtime — an empty/broken trust
  store on some host **would** break the fetch. It resolves correctly here, so it was not the cause,
  but it is the thing to check first if this recurs on a stripped-down image.
- The error message truncates the source chain. If this symptom returns, reproduce with a probe that
  walks `std::error::Error::source()` recursively (or temporarily log the chain in `current.rs`) —
  the top-level Display alone can't distinguish DNS vs connect vs TLS.

## Regression test

None warranted — a transient upstream-network condition is not reproducible in a hermetic test, and
the code path is already covered by `tests/weather_tool_test.rs` (real node + store + caps, local
HTTP stub for the sanctioned external boundary). Adding a "network flakes" test would be a fake.
