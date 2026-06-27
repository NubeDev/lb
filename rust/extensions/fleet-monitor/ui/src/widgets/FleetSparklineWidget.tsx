import { TrendingUp } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui";

/** Dashboard tile (manifest widget "Fleet Sparkline", icon `trending-up`). Placeholder this slice — a
 *  real shadcn Card, honestly labelled. Wires to `series.read` next. */
export function FleetSparklineWidget() {
  return (
    <Card>
      <CardHeader>
        <CardTitle>
          <TrendingUp className="h-4 w-4 text-accent" aria-hidden />
          Fleet Sparkline
        </CardTitle>
      </CardHeader>
      <CardContent>placeholder — wires to series.read next.</CardContent>
    </Card>
  );
}
