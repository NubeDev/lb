// The schema-designer page (schema-designer scope) — owns the loaded `DbSchemaRecord`, load/save
// via `dbschema.*`, import-from-source, and the migrate dialog. Renders the canvas + side panel +
// a toolbar. The canvas + side-panel own the live graph edits; this page persists them on save.
// shadcn-first. One responsibility, one file (FILE-LAYOUT).

import { useCallback, useEffect, useMemo, useState } from "react";
import { Loader2, Save, Wand2 } from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import {
  deleteDbSchema,
  getDbSchema,
  listDatasources,
  listDbSchemas,
  saveDbSchema,
  type DbSchemaRecord,
  type DatasourceSummary,
} from "@/lib/datasources";
import { MigrateDialog } from "./MigrateDialog";
import { SchemaDesignerCanvas } from "./SchemaDesignerCanvas";
import { TableSidePanel } from "./TableSidePanel";
import type { EditableTableNodeData } from "./recordFlow";

interface Props {
  /** The workspace id (for the page header). */
  ws: string;
  /** The schema name to load (`new` for a fresh unsaved design). */
  name: string;
  /** Navigate back to the list. */
  onBack: () => void;
}

/** A fresh record the canvas starts from when the page is `new`. */
function emptyRecord(name: string): DbSchemaRecord {
  return { name, version: 1, tables: [], fks: [], layout: {} };
}

