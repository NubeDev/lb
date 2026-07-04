# channel-chat — pure-app extension: channels + in-channel AI agent

**Status: scaffold only.** Scope: `docs/scope/app/app-extensions-scope.md` (example 2).

A **pure-app extension** — no backend component, no `[runtime]`, no tools of its own.
The whole extension is a phone surface over granted platform verbs, proving that "only
a mobile UI" is a legitimate extension shape:

- Channel list (`channel.list`) → durable history (`channel.history`) → live SSE
  stream → post (`channel.post`).
- **Ask the agent**: posts an item with `kind:"agent"` and renders the durable agent
  run's RunEvent stream live, then the final answer as a normal channel item — the
  same path as the web composer (`docs/scope/channels/channels-agent-scope.md`).

The manifest lives here (`extension.toml`) because this folder *is* the entire
extension. It requests only existing capabilities (`mcp:channel.*:call`,
`bus:chan/*` pub/sub, `mcp:agent.invoke:call`); nothing is pre-approved —
`granted = requested ∩ admin_approved`, same as any third-party extension.

Planned layout (per `docs/FILE-LAYOUT.md`):

```
extension.toml
src/
  remote.ts            ← MF container entry: exposes { Page } (AppRemote)
  Page.tsx             ← channel list ⇄ conversation navigation
  Conversation.tsx     ← history + live stream + composer
  AgentRun.tsx         ← RunEvent stream renderer for kind:"agent" items
  useChannel.ts        ← history + stream hook (bridge call + watch)
  usePost.ts           ← post / ask-agent hook
build.sh
```

The mandatory tests ride this extension: capability-deny (post without
`mcp:channel.post:call`) and workspace-isolation (two tokens, disjoint channels) —
against the real spawned gateway, per rule 9.
