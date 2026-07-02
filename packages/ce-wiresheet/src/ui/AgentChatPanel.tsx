// "Agent" extension panel: an interactive chat with a locally-running opencode
// agent that is wired (via the nubeio-mcp extension) to THIS Control Engine, so
// it can read and build the wiresheet for you in natural language.
//
// Transport lives in lib/opencode/client.ts. This widget owns the connection
// lifecycle, the message list (rebuilt live from the event stream), and the
// composer. Registered as the `agentChat` widget (see lib/ui/root-ext-stub.ts,
// imported by UiTabHost).

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Bot, Send, Plug, Square, ChevronRight } from "lucide-react";
import { registerWidget, type WidgetProps } from "./registry";
import {
  abort,
  connect,
  createSession,
  ensureEngineMcp,
  listModels,
  loadOpencodeUrl,
  replyPermission,
  saveOpencodeUrl,
  sendPrompt,
  subscribeEvents,
  type ModelOption,
  type ModelSelection,
  type OpencodeClient,
  type OpencodeEvent,
} from "../lib/opencode/client";

type Phase = "disconnected" | "connecting" | "connected" | "error";

interface ChatMsg {
  id: string;
  role: string; // "user" | "assistant"
}

interface ChatPart {
  id: string;
  messageID: string;
  kind: "text" | "reasoning" | "tool";
  text?: string;
  tool?: string;
  status?: string;
  input?: unknown;
  output?: string;
}

interface PermReq {
  id: string;
  title: string;
}

