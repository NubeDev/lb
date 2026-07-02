// NubeIO-js editor — three submenu UIs, each a TabShell (pinned "All" index +
// closeable item tabs, open/active persisted per submenu):
//
//   Components (`jsComponents`) — jsLogic instances: assign a script, edit its
//       source inline, live `log`, Save & Assign. Shows how many components share
//       the open script (saving hot-swaps them all).
//   Scripts (`jsScripts`)      — the script library itself: edit source directly,
//       create (seeds template). Editing a script affects every component using it.
//   Examples (`jsExamples`)    — read-only example library (getExamples) to copy.
//
// Unsaved source is cached per scriptId (scriptDraft) so switching submenus (e.g.
// to copy from Examples and back) never loses edits.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { javascript } from "@codemirror/lang-javascript";
import { oneDark } from "@codemirror/theme-one-dark";
import { Save, RefreshCw, Trash2, Plus, Locate, BookOpen, Copy, FileCode2 } from "lucide-react";
import { useStructural, useValues } from "../lib/store";
import type { Component, FlexValue } from "../lib/engine-types";
import { getNodeByUid, getRootNodes } from "../lib/rest";
import { resolveJsStore } from "./jsScriptStore";
import { loadJsApiRaw, loadExamples, type JsSymbol, type JsExample } from "./jsApi";
import { tsEditorExtensions } from "./tsExtensions";
import { getDraft, hasDraft, setDraft, clearDraft } from "./scriptDraft";
import { TabShell, shellIconBtn } from "./TabShell";
import { registerWidget, type WidgetProps } from "./registry";
import type { RenderCtx } from "./registry";

const MAX_LOG_LINES = 500;
const asStr = (v: unknown): string => (typeof v === "string" ? v : v == null ? "" : String(v));
const splitIds = (s: string): string[] => s.split(",").map((x) => x.trim()).filter(Boolean);
const errMsg = (e: unknown): string => (e instanceof Error ? e.message : String(e));

// The leading comment block of a script (a run of "//" lines, or a top block
// comment) joined into one line — used as the description on the Scripts page.
function topComment(src: string): string {
  const lines = src.split("\n");
  let i = 0;
  while (i < lines.length && lines[i].trim() === "") i++;
  const out: string[] = [];
  if ((lines[i]?.trim() ?? "").startsWith("/*")) {
    for (; i < lines.length; i++) {
      const cleaned = lines[i].replace(/^\s*\/\*+/, "").replace(/\*+\/\s*$/, "").replace(/^\s*\*+\s?/, "").trim();
      if (cleaned) out.push(cleaned);
      if (lines[i].includes("*/")) break;
    }
  } else {
    for (; i < lines.length; i++) {
      const t = lines[i].trim();
      if (t.startsWith("//")) out.push(t.replace(/^\/+\s?/, ""));
      else break;
    }
  }
  return out.join(" ").trim();
}

interface JsRow { uid: number; name: string; path?: string; scriptId: string }

async function loadJsLogic(fullType: string, scriptIdProp: string): Promise<JsRow[]> {
  const r = await getRootNodes({ type: fullType, values: true });
  return r.nodes.map((c) => ({ uid: c.uid, name: c.name ?? `#${c.uid}`, path: c.path, scriptId: asStr(c.properties?.[scriptIdProp]?.value) }));
}

// Descriptor config (defaults = NubeIO-js manifest), shared by the three panels.
function readCfg(node: WidgetProps["node"]) {
  return {
    fullType: (node.fullType as string) ?? "NubeIO-js::jsLogic",
    serviceType: (node.serviceType as string) ?? "jsScriptStore",
    loadAction: (node.loadAction as string) ?? "getScript",
    saveAction: node.action?.name ?? "putScript",
    sourceKey: (node.sourceKey as string) ?? "source",
    scriptIdProp: (node.scriptIdProp as string) ?? "scriptId",
    scriptIdParam: (node.scriptIdParam as string) ?? "scriptId",
    scriptIdSetAction: (node.scriptIdSetAction as string) ?? "setScript",
    availProp: (node.availableScriptsProp as string) ?? "availableScripts",
    listAction: (node.listAction as string) ?? "listScripts",
    apiAction: (node.apiAction as string) ?? "getApi",
    exampleAction: (node.exampleAction as string) ?? "getExamples",
    logProp: node.bind?.prop ?? "log",
  };
}
type Cfg = ReturnType<typeof readCfg>;

// --- shared hooks ----------------------------------------------------------

function useJsService(serviceType: string) {
  const [service, setService] = useState<Component | null>(null);
  const [serviceState, setServiceState] = useState<"loading" | "ok" | "missing">("loading");
  useEffect(() => {
    let alive = true; setServiceState("loading");
    resolveJsStore(serviceType)
      .then((svc) => { if (!alive) return; if (svc) { setService(svc); setServiceState("ok"); } else setServiceState("missing"); })
      .catch(() => alive && setServiceState("missing"));
    return () => { alive = false; };
  }, [serviceType]);
  return { service, serviceUid: service?.uid ?? null, serviceState };
}

