# Full desktop binary crashes at window init with a snap `libpthread` symbol error

**Symptom:** the `full`-feature desktop binary boots cleanly — the logs show
`full: seeded 38 core skills` and `full: loopback gateway on http://127.0.0.1:8800` — then
dies within milliseconds with:

```
symbol lookup error: /snap/core20/current/lib/x86_64-linux-gnu/libpthread.so.0:
  undefined symbol: __libc_pthread_init, version GLIBC_PRIVATE
```

**Root cause:** NOT a bug in the shell or the gateway. The Rust boot (`boot_full`) completes;
the crash is during **GTK/webkit2gtk window init**, when one of webkit's transitive shared
libraries pulls in `/snap/core20/current/lib/x86_64-linux-gnu/libpthread.so.0`. snap's
`core20` base ships Ubuntu 20.04's glibc (2.31), whose `libpthread.so.0` is the OLD separate
threading lib. Loading it into a process built against the host's modern glibc (≥ 2.34, where
`libpthread` is a stub merged into `libc`) trips the `GLIBC_PRIVATE` symbol version mismatch.

This only reproduces on a **dev box with snap runtime pollution** (snap-installed apps that
leave the `core20` runtime visible to the dynamic loader via a transitive library's
`DT_NEEDED`/dlopen). It does NOT reproduce in the `desktop/docker/Dockerfile`'s Ubuntu 22.04
container, which has no snap — `make -C desktop smoke-full` is the canonical proof and runs
clean there. `LD_LIBRARY_PATH` was empty and the binary's own `RUNPATH` is clean; the leak is
from a webkit dependency, not the shell.

**Fix:** none needed in code. The canonical build + smoke run in the container (no snap).
On a snap-polluted host, either (a) run the container smoke (`make -C desktop smoke-full`),
or (b) remove the snap runtime that's leaking (`snap remove core20` if unused), or (c) prove
the standalone backend another way — the non-windowed integration test
(`ui/src-tauri/tests/full_loopback_test.rs`) exercises login + `POST /mcp/call` over the
loopback gateway with NO display, so it is unaffected by the webkit/snap collision entirely.

**Lesson:** a windowed-binary smoke that depends on the host's GTK/webkit stack inherits the
host's library-graph pathologies. A non-windowed integration test over the SAME in-process
gateway (`boot_full` + reqwest) is the portable proof — keep both, and reach for the
container smoke when the host smoke is blocked by environment quirks unrelated to the change.
