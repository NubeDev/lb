// The webhooks roster table (webhooks scope) — list, rotate, revoke. Renders one row per
// webhook with `Name · Mode · URL · Status · actions`; status uses the shadcn `Badge` so the
// revoked rows read as destructive and active rows read as data. The list NEVER renders a hash,
// shared secret, `bearer_key_id`, or `secret_ref` — those fields aren't on `WebhookView`, and
// the Rust test pins the wire; this component only renders the credential-free view. Reached
// from the page body when there is at least one webhook. One component per file (FILE-LAYOUT).

import { RotateCw, Trash2 } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { gatewayUrl } from "@/lib/ipc/http";
import type { WebhookView } from "@/lib/admin/webhooks.api";

interface Props {
  webhooks: WebhookView[];
  onRotate: (id: string) => Promise<void>;
  onRevoke: (id: string) => Promise<void>;
}

/** Compose the public inbound URL from the gateway origin + the hook's url_path. Empty origin
 *  (no window, e.g. jsdom) → just the path. */
function publicUrl(urlPath: string): string {
  const origin = gatewayUrl();
  return origin ? `${origin}${urlPath}` : urlPath;
}

export function WebhookRoster({ webhooks, onRotate, onRevoke }: Props) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Name</TableHead>
          <TableHead>Mode</TableHead>
          <TableHead>URL</TableHead>
          <TableHead>Status</TableHead>
          <TableHead aria-label="actions" />
        </TableRow>
      </TableHeader>
      <TableBody>
        {webhooks.map((w) => {
          const revoked = w.status === "__revoked__";
          return (
            <TableRow key={w.id}>
              <TableCell className="font-medium text-fg">{w.name}</TableCell>
              <TableCell className="text-muted">{w.auth_mode}</TableCell>
              <TableCell>
                <code className="break-all font-mono text-muted">{publicUrl(w.url_path)}</code>
              </TableCell>
              <TableCell>
                <Badge variant={revoked ? "destructive" : "success"}>
                  {revoked ? "revoked" : w.status}
                </Badge>
              </TableCell>
              <TableCell className="text-right">
                {!revoked && (
                  <span className="flex justify-end gap-1">
                    <Button
                      variant="ghost"
                      size="sm"
                      aria-label={`rotate webhook ${w.id}`}
                      onClick={() => void onRotate(w.id)}
                    >
                      <RotateCw size={13} /> Rotate
                    </Button>
                    <Button
                      variant="destructive"
                      size="sm"
                      aria-label={`revoke webhook ${w.id}`}
                      onClick={() => void onRevoke(w.id)}
                    >
                      <Trash2 size={13} /> Revoke
                    </Button>
                  </span>
                )}
              </TableCell>
            </TableRow>
          );
        })}
      </TableBody>
    </Table>
  );
}
