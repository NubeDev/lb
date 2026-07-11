# native serve-test hangs on EOF — `drop(host_w)` alone doesn't close the duplex

Area: extensions (SDK — `lb-ext-native`) · Status: **fixed** (2026-07-11)

## Symptom

Running `lb-ext-native`'s `serve` unit tests, `serve::tests::health_replies_ok` (and any other
non-`shutdown` case) **hangs forever** when run in isolation — the test binary never exits, `cargo
test` times out. It only *appeared* green in a full run because `shutdown_replies_then_ends_the_loop`
happened to end the process fast enough afterward, masking the hang. Reproduced identically on the
clean `sdk-v0.2.1` tag, so it predates this session's work.

```
$ ./deps/lb_ext_native-… serve::tests::health_replies_ok --exact
# …never returns…
```

## Root cause

The `round_trip` test harness in `crates/lb-ext-native/src/serve.rs`:

```rust
let (host, child) = duplex(64 * 1024);
let (child_r, child_w) = tokio::io::split(child);
let server = tokio::spawn(async move { serve(child_r, child_w, Echo).await });
let (mut host_r, mut host_w) = tokio::io::split(host);
// …write requests, read replies…
drop(host_w); // EOF → server returns   ← WRONG: not enough
server.await.unwrap().unwrap();
```

`serve` returns on `shutdown` **or on EOF** (the host closing the stream). A `tokio::io::duplex`
stays open until **both** split halves of a side are dropped. Dropping only `host_w` leaves `host_r`
alive, holding the pipe open — so the child's `read_frame` never observes EOF and `serve` never
returns, so `server.await` blocks forever. The `full_lifecycle`/`shutdown` tests dodged it because
they end with an explicit `Method::Shutdown` (serve returns without needing EOF); `health_replies_ok`
relies on the EOF path and so hangs.

## Fix

Drop **both** host halves before awaiting the server, so the write side genuinely closes and the
child sees EOF:

```rust
drop(host_w);
drop(host_r);
server.await.unwrap().unwrap();
```

(`host_r` is only used inside the request loop above; dropping it after the loop is safe.)

## Regression coverage

`serve::tests::health_replies_ok` (and the other non-shutdown cases) now **pass in isolation** — the
hang is the failure, so a single-test run that returns green *is* the regression check. Verified:

```
$ cargo test -p lb-ext-native --lib serve::tests::health_replies_ok
test result: ok. 1 passed; 0 failed
$ cargo test -p lb-ext-native --lib
test result: ok. 17 passed; 0 failed   # whole suite, un-masked
```

## Lesson

A `tokio::io::duplex` (and any split stream) signals EOF only when **every** handle to the writing
side is dropped — closing "the write half" means dropping the read half too, or reuniting and
dropping the whole thing. A test that only passes as part of a larger run (because a later test exits
the process) is hiding a hang, not proving correctness — run the suspect case alone.

Surfaced while publishing the native host-callback client through the SDK
([`sessions/extensions/native-callback-sdk-export-session.md`](../../sessions/extensions/native-callback-sdk-export-session.md)):
adding the `lb-sidecar-client` dependency changed how fast the test process torn down, exposing the
latent hang.