function useJsLogicRows(fullType: string, scriptIdProp: string) {
  const [rows, setRows] = useState<JsRow[]>([]);
  useEffect(() => {
    let alive = true;
    const tick = () => loadJsLogic(fullType, scriptIdProp).then((r) => { if (alive) setRows(r); }).catch(() => {});
    tick(); const id = setInterval(tick, 4000);
    return () => { alive = false; clearInterval(id); };
  }, [fullType, scriptIdProp]);
  return rows;
}

function useJsExtensions(serviceUid: number | null, ctxRef: React.MutableRefObject<RenderCtx>, apiAction: string) {
  const symbolsRef = useRef<JsSymbol[]>([]);
  const dtsRef = useRef<string>("");
  useEffect(() => {
    const call = ctxRef.current.callAction;
    if (serviceUid == null || !call) return;
    let alive = true;
    loadJsApiRaw(call, serviceUid, apiAction).then((r) => { if (alive) { symbolsRef.current = r.symbols; dtsRef.current = r.dts; } }).catch(() => {});
    return () => { alive = false; };
  }, [serviceUid, apiAction]);
  // Full TS service (autocomplete + hover + lint), falling back to the symbol
  // source until the dts loads. Refs keep the extension list stable.
  return useMemo(() => [javascript(), ...tsEditorExtensions(dtsRef, symbolsRef)], []);
}

// Source state for one scriptId: load (draft-aware), edit (caches a draft so it
// survives remounts), save. Returns helpers shared by both source editors.
function useScriptSource(scriptId: string, serviceUid: number | null, ctxRef: React.MutableRefObject<RenderCtx>, c: Cfg, log?: (s: string) => void) {
  const [code, setCode] = useState("");
  const [dirty, setDirty] = useState(false);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const loadToken = useRef(0);

  const doLoad = useCallback((force: boolean) => {
    const call = ctxRef.current.callAction;
    if (serviceUid == null || !call || scriptId === "") { setCode(""); setDirty(false); return; }
    if (!force && hasDraft(scriptId)) { setCode(getDraft(scriptId) ?? ""); setDirty(true); return; }
    const token = ++loadToken.current; setLoading(true);
    call(serviceUid, c.loadAction, { [c.scriptIdParam]: scriptId })
      .then((ret) => {
        if (token !== loadToken.current) return;
        const err = asStr(ret?.error);
        if (err) { log?.(`[load] ${scriptId}: ${err}`); }
        else { const src = asStr(ret?.[c.sourceKey]); setCode(src); setDirty(false); clearDraft(scriptId); log?.(`[load] ${scriptId} (${src.length} chars)`); }
        setLoading(false);
      })
      .catch((e: unknown) => { if (token !== loadToken.current) return; log?.(`[load] ${scriptId}: ${errMsg(e)}`); setLoading(false); });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [serviceUid, scriptId, c.loadAction, c.scriptIdParam, c.sourceKey]);
  useEffect(() => { doLoad(false); return () => { loadToken.current++; }; }, [doLoad]);

  const onChange = useCallback((v: string) => { setCode(v); setDirty(true); setDraft(scriptId, v); }, [scriptId]);

  const saveSource = useCallback(async (): Promise<boolean> => {
    const call = ctxRef.current.callAction;
    if (serviceUid == null || !call || scriptId === "") return false;
    setSaving(true);
    try {
      log?.(`[save] putScript ${scriptId}…`);
      const ret = await call(serviceUid, c.saveAction, { [c.scriptIdParam]: scriptId, [c.sourceKey]: code as FlexValue });
      const ok = ret?.ok === true || asStr(ret?.ok) === "true";
      const e = asStr(ret?.error);
      if (!ok || e) { log?.(`[save] ${scriptId}: ${e || "rejected"}`); return false; }
      setDirty(false); clearDraft(scriptId); log?.(`[save] ${scriptId}: ok`); return true;
    } catch (e) { log?.(`[save] ${scriptId}: ${errMsg(e)}`); return false; }
    finally { setSaving(false); }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [serviceUid, scriptId, c.saveAction, c.scriptIdParam, c.sourceKey, code]);

  return { code, dirty, loading, saving, onChange, reload: () => doLoad(true), saveSource };
}

// Ctrl/Cmd+S → save (not the browser dialog), only on the active tab.
function useSaveShortcut(active: boolean, save: () => void) {
  const ref = useRef(save); ref.current = save;
  useEffect(() => {
    if (!active) return;
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") { e.preventDefault(); e.stopPropagation(); ref.current(); }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [active]);
}

const folderOf = (path?: string) => (path ? path.replace(/^root\/?/, "").split("/").slice(0, -1).join("/") : "");

// "used by N components" — clarifies that editing a shared script affects all of
// them (amber when >1). Click to expand the list and Locate each on the canvas.
function SharingBadge({ rows, scriptId, onLocate }: { rows: JsRow[]; scriptId: string; onLocate?: (uid: number) => void }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => { if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false); };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);
  if (!scriptId) return null;
  const users = rows.filter((r) => r.scriptId === scriptId);
  if (users.length === 0) return null;
  const multi = users.length > 1;
  return (
    <span ref={ref} style={{ position: "relative" }}>
      <button
        onClick={() => setOpen((o) => !o)}
        title="Components using this script"
        style={{ display: "inline-flex", alignItems: "center", gap: 3, fontSize: 11, padding: "1px 6px", borderRadius: 10, cursor: "pointer", whiteSpace: "nowrap", background: multi ? "hsl(var(--amber) / 0.18)" : "transparent", color: multi ? "hsl(var(--amber))" : "hsl(var(--green))", border: `1px solid ${multi ? "hsl(var(--amber) / 0.4)" : "hsl(var(--green) / 0.25)"}` }}
      >
        used by {users.length}{multi ? " ⚠" : ""} <span style={{ fontSize: 9, opacity: 0.7 }}>▾</span>
      </button>
      {open && (
        <div style={{ position: "absolute", top: "100%", left: 0, marginTop: 4, zIndex: 50, minWidth: 200, maxHeight: 240, overflow: "auto", background: "hsl(var(--card))", border: "1px solid hsl(var(--border))", borderRadius: 6, boxShadow: "0 6px 20px rgba(0,0,0,0.4)", padding: 4 }}>
          <div style={{ color: "hsl(var(--muted-foreground))", fontSize: 10, padding: "2px 6px 4px", textTransform: "uppercase", letterSpacing: 0.4 }}>running this script</div>
          {users.map((u) => (
            <div key={u.uid} style={{ display: "flex", alignItems: "center", gap: 8, padding: "5px 6px", fontSize: 12, color: "hsl(var(--foreground))", borderRadius: 4 }}>
              <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{u.name}</span>
              {folderOf(u.path) && <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10 }}>{folderOf(u.path)}</span>}
              <span style={{ flex: 1 }} />
              {onLocate && (
                <button onClick={() => { onLocate(u.uid); setOpen(false); }} title="Locate on canvas" style={{ display: "inline-flex", alignItems: "center", gap: 3, fontSize: 10, color: "hsl(var(--muted-foreground))", background: "hsl(var(--border))", border: "1px solid hsl(var(--input))", borderRadius: 3, padding: "2px 6px", cursor: "pointer" }}>
                  <Locate size={11} /> Locate
                </button>
              )}
            </div>
          ))}
        </div>
      )}
    </span>
  );
}

