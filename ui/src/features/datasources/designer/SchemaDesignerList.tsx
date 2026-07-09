// The schema-designer list page (schema-designer scope) — the roster of designed `db_schema`
// records in the workspace. The "New" action opens a fresh unsaved canvas; clicking a row opens the
// designer loaded with that record. shadcn-first. One responsibility, one file (FILE-LAYOUT).

import { useEffect, useState } from "react";
import { Loader2, Plus, Wand2 } from "lucide-react";

import { AppPage } from "@/components/app/page";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { listDbSchemas, type DbSchemaSummary } from "@/lib/datasources";

interface Props {
  /** The workspace id. */
  ws: string;
  /** Open a schema by name (or `new` for a fresh canvas). */
  onOpen: (name: string) => void;
}

/** The schema roster page. */
export function SchemaDesignerList({ ws, onOpen }: Props) {
  const [schemas, setSchemas] = useState<DbSchemaSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = () => {
    setLoading(true);
    listDbSchemas()
      .then(setSchemas)
      .catch((e) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  };

  useEffect(refresh, []);

  return (
    <AppPage
      workspace={ws}
      icon={Wand2}
      label="Schemas"
      title="Schema designer"
      description="design tables, columns, PKs, and relationships — then migrate to a datasource"
      error={error}
      actions={
        <Button size="sm" className="gap-1.5" onClick={() => onOpen("new")} aria-label="new schema">
          <Plus size={13} /> New schema
        </Button>
      }
    >
      {loading ? (
        <div className="flex h-full items-center justify-center text-sm text-muted">
          <Loader2 size={14} className="mr-2 animate-spin" /> Loading schemas…
        </div>
      ) : schemas.length === 0 ? (
        <div className="flex h-full items-center justify-center p-8 text-center text-sm text-muted">
          <div className="max-w-sm">
            <Wand2 size={28} className="mx-auto mb-3 opacity-40" />
            <p>No designed schemas yet.</p>
            <p className="mt-1 text-xs">
              Click <strong>New schema</strong> to design tables + relationships, then migrate to a
              datasource.
            </p>
          </div>
        </div>
      ) : (
        <div className="overflow-auto p-3">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-border text-left text-xs text-muted">
                <th className="py-2 pr-3 font-medium">Name</th>
                <th className="py-2 pr-3 font-medium">Tables</th>
                <th className="py-2 pr-3 font-medium">Version</th>
              </tr>
            </thead>
            <tbody>
              {schemas.map((s) => (
                <tr
                  key={s.name}
                  className="border-b border-border/60 hover:bg-panel/40"
                >
                  <td className="py-2 pr-3">
                    <Button
                      variant="ghost"
                      className="h-auto gap-1.5 p-0 font-mono text-accent"
                      onClick={() => onOpen(s.name)}
                    >
                      {s.name}
                    </Button>
                  </td>
                  <td className="py-2 pr-3 text-muted">{s.tableCount}</td>
                  <td className="py-2 pr-3">
                    <Badge variant="outline" className="font-mono text-[10px]">
                      v{s.version}
                    </Badge>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </AppPage>
  );
}
