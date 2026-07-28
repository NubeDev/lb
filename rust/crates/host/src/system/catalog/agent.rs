//! The `agent.*` family — the central agent's policy, config, catalog, persona, memory and run verbs.
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const AGENT: &[HostTool] = &[
    // agent.* — the central agent's policy/decision/run verbs (agent + agent-run scope).
    HostTool {
        tool: "agent.policy.set",
        group: "agent",
        description: "set the per-workspace autonomy policy the agent decides under",
    },
    HostTool {
        tool: "agent.decide",
        group: "agent",
        description: "record a decision on a suspended agent run (approve/deny/edit)",
    },
    HostTool {
        tool: "agent.watch",
        group: "agent",
        description: "subscribe to a run's RunEvent feed (the live turn projection)",
    },
    HostTool {
        tool: "agent.control",
        group: "agent",
        description: "stop / pause / resume a live agent run",
    },
    HostTool {
        tool: "agent.policy.get",
        group: "agent",
        description:
            "read the per-workspace autonomy policy (member-level; the editor's round-trip read)",
    },
    HostTool {
        tool: "agent.runtimes",
        group: "agent",
        description: "the runtimes this node has configured (the composer's runtime picker)",
    },
    // agent.config.* — the per-workspace default runtime + model endpoint (agent-config scope).
    HostTool {
        tool: "agent.config.get",
        group: "agent",
        description: "read the workspace's default runtime + model endpoint (member-level)",
    },
    HostTool {
        tool: "agent.config.set",
        group: "agent",
        description: "set the workspace's default runtime + model endpoint (admin-only)",
    },
    // agent.def.* — the named (runtime, model_endpoint) preset catalog (agent-catalog scope).
    HostTool {
        tool: "agent.def.list",
        group: "agent",
        description:
            "list the agent definitions — seeded built-ins plus workspace-authored presets",
    },
    HostTool {
        tool: "agent.def.get",
        group: "agent",
        description: "read one agent definition by name",
    },
    HostTool {
        tool: "agent.def.create",
        group: "agent",
        description: "create a custom agent definition (admin-only; the built-in tier is reserved)",
    },
    HostTool {
        tool: "agent.def.update",
        group: "agent",
        description: "patch a custom agent definition (admin-only; built-ins are immutable)",
    },
    HostTool {
        tool: "agent.def.delete",
        group: "agent",
        description: "delete a custom agent definition (admin-only; built-ins are immutable)",
    },
    // agent.persona.* — the tool/skill/identity bundle that NARROWS a run (agent-personas scope).
    HostTool {
        tool: "agent.persona.list",
        group: "agent",
        description: "list the personas — seeded built-ins plus workspace-authored ones",
    },
    HostTool {
        tool: "agent.persona.get",
        group: "agent",
        description: "read one persona by name (as authored, before inheritance)",
    },
    HostTool {
        tool: "agent.persona.resolve",
        group: "agent",
        description: "the effective persona after `extends` inheritance (what a run would apply)",
    },
    HostTool {
        tool: "agent.persona.create",
        group: "agent",
        description: "create a custom persona (admin-only; the built-in tier is reserved)",
    },
    HostTool {
        tool: "agent.persona.update",
        group: "agent",
        description: "patch a custom persona (admin-only; built-ins are immutable)",
    },
    HostTool {
        tool: "agent.persona.delete",
        group: "agent",
        description: "delete a custom persona (admin-only; built-ins are immutable)",
    },
    // agent.memory.* — the durable fact records a run recalls under the derived principal.
    HostTool {
        tool: "agent.memory.list",
        group: "agent",
        description:
            "the memory index for a scope (workspace-shared or the caller's own member scope)",
    },
    HostTool {
        tool: "agent.memory.get",
        group: "agent",
        description: "read one memory fact by scope + slug",
    },
    HostTool {
        tool: "agent.memory.set",
        group: "agent",
        description:
            "write one memory fact (member scope is derived from the caller, never an argument)",
    },
    HostTool {
        tool: "agent.memory.delete",
        group: "agent",
        description: "forget one memory fact by scope + slug",
    },
];
