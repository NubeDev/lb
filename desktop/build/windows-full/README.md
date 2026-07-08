# build/windows-full/

The **full standalone** Windows build type — `lazybones-shell.exe` with the `full` cargo
feature on. The Windows peer of `../linux/full/`: one `.exe` that boots the node, runs the
boot seeders, and mounts the SSE/HTTP gateway in-process on `127.0.0.1:8800`, so the app
works 100% standalone (login, MCP, SSE, agents, flows, insights) — no external node.

`make -C desktop windows-full` cross-builds the `.exe` via the container and copies it here:

```
desktop/build/windows-full/lazybones-shell.exe      ← the PE (full feature on)
```

Gitignored — regenerated per build. Only this README is tracked. The thin Windows shell
lands in `../windows/` via `make windows-executable`.

## Run it

WebView2 is OS-provided on Win10/11, so the `.exe` is genuinely standalone — double-click it.
On boot, the console window (or the terminal that launched it) prints
`full: loopback gateway on http://127.0.0.1:8800 (login as user:ada / acme)`. The window opens
against `acme`; the login screen accepts `user:ada` / `acme`.

## What's different from `../windows/`

One cargo feature (`full`), same source. See [`../linux/full/README.md`](../linux/full/README.md)
for the full design — it is identical on both OSes (symmetric nodes, §3.1). The only OS delta
is the runtime webview: Linux links `webkit2gtk-4.1`; Windows uses the OS WebView2.

## Related

- Thin Windows shell: [`../windows/README.md`](../windows/README.md) (when it lands).
- Linux full build: [`../linux/full/README.md`](../linux/full/README.md).
- Scope: [`../../../../docs/scope/desktop/desktop-standalone-backend-scope.md`](../../../../docs/scope/desktop/desktop-standalone-backend-scope.md).
