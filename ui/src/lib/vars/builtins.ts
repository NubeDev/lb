// Resolve the built-in globals (widget-config-vars scope, "builtins.ts"). PURE — given trusted inputs
// the SHELL supplies from the verified token + the URL time range. An extension/iframe NEVER calls this
// with its own identity: the shell resolves the scope and hands the widget resolved `ctx.vars` (Slice 3),
// so `${__user.*}`/`${__workspace}` are un-spoofable. Pure TS, no React.
//
// Returns a flat `Builtins` map keyed by the built-in's BARE name (no leading `$__`): `from`, `to`,
// `interval`, `interval_ms`, `range`, `range_s`, `range_ms`, `__user.login`, `__user.email`,
// `__dashboard`, `__workspace`, `__value`. (User-prefixed names keep their `__` so `${__user.login}`
// resolves; the time built-ins are the `$__from`-style ones whose bare name is `__from`.)

import type { Builtins } from "./types";

/** The trusted inputs the shell supplies (token + URL). All optional — a missing input yields no key
 *  (so the reference stays literal rather than rendering an empty/fake value). */
export interface BuiltinInputs {
  /** The URL time range, as epoch milliseconds. */
  timeRange?: { fromMs: number; toMs: number };
  /** The verified session identity (from the token, never the cell). */
  identity?: { login?: string; email?: string };
  /** The dashboard id/title. */
  dashboardId?: string;
  /** The tenant (the hard wall) — the viewer's workspace, from the token. */
  workspace?: string;
  /** The chosen interval (an `interval`-type variable's value, e.g. `5m`). */
  interval?: string;
  /** A control's current interaction value (generalizes the shipped `{{value}}`). */
  value?: string;
}

/** Parse a duration like `5m`/`30s`/`1h`/`250ms` to milliseconds (best-effort; unknown → 0). */
function durationMs(d: string): number {
  const m = /^(\d+)(ms|s|m|h|d)$/.exec(d.trim());
  if (!m) return 0;
  const n = Number(m[1]);
  switch (m[2]) {
    case "ms":
      return n;
    case "s":
      return n * 1000;
    case "m":
      return n * 60_000;
    case "h":
      return n * 3_600_000;
    case "d":
      return n * 86_400_000;
    default:
      return 0;
  }
}

/** Build the `$__*` / `${__user.*}` / `${__dashboard}` / `${__workspace}` map from trusted inputs. */
export function resolveBuiltins(inputs: BuiltinInputs): Builtins {
  const out: Builtins = {};
  const { timeRange, identity, dashboardId, workspace, interval, value } = inputs;

  if (timeRange) {
    const { fromMs, toMs } = timeRange;
    out["__from"] = String(fromMs);
    out["__to"] = String(toMs);
    const rangeMs = Math.max(0, toMs - fromMs);
    out["__range_ms"] = String(rangeMs);
    out["__range_s"] = String(Math.floor(rangeMs / 1000));
    out["__range"] = `${Math.floor(rangeMs / 1000)}s`;
  }
  if (interval) {
    out["__interval"] = interval;
    out["__interval_ms"] = String(durationMs(interval));
  }
  if (identity?.login !== undefined) out["__user.login"] = identity.login;
  if (identity?.email !== undefined) out["__user.email"] = identity.email;
  if (dashboardId !== undefined) out["__dashboard"] = dashboardId;
  if (workspace !== undefined) out["__workspace"] = workspace;
  if (value !== undefined) out["__value"] = value;

  return out;
}
