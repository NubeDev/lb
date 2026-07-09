// The console's second lane: the AUDIT ledger (telemetry-console scope). Audit is the immutable,
// hash-chained mutation record — a SEPARATE store from the evictable telemetry ring, NEVER merged
// into it. This lane reads `audit.query` (requiring the audit grant too).
//
// Audit has not shipped yet (no `audit.query` host verb / no `lb/audit` client). Per the scope, the
// lane therefore degrades to a clearly-labelled empty state — NOT fake rows, NOT an error. When audit
// lands, this component grows an `audit.api` read + a row list mirroring `TelemetryList`; the
// telemetry ring is never used as a stand-in for the mutation record.

interface Props {
  /** Whether the session holds the audit read grant. Without it the lane is a permission notice, not
   *  an error (a telemetry-only session sees telemetry + this labelled lane). */
  hasAuditGrant: boolean;
}

/** The audit lane. Today: a labelled "not yet available" state (audit unshipped) or a "needs the
 *  audit grant" notice — both honest, neither a fake row nor a misrepresentation of the ring. */
export function AuditLane({ hasAuditGrant }: Props) {
  return (
    <div className="flex flex-col items-center gap-2 py-12 text-center">
      <div className="text-sm font-medium">Audit ledger</div>
      {hasAuditGrant ? (
        <p className="max-w-md text-sm text-muted-foreground">
          The immutable, hash-chained mutation ledger ("who deleted/updated what") has not shipped on
          this node yet. When it does, it appears here as its own lane — a separate store from the
          telemetry ring, with its own retention and integrity guarantees. No rows are shown rather
          than synthesising them from the evictable telemetry sink.
        </p>
      ) : (
        <p className="max-w-md text-sm text-muted-foreground">
          This session does not hold the audit read grant, so the audit lane is hidden. The telemetry
          lane (operational, sampled, evictable) is independent and requires only{" "}
          <code className="rounded-md bg-muted-bg px-1 text-fg">telemetry:read</code>.
        </p>
      )}
    </div>
  );
}