function AgentChatPanel(_props: WidgetProps) {
  void _props;
  const [url, setUrl] = useState(loadOpencodeUrl);
  const [phase, setPhase] = useState<Phase>("disconnected");
  const [error, setError] = useState<string | null>(null);

  const [models, setModels] = useState<ModelOption[]>([]);
  const [model, setModel] = useState<ModelSelection | null>(null);

  const [messages, setMessages] = useState<ChatMsg[]>([]);
  const [parts, setParts] = useState<ChatPart[]>([]);
  const [perms, setPerms] = useState<PermReq[]>([]);
  const [busy, setBusy] = useState(false);
  const [draft, setDraft] = useState("");

  const clientRef = useRef<OpencodeClient | null>(null);
  const sessionRef = useRef<string | null>(null);
  const unsubRef = useRef<(() => void) | null>(null);
  const scrollRef = useRef<HTMLDivElement | null>(null);

  // --- event stream → chat state ---------------------------------------------
  const handleEvent = useCallback((ev: OpencodeEvent) => {
    const p = ev.properties ?? {};
    const sid = sessionRef.current;
    const evSid =
      (p.sessionID as string | undefined) ??
      ((p.info as { sessionID?: string } | undefined)?.sessionID) ??
      ((p.part as { sessionID?: string } | undefined)?.sessionID);
    if (sid && evSid && evSid !== sid) return; // not our session

    switch (ev.type) {
      case "message.updated": {
        const info = p.info as { id: string; role: string } | undefined;
        if (!info?.id) break;
        setMessages((prev) =>
          prev.some((m) => m.id === info.id) ? prev : [...prev, { id: info.id, role: info.role }],
        );
        break;
      }
      case "message.part.updated": {
        const part = p.part as RawPart | undefined;
        if (!part?.id) break;
        const next = toChatPart(part);
        if (!next) break;
        setParts((prev) => {
          const i = prev.findIndex((x) => x.id === next.id);
          if (i === -1) return [...prev, next];
          const copy = prev.slice();
          copy[i] = next;
          return copy;
        });
        break;
      }
      case "message.part.removed": {
        const id = p.partID as string | undefined;
        if (id) setParts((prev) => prev.filter((x) => x.id !== id));
        break;
      }
      case "permission.updated": {
        const perm = p as unknown as { id?: string; title?: string };
        if (perm.id) {
          setPerms((prev) =>
            prev.some((x) => x.id === perm.id)
              ? prev
              : [...prev, { id: perm.id!, title: perm.title || "Permission requested" }],
          );
        }
        break;
      }
      case "permission.replied": {
        const id = (p.permissionID as string | undefined) ?? (p.id as string | undefined);
        if (id) setPerms((prev) => prev.filter((x) => x.id !== id));
        break;
      }
      case "session.idle":
        setBusy(false);
        break;
      case "session.error": {
        const msg = (p.error as { data?: { message?: string } } | undefined)?.data?.message;
        setError(msg || "agent error");
        setBusy(false);
        break;
      }
    }
  }, []);

  // keep the latest handler without resubscribing
  const handlerRef = useRef(handleEvent);
  handlerRef.current = handleEvent;

  const doConnect = useCallback(async () => {
    setError(null);
    setPhase("connecting");
    try {
      const client = await connect(url);
      saveOpencodeUrl(url);
      clientRef.current = client;

      const { models: ms, defaultModel } = await listModels(client);
      setModels(ms);
      setModel((cur) => cur ?? defaultModel ?? (ms[0] ? { providerID: ms[0].providerID, modelID: ms[0].modelID } : null));

      await ensureEngineMcp(client);

      const sid = await createSession(client);
      sessionRef.current = sid;
      setMessages([]);
      setParts([]);
      setPerms([]);

      unsubRef.current?.();
      unsubRef.current = subscribeEvents(
        client,
        (ev) => handlerRef.current(ev),
        (err) => setError(String(err)),
      );
      setPhase("connected");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setPhase("error");
    }
  }, [url]);

  useEffect(() => () => unsubRef.current?.(), []);

  // autoscroll on new content
  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [parts, messages, busy]);

  const send = useCallback(async () => {
    const client = clientRef.current;
    const sid = sessionRef.current;
    const text = draft.trim();
    if (!client || !sid || !model || !text || busy) return;
    setDraft("");
    setBusy(true);
    setError(null);
    try {
      await sendPrompt(client, sid, model, text);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setBusy(false);
    }
  }, [draft, model, busy]);

  const stop = useCallback(() => {
    const client = clientRef.current;
    const sid = sessionRef.current;
    if (client && sid) void abort(client, sid);
    setBusy(false);
  }, []);

  const answerPerm = useCallback((id: string, response: "once" | "always" | "reject") => {
    const client = clientRef.current;
    const sid = sessionRef.current;
    setPerms((prev) => prev.filter((x) => x.id !== id));
    if (client && sid) void replyPermission(client, sid, id, response);
  }, []);

  // group parts under their message, in message order
  const thread = useMemo(() => {
    return messages.map((m) => ({ msg: m, parts: parts.filter((pt) => pt.messageID === m.id) }));
  }, [messages, parts]);

  if (phase !== "connected") {
    return (
      <ConnectScreen
        url={url}
        setUrl={setUrl}
        phase={phase}
        error={error}
        onConnect={doConnect}
      />
    );
  }

  return (
    <div style={S.root}>
      <div style={S.header}>
        <Bot size={15} style={{ color: "hsl(var(--foreground))" }} />
        <span style={S.headerTitle}>Agent</span>
        <span style={S.dot} title="connected" />
        <span style={S.headerUrl}>{url}</span>
        <div style={{ flex: 1 }} />
        <select
          value={model ? `${model.providerID}/${model.modelID}` : ""}
          onChange={(e) => {
            const [providerID, ...rest] = e.target.value.split("/");
            setModel({ providerID, modelID: rest.join("/") });
          }}
          style={S.modelSelect}
          title="Model"
        >
          {models.map((m) => (
            <option key={`${m.providerID}/${m.modelID}`} value={`${m.providerID}/${m.modelID}`}>
              {m.label}
            </option>
          ))}
        </select>
      </div>

      <div ref={scrollRef} style={S.scroll}>
        {thread.length === 0 && (
          <div style={S.empty}>
            Ask the agent to inspect or build the wiresheet — e.g.{" "}
            <em>“list the components at /”</em> or{" "}
            <em>“add two add blocks and wire them together”</em>.
          </div>
        )}
        {thread.map(({ msg, parts: mp }) => (
          <MessageRow key={msg.id} role={msg.role} parts={mp} />
        ))}
        {busy && <div style={S.thinking}>● thinking…</div>}
      </div>

      {perms.map((perm) => (
        <div key={perm.id} style={S.perm}>
          <span style={{ flex: 1 }}>{perm.title}</span>
          <button style={S.permBtn} onClick={() => answerPerm(perm.id, "once")}>Allow</button>
          <button style={S.permBtn} onClick={() => answerPerm(perm.id, "always")}>Always</button>
          <button style={{ ...S.permBtn, color: "hsl(var(--crit))" }} onClick={() => answerPerm(perm.id, "reject")}>Deny</button>
        </div>
      ))}

      {error && <div style={S.error}>{error}</div>}

      <div style={S.composer}>
        <textarea
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              void send();
            }
          }}
          placeholder="Message the agent…  (Enter to send, Shift+Enter for newline)"
          rows={2}
          style={S.textarea}
        />
        {busy ? (
          <button style={S.sendBtn} onClick={stop} title="Stop">
            <Square size={14} />
          </button>
        ) : (
          <button style={S.sendBtn} onClick={() => void send()} disabled={!draft.trim()} title="Send">
            <Send size={14} />
          </button>
        )}
      </div>
    </div>
  );
}

