'use client'

import { useState } from 'react'

// ─────────────────────────────────────────────────────────────────────────────
// Lazybones — architecture & access visual reference.
// Pure React + SVG + styled-jsx. No extra dependencies.
// Two views: the whole stack, and the auth / access services.
// ─────────────────────────────────────────────────────────────────────────────

const TABS = [
  { key: 'stack', label: 'Stack' },
  { key: 'auth', label: 'Auth & Access' },
]

export default function Architecture() {
  const [tab, setTab] = useState('stack')
  return (
    <div className="lb-arch">
      <header className="lb-head">
        <div>
          <h1>Lazybones — how it fits together</h1>
          <p className="lb-sub">
            One binary, two roles (edge + cloud). Every surface routes through a single
            capability chokepoint. These diagrams show the stack, then who-can-do-what.
          </p>
        </div>
        <nav className="lb-tabs" role="tablist">
          {TABS.map((t) => (
            <button
              key={t.key}
              role="tab"
              aria-selected={tab === t.key}
              className={`lb-tab ${tab === t.key ? 'is-active' : ''}`}
              onClick={() => setTab(t.key)}
            >
              {t.label}
            </button>
          ))}
        </nav>
      </header>

      {tab === 'stack' ? <StackView /> : <AuthView />}

      <footer className="lb-foot">
        Grounded in <code>doc-site/content/public/SCOPE.mdx</code>,{' '}
        <code>doc-site/content/public/DIAGRAMS.mdx</code>, and{' '}
        <code>doc-site/content/public/auth-caps/auth-caps.mdx</code>. Working scopes live in{' '}
        <code>docs/scope/</code>.
      </footer>

      <style jsx global>{`
        .lb-arch {
          max-width: 1180px;
          margin: 0 auto;
          padding: 32px 24px 64px;
          color: var(--lb-fg, #e7ecf3);
          --lb-bg: #0b1120;
          --lb-panel: #121a2e;
          --lb-panel2: #0f1626;
          --lb-border: #243049;
          --lb-fg: #e7ecf3;
          --lb-muted: #93a1bd;
          --lb-amber: #f5b440;
          --lb-teal: #2dd4bf;
          --lb-blue: #60a5fa;
          --lb-violet: #a78bfa;
          --lb-red: #f87171;
          --lb-green: #34d399;
        }
        .lb-head {
          display: flex;
          flex-wrap: wrap;
          gap: 16px;
          align-items: flex-end;
          justify-content: space-between;
          margin-bottom: 28px;
        }
        .lb-head h1 {
          font-size: 1.9rem;
          font-weight: 700;
          margin: 0 0 6px;
          letter-spacing: -0.02em;
        }
        .lb-sub {
          margin: 0;
          color: var(--lb-muted);
          max-width: 640px;
          line-height: 1.5;
          font-size: 0.95rem;
        }
        .lb-tabs {
          display: inline-flex;
          gap: 4px;
          padding: 4px;
          background: var(--lb-panel2);
          border: 1px solid var(--lb-border);
          border-radius: 12px;
        }
        .lb-tab {
          appearance: none;
          border: 0;
          background: transparent;
          color: var(--lb-muted);
          font-size: 0.85rem;
          font-weight: 600;
          padding: 8px 16px;
          border-radius: 8px;
          cursor: pointer;
          transition: all 0.15s ease;
        }
        .lb-tab:hover {
          color: var(--lb-fg);
        }
        .lb-tab.is-active {
          background: var(--lb-amber);
          color: #1a1206;
        }
        .lb-foot {
          margin-top: 40px;
          padding-top: 18px;
          border-top: 1px solid var(--lb-border);
          color: var(--lb-muted);
          font-size: 0.82rem;
          line-height: 1.6;
        }
        .lb-foot code,
        .lb-arch code {
          font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
          font-size: 0.85em;
          background: var(--lb-panel2);
          border: 1px solid var(--lb-border);
          padding: 1px 5px;
          border-radius: 5px;
          color: var(--lb-teal);
        }

        /* shared building blocks */
        .lb-section {
          margin-bottom: 36px;
        }
        .lb-section-title {
          display: flex;
          align-items: center;
          gap: 10px;
          font-size: 0.78rem;
          font-weight: 700;
          text-transform: uppercase;
          letter-spacing: 0.12em;
          color: var(--lb-muted);
          margin: 0 0 14px;
        }
        .lb-section-title::before {
          content: '';
          width: 18px;
          height: 2px;
          background: var(--lb-amber);
          border-radius: 2px;
        }
        .lb-card {
          background: var(--lb-panel);
          border: 1px solid var(--lb-border);
          border-radius: 16px;
          padding: 18px;
        }

        .lb-nodes {
          display: grid;
          grid-template-columns: 1fr 64px 1fr;
          gap: 0;
          align-items: stretch;
        }
        @media (max-width: 820px) {
          .lb-nodes {
            grid-template-columns: 1fr;
          }
          .lb-conduit {
            flex-direction: row !important;
            height: auto !important;
            min-height: 64px;
            padding: 12px 0;
          }
        }
        .lb-node {
          background: var(--lb-panel);
          border: 1px solid var(--lb-border);
          border-radius: 16px;
          overflow: hidden;
          display: flex;
          flex-direction: column;
        }
        .lb-node-head {
          padding: 14px 16px;
          border-bottom: 1px solid var(--lb-border);
          display: flex;
          align-items: center;
          gap: 10px;
        }
        .lb-node-head .dot {
          width: 10px;
          height: 10px;
          border-radius: 50%;
          box-shadow: 0 0 12px currentColor;
        }
        .lb-node-head h3 {
          margin: 0;
          font-size: 1rem;
          font-weight: 700;
        }
        .lb-node-head .tag {
          margin-left: auto;
          font-size: 0.7rem;
          text-transform: uppercase;
          letter-spacing: 0.08em;
          color: var(--lb-muted);
        }
        .lb-layer {
          padding: 10px 16px;
          border-bottom: 1px solid var(--lb-border);
          font-size: 0.83rem;
          display: flex;
          flex-direction: column;
          gap: 2px;
        }
        .lb-layer:last-child {
          border-bottom: 0;
        }
        .lb-layer .l-title {
          font-weight: 600;
          color: var(--lb-fg);
        }
        .lb-layer .l-desc {
          color: var(--lb-muted);
          font-size: 0.78rem;
          line-height: 1.4;
        }
        .lb-layer.is-accent {
          background: linear-gradient(90deg, rgba(245, 180, 64, 0.07), transparent);
        }
        .lb-layer.is-platform {
          background: var(--lb-panel2);
        }
        .lb-layer.is-data {
          background: linear-gradient(180deg, rgba(45, 212, 191, 0.06), rgba(45, 212, 191, 0.02));
        }

        .lb-conduit {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          gap: 12px;
          color: var(--lb-muted);
          font-size: 0.68rem;
          text-align: center;
          padding: 8px 4px;
        }
        .lb-conduit .pill {
          background: var(--lb-panel2);
          border: 1px solid var(--lb-border);
          border-radius: 999px;
          padding: 4px 8px;
          white-space: nowrap;
        }

        /* pipeline (tool-call anatomy) */
        .lb-pipe {
          display: flex;
          align-items: stretch;
          gap: 0;
          overflow-x: auto;
          padding-bottom: 6px;
        }
        .lb-pipe-step {
          flex: 1 1 0;
          min-width: 120px;
          background: var(--lb-panel2);
          border: 1px solid var(--lb-border);
          border-radius: 12px;
          padding: 12px;
          position: relative;
          display: flex;
          flex-direction: column;
          gap: 4px;
        }
        .lb-pipe-step .ps-k {
          font-size: 0.66rem;
          text-transform: uppercase;
          letter-spacing: 0.1em;
          color: var(--lb-muted);
        }
        .lb-pipe-step .ps-t {
          font-weight: 700;
          font-size: 0.9rem;
        }
        .lb-pipe-step .ps-d {
          font-size: 0.76rem;
          color: var(--lb-muted);
          line-height: 1.4;
        }
        .lb-pipe-step.is-gate {
          border-color: var(--lb-amber);
          background: linear-gradient(180deg, rgba(245, 180, 64, 0.1), rgba(245, 180, 64, 0.02));
        }
        .lb-pipe-step.is-deny {
          border-color: var(--lb-red);
          background: rgba(248, 113, 113, 0.06);
        }
        .lb-pipe-arrow {
          align-self: center;
          color: var(--lb-muted);
          font-size: 1.1rem;
          padding: 0 6px;
          flex: 0 0 auto;
        }

        /* auth grid */
        .lb-grid2 {
          display: grid;
          grid-template-columns: 1fr 1fr;
          gap: 16px;
        }
        @media (max-width: 820px) {
          .lb-grid2 {
            grid-template-columns: 1fr;
          }
        }

        .lb-token {
          display: flex;
          gap: 6px;
          flex-wrap: wrap;
          margin: 10px 0;
        }
        .lb-token .seg {
          font-family: ui-monospace, monospace;
          font-size: 0.74rem;
          padding: 6px 9px;
          border-radius: 7px;
          border: 1px solid var(--lb-border);
        }
        .lb-token .seg.h {
          background: rgba(96, 165, 250, 0.12);
          color: var(--lb-blue);
        }
        .lb-token .seg.p {
          background: rgba(167, 139, 250, 0.12);
          color: var(--lb-violet);
        }
        .lb-token .seg.s {
          background: rgba(248, 113, 113, 0.12);
          color: var(--lb-red);
        }
        .lb-claims {
          font-family: ui-monospace, monospace;
          font-size: 0.76rem;
          background: var(--lb-panel2);
          border: 1px solid var(--lb-border);
          border-radius: 10px;
          padding: 10px 12px;
          color: var(--lb-teal);
          line-height: 1.6;
          overflow-x: auto;
        }
        .lb-claims .k {
          color: var(--lb-blue);
        }
        .lb-claims .c {
          color: var(--lb-muted);
        }

        .lb-gates {
          display: flex;
          flex-direction: column;
          gap: 10px;
        }
        .lb-gate {
          display: grid;
          grid-template-columns: 44px 1fr;
          gap: 12px;
          align-items: center;
          background: var(--lb-panel2);
          border: 1px solid var(--lb-border);
          border-radius: 12px;
          padding: 12px;
        }
        .lb-gate .num {
          width: 44px;
          height: 44px;
          border-radius: 12px;
          display: grid;
          place-items: center;
          font-weight: 800;
          font-size: 1.1rem;
          color: #1a1206;
        }
        .lb-gate .gt {
          font-weight: 700;
          font-size: 0.9rem;
        }
        .lb-gate .gd {
          font-size: 0.78rem;
          color: var(--lb-muted);
          line-height: 1.45;
        }
        .lb-gate .speed {
          display: inline-block;
          margin-top: 4px;
          font-size: 0.68rem;
          padding: 2px 8px;
          border-radius: 999px;
          border: 1px solid var(--lb-border);
        }

        .lb-subjects {
          display: grid;
          grid-template-columns: repeat(4, 1fr);
          gap: 10px;
        }
        @media (max-width: 720px) {
          .lb-subjects {
            grid-template-columns: repeat(2, 1fr);
          }
        }
        .lb-subj {
          background: var(--lb-panel2);
          border: 1px solid var(--lb-border);
          border-radius: 12px;
          padding: 12px;
          text-align: center;
        }
        .lb-subj .ic {
          font-size: 1.3rem;
        }
        .lb-subj .nm {
          font-weight: 700;
          font-size: 0.85rem;
          margin: 4px 0 2px;
        }
        .lb-subj .ds {
          font-size: 0.72rem;
          color: var(--lb-muted);
          line-height: 1.4;
        }

        .lb-callout {
          border-left: 3px solid var(--lb-amber);
          background: rgba(245, 180, 64, 0.05);
          border-radius: 0 12px 12px 0;
          padding: 12px 14px;
          font-size: 0.82rem;
          color: var(--lb-fg);
          line-height: 1.55;
        }
        .lb-callout.is-teal {
          border-color: var(--lb-teal);
          background: rgba(45, 212, 191, 0.05);
        }
        .lb-callout strong {
          color: var(--lb-amber);
        }
        .lb-callout.is-teal strong {
          color: var(--lb-teal);
        }

        .lb-formula {
          font-family: ui-monospace, monospace;
          font-size: 0.82rem;
          background: var(--lb-panel2);
          border: 1px solid var(--lb-border);
          border-radius: 10px;
          padding: 12px 14px;
          text-align: center;
          color: var(--lb-fg);
          overflow-x: auto;
        }
        .lb-formula b {
          color: var(--lb-amber);
        }

        @keyframes lb-flow {
          to {
            stroke-dashoffset: -16;
          }
        }
      `}</style>
    </div>
  )
}

