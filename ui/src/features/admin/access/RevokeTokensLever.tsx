// The live-token revoke lever (access-console scope) — the secondary "Apply now — end active
// sessions" action shown beside revoking admin actions. It calls `authz.revoke-tokens`, which writes
// the verify-path marker (the subject's CURRENT token is refused on the next request) AND composes
// with the shipped `revoke_subject` (tombstones grants → next re-mint). The default note states the
// honest timing (cached caps drop on next sign-in / within TTL); THIS lever is for a genuine
// immediate lockout. Multi-node worst case is bounded by TTL — stated, never "instant global".
// One responsibility per file (FILE-LAYOUT).

import { useState } from "react";
import { Power } from "lucide-react";

import { Button } from "@/components/ui/button";
import { revokeTokens } from "@/lib/admin/grants.api";
import { hasCap } from "@/lib/session";
import { CAP } from "@/lib/session/admin-caps";

interface Props {
  /** The `kind:name` subject whose live tokens + grants to revoke. */
  subject: string;
  /** The admin's session caps — the lever is shown only for a session holding the revoke cap. */
  caps: string[] | undefined;
  /** Optional label for what was just revoked (e.g. "team membership") — used in the success note. */
  context?: string;
  /** Called after a successful apply, so the caller can refetch. */
  onApplied?: () => void;
}

export function RevokeTokensLever({ subject, caps, context, onApplied }: Props) {
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  if (!hasCap(caps, CAP.authzRevokeTokens)) return null;

  async function apply() {
    setBusy(true);
    setError(null);
    try {
      const { grants_revoked } = await revokeTokens(subject);
      setDone(grants_revoked);
      onApplied?.();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  if (done !== null) {
    return (
      <p className="text-xs text-muted">
        Active sessions ended — {subject}&apos;s current token is refused on the next request
        {context ? ` (${context})` : ""}. {done} grant{done === 1 ? "" : "s"} tombstoned.
      </p>
    );
  }

  return (
    <div className="space-y-1">
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="gap-1.5"
        disabled={busy}
        onClick={() => void apply()}
      >
        <Power size={13} /> {busy ? "Applying…" : "Apply now — end active sessions"}
      </Button>
      <p className="text-[0.6875rem] leading-relaxed text-muted">
        For a genuine immediate lockout: kills {subject}&apos;s live token (refused on next request,
        single-node instant) AND tombstones their grants (next re-mint). Multi-node worst case is
        bounded by the token TTL — not instant global. Most revocations need only the default above.
      </p>
      {error && (
        <p className="text-xs text-destructive" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