function ConnectScreen({
  url,
  setUrl,
  phase,
  error,
  onConnect,
}: {
  url: string;
  setUrl: (v: string) => void;
  phase: Phase;
  error: string | null;
  onConnect: () => void;
}) {
  return (
    <div style={S.connectRoot}>
      <Bot size={28} style={{ color: "hsl(var(--muted-foreground))" }} />
      <div style={S.connectTitle}>Connect to opencode</div>
      <div style={S.connectHint}>
        Start a local agent server, then connect. The agent is wired to this engine’s MCP tools.
      </div>
      <code style={S.cmd}>pnpm opencode</code>
      <input
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && onConnect()}
        placeholder="http://localhost:4096"
        style={S.urlInput}
      />
      <button style={S.connectBtn} onClick={onConnect} disabled={phase === "connecting"}>
        <Plug size={14} />
        {phase === "connecting" ? "Connecting…" : "Connect"}
      </button>
      {error && <div style={{ ...S.error, maxWidth: 360 }}>{error}</div>}
    </div>
  );
}

function MessageRow({ role, parts }: { role: string; parts: ChatPart[] }) {
  const isUser = role === "user";
  // hide empty/synthetic-only assistant rows
  const visible = parts.filter((p) => (p.kind === "tool" ? true : (p.text ?? "").length > 0));
  if (visible.length === 0) return null;
  return (
    <div style={{ ...S.msg, alignItems: isUser ? "flex-end" : "stretch" }}>
      <div style={S.role}>{isUser ? "you" : "agent"}</div>
      {visible.map((p) =>
        p.kind === "tool" ? (
          <ToolCard key={p.id} part={p} />
        ) : (
          <div key={p.id} style={{ ...S.bubble, ...(isUser ? S.bubbleUser : S.bubbleAgent), ...(p.kind === "reasoning" ? S.reasoning : null) }}>
            {p.text}
          </div>
        ),
      )}
    </div>
  );
}

function ToolCard({ part }: { part: ChatPart }) {
  const [open, setOpen] = useState(false);
  const name = (part.tool ?? "tool").replace(/^ce-engine_/, "");
  const color =
    part.status === "error" ? "hsl(var(--crit))" : part.status === "completed" ? "hsl(var(--ok, 142 70% 45%))" : "hsl(var(--muted-foreground))";
  return (
    <div style={S.tool}>
      <div style={S.toolHead} onClick={() => setOpen((o) => !o)}>
        <ChevronRight size={12} style={{ transform: open ? "rotate(90deg)" : "none", transition: "transform .1s" }} />
        <span style={{ fontFamily: "var(--font-mono, monospace)" }}>{name}</span>
        <span style={{ color, fontSize: 10 }}>{part.status ?? ""}</span>
      </div>
      {open && (
        <div style={S.toolBody}>
          {part.input != null && (
            <pre style={S.pre}>{safeJson(part.input)}</pre>
          )}
          {part.output && <pre style={S.pre}>{truncate(part.output, 4000)}</pre>}
        </div>
      )}
    </div>
  );
}

// --- raw SDK part → chat part ----------------------------------------------

interface RawPart {
  id: string;
  messageID: string;
  type: string;
  text?: string;
  tool?: string;
  state?: { status?: string; input?: unknown; output?: string };
  synthetic?: boolean;
}

function toChatPart(part: RawPart): ChatPart | null {
  if (part.type === "text") {
    if (part.synthetic) return null;
    return { id: part.id, messageID: part.messageID, kind: "text", text: part.text ?? "" };
  }
  if (part.type === "reasoning") {
    return { id: part.id, messageID: part.messageID, kind: "reasoning", text: part.text ?? "" };
  }
  if (part.type === "tool") {
    return {
      id: part.id,
      messageID: part.messageID,
      kind: "tool",
      tool: part.tool,
      status: part.state?.status,
      input: part.state?.input,
      output: part.state?.output,
    };
  }
  return null; // step-start/finish, snapshot, etc. — not rendered
}

function safeJson(v: unknown): string {
  try {
    return JSON.stringify(v, null, 2);
  } catch {
    return String(v);
  }
}
function truncate(s: string, n: number): string {
  return s.length > n ? s.slice(0, n) + "\n… (truncated)" : s;
}

// --- styles (inline, matching the repo's CSS-var token convention) ----------