// ─────────────────────────────────────────────────────────────────────────────
// STACK VIEW
// ─────────────────────────────────────────────────────────────────────────────

function StackView() {
  return (
    <>
      {/* Two nodes */}
      <section className="lb-section">
        <h2 className="lb-section-title">Two roles, one binary</h2>
        <div className="lb-nodes">
          <NodeCard
            color="var(--lb-blue)"
            title="Edge node"
            tag="user / device role"
            layers={[
              { t: 'Tauri / local UI', d: 'workspace switcher · local settings · extension UI surfaces' },
              { t: 'Host + MCP server', d: 'local tools · routed calls to cloud · capability checks', accent: true },
              { t: 'Extension runtime', d: 'WASM components · optional native sidecars · optional local AI gateway' },
              { t: 'Platform crates', d: 'auth · caps · tags · inbox/outbox · jobs · secrets · sync · ext-loader', platform: true },
              { t: 'Zenoh peer', d: 'local pub/sub · connects to cloud router when online' },
              { t: 'Embedded SurrealDB', d: 'node-local + cached workspace data · offline-first', data: true },
            ]}
          />

          <div className="lb-conduit">
            <span className="pill">routed MCP</span>
            <span className="pill">sync §6.8</span>
            <span className="pill">SSE</span>
            <FlowArrows />
          </div>

          <NodeCard
            color="var(--lb-amber)"
            title="Cloud hub"
            tag="shared authority"
            layers={[
              { t: 'Web entry / gateway', d: 'browser UI · SSE streams · HTTP commands · admin console' },
              { t: 'Shared AI gateway', d: 'model/provider routing · quotas · audit — NOT an agent' },
              { t: 'Central AI agents', d: 'workspace-scoped actors · own the tool-call loop' },
              { t: 'Host + MCP server', d: 'cloud tools · registry/admin · workflow + agent tools', accent: true },
              { t: 'Extension runtime', d: 'cloud WASM · either-placement · supervised sidecars' },
              { t: 'Platform crates', d: 'auth · caps · tags · inbox/outbox · jobs · secrets · sync · registry-host', platform: true },
              { t: 'Zenoh router', d: 'accepts edge peers · routes messages, MCP, streams, presence' },
              { t: 'SurrealDB + buckets', d: 'workspace authority · identity/teams/channels · registry · audit', data: true },
            ]}
          />
        </div>
      </section>

      {/* Tool-call anatomy */}
      <section className="lb-section">
        <h2 className="lb-section-title">Anatomy of a tool call — the one chokepoint</h2>
        <div className="lb-card">
          <p style={{ margin: '0 0 14px', color: 'var(--lb-muted)', fontSize: '0.85rem' }}>
            Every surface — store, bus, secret, mcp — routes through <code>caps::check</code>{' '}
            <em>before</em> touching the resource. There is no other path.
          </p>
          <div className="lb-pipe">
            <PipeStep k="caller" t="Client" d="UI · Tauri · API key (bearer)" />
            <PipeArrow />
            <PipeStep k="transport" t="Gateway" d="HTTP / SSE (or in-process)" />
            <PipeArrow />
            <PipeStep k="gate 1" t="Workspace wall" d="principal.ws == request.ws, else Denied::Workspace" gate />
            <PipeArrow />
            <PipeStep k="gate 2" t="Capability" d="a held cap pattern-matches surface:resource:action" gate />
            <PipeStep
              k="deny"
              t="→ Denied"
              d="opaque — a denial never reveals whether the resource exists"
              deny
            />
            <PipeArrow />
            <PipeStep k="dispatch" t="Host verb" d="one responsibility per file · workspace-scoped write_tx" />
            <PipeArrow />
            <PipeStep k="surface" t="store · bus · secret · mcp" d="the actual resource, namespace-scoped" />
          </div>
          <div className="lb-callout" style={{ marginTop: 16 }}>
            <strong>Gate 3 (membership / visibility)</strong> sits below the wall for shared
            assets: owner · member of a shared team · <code>sub</code>-grantee of a linked channel.
            Re-resolved <em>live</em> on every read — a revoke is one delete.
          </div>
        </div>
      </section>

      {/* Grammar */}
      <section className="lb-section">
        <h2 className="lb-section-title">Capability grammar</h2>
        <div className="lb-card">
          <div className="lb-formula">
            <b>{'<surface>:<resource>:<action>'}</b>
          </div>
          <div className="lb-grid2" style={{ marginTop: 14 }}>
            <GramRow surface="mcp" ex="mcp:hvac.setpoint:call" d="an extension tool" />
            <GramRow surface="store" ex="store:series/hvac.*:read" d="data reads / writes" />
            <GramRow surface="bus" ex="bus:chan/123:pub" d="pub/sub (host adds ws/{id}/)" />
            <GramRow surface="secret" ex="secret:federation/dsn:get" d="mediated secret access" />
          </div>
          <p style={{ margin: '14px 0 0', color: 'var(--lb-muted)', fontSize: '0.8rem' }}>
            Deny-by-default. <code>*</code> = one segment, <code>**</code> = recursive trailing run.
            An unparseable capability grants nothing.
          </p>
        </div>
      </section>
    </>
  )
}