// --- Components submenu -----------------------------------------------------

function JsComponentsPanel({ node, ctx }: WidgetProps) {
  const c = readCfg(node);
  const { service, serviceState } = useJsService(c.serviceType);
  const rows = useJsLogicRows(c.fullType, c.scriptIdProp);
  const dirtyRef = useRef<Record<string, boolean>>({});
  return (
    <TabShell
      persistKey="js:components"
      openRequest={ctx.focusUid != null ? { id: String(ctx.focusUid), nonce: ctx.focusNonce } : undefined}
      pinned={{ id: "index", label: "All", icon: <FileCode2 size={13} /> }}
      tabLabel={(id) => rows.find((r) => r.uid === Number(id))?.name ?? `#${id}`}
      tabExtra={(id) => ctx.locate ? (
        <span role="button" title="Locate on canvas" onClick={(e) => { e.stopPropagation(); ctx.locate!(Number(id)); }} style={shellIconBtn}><Locate size={12} /></span>
      ) : null}
      closeGuard={(id) => !dirtyRef.current[id] || window.confirm("Discard unsaved changes to this script?")}
      renderIndex={(open) => <ComponentsIndex rows={rows} serviceState={serviceState} onOpen={(uid) => open(String(uid))} />}
      renderTab={(id, active) => (
        <JsScriptEditor componentUid={Number(id)} active={active} cfg={c} ctx={ctx} service={service} serviceState={serviceState} rows={rows} onDirty={(d) => { dirtyRef.current[id] = d; }} />
      )}
    />
  );
}

function ComponentsIndex({ rows, serviceState, onOpen }: { rows: JsRow[]; serviceState: string; onOpen: (uid: number) => void }) {
  return (
    <div style={{ padding: 8, display: "flex", flexDirection: "column", gap: 4 }}>
      <IndexHeader label="jsLogic components" count={rows.length} serviceState={serviceState} />
      {rows.length === 0 ? <Empty>No jsLogic components on this engine.</Empty> : rows.map((r) => (
        <Row key={r.uid} onClick={() => onOpen(r.uid)}>
          <span style={{ fontWeight: 500, flexShrink: 0 }}>{r.name}</span>
          {r.path && <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{r.path}</span>}
          <span style={{ flex: 1 }} />
          <span style={{ color: r.scriptId ? "hsl(var(--cool))" : "hsl(var(--amber) / 0.4)", fontSize: 11, fontFamily: "var(--font-mono)", flexShrink: 0 }}>{r.scriptId || "no script"}</span>
        </Row>
      ))}
    </div>
  );
}

// --- Scripts submenu --------------------------------------------------------

