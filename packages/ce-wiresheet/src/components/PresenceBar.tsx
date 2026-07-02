import { usePresence, PRESENCE_PALETTE } from "../lib/presence";

// Compact list of other collaborators connected to the same engine, with the
// color that identifies their selection ring on the canvas. Hidden entirely
// when alone (no clutter for the common single-user case).
export function PresenceBar() {
  const collaborators = usePresence((s) => s.collaborators);
  if (collaborators.size === 0) return null;
  const list = [...collaborators.values()];
  return (
    <div
      style={{
        // Sit just above the breadcrumb (also bottom-centered at bottom:12) so
        // the two don't overlap.
        position: "fixed",
        bottom: 52,
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: 25,
        display: "flex",
        alignItems: "center",
        gap: 8,
        background: "hsl(var(--card) / 0.92)",
        border: "1px solid hsl(var(--border))",
        borderRadius: 20,
        padding: "5px 12px",
        fontSize: 11,
        fontFamily: "var(--font-mono)",
        color: "hsl(var(--foreground))",
      }}
    >
      <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10 }}>
        {list.length} other{list.length === 1 ? "" : "s"}
      </span>
      {list.map((c) => {
        const color = PRESENCE_PALETTE[c.colorIdx];
        const name = c.state.userName ?? c.sessionId.slice(0, 6);
        const selCount = c.state.selectedComponents?.length ?? 0;
        return (
          <span
            key={c.sessionId}
            title={
              selCount > 0
                ? `${name} — ${selCount} selected`
                : `${name} — viewing`
            }
            style={{ display: "flex", alignItems: "center", gap: 4 }}
          >
            <span
              style={{
                width: 8,
                height: 8,
                borderRadius: 4,
                background: color,
                display: "inline-block",
              }}
            />
            <span>{name}</span>
            {selCount > 0 && (
              <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 9 }}>×{selCount}</span>
            )}
          </span>
        );
      })}
    </div>
  );
}
