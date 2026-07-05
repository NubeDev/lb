// The Permissions pane (agent-personas scope #1) — the per-tool Allow / Ask / Deny SUPERVISION editor
// over the shipped `agent.policy.get`/`set` machinery. A table of rules (tool glob + effect dropdown),
// add/remove rows, Save calls `agent.policy.set`. Members see it read-only (`agent.policy.get`); only a
// session holding `agent.policy.set` gets the editable table + Save.
//
// This edits SUPERVISION, never the wall — an Ask/Deny tightens what the agent does unattended; it
// grants/revokes nothing. If a selected persona carries a `policy_preset`, its ask/deny entries are the
// FLOOR: an admin may tighten but not loosen below the preset. For v1 we VISUALLY mark preset rules and
// WARN on a loosening rather than hard-block (loosening is the explicit admin write — persona-catalog
// #4). Tool globs are OPAQUE (rule 10) — plain strings, never branched on.

import { useEffect, useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { getAgentPolicy, setAgentPolicy, type Effect, type Rule } from "@/lib/agent/policy.api";
import type { PolicyPreset } from "@/lib/agent/agentPersona.api";

interface Props {
  /** May the caller write the policy (`agent.policy.set`)? Members without it see it read-only. */
  canEdit: boolean;
  /** The selected persona's supervision floor, if any — its ask/deny entries can be tightened, not
   *  loosened below. Shown as preset-marked rows with a loosening warning. */
  preset?: PolicyPreset;
}

const EFFECTS: Effect[] = ["allow", "ask", "deny"];
/** Supervision strength ordering (higher = stricter). Loosening = moving a rule below its preset floor. */
const STRENGTH: Record<Effect, number> = { allow: 0, ask: 1, deny: 2 };

/** The floor effect the preset pins for a given tool (the strictest preset lane it appears in), or
 *  undefined when the preset does not mention it. `deny` outranks `ask`. */
function presetFloor(preset: PolicyPreset | undefined, tool: string): Effect | undefined {
  if (!preset) return undefined;
  if (preset.deny.includes(tool)) return "deny";
  if (preset.ask.includes(tool)) return "ask";
  return undefined;
}

export function PolicyPane({ canEdit, preset }: Props) {
  const [rules, setRules] = useState<Rule[]>([]);
  const [loading, setLoading] = useState(true);
  const [status, setStatus] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setLoading(true);
    void getAgentPolicy()
      .then((r) => setRules(r))
      .catch((e) => setError(e instanceof Error ? e.message : "load failed"))
      .finally(() => setLoading(false));
  }, []);

  const addRow = () => setRules((prev) => [...prev, { tool: "", effect: "ask" }]);
  const removeRow = (i: number) => setRules((prev) => prev.filter((_, idx) => idx !== i));
  const setTool = (i: number, tool: string) =>
    setRules((prev) => prev.map((r, idx) => (idx === i ? { ...r, tool } : r)));
  const setEffect = (i: number, effect: Effect) =>
    setRules((prev) => prev.map((r, idx) => (idx === i ? { ...r, effect } : r)));

  // Rows whose effect sits BELOW their persona preset floor — a loosening (allowed, but flagged as the
  // explicit admin write). Computed for the warning banner, not a hard block (v1 posture).
  const loosened = useMemo(
    () =>
      rules.filter((r) => {
        const floor = presetFloor(preset, r.tool);
        return floor !== undefined && STRENGTH[r.effect] < STRENGTH[floor];
      }),
    [rules, preset],
  );

  const save = async () => {
    setStatus("saving");
    setError(null);
    try {
      // Drop empty-tool rows before the write — a blank glob is an unfinished row, not a rule.
      const clean = rules.filter((r) => r.tool.trim().length > 0);
      await setAgentPolicy(clean);
      setStatus("saved");
    } catch (e) {
      setStatus("error");
      setError(e instanceof Error ? e.message : "save failed");
    }
  };

  if (loading) return <p className="text-sm text-muted">Loading…</p>;

  return (
    <div className="flex flex-col gap-3" aria-label="policy pane">
      <p className="text-[11px] leading-snug text-muted">
        Per-tool <span className="font-medium">Allow / Ask / Deny</span> supervision. An{" "}
        <span className="font-medium">Ask</span> rule suspends a run for approval;{" "}
        <span className="font-medium">Deny</span> blocks the call. This tightens how the agent is
        watched — it grants or revokes nothing. Unmatched tools default to Allow.
      </p>

      {loosened.length > 0 && (
        <p role="alert" className="rounded-md border border-amber-500/40 px-3 py-2 text-[11px] text-amber-500">
          {loosened.length} rule{loosened.length === 1 ? "" : "s"} loosen the selected persona's
          supervision floor. Loosening below a persona preset is an explicit admin decision — the floor
          is a recommendation, not a lock.
        </p>
      )}

      {rules.length === 0 ? (
        <p className="rounded-md border border-dashed border-border px-4 py-4 text-center text-sm text-muted">
          No supervision rules — every reachable tool runs unattended (Allow).
        </p>
      ) : (
        <ul className="flex flex-col gap-1.5" aria-label="policy rules">
          {rules.map((r, i) => {
            const floor = presetFloor(preset, r.tool);
            return (
              <li
                key={i}
                aria-label={`rule ${i}`}
                className="flex items-center gap-2 rounded-md border border-border px-3 py-1.5"
              >
                <Input
                  aria-label={`rule ${i} tool`}
                  value={r.tool}
                  placeholder="tool id or glob (e.g. flows.*)"
                  disabled={!canEdit}
                  onChange={(e) => setTool(i, e.target.value)}
                  className="flex-1 font-mono"
                />
                <Select
                  aria-label={`rule ${i} effect`}
                  value={r.effect}
                  disabled={!canEdit}
                  onChange={(e) => setEffect(i, e.target.value as Effect)}
                  className="w-28"
                >
                  {EFFECTS.map((ef) => (
                    <option key={ef} value={ef}>
                      {ef}
                    </option>
                  ))}
                </Select>
                {floor && (
                  <span
                    className="shrink-0 rounded-md bg-panel px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-muted"
                    title={`Persona floor: ${floor}`}
                    role="note"
                  >
                    Floor {floor}
                  </span>
                )}
                {canEdit && (
                  <Button
                    size="sm"
                    variant="ghost"
                    aria-label={`remove rule ${i}`}
                    onClick={() => removeRow(i)}
                  >
                    Remove
                  </Button>
                )}
              </li>
            );
          })}
        </ul>
      )}

      {canEdit && (
        <div className="flex items-center gap-3 pt-1">
          <Button size="sm" variant="outline" onClick={addRow} aria-label="add rule">
            Add rule
          </Button>
          <Button
            size="sm"
            onClick={save}
            disabled={status === "saving"}
            aria-label="save policy"
          >
            {status === "saving" ? "Saving…" : "Save policy"}
          </Button>
          {status === "saved" && <span className="text-xs text-accent">Saved.</span>}
          {status === "error" && (
            <span role="alert" className="text-xs text-red-500">
              {error}
            </span>
          )}
        </div>
      )}

      {!canEdit && (
        <p className="border-t border-border pt-3 text-[11px] text-muted">
          You can view the supervision policy. Editing it requires an administrator.
        </p>
      )}
    </div>
  );
}