function JsScriptsPanel({ node, ctx }: WidgetProps) {
  const c = readCfg(node);
  const { serviceUid, serviceState } = useJsService(c.serviceType);
  const rows = useJsLogicRows(c.fullType, c.scriptIdProp); // for the "used by N" column
  const ctxRef = useRef(ctx); ctxRef.current = ctx;
  const dirtyRef = useRef<Record<string, boolean>>({});

  const [scripts, setScripts] = useState<string[]>([]);
  const [localIds, setLocalIds] = useState<string[]>([]);
  useEffect(() => {
    const call = ctxRef.current.callAction;
    if (serviceUid == null || !call) return;
    let alive = true;
    const tick = () => call(serviceUid, c.listAction, {}).then((r) => { if (alive) setScripts(splitIds(asStr(r?.ids))); }).catch(() => {});
    tick(); const id = setInterval(tick, 4000);
    return () => { alive = false; clearInterval(id); };
  }, [serviceUid, c.listAction]);
  const all = useMemo(() => Array.from(new Set([...scripts, ...localIds])).sort(), [scripts, localIds]);

  // Descriptions = each script's leading comment. Fetched lazily once per id
  // (a getScript per script); may be stale after an edit until reopened.
  const [descs, setDescs] = useState<Record<string, string>>({});
  const fetchedRef = useRef<Set<string>>(new Set());
  useEffect(() => {
    const call = ctxRef.current.callAction;
    if (serviceUid == null || !call) return;
    let alive = true;
    for (const id of all) {
      if (fetchedRef.current.has(id)) continue;
      fetchedRef.current.add(id);
      call(serviceUid, c.loadAction, { [c.scriptIdParam]: id })
        .then((r) => { if (alive) setDescs((p) => ({ ...p, [id]: topComment(asStr(r?.[c.sourceKey])) })); })
        .catch(() => { fetchedRef.current.delete(id); });
    }
    return () => { alive = false; };
  }, [all, serviceUid, c.loadAction, c.scriptIdParam, c.sourceKey]);

  const create = useCallback((id: string, open: (id: string) => void) => {
    const call = ctxRef.current.callAction;
    if (serviceUid == null || !call || !id) return;
    call(serviceUid, c.saveAction, { [c.scriptIdParam]: id, [c.sourceKey]: "" as FlexValue })
      .then(() => { setLocalIds((p) => (p.includes(id) ? p : [...p, id])); open(id); })
      .catch(() => {});
  }, [serviceUid, c.saveAction, c.scriptIdParam, c.sourceKey]);

  return (
    <TabShell
      persistKey="js:scripts"
      pinned={{ id: "index", label: "All", icon: <FileCode2 size={13} /> }}
      tabLabel={(id) => id}
      closeGuard={(id) => !dirtyRef.current[id] || window.confirm("Discard unsaved changes to this script?")}
      renderIndex={(open) => <ScriptsIndex scripts={all} rows={rows} descs={descs} serviceState={serviceState} onOpen={open} onCreate={(id) => create(id, open)} />}
      renderTab={(id, active) => (
        <ScriptEditor scriptId={id} active={active} cfg={c} ctx={ctx} serviceUid={serviceUid} serviceState={serviceState} rows={rows} onDirty={(d) => { dirtyRef.current[id] = d; }} />
      )}
    />
  );
}

function ScriptsIndex({ scripts, rows, descs, serviceState, onOpen, onCreate }: {
  scripts: string[]; rows: JsRow[]; descs: Record<string, string>; serviceState: string; onOpen: (id: string) => void; onCreate: (id: string) => void;
}) {
  const [creating, setCreating] = useState(false);
  const [newId, setNewId] = useState("");
  const submit = () => { const id = newId.trim(); if (id) { onCreate(id); setNewId(""); setCreating(false); } };
  return (
    <div style={{ padding: 8, display: "flex", flexDirection: "column", gap: 4 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 6, padding: "2px 4px 6px" }}>
        <span style={{ fontSize: 12, fontWeight: 600 }}>scripts</span>
        <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 11 }}>({scripts.length})</span>
        {serviceState === "missing" && <span style={{ color: "hsl(var(--crit))", fontSize: 11 }}>jsScriptStore not found</span>}
        <span style={{ flex: 1 }} />
        {creating ? (
          <span style={{ display: "flex", gap: 4 }}>
            <input autoFocus value={newId} onChange={(e) => setNewId(e.target.value)} placeholder="new script id"
              onKeyDown={(e) => { if (e.key === "Enter") submit(); if (e.key === "Escape") { setCreating(false); setNewId(""); } }} style={{ ...inp, width: 140 }} />
            <button onClick={submit} disabled={!newId.trim()} style={{ ...btn, ...btnPrimary, opacity: newId.trim() ? 1 : 0.45 }}>Create</button>
            <button onClick={() => { setCreating(false); setNewId(""); }} style={btn}>Cancel</button>
          </span>
        ) : (
          <button onClick={() => setCreating(true)} disabled={serviceState !== "ok"} style={btn}><Plus size={13} /> New</button>
        )}
      </div>
      {scripts.length === 0 ? <Empty>No scripts yet. Create one above.</Empty> : scripts.map((id) => {
        const users = rows.filter((r) => r.scriptId === id).length;
        return (
          <Row key={id} onClick={() => onOpen(id)}>
            <FileCode2 size={13} style={{ flexShrink: 0, color: "hsl(var(--muted-foreground))" }} />
            <span style={{ fontWeight: 500, fontFamily: "var(--font-mono)", flexShrink: 0 }}>{id}</span>
            {descs[id] && <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 11, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{descs[id]}</span>}
            <span style={{ flex: 1 }} />
            <span style={{ color: users > 1 ? "hsl(var(--amber))" : "hsl(var(--muted-foreground))", fontSize: 10, flexShrink: 0 }}>{users === 0 ? "unused" : `used by ${users}`}</span>
          </Row>
        );
      })}
    </div>
  );
}

