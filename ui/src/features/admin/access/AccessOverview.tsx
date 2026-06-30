// The Access overview (access-console scope) — the access-first LANDING of the admin console. A grid
// of honest, read-only tiles: People / Teams / Roles / Keys counts, plus the security-posture tiles
// (direct-grant subjects bypassing roles, keys expiring <7d, subjects holding any admin cap). Every
// count comes from a real verb; a tile whose source verb/cap is absent is HIDDEN with a reason,
// never a fabricated 0. The resolver-driven tiles (direct grants, admin-cap holders) use
// `authz.resolve` per subject — bounded workspace, client-side aggregation (scope decision #2).
// One responsibility per file (FILE-LAYOUT).

import { useEffect, useState } from "react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { listApiKeys, type ApiKeyView } from "@/lib/admin/apikeys.api";
import { listRoles } from "@/lib/admin/roles.api";
import { listTeams } from "@/lib/admin/teams.api";
import { listUsers } from "@/lib/admin/users.api";
import { resolveCaps } from "@/lib/admin/grants.api";
import { CAP, hasCap } from "@/lib/session/admin-caps";

interface Props {
  ws: string;
  caps: string[] | undefined;
  /** Navigate to a tab when a tile is clicked (e.g. the People tile → the People tab). */
  onJump?: (tab: "people" | "teams" | "roles" | "apikeys") => void;
}

interface Counts {
  people: number;
  teams: number;
  roles: number;
  keys: number;
  keysExpiringSoon: number;
  /** Subjects (users) with at least one DIRECT (non-role) grant, or null when the resolve cap is absent. */
  directGrantSubjects: number | null;
  /** Subjects holding any admin cap, or null when the resolve cap is absent. */
  adminCapHolders: number | null;
  error: string | null;
}

const WEEK_S = 7 * 24 * 60 * 60;

/** A cap "looks admin" if it grants a `manage`/`delete`/`purge` action — the posture signal. */
function looksAdmin(cap: string): boolean {
  return /:(manage|delete|purge|revoke-tokens|resolve):call$/.test(cap) || cap.startsWith("mcp:authz.");
}

function expiringSoon(k: ApiKeyView, nowSec: number): boolean {
  return k.expires_at > 0 && k.expires_at - nowSec <= WEEK_S;
}

export function AccessOverview({ ws, caps, onJump }: Props) {
  const [c, setC] = useState<Counts>({
    people: 0,
    teams: 0,
    roles: 0,
    keys: 0,
    keysExpiringSoon: 0,
    directGrantSubjects: null,
    adminCapHolders: null,
    error: null,
  });

  const canResolve = hasCap(caps, CAP.authzResolve);
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [users, teams, roles, keys] = await Promise.all([
          listUsers(),
          listTeams(),
          listRoles(),
          listApiKeys(),
        ]);
        if (cancelled) return;
        const nowSec = Math.floor(Date.now() / 1000);
        let directGrantSubjects: number | null = null;
        let adminCapHolders: number | null = null;
        if (canResolve) {
          // Per-subject resolve — bounded by workspace size; client-side aggregation (no `who_has` verb).
          const results = await Promise.all(
            users.map((u) => resolveCaps(`user:${u.user}`).catch(() => [])),
          );
          directGrantSubjects = results.filter((caps) =>
            caps.some((c) => c.source.some((s) => s.kind === "direct")),
          ).length;
          adminCapHolders = results.filter((caps) =>
            caps.some((c) => looksAdmin(c.cap)),
          ).length;
        }
        if (cancelled) return;
        setC({
          people: users.length,
          teams: teams.length,
          roles: roles.length,
          keys: keys.length,
          keysExpiringSoon: keys.filter((k) => expiringSoon(k, nowSec)).length,
          directGrantSubjects,
          adminCapHolders,
          error: null,
        });
      } catch (e) {
        if (!cancelled) {
          setC((prev) => ({
            ...prev,
            error: e instanceof Error ? e.message : String(e),
          }));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [ws, canResolve]);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="min-h-0 flex-1 overflow-y-auto p-4">
        {c.error && (
          <p className="mb-3 text-xs text-destructive" role="alert">
            Some tiles could not load: {c.error}
          </p>
        )}
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <Tile label="People" value={c.people} hint="users in this workspace" onClick={() => onJump?.("people")} />
          <Tile label="Teams" value={c.teams} hint="groups that inherit a role's caps" onClick={() => onJump?.("teams")} />
          <Tile label="Roles" value={c.roles} hint="named capability bundles" onClick={() => onJump?.("roles")} />
          <Tile label="API keys" value={c.keys} hint="machine principals" onClick={() => onJump?.("apikeys")} />
          <Tile
            label="Keys expiring < 7d"
            value={c.keysExpiringSoon}
            hint={c.keysExpiringSoon > 0 ? "rotate before they lapse" : "none due soon"}
            tone={c.keysExpiringSoon > 0 ? "warn" : "ok"}
          />
          {canResolve ? (
            <>
              <Tile
                label="Direct-grant subjects"
                value={c.directGrantSubjects ?? 0}
                hint="subjects with a cap granted directly, bypassing roles"
              />
              <Tile
                label="Subjects holding an admin cap"
                value={c.adminCapHolders ?? 0}
                hint="any manage/delete/purge/authz cap"
                tone={(c.adminCapHolders ?? 0) > 0 ? "warn" : "ok"}
              />
            </>
          ) : (
            <Tile
              label="Direct-grant + admin-cap subjects"
              value={null}
              hint="hidden — the session lacks mcp:authz.resolve:call"
            />
          )}
        </div>
        <p className="mt-4 text-[0.6875rem] leading-relaxed text-muted">
          Tiles are honest counts from the real verbs. Empty is empty, not unknown.
        </p>
      </div>
    </div>
  );
}

function Tile({
  label,
  value,
  hint,
  tone = "neutral",
  onClick,
}: {
  label: string;
  value: number | null;
  hint: string;
  tone?: "neutral" | "warn" | "ok";
  onClick?: () => void;
}) {
  return (
    <Card
      className={onClick ? "cursor-pointer transition-colors hover:bg-bg/60" : undefined}
      onClick={onClick}
      role={onClick ? "button" : undefined}
      aria-label={label}
    >
      <CardHeader className="pb-1">
        <CardTitle className="text-xs font-medium uppercase tracking-wide text-muted">
          {label}
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="text-2xl font-semibold text-fg">
          {value === null ? "—" : value}
        </div>
        <p className="mt-1 text-[0.6875rem] leading-snug text-muted">{hint}</p>
        {tone !== "neutral" && value !== null && (
          <Badge
            variant={tone === "warn" ? "destructive" : "secondary"}
            className="mt-1.5"
          >
            {tone === "warn" ? "review" : "ok"}
          </Badge>
        )}
      </CardContent>
    </Card>
  );
}