/** The designer page. Loads the record on mount, owns mutations, persists on save. */
export function SchemaDesignerPage({ ws, name, onBack }: Props) {
  const isNew = name === "new" || name === "";
  const [record, setRecord] = useState<DbSchemaRecord>(emptyRecord(isNew ? "untitled" : name));
  const [loading, setLoading] = useState(!isNew);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const [selectedTable, setSelectedTable] = useState<string | null>(null);
  const [migrateOpen, setMigrateOpen] = useState(false);
  const [importSource, setImportSource] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [sources, setSources] = useState<DatasourceSummary[]>([]);
  const [schemaNames, setSchemaNames] = useState<string[]>([]);
  const [saveName, setSaveName] = useState(isNew ? "" : name);

  // Load the record + the sources + the existing schema names (for the save-name collision hint).
  useEffect(() => {
    if (isNew) {
      setRecord(emptyRecord("untitled"));
      setSaveName("");
      setLoading(false);
    } else {
      setLoading(true);
      getDbSchema(name)
        .then((r) => {
          if (r) {
            setRecord(r);
            setSaveName(r.name);
          } else {
            setError(`No schema named "${name}" in this workspace.`);
          }
        })
        .catch((e) => setError(e instanceof Error ? e.message : String(e)))
        .finally(() => setLoading(false));
    }
    listDatasources()
      .then(setSources)
      .catch(() => {});
    listDbSchemas()
      .then((s) => setSchemaNames(s.map((x) => x.name)))
      .catch(() => {});
  }, [name, isNew]);

  const onChange = useCallback((next: DbSchemaRecord) => {
    setRecord(next);
    setDirty(true);
  }, []);

  const onSave = async () => {
    if (!saveName.trim()) {
      setError("Give the schema a name before saving.");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const toSave: DbSchemaRecord = { ...record, name: saveName.trim() };
      await saveDbSchema(saveName.trim(), toSave);
      setRecord(toSave);
      setDirty(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const onDelete = async () => {
    if (isNew) return;
    if (!confirm(`Delete the "${name}" schema? This removes the design record (not any live DB).`)) {
      return;
    }
    try {
      await deleteDbSchema(name);
      onBack();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const onImportSource = (s: string) => {
    setImportSource(s);
    setImporting(true);
  };

  // The selected table's data (for the side panel). Kept in sync with `record` by name.
  const selectedData: EditableTableNodeData | null = useMemo(() => {
    if (!selectedTable) return null;
    const t = record.tables.find((x) => x.name === selectedTable);
    if (!t) return null;
    const pk = new Set(t.pk);
    return {
      name: t.name,
      columns: t.columns.map((c) => ({
        name: c.name,
        type: c.type,
        nullable: c.nullable,
        pk: pk.has(c.name),
      })),
    };
  }, [selectedTable, record]);

  const updateSelectedTable = (mutate: (data: EditableTableNodeData) => EditableTableNodeData) => {
    if (!selectedTable) return;
    const next = record.tables.map((t) => {
      if (t.name !== selectedTable) return t;
      const pkSet = new Set(t.pk);
      const updated = mutate({
        name: t.name,
        columns: t.columns.map((c) => ({
          name: c.name,
          type: c.type,
          nullable: c.nullable,
          pk: pkSet.has(c.name),
        })),
      });
      return {
        name: updated.name,
        pk: updated.columns.filter((c) => c.pk).map((c) => c.name),
        columns: updated.columns.map((c) => ({ name: c.name, type: c.type, nullable: c.nullable })),
      };
    });
    // If the table was renamed, follow the new name (so the side panel stays bound).
    const renamed = next.find((t) => t.name !== selectedTable && record.tables.find((x) => x.name === selectedTable));
    if (renamed) setSelectedTable(renamed.name);
    setRecord({ ...record, tables: next });
    setDirty(true);
  };

  const onDeleteTable = () => {
    if (!selectedTable) return;
    setRecord({
      ...record,
      tables: record.tables.filter((t) => t.name !== selectedTable),
      fks: record.fks.filter(
        (fk) => fk.fromTable !== selectedTable && fk.toTable !== selectedTable,
      ),
    });
    setSelectedTable(null);
    setDirty(true);
  };

  return (
    <section className="flex h-full min-w-0 flex-col bg-bg" data-testid="schema-designer-page">
      <AppPageHeader
        icon={Wand2}
        title={isNew ? "New schema" : record.name}
        description="design tables, columns, PKs, and relationships — then migrate to a datasource"
        workspace={ws}
        actions={
          <>
            <Button variant="ghost" size="sm" onClick={onBack}>
              Back
            </Button>
            {!isNew && (
              <Button variant="ghost" size="sm" className="text-destructive" onClick={onDelete}>
                Delete
              </Button>
            )}
            <Button
              variant="default"
              size="sm"
              className="gap-1.5"
              onClick={onSave}
              disabled={saving || (dirty === false && saveName === record.name && !isNew)}
              aria-label="save schema"
            >
              {saving ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />}
              Save
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setMigrateOpen(true)}
              disabled={record.tables.length === 0}
            >
              Migrate…
            </Button>
          </>
        }
      />

      <div className="flex items-center gap-2 border-b border-border bg-bg px-3 py-2">
        <label className="flex items-center gap-1.5 text-xs font-medium text-fg">
          Name
          <Input
            aria-label="schema name"
            value={saveName}
            onChange={(e) => setSaveName(e.target.value)}
            className="h-6 w-40 font-mono text-xs"
            placeholder="shop"
          />
        </label>
        {dirty && (
          <Badge variant="outline" className="text-[10px] text-amber-500">
            unsaved
          </Badge>
        )}
        <div className="ml-auto flex items-center gap-1.5">
          <span className="text-xs text-muted">Import from:</span>
          <Select
            aria-label="import from datasource"
            value={importSource ?? ""}
            onChange={(e) => {
              if (e.target.value) onImportSource(e.target.value);
            }}
            className="h-6 w-40 text-xs"
          >
            <option value="">— none —</option>
            {sources.map((s) => (
              <option key={s.name} value={s.name}>
                {s.name}
              </option>
            ))}
          </Select>
        </div>
      </div>

      {error && (
        <Alert variant="destructive">
          <AlertTitle>Error</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      {loading ? (
        <div className="flex h-full items-center justify-center text-sm text-muted">
          <Loader2 size={14} className="mr-2 animate-spin" /> Loading schema…
        </div>
      ) : (
        <div className="flex min-h-0 flex-1">
          <div
            className="min-h-0 min-w-0 flex-1"
            onClick={(e) => {
              // Clicking the canvas backdrop (not a node) clears the selection.
              if (e.target === e.currentTarget) setSelectedTable(null);
            }}
          >
            <SchemaDesignerCanvas
              record={record}
              onChange={onChange}
              importSource={importSource}
              importing={importing}
              onImportDone={() => {
                setImporting(false);
                setImportSource(null);
                setDirty(true);
              }}
            />
          </div>
          <aside className="w-80 shrink-0 border-l border-border">
            <TableSidePanel
              data={selectedData}
              onRename={(n) => updateSelectedTable((d) => ({ ...d, name: n }))}
              onRenameColumn={(i, name) =>
                updateSelectedTable((d) => ({
                  ...d,
                  columns: d.columns.map((c, idx) => (idx === i ? { ...c, name } : c)),
                }))
              }
              onChangeType={(i, type) =>
                updateSelectedTable((d) => ({
                  ...d,
                  columns: d.columns.map((c, idx) => (idx === i ? { ...c, type } : c)),
                }))
              }
              onToggleNullable={(i) =>
                updateSelectedTable((d) => ({
                  ...d,
                  columns: d.columns.map((c, idx) =>
                    idx === i ? { ...c, nullable: !c.nullable } : c,
                  ),
                }))
              }
              onTogglePk={(i) =>
                updateSelectedTable((d) => ({
                  ...d,
                  columns: d.columns.map((c, idx) => (idx === i ? { ...c, pk: !c.pk } : c)),
                }))
              }
              onDeleteTable={onDeleteTable}
            />
          </aside>
        </div>
      )}

      <MigrateDialog
        open={migrateOpen}
        onOpenChange={setMigrateOpen}
        schema={record}
        onApplied={() => setDirty(false)}
      />

      {/* schemaNames is loaded to warn on save-name collisions (v1: silent overwrite, like upsert) */}
      {schemaNames.length > 0 && saveName && schemaNames.includes(saveName) && saveName !== name && (
        <div className="border-t border-amber-500/30 bg-amber-500/10 px-3 py-1 text-[11px] text-amber-600">
          A schema named "{saveName}" already exists — saving will overwrite it.
        </div>
      )}
    </section>
  );
}
