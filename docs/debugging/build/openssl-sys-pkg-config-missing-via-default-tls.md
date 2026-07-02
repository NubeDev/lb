# `make dev CE=1` fails: `openssl-sys` can't find pkg-config / system OpenSSL

- Area: build / dev-environment / dependencies
- Status: resolved
- First seen: 2026-07-03
- Session: ../../sessions/build/openssl-sys-vendored-tls-session.md
- Regression test: n/a (dependency-graph/toolchain constraint, not a product bug — guarded by the `openssl/vendored` dep on `control-engine` + the `rustls-tls`/`default-features=false` pins; if it regresses the build fails immediately and loudly, its own check). Verified by `cargo build --workspace` green with no system OpenSSL/pkg-config installed.

## Symptom

```
Could not find openssl via pkg-config:
  The pkg-config command could not be found.
...
  It looks like you're compiling on Linux and also targeting Linux. Currently this
  requires the `pkg-config` utility to find OpenSSL ...
openssl-sys = 0.9.117
warning: build failed, waiting for other jobs to finish...
make: *** [Makefile:227: control-engine] Error 101
```

Hit by `make dev CE=1` (and `make dev EXTAGENT=1 ... CE=1`) at the `control-engine`
target (Makefile:227 → `extensions/control-engine/build.sh`). This box has **no
pkg-config and no libssl-dev** (and no root to `apt install` them) — it is the same
zig-only toolchain box as [no-c-compiler-linker-cc-not-found](no-c-compiler-linker-cc-not-found.md).

## Reproduce

`make dev CE=1` on this dev box (with `pkg-config`/`libssl-dev` absent). The
`federation` target already dodged this via `openssl/vendored`; `control-engine` did not.

## Investigation

`cargo tree -p control-engine -i openssl-sys` showed `openssl-sys` pulled in through
`reqwest`'s **`default-tls`** feature (→ `hyper-tls` → `native-tls` → `openssl-sys`),
from **three independent requesters**:

1. `role/gateway` redeclared `reqwest` locally as `{ default-features = false,
   features = ["json", "stream"] }` — **no TLS backend named**, so a transitive
   consumer's `default-tls` won by feature-unification. (The workspace root pins
   `reqwest` to `rustls-tls`; this second declaration had drifted.)
2. `jsonschema` (via `lb-flows`) used **default features**, whose `resolve-http` pulls
   `reqwest` with default `default-tls`. We only compile+validate local schemas
   (`validator_for`), never fetch remote `$ref`s — so that retriever was dead weight.
3. `rubix-ce` (the external CE client git dep, **not editable**) depends on `reqwest`
   with default features → `default-tls`.

Because Cargo unifies features across the one shared `reqwest v0.12.28`, **any single**
`default-tls` requester turns `openssl-sys` on for the whole graph — so fixing (1) and
(2) alone was necessary but not sufficient while (3) remained.

## Root cause

An external git dep (`rubix-ce`) forces `reqwest`'s `default-tls` (native-tls →
`openssl-sys`), which needs a system OpenSSL + pkg-config to link — neither of which
exists on this toolchain. We can't edit the external crate to switch it to rustls.

## Fix

Two parts:

1. **Stop our own crates from adding to the `default-tls` pull** (least surprise, keeps
   the graph rustls-only where we control it):
   - `role/gateway/Cargo.toml`: add `rustls-tls` to the local `reqwest` features so it
     matches the workspace pin.
   - workspace `Cargo.toml`: `jsonschema = { version = "0.30", default-features = false }`
     (drops `resolve-http` + its reqwest/native-tls chain; local validation unaffected).

2. **Force the one unavoidable `openssl-sys` (from `rubix-ce`) to build vendored** —
   compile OpenSSL from source, no system dep. Add to `extensions/control-engine/Cargo.toml`:
   ```toml
   openssl = { version = "0.10", features = ["vendored"] }
   ```
   The `vendored` feature forwards to `openssl-sys` and **unifies globally**, so the
   single shared `openssl-sys` builds from source for everyone. This mirrors
   `federation`'s existing `--features postgres` precedent (its Postgres connector has
   the identical native-tls pull). The vendored C source compiles via the zig shims
   already wired in `rust/.cargo/config.toml` (`CC`/`AR`/`RANLIB`).

## Verification

With **no** pkg-config/libssl-dev installed:

- `cargo clean -p openssl-sys && cargo build -p control-engine` → `openssl-sys`
  compiles vendored, **Finished**.
- `bash extensions/control-engine/build.sh` → debug + release + both UI bundles built.
- `cargo build --workspace` → **Finished** (control-engine, federation, node, rubix-ce).
- `cargo build -p node --features external-agent` → **Finished** (the `EXTAGENT=1` path);
  its graph has no `openssl-sys` at all (all rustls).
- `cargo test -p lb-flows config_schema` → **5 passed** (schema validation intact after
  dropping jsonschema default features).
- `make dev CE=1` gets past Makefile:227 (control-engine) into the federation/node build.

## Prevention

- Any new crate that adds `reqwest` MUST use `default-features = false` + `rustls-tls`
  (match the workspace pin). A bare `default-features = false` with no TLS feature is a
  trap: it silently inherits a transitive `default-tls`.
- When an **external** dep forces `default-tls`, don't chase it — pin `openssl/vendored`
  on the nearest workspace crate that pulls it, per the `federation`/`control-engine`
  precedent.
- Related: [no-c-compiler-linker-cc-not-found](no-c-compiler-linker-cc-not-found.md)
  (same box; the zig toolchain that lets the vendored OpenSSL C source compile at all).