// Source-only editor for the Scripts submenu (no component / log / assign).
function ScriptEditor({ scriptId, active, cfg: c, ctx, serviceUid, serviceState, rows, onDirty }: {
  scriptId: string; active: boolean; cfg: Cfg; ctx: RenderCtx; serviceUid: number | null; serviceState: string; rows: JsRow[]; onDirty: (d: boolean) => void;
}) {
  const ctxRef = useRef(ctx); ctxRef.current = ctx;
  const { code, dirty, loading, saving, onChange, reload, saveSource } = useScriptSource(scriptId, serviceUid, ctxRef, c);
  useEffect(() => { onDirty(dirty); }, [dirty, onDirty]);
  const cmExtensions = useJsExtensions(serviceUid, ctxRef, c.apiAction);
  const canSave = serviceUid != null && !!ctx.callAction && !saving;
  useSaveShortcut(active, () => { void saveSource(); });
  const shared = rows.filter((r) => r.scriptId === scriptId).length;

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0, color: "hsl(var(--foreground))" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "6px 10px", borderBottom: "1px solid hsl(var(--border))", flexShrink: 0, flexWrap: "wrap" }}>
        <FileCode2 size={13} style={{ color: "hsl(var(--muted-foreground))" }} />
        <span style={{ fontSize: 12, fontWeight: 600, fontFamily: "var(--font-mono)" }}>{scriptId}</span>
        <SharingBadge rows={rows} scriptId={scriptId} onLocate={ctx.locate} />
        {serviceState === "missing" && <span style={{ color: "hsl(var(--crit))", fontSize: 11 }}>jsScriptStore not found</span>}
        {loading && <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 11 }}>loading…</span>}
        {dirty && <span style={{ color: "hsl(var(--amber))", fontSize: 11 }}>● unsaved</span>}
        <span style={{ flex: 1 }} />
        <button onClick={reload} disabled={loading} style={{ ...btn, opacity: loading ? 0.5 : 1 }}><RefreshCw size={13} /> Reload</button>
        <button onClick={() => void saveSource()} disabled={!canSave} title={shared > 1 ? `Updates this script for all ${shared} components using it` : "Save the script source"} style={{ ...btn, ...btnPrimary, opacity: !canSave ? 0.45 : 1 }}>
          <Save size={13} /> {saving ? "Saving…" : "Save"}
        </button>
      </div>
      {shared > 1 && (
        <div style={{ padding: "3px 10px", background: "hsl(var(--amber) / 0.18)", color: "hsl(var(--amber))", fontSize: 11, borderBottom: "1px solid hsl(var(--border))", flexShrink: 0 }}>
          ⚠ Editing this script updates all {shared} components running it.
        </div>
      )}
      <div style={{ flex: 1, minHeight: 0, overflow: "hidden" }}>
        <CodeMirror value={code} height="100%" theme={oneDark} extensions={cmExtensions} onChange={onChange} style={{ height: "100%", fontSize: 13 }} />
      </div>
    </div>
  );
}

// --- Examples submenu -------------------------------------------------------

function JsExamplesPanel({ node, ctx }: WidgetProps) {
  const c = readCfg(node);
  const { serviceUid, serviceState } = useJsService(c.serviceType);
  const ctxRef = useRef(ctx); ctxRef.current = ctx;
  const [examples, setExamples] = useState<JsExample[]>([]);
  useEffect(() => {
    const call = ctxRef.current.callAction;
    if (serviceUid == null || !call) return;
    let alive = true;
    loadExamples(call, serviceUid, c.exampleAction).then((e) => { if (alive) setExamples(e); }).catch(() => {});
    return () => { alive = false; };
  }, [serviceUid, c.exampleAction]);

  return (
    <TabShell
      persistKey="js:examples"
      pinned={{ id: "index", label: "All", icon: <BookOpen size={13} /> }}
      tabLabel={(id) => id}
      renderIndex={(open) => <ExamplesIndex examples={examples} serviceState={serviceState} onOpen={open} />}
      renderTab={(id) => {
        const ex = examples.find((e) => e.label === id);
        return ex ? <ExampleViewer ex={ex} /> : <div style={{ padding: 12, color: "hsl(var(--muted-foreground))", fontSize: 12 }}>example not found</div>;
      }}
    />
  );
}

function ExamplesIndex({ examples, serviceState, onOpen }: { examples: JsExample[]; serviceState: string; onOpen: (label: string) => void }) {
  return (
    <div style={{ padding: 8, display: "flex", flexDirection: "column", gap: 4 }}>
      <IndexHeader label="example scripts" count={examples.length} serviceState={serviceState} />
      {examples.length === 0 ? <Empty>No examples available.</Empty> : examples.map((ex) => (
        <Row key={ex.label} onClick={() => onOpen(ex.label)}>
          <BookOpen size={13} style={{ flexShrink: 0, color: "hsl(var(--muted-foreground))" }} />
          <span style={{ fontWeight: 500, flexShrink: 0 }}>{ex.label}</span>
          {ex.desc && <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 11, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{ex.desc}</span>}
        </Row>
      ))}
    </div>
  );
}