function NodeCard({ color, title, tag, layers }) {
  return (
    <div className="lb-node">
      <div className="lb-node-head">
        <span className="dot" style={{ background: color, color }} />
        <h3>{title}</h3>
        <span className="tag">{tag}</span>
      </div>
      {layers.map((l, i) => (
        <div
          key={i}
          className={`lb-layer ${l.accent ? 'is-accent' : ''} ${l.platform ? 'is-platform' : ''} ${
            l.data ? 'is-data' : ''
          }`}
        >
          <span className="l-title">{l.t}</span>
          <span className="l-desc">{l.d}</span>
        </div>
      ))}
    </div>
  )
}

function FlowArrows() {
  return (
    <svg width="40" height="70" viewBox="0 0 40 70" fill="none" aria-hidden>
      <defs>
        <marker id="ah-up" markerWidth="6" markerHeight="6" refX="3" refY="3" orient="auto">
          <path d="M0,0 L6,3 L0,6 Z" fill="var(--lb-muted)" />
        </marker>
        <marker id="ah-dn" markerWidth="6" markerHeight="6" refX="3" refY="3" orient="auto">
          <path d="M0,0 L6,3 L0,6 Z" fill="var(--lb-muted)" />
        </marker>
      </defs>
      <path
        d="M12,62 C12,40 12,30 12,8"
        stroke="var(--lb-blue)"
        strokeWidth="1.6"
        strokeDasharray="4 4"
        markerEnd="url(#ah-up)"
        style={{ animation: 'lb-flow 1s linear infinite' }}
      />
      <path
        d="M28,8 C28,30 28,40 28,62"
        stroke="var(--lb-amber)"
        strokeWidth="1.6"
        strokeDasharray="4 4"
        markerEnd="url(#ah-dn)"
        style={{ animation: 'lb-flow 1s linear infinite' }}
      />
    </svg>
  )
}