const S: Record<string, React.CSSProperties> = {
  root: { display: "flex", flexDirection: "column", height: "100%", background: "hsl(var(--background))", color: "hsl(var(--foreground))" },
  header: { display: "flex", alignItems: "center", gap: 6, padding: "6px 10px", borderBottom: "1px solid hsl(var(--border))", fontSize: 12 },
  headerTitle: { fontWeight: 600 },
  dot: { width: 7, height: 7, borderRadius: "50%", background: "hsl(142 70% 45%)" },
  headerUrl: { color: "hsl(var(--muted-foreground))", fontSize: 11 },
  modelSelect: { fontSize: 11, background: "hsl(var(--input))", color: "hsl(var(--foreground))", border: "1px solid hsl(var(--border))", borderRadius: 4, padding: "2px 4px", maxWidth: 200 },

  scroll: { flex: 1, overflowY: "auto", padding: "10px 12px", display: "flex", flexDirection: "column", gap: 10 },
  empty: { color: "hsl(var(--muted-foreground))", fontSize: 12, lineHeight: 1.6, margin: "auto", maxWidth: 320, textAlign: "center" },
  thinking: { color: "hsl(var(--muted-foreground))", fontSize: 12, fontStyle: "italic" },

  msg: { display: "flex", flexDirection: "column", gap: 4 },
  role: { fontSize: 10, textTransform: "uppercase", letterSpacing: 0.5, color: "hsl(var(--muted-foreground))" },
  bubble: { fontSize: 13, lineHeight: 1.5, whiteSpace: "pre-wrap", wordBreak: "break-word", borderRadius: 8, padding: "7px 10px" },
  bubbleUser: { background: "hsl(var(--primary, 217 91% 60%))", color: "hsl(var(--primary-foreground, 0 0% 100%))", alignSelf: "flex-end", maxWidth: "85%" },
  bubbleAgent: { background: "hsl(var(--muted, var(--border)))", color: "hsl(var(--foreground))" },
  reasoning: { fontStyle: "italic", opacity: 0.7, fontSize: 12 },

  tool: { border: "1px solid hsl(var(--border))", borderRadius: 6, fontSize: 12, overflow: "hidden" },
  toolHead: { display: "flex", alignItems: "center", gap: 6, padding: "4px 8px", cursor: "pointer", background: "hsl(var(--input))" },
  toolBody: { borderTop: "1px solid hsl(var(--border))", padding: 8 },
  pre: { margin: 0, fontSize: 11, whiteSpace: "pre-wrap", wordBreak: "break-word", fontFamily: "var(--font-mono, monospace)", color: "hsl(var(--muted-foreground))", maxHeight: 240, overflow: "auto" },

  perm: { display: "flex", alignItems: "center", gap: 8, padding: "6px 10px", borderTop: "1px solid hsl(var(--border))", fontSize: 12, background: "hsl(var(--input))" },
  permBtn: { fontSize: 11, padding: "3px 8px", borderRadius: 4, border: "1px solid hsl(var(--border))", background: "hsl(var(--background))", color: "hsl(var(--foreground))", cursor: "pointer" },

  error: { color: "hsl(var(--crit))", fontSize: 12, padding: "6px 12px", borderTop: "1px solid hsl(var(--border))", whiteSpace: "pre-wrap" },

  composer: { display: "flex", gap: 6, padding: 8, borderTop: "1px solid hsl(var(--border))", alignItems: "flex-end" },
  textarea: { flex: 1, resize: "none", fontSize: 13, fontFamily: "inherit", lineHeight: 1.4, padding: "6px 8px", borderRadius: 6, border: "1px solid hsl(var(--border))", background: "hsl(var(--input))", color: "hsl(var(--foreground))" },
  sendBtn: { display: "flex", alignItems: "center", justifyContent: "center", width: 34, height: 34, borderRadius: 6, border: "1px solid hsl(var(--border))", background: "hsl(var(--primary, 217 91% 60%))", color: "hsl(var(--primary-foreground, 0 0% 100%))", cursor: "pointer" },

  connectRoot: { display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 10, height: "100%", padding: 24, textAlign: "center", background: "hsl(var(--background))", color: "hsl(var(--foreground))" },
  connectTitle: { fontSize: 15, fontWeight: 600 },
  connectHint: { fontSize: 12, color: "hsl(var(--muted-foreground))", maxWidth: 320, lineHeight: 1.5 },
  cmd: { fontSize: 12, fontFamily: "var(--font-mono, monospace)", background: "hsl(var(--input))", padding: "3px 8px", borderRadius: 4, border: "1px solid hsl(var(--border))" },
  urlInput: { width: 260, fontSize: 13, padding: "6px 8px", borderRadius: 6, border: "1px solid hsl(var(--border))", background: "hsl(var(--input))", color: "hsl(var(--foreground))", textAlign: "center" },
  connectBtn: { display: "flex", alignItems: "center", gap: 6, fontSize: 13, padding: "7px 16px", borderRadius: 6, border: "1px solid hsl(var(--border))", background: "hsl(var(--primary, 217 91% 60%))", color: "hsl(var(--primary-foreground, 0 0% 100%))", cursor: "pointer" },
};

registerWidget("agentChat", AgentChatPanel);

export default AgentChatPanel;