function ExampleViewer({ ex }: { ex: JsExample }) {
  const [copied, setCopied] = useState(false);
  const extensions = useMemo(() => [javascript()], []);
  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0, color: "hsl(var(--foreground))" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "6px 10px", borderBottom: "1px solid hsl(var(--border))", flexShrink: 0 }}>
        <BookOpen size={13} style={{ color: "hsl(var(--muted-foreground))" }} />
        <span style={{ fontSize: 12, fontWeight: 600 }}>{ex.label}</span>
        {ex.desc && <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 11, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{ex.desc}</span>}
        <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10 }}>read-only — select to copy</span>
        <span style={{ flex: 1 }} />
        <button onClick={() => { void navigator.clipboard?.writeText(ex.source).then(() => { setCopied(true); window.setTimeout(() => setCopied(false), 1200); }).catch(() => {}); }} style={{ ...btn, ...(copied ? btnPrimary : {}) }}>
          <Copy size={13} /> {copied ? "Copied" : "Copy all"}
        </button>
      </div>
      <div style={{ flex: 1, minHeight: 0, overflow: "hidden" }}>
        <CodeMirror value={ex.source} height="100%" theme={oneDark} editable={false} extensions={extensions} style={{ height: "100%", fontSize: 13 }} />
      </div>
    </div>
  );
}

// --- Per-component editor (Components submenu) ------------------------------

