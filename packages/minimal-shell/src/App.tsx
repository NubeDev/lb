// The minimal shell App — login → full-screen extension mount. No sidebar, no dock, no chrome.
// The extension id is OPAQUE CONFIG DATA (rule 10): the shell never branches on it.

import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { ThemeProvider } from "./theme";
import { useSession, signIn, signOut } from "./session";
import { listExtensions, type ExtRow } from "./ipc";
import { loadRemoteMount, makeBridge, type ExtPage } from "./federation";

// Config: which extension page is "home" (set by the product host at build time).
const HOME_EXT = (import.meta as any).env?.VITE_HOME_EXT || "";
const HOME_ENTRY = (import.meta as any).env?.VITE_HOME_ENTRY || "remoteEntry.js";
const HOME_SCOPE = ((import.meta as any).env?.VITE_HOME_SCOPE || "").split(",").filter(Boolean);

export function App() {
  return (
    <ThemeProvider>
      <Shell />
    </ThemeProvider>
  );
}

function Shell() {
  const session = useSession();
  if (!session) return <LoginView />;
  return <ExtMount session={session} />;
}

function LoginView() {
  const [user, setUser] = useState("");
  const [ws, setWs] = useState("");
  const [secret, setSecret] = useState("");
  const [err, setErr] = useState("");

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErr("");
    try {
      await signIn(user, ws, secret);
    } catch (e: any) {
      setErr(e.message || "login failed");
    }
  };

  return (
    <div style={{ minHeight: "100vh", display: "flex", alignItems: "center", justifyContent: "center", background: "var(--bg, #0f172a)", color: "var(--text, #f8fafc)" }}>
      <form onSubmit={submit} style={{ display: "flex", flexDirection: "column", gap: "0.75rem", width: "100%", maxWidth: "320px" }}>
        <h1 style={{ fontSize: "1.25rem", fontWeight: 600, textAlign: "center" }}>Sign in</h1>
        <input value={user} onChange={(e) => setUser(e.target.value)} placeholder="user" style={inputStyle} />
        <input value={ws} onChange={(e) => setWs(e.target.value)} placeholder="workspace" style={inputStyle} />
        <input type="password" value={secret} onChange={(e) => setSecret(e.target.value)} placeholder="password" style={inputStyle} />
        {err && <p style={{ color: "#f87171", fontSize: "0.875rem" }}>{err}</p>}
        <button type="submit" style={btnStyle}>Sign in</button>
      </form>
    </div>
  );
}

const inputStyle: React.CSSProperties = {
  padding: "0.5rem 0.75rem",
  borderRadius: "0.375rem",
  border: "1px solid rgba(255,255,255,0.1)",
  background: "rgba(255,255,255,0.05)",
  color: "inherit",
};
const btnStyle: React.CSSProperties = {
  padding: "0.5rem 1rem",
  borderRadius: "0.375rem",
  border: "none",
  background: "var(--accent, #3b82f6)",
  color: "#fff",
  fontWeight: 500,
  cursor: "pointer",
};

function ExtMount({ session }: { session: NonNullable<ReturnType<typeof useSession>> }) {
  const [page, setPage] = useState<ExtPage | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      // If a home ext is configured, mount it directly (no ext.list needed).
      if (HOME_EXT) {
        if (!cancelled) setPage({ ext: HOME_EXT, entry: HOME_ENTRY });
        return;
      }
      // Otherwise discover via ext.list (the generic seam).
      try {
        const rows = await listExtensions();
        const withUi = rows.filter((r) => r.ui?.entry);
        if (!cancelled && withUi.length > 0) {
          setPage({ ext: withUi[0].ext, entry: withUi[0].ui!.entry });
        } else if (!cancelled) {
          setError("No extension UI available.");
        }
      } catch (e: any) {
        if (!cancelled) setError(e.message || "ext.list failed");
      }
    })();
    return () => { cancelled = true; };
  }, [session.workspace]);

  if (error) {
    return (
      <div style={{ minHeight: "100vh", display: "flex", alignItems: "center", justifyContent: "center", color: "#f87171" }}>
        {error}
      </div>
    );
  }

  if (!page) {
    return (
      <div style={{ minHeight: "100vh", display: "flex", alignItems: "center", justifyContent: "center" }}>
        Loading…
      </div>
    );
  }

  return (
    <div style={{ minHeight: "100vh" }}>
      <RemoteExt page={page} workspace={session.workspace} />
      <button onClick={signOut} style={{ position: "fixed", bottom: "0.5rem", right: "0.5rem", opacity: 0.3, ...btnStyle }}>
        Sign out
      </button>
    </div>
  );
}

function RemoteExt({ page, workspace }: { page: ExtPage; workspace: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    if (!ref.current) return;
    let teardown: (() => void) | void;
    let cancelled = false;
    (async () => {
      try {
        const { mount } = await loadRemoteMount(page.ext, page.entry);
        if (cancelled) return;
        const bridge = makeBridge(HOME_SCOPE);
        teardown = mount(ref.current!, { workspace }, bridge);
      } catch (e: any) {
        if (!cancelled) setErr(e.message || "mount failed");
      }
    })();
    return () => {
      cancelled = true;
      if (typeof teardown === "function") teardown();
    };
  }, [page.ext, page.entry, workspace]);

  if (err) {
    return (
      <div style={{ minHeight: "100vh", display: "flex", alignItems: "center", justifyContent: "center", color: "#f87171" }}>
        {err}
      </div>
    );
  }

  return <div ref={ref} style={{ width: "100%", height: "100vh" }} />;
}

import { useRef } from "react";
