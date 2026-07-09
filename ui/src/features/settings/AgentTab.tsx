// The Agent tab (agent-catalog scope) — a catalog manager + active-selection picker. Renders the
// pickable presets from `agent.def.list` (seeded read-only built-ins ∪ the workspace's custom
// definitions), highlights the active selection (resolved from the shipped `agent.config`), and — for
// an admin — lets you pick one (writes `agent.config`), or create/edit/delete a custom definition.
// A member without the write caps sees the catalog + active pick read-only.
//
// Picking a definition sets the workspace default RUNTIME today; the invoke path's
// `resolve_effective_runtime` honors it. Routing the in-house loop to a per-workspace model ENDPOINT
// is gated on the ai-gateway provider adapter (default-agent-wiring) — the copy says so honestly and
// does not over-promise that the key/model is live per workspace beyond what is wired.

import { AgentCatalogSection } from "./agent/AgentCatalogSection";
import { PersonaSection } from "./agent/PersonaSection";

interface Props {
  ws: string;
  caps: string[] | undefined;
}

export function AgentTab({ caps }: Props) {
  return (
    <div className="mx-auto max-w-3xl px-4 py-4">
      {/* "Who runs": the definition catalog + active pick (extracted so the Setup agent wizard shares
          this exact editor — setup rule 3). */}
      <AgentCatalogSection caps={caps} />

      {/* The "what for" half: the persona picker/editor + the Effective-tools view + the Permissions
          (Allow/Ask/Deny) pane. Same cap-gating pattern; edits advertisement + supervision, never the
          wall (agent-personas scope #1). */}
      <PersonaSection caps={caps} />
    </div>
  );
}