function JsScriptEditor({ componentUid, active, cfg: c, ctx, service, serviceState, rows, onDirty }: {
  componentUid: number; active: boolean; cfg: Cfg; ctx: RenderCtx; service: Component | null; serviceState: "loading" | "ok" | "missing"; rows: JsRow[]; onDirty: (d: boolean) => void;
}) {
  const ctxRef = useRef(ctx); ctxRef.current = ctx;

  // Component (fetched if off-folder) for its prop uids.
  const structComp = useStructural((s) => s.components.get(componentUid));
  const [fetched, setFetched] = useState<Component | null>(null);
  useEffect(() => {
    if (structComp) return;
    let alive = true;
    getNodeByUid(componentUid, { depth: 0 }).then((r) => { if (alive) setFetched(r.nodes[0] ?? null); }).catch(() => {});
    return () => { alive = false; };
  }, [componentUid, structComp]);
  const comp = structComp ?? fetched;

  const serviceUid = service?.uid ?? null;
  const scriptIdUid = comp?.properties[c.scriptIdProp]?.uid;
  const logUid = comp?.properties[c.logProp]?.uid;
  const availUid = service?.properties[c.availProp]?.uid;

  useEffect(() => {
    if (!active) return;
    const sub = ctxRef.current.subscribeProps;
    if (!sub) return;
    const uids = [scriptIdUid, logUid, availUid].filter((x): x is number => x != null);
    if (uids.length === 0) return;
    return sub(uids);
  }, [active, scriptIdUid, logUid, availUid]);

  const liveScriptId = useValues((s) => (scriptIdUid != null ? s.values.get(scriptIdUid) : undefined));
  const [optimisticBound, setOptimisticBound] = useState<string | null>(null);
  const streamedBound = asStr(liveScriptId) || asStr(comp?.properties[c.scriptIdProp]?.value);
  const boundId = optimisticBound ?? streamedBound;
  useEffect(() => { if (optimisticBound != null && streamedBound === optimisticBound) setOptimisticBound(null); }, [streamedBound, optimisticBound]);

  const [editingId, setEditingId] = useState<string | null>(null);
  const scriptId = editingId ?? boundId;
  const assigned = scriptId !== "" && scriptId === boundId;

  const liveAvail = useValues((s) => (availUid != null ? s.values.get(availUid) : undefined));
  const [listFallback, setListFallback] = useState<string[]>([]);
  useEffect(() => {
    const call = ctxRef.current.callAction;
    if (!active || serviceUid == null || !call || availUid != null) return;
    let alive = true;
    call(serviceUid, c.listAction, {}).then((r) => { if (alive) setListFallback(splitIds(asStr(r?.ids))); }).catch(() => {});
    return () => { alive = false; };
  }, [active, serviceUid, availUid, c.listAction]);
  const [localIds, setLocalIds] = useState<string[]>([]);
  const available = useMemo(() => {
    const fromProp = splitIds(asStr(liveAvail) || asStr(service?.properties[c.availProp]?.value));
    const base = fromProp.length ? fromProp : listFallback;
    return Array.from(new Set([...base, ...localIds])).sort();
  }, [liveAvail, service, c.availProp, listFallback, localIds]);

  const cmExtensions = useJsExtensions(serviceUid, ctxRef, c.apiAction);

  const [logLines, setLogLines] = useState<string[]>([]);
  const pushLog = useCallback((line: string) => {
    setLogLines((prev) => { const next = prev.concat(line.split("\n")); return next.length > MAX_LOG_LINES ? next.slice(next.length - MAX_LOG_LINES) : next; });
  }, []);

  const { code, dirty, loading, saving, onChange, reload, saveSource } = useScriptSource(scriptId, serviceUid, ctxRef, c, pushLog);
  useEffect(() => { onDirty(dirty); }, [dirty, onDirty]);

  const ready = serviceUid != null && !!ctx.callAction;
  const canEdit = ready && scriptId !== "";
  const canSave = canEdit && !saving;

  const assign = useCallback((id: string): Promise<boolean> => {
    const call = ctxRef.current.callAction;
    if (!call || id === "") return Promise.resolve(false);
    pushLog(`[assign] setScript ${id}…`);
    return call(componentUid, c.scriptIdSetAction, { [c.scriptIdParam]: id })
      .then((ret) => {
        const err = asStr(ret?.error);
        if (err) { pushLog(`[assign] ${id}: ${err}`); return false; }
        setOptimisticBound(id); setEditingId(null); pushLog(`[assign] ${id}: ok`); return true;
      })
      .catch((e: unknown) => { pushLog(`[assign] ${id}: ${errMsg(e)}`); return false; });
  }, [componentUid, c.scriptIdSetAction, c.scriptIdParam, pushLog]);

  const save = useCallback(async () => {
    if (!canSave) return;
    const id = scriptId;
    if (dirty) { const ok = await saveSource(); if (!ok) return; }
    await assign(id);
  }, [canSave, scriptId, dirty, saveSource, assign]);
  useSaveShortcut(active, () => { void save(); });

  const [creating, setCreating] = useState(false);
  const [newId, setNewId] = useState("");
  const createScript = () => {
    const id = newId.trim();
    const call = ctxRef.current.callAction;
    if (serviceUid == null || !call || id === "") return;
    call(serviceUid, c.saveAction, { [c.scriptIdParam]: id, [c.sourceKey]: "" as FlexValue })
      .then((ret) => {
        const err = asStr(ret?.error);
        if (err) { pushLog(`[create] ${id}: ${err}`); return; }
        setCreating(false); setNewId("");
        setLocalIds((prev) => (prev.includes(id) ? prev : [...prev, id]));
        setEditingId(id);
      })
      .catch((e: unknown) => pushLog(`[create] ${id}: ${errMsg(e)}`));
  };

  const liveLog = useValues((s) => (logUid != null ? s.values.get(logUid) : undefined));
  const lastLive = useRef<string | null>(null);
  useEffect(() => { if (liveLog == null) return; const s = asStr(liveLog); if (s === lastLive.current) return; lastLive.current = s; if (s) pushLog(s); }, [liveLog, pushLog]);
  const logRef = useRef<HTMLDivElement>(null);
  useEffect(() => { const el = logRef.current; if (el) el.scrollTop = el.scrollHeight; }, [logLines]);

  const shared = rows.filter((r) => r.scriptId === scriptId).length;

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0, color: "hsl(var(--foreground))" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "6px 10px", borderBottom: "1px solid hsl(var(--border))", flexShrink: 0, flexWrap: "wrap" }}>
        <span style={{ fontSize: 12, fontWeight: 600 }}>{comp?.name ?? `#${componentUid}`}</span>
        <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 11 }}>script</span>
        <select value={scriptId || ""} onChange={(e) => setEditingId(e.target.value)} disabled={!ready} style={inp}>
          {scriptId === "" && <option value="" disabled>— select —</option>}
          {Array.from(new Set([scriptId, ...available].filter(Boolean))).map((id) => <option key={id} value={id}>{id}</option>)}
        </select>
        <SharingBadge rows={rows} scriptId={scriptId} onLocate={ctx.locate} />
        {creating ? (
          <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
            <input autoFocus value={newId} onChange={(e) => setNewId(e.target.value)} placeholder="new script id"
              onKeyDown={(e) => { if (e.key === "Enter") createScript(); if (e.key === "Escape") { setCreating(false); setNewId(""); } }} style={{ ...inp, width: 140 }} />
            <button onClick={createScript} disabled={!newId.trim()} style={{ ...btn, ...btnPrimary, opacity: newId.trim() ? 1 : 0.45 }}>Create</button>
            <button onClick={() => { setCreating(false); setNewId(""); }} style={btn}>Cancel</button>
          </span>
        ) : (
          <button onClick={() => setCreating(true)} disabled={!ready} title="Create a new script" style={btn}><Plus size={13} /> New</button>
        )}
        {serviceState === "missing" && <span style={{ color: "hsl(var(--crit))", fontSize: 11 }}>jsScriptStore not found</span>}
        {loading && <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 11 }}>loading…</span>}
        {scriptId !== "" && (assigned ? <span style={{ color: "hsl(var(--green))", fontSize: 11 }}>● assigned</span> : <span style={{ color: "hsl(var(--amber))", fontSize: 11 }}>not assigned — Save to assign</span>)}
        {dirty && <span style={{ color: "hsl(var(--amber))", fontSize: 11 }}>● unsaved</span>}
        <span style={{ flex: 1 }} />
        <button onClick={reload} disabled={!canEdit || loading} style={{ ...btn, opacity: !canEdit || loading ? 0.45 : 1 }}><RefreshCw size={13} /> Reload</button>
        <button onClick={save} disabled={!canSave} title={shared > 1 ? `Saves the source (updates all ${shared} components using it) and assigns it here` : "Save the source and assign this script to the component"} style={{ ...btn, ...btnPrimary, opacity: !canSave ? 0.45 : 1 }}>
          <Save size={13} /> {saving ? "Saving…" : assigned ? "Save" : "Save & Assign"}
        </button>
      </div>
      {shared > 1 && scriptId !== "" && (
        <div style={{ padding: "3px 10px", background: "hsl(var(--amber) / 0.18)", color: "hsl(var(--amber))", fontSize: 11, borderBottom: "1px solid hsl(var(--border))", flexShrink: 0 }}>
          ⚠ Script <b>{scriptId}</b> runs on {shared} components — saving updates them all.
        </div>
      )}
      <div style={{ flex: 1, minHeight: 0, overflow: "hidden" }}>
        {scriptId === "" ? (
          <div style={{ padding: 16, color: "hsl(var(--muted-foreground))", fontSize: 12 }}>
            No script open. Pick one from the dropdown above, or{" "}
            <button onClick={() => setCreating(true)} style={{ ...btn, display: "inline-flex", padding: "2px 8px" }}>create a new script</button>.
            {boundId === "" && <span> <b style={{ color: "hsl(var(--foreground))" }}>{comp?.name ?? `#${componentUid}`}</b> has no script assigned yet.</span>}
          </div>
        ) : (
          <CodeMirror value={code} height="100%" theme={oneDark} extensions={cmExtensions} onChange={onChange} style={{ height: "100%", fontSize: 13 }} />
        )}
      </div>
      <div style={{ flexShrink: 0, borderTop: "1px solid hsl(var(--border))", display: "flex", flexDirection: "column", height: 160 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 6, padding: "4px 10px", borderBottom: "1px solid hsl(var(--secondary))", flexShrink: 0 }}>
          <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10, textTransform: "uppercase", letterSpacing: 0.4 }}>Debug log</span>
          <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10 }}>{logLines.length}</span>
          <span style={{ flex: 1 }} />
          <button onClick={() => setLogLines([])} title="Clear log" style={{ ...btn, padding: "2px 8px" }}><Trash2 size={12} /> clear</button>
        </div>
        <div ref={logRef} style={{ flex: 1, overflowY: "auto", padding: "4px 10px", fontFamily: "var(--font-mono)", fontSize: 11, lineHeight: 1.5, whiteSpace: "pre-wrap", color: "hsl(var(--foreground))", background: "hsl(var(--background))" }}>
          {logLines.length === 0 ? <span style={{ color: "hsl(var(--muted-foreground))" }}>{logUid == null ? "no log output" : "waiting for output…"}</span> : logLines.map((l, i) => <div key={i}>{l}</div>)}
        </div>
      </div>
    </div>
  );
}