function PipeStep({ k, t, d, gate, deny }) {
  return (
    <div className={`lb-pipe-step ${gate ? 'is-gate' : ''} ${deny ? 'is-deny' : ''}`}>
      <span className="ps-k">{k}</span>
      <span className="ps-t">{t}</span>
      <span className="ps-d">{d}</span>
    </div>
  )
}

function PipeArrow() {
  return <span className="lb-pipe-arrow" aria-hidden>→</span>
}

function GramRow({ surface, ex, d }) {
  return (
    <div>
      <div style={{ fontWeight: 700, fontSize: '0.8rem', color: 'var(--lb-teal)' }}>{surface}</div>
      <code style={{ display: 'inline-block', marginTop: 2 }}>{ex}</code>
      <div style={{ fontSize: '0.74rem', color: 'var(--lb-muted)', marginTop: 2 }}>{d}</div>
    </div>
  )
}

// ─────────────────────────────────────────────────────────────────────────────
// AUTH VIEW
// ─────────────────────────────────────────────────────────────────────────────

function AuthView() {
  return (
    <>
      {/* The principal */}
      <section className="lb-section">
        <h2 className="lb-section-title">The principal — two ways in</h2>
        <div className="lb-card">
          <p style={{ margin: '0 0 6px', color: 'var(--lb-muted)', fontSize: '0.85rem' }}>
            A token is an <strong style={{ color: 'var(--lb-fg)' }}>Ed25519-signed JWT</strong>{' '}
            (<code>alg: EdDSA</code>). One token authorizes exactly one workspace. No unverified
            principal can exist.
          </p>
          <div className="lb-token">
            <span className="seg h">header</span>
            <span style={{ alignSelf: 'center', color: 'var(--lb-muted)' }}>.</span>
            <span className="seg p">payload (claims)</span>
            <span style={{ alignSelf: 'center', color: 'var(--lb-muted)' }}>.</span>
            <span className="seg s">signature</span>
          </div>
          <div className="lb-claims">
        {'{ '}
        <span className="k">"sub"</span>: <span className="c">"user:ada"</span>,{' '}
        <span className="k">"ws"</span>: <span className="c">"acme"</span>,{' '}
        <span className="k">"role"</span>: <span className="c">"member"</span>,{' '}
        <span className="k">"caps"</span>: <span className="c">["mcp:hvac.setpoint:call"]</span>,{' '}
        <span className="k">"exp"</span>: <span className="c">100</span>{' '}
        {' }'}
          </div>

          <div className="lb-grid2" style={{ marginTop: 16 }}>
            <div className="lb-callout">
              <strong>Human login</strong>
              <div style={{ marginTop: 4 }}>
                dev credential → <code>session.mint()</code> resolves grants → signed token.{' '}
                (Password / OIDC / SSO / MFA are the later pluggable slice behind the same{' '}
                <code>verify</code> seam.)
              </div>
            </div>
            <div className="lb-callout is-teal">
              <strong style={{ color: 'var(--lb-teal)' }}>API key (machine)</strong>
              <div style={{ marginTop: 4 }}>
                bearer <code>lbk_{'{ws}.{id}.{secret}'}</code> → per-request verify (HMAC-SHA256
                pepper) → <code>Principal::for_key</code>. The credential <em>is</em> the bearer —
                never exchanged for a token.
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Three gates */}
      <section className="lb-section">
        <h2 className="lb-section-title">caps::check — three gates, one chokepoint</h2>
        <div className="lb-card">
          <div className="lb-gates">
            <Gate
              n="1"
              color="var(--lb-red)"
              title="Workspace isolation — the hard wall"
              d="principal.ws == request.ws, else Denied::Workspace. Checked before any capability. Structural, never a runtime if."
              speed="always"
            />
            <Gate
              n="2"
              color="var(--lb-amber)"
              title="Capability"
              d="some held cap pattern-matches (surface, resource, action). Caps are a cached projection in the token — stale until re-mint."
              speed="stale until re-mint"
            />
            <Gate
              n="3"
              color="var(--lb-teal)"
              title="Membership / visibility (shared assets)"
              d="owner · team-member · channel sub-grantee. SurrealDB relation edges re-resolved on every read."
              speed="live"
            />
          </div>
          <div className="lb-callout" style={{ marginTop: 16 }}>
            <strong>The freshness asymmetry (load-bearing).</strong> Removing Bob from a team drops
            resource access <em>live</em> (Gate 3, next read) but his <em>inherited caps</em> linger
            in his current token until it expires / re-mints (Gate 2). Short TTLs +{' '}
            <code>user.disable</code> (kills minting) close the window.
          </div>
        </div>
      </section>

      {/* Where caps come from */}
      <section className="lb-section">
        <h2 className="lb-section-title">Where caps come from — the grant model</h2>
        <div className="lb-card">
          <p style={{ margin: '0 0 12px', color: 'var(--lb-muted)', fontSize: '0.85rem' }}>
            The token is a <em>cached projection</em>. The grant store is the source of truth.
          </p>
          <div className="lb-formula">
            token.caps = (<b>user grants</b> ∪ <b>role caps</b> ∪ <b>team-inherited grants</b>) ∩
            workspace
          </div>

          <div className="lb-subjects" style={{ margin: '16px 0' }}>
            <Subject ic="👤" nm="user" d="direct grants + assigned roles" />
            <Subject ic="👥" nm="team" d="members inherit team's grants" />
            <Subject ic="🔖" nm="role" d="a named cap bundle (3 built-ins + custom)" />
            <Subject ic="🔑" nm="key" d="machine principal; caps = its grants" />
          </div>

          <div className="lb-grid2">
            <div className="lb-callout">
              <strong>Administered as data</strong>
              <div style={{ marginTop: 4 }}>
                <code>grants.assign/revoke</code>, <code>roles.define/assign</code>,{' '}
                <code>teams.create/add_member/remove_member</code>, <code>members.remove</code> — all
                capability-gated admin verbs writing the records the gates read.
              </div>
            </div>
            <div className="lb-callout is-teal">
              <strong style={{ color: 'var(--lb-teal)' }}>No widening</strong>
              <div style={{ marginTop: 4 }}>
                A definer/assigner may grant only caps <em>they themselves hold</em>. Built-in roles
                (super-admin · workspace-admin · member) are seeded & immutable.
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Admin console map */}
      <section className="lb-section">
        <h2 className="lb-section-title">The admin console — one place, five tabs</h2>
        <div className="lb-card">
          <p style={{ margin: '0 0 14px', color: 'var(--lb-muted)', fontSize: '0.85rem' }}>
            Cap-gated for display only. The gateway re-checks every verb server-side — the UI gate
            is convenience, never the boundary.
          </p>
          <div className="lb-gates">
            <AdminRow tab="People" verbs="user.create · disable · delete" maps="users + their teams / roles / access" />
            <AdminRow tab="Teams" verbs="teams.create · rename · delete · members add/remove" maps="team record + inline roster + access editor" />
            <AdminRow tab="Roles" verbs="roles.define · assign · revoke" maps="the cap-bundle editor (no widening)" />
            <AdminRow tab="Workspaces" verbs="workspace.rename · archive · purge" maps="workspace lifecycle (soft then guarded hard-delete)" />
            <AdminRow tab="API Keys" verbs="apikey.create · rotate · revoke" maps="machine principals + their resolved caps" />
          </div>
        </div>
      </section>
    </>
  )
}

function Gate({ n, color, title, d, speed }) {
  return (
    <div className="lb-gate">
      <div className="num" style={{ background: color }}>
        {n}
      </div>
      <div>
        <div className="gt">{title}</div>
        <div className="gd">{d}</div>
        <span className="speed" style={{ color }}>
          {speed}
        </span>
      </div>
    </div>
  )
}

function Subject({ ic, nm, d }) {
  return (
    <div className="lb-subj">
      <div className="ic">{ic}</div>
      <div className="nm">{nm}</div>
      <div className="ds">{d}</div>
    </div>
  )
}

function AdminRow({ tab, verbs, maps }) {
  return (
    <div className="lb-gate">
      <div
        className="num"
        style={{ background: 'var(--lb-violet)', color: '#1a1033', fontSize: '0.7rem', lineHeight: 1.1, textAlign: 'center' }}
      >
        {tab}
      </div>
      <div>
        <div className="gt">{maps}</div>
        <div className="gd">
          <code>{verbs}</code>
        </div>
      </div>
    </div>
  )
}
