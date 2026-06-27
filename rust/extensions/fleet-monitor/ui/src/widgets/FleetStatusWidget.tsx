import { Activity } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui";

/** Dashboard tile (manifest widget "Fleet Status", icon `activity`). Placeholder this slice — a real
 *  shadcn Card, honestly labelled. Wires to `series.latest` next. */
export function FleetStatusWidget() {
  return (
    <Card>
      <CardHeader>
        <CardTitle>
          <Activity className="h-4 w-4 text-accent" aria-hidden />
          Fleet Status
        </CardTitle>
      </CardHeader>
      <CardContent>placeholder — wires to series.latest next.</CardContent>
    </Card>
  );
}
