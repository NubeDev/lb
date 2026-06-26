# Diagrams

Text diagrams for the first architecture pass.

## 1. Edge Node

An edge node is a user/device role. It runs the same core crates as cloud, but is
configured for local-first work, offline operation, local tools, and cached
workspace data.

```text
┌──────────────────────────────────────────────────────────────┐
│ Edge device: desktop / laptop / Pi / mobile                  │
├──────────────────────────────────────────────────────────────┤
│ Tauri / local UI                                             │
│ - workspace switcher                                         │
│ - local settings                                             │
│ - extension UI surfaces                                      │
├──────────────────────────────────────────────────────────────┤
│ Host + MCP server                                            │
│ - local tools                                                │
│ - routed calls to cloud tools / central AI agent             │
│ - capability checks                                          │
├──────────────────────────────────────────────────────────────┤
│ Extension runtime                                            │
│ - WASM components                                            │
│ - optional native sidecars: filesystem, IDE, GPU, devices    │
│ - optional local AI gateway sidecar (local-only / offline)   │
├──────────────────────────────────────────────────────────────┤
│ Platform crates                                              │
│ auth · caps · tags · inbox/outbox · jobs · secrets · sync ·  │
│ ext-loader (pull · verify · cache)                           │
├──────────────────────────────────────────────────────────────┤
│ Zenoh peer                                                   │
│ - local pub/sub                                              │
│ - connects to cloud router when online                       │
├──────────────────────────────────────────────────────────────┤
│ Embedded SurrealDB                                           │
│ - node-local data                                            │
│ - cached workspace data                                      │
│ - local jobs, inbox/outbox, docs, skills, files              │
└──────────────────────────────────────────────────────────────┘
```

## 2. Cloud Hub

The cloud hub is the shared authority role. It hosts many workspaces, routes edge
traffic, serves the web UI, hosts the extension registry, and exposes shared AI.

```text
┌──────────────────────────────────────────────────────────────┐
│ Cloud hub                                                    │
├──────────────────────────────────────────────────────────────┤
│ Web entry / gateway                                          │
│ - browser UI                                                 │
│ - SSE streams                                                │
│ - HTTP commands                                              │
│ - bootstrap/admin UI                                         │
├──────────────────────────────────────────────────────────────┤
│ Shared AI gateway (swappable Tier-2 sidecar)                 │
│ - model/provider routing + workspace secrets                 │
│ - streaming, quotas, retention policy, audit                 │
│ - NOT an agent; does not run the tool-call loop              │
├──────────────────────────────────────────────────────────────┤
│ Central AI agents                                            │
│ - workspace-scoped actors                                    │
│ - callable by edge users and workflow extensions over MCP    │
│ - call the gateway for models; own the tool-call loop        │
├──────────────────────────────────────────────────────────────┤
│ Host + MCP server                                            │
│ - cloud extension tools                                      │
│ - registry/admin tools                                       │
│ - workflow and agent tools                                   │
├──────────────────────────────────────────────────────────────┤
│ Extension runtime                                            │
│ - cloud-only WASM components                                 │
│ - either-placement components                                │
│ - supervised sidecars where needed                           │
├──────────────────────────────────────────────────────────────┤
│ Platform crates                                              │
│ auth · caps · tags · inbox/outbox · jobs · secrets · sync ·  │
│ ext-loader · registry-host                                   │
├──────────────────────────────────────────────────────────────┤
│ Zenoh router                                                 │
│ - accepts edge peers                                         │
│ - routes workspace messages, MCP calls, streams, presence    │
├──────────────────────────────────────────────────────────────┤
│ SurrealDB                                                    │
│ - workspace authority                                        │
│ - identity / teams / channels                                │
│ - registry metadata                                          │
│ - jobs, inbox/outbox, docs, skills, audit                    │
├──────────────────────────────────────────────────────────────┤
│ Buckets / object backing                                     │
│ - docs, files, extension artifacts, generated outputs        │
└──────────────────────────────────────────────────────────────┘
```

## 3. Full Stack Flow

This shows the main edge-to-cloud path for a workflow extension using a central
AI agent, shared docs/skills, inbox approvals, jobs, and outbox delivery.

Sequence matters: the AI triages and drafts a scope doc first; a human/team
approval gates the durable job; only then does the remote coding session run and
emit external effects through the outbox.

```text
┌──────────────────────────┐
│ External event           │
│ GitHub issue / email / CI│
└────────────┬─────────────┘
             │ webhook / integration
             v
┌──────────────────────────┐        ┌──────────────────────────┐
│ Cloud extension          │───────▶│ Workspace inbox           │
│ github-bridge / email    │        │ source · payload · tags   │
└──────────────────────────┘        │ needs:triage              │
                                     └────────────┬─────────────┘
                                                  │ observed by
                                                  v
┌──────────────────────────┐  MCP   ┌──────────────────────────┐
│ Workflow extension       │───────▶│ Central AI agent          │
│ coding-workflow          │        │ workspace-scoped actor    │
└──────────────────────────┘        └───┬───────────────────▲──┘
                                model    │                   │ tool calls
                                 calls   v                   │ (capability-checked,
                                ┌────────────────────┐       │  over MCP)
                                │ Shared AI gateway   │       │
                                │ providers·quota·    │       │
                                │ audit (no tool loop)│       │
                                └─────────────────────┘       │
                                         │ capability-filtered │
                                         │ context             │
                                         v                     │
                                ┌──────────────────────────┐  │
                                │ Workspace assets          │──┘
                                │ docs · skills · messages  │
                                │ files · repo tools        │
                                └────────────┬─────────────┘
                                             │ scope doc + channel summary
                                             v
┌──────────────────────────┐        ┌──────────────────────────┐
│ Human / team approval     │◀──────│ Approval inbox item       │
│ approve · reject · edit   │        │ assigned to user/team     │
└────────────┬─────────────┘        └──────────────────────────┘
             │ approved
             v
┌──────────────────────────┐        ┌──────────────────────────┐
│ Remote workflow job       │───────▶│ Channel                   │
│ durable coding session    │        │ progress + discussion     │
└────────────┬─────────────┘        └──────────────────────────┘
             │ external effects
             v
┌──────────────────────────┐        ┌──────────────────────────┐
│ Outbox                   │───────▶│ Durable external effects  │
│ transactional delivery   │        │ GitHub PR / comment / msg │
└──────────────────────────┘        │ notifications · sync      │
                                     └──────────────────────────┘

Edge users can enter the same flow from the UI:

┌──────────────────────────┐
│ Edge UI / Tauri          │
│ user asks central agent  │
└────────────┬─────────────┘
             │ routed MCP over Zenoh
             v
┌──────────────────────────┐
│ Cloud central AI agent   │
└──────────────────────────┘
```