// --- shared index bits + styles --------------------------------------------

function IndexHeader({ label, count, serviceState }: { label: string; count: number; serviceState: string }) {
  return (
    <div style={{ display: "flex", alignItems: "baseline", gap: 6, padding: "2px 4px 6px" }}>
      <span style={{ fontSize: 12, fontWeight: 600 }}>{label}</span>
      <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 11 }}>({count})</span>
      {serviceState === "missing" && <span style={{ color: "hsl(var(--crit))", fontSize: 11, marginLeft: 6 }}>jsScriptStore not found</span>}
    </div>
  );
}
function Empty({ children }: { children: React.ReactNode }) {
  return <div style={{ color: "hsl(var(--muted-foreground))", fontSize: 12, padding: 6 }}>{children}</div>;
}
function Row({ onClick, children }: { onClick: () => void; children: React.ReactNode }) {
  return (
    <button onClick={onClick} style={{ display: "flex", alignItems: "center", gap: 8, padding: "7px 9px", textAlign: "left", background: "hsl(var(--card))", border: "1px solid hsl(var(--border))", borderRadius: 5, cursor: "pointer", color: "hsl(var(--foreground))", fontSize: 12 }}>
      {children}
    </button>
  );
}

const inp: React.CSSProperties = { background: "hsl(var(--background))", color: "hsl(var(--foreground))", border: "1px solid hsl(var(--border))", borderRadius: 4, padding: "3px 7px", fontSize: 12, outline: "none" };
const btn: React.CSSProperties = { display: "flex", alignItems: "center", gap: 5, padding: "4px 10px", fontSize: 11, color: "hsl(var(--foreground))", background: "hsl(var(--border))", border: "1px solid hsl(var(--input))", borderRadius: 4, cursor: "pointer" };
const btnPrimary: React.CSSProperties = { background: "hsl(var(--cool) / 0.18)", borderColor: "hsl(var(--cool))", color: "hsl(var(--cool))" };

registerWidget("jsComponents", JsComponentsPanel);
registerWidget("jsScripts", JsScriptsPanel);
registerWidget("jsExamples", JsExamplesPanel);

export default JsComponentsPanel;
