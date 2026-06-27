# 0004: Worked example — Consumer IoT (the Daikin model)

The **B2C sibling** of `0003-iot-dashboard.md`. Same core, same extensions, same "the kernel never
knows the word 'sensor'" thesis — but a different **tenancy shape**. Where 0003 is **B2B** (one
workspace = one *company*, e.g. KFC vs McDonald's), this is **B2C**: **one workspace = one
*household*.** A person owns their home, can be invited into a friend's home for a weekend, and
leaves cleanly when it's over.

> **The one thing to take away:** nothing new is needed in the core for this. A **global identity
> with many workspace memberships** + the **workspace switcher** + **member invite/remove** +
> **grant-by-tag** — all already shipped — *are* the consumer model. The only thing this shape wants
> that the platform does not yet have is a level **above** the workspace (the brand seeing across all
> households) — the deferred **org tier** (README §2, §7). That is the single finding.

---

## 1. The product

A vendor — call it **Daikin** — sells home HVAC (an AC unit that joins home Wi-Fi over a P1/P2
bridge) and a phone/web app to control it. A customer:

- creates an **account**,
- has **one or more homes** (a primary house, a holiday flat),
- connects appliances (the AC) to a home,
- and sometimes wants **another person** to control an appliance — a partner permanently, a
  weekend guest temporarily.

The hard requirement is the same as 0003's: **one customer must never see another customer's
home.** In B2C this matters even more — the tenants are *people*, and the blast radius of a leak is
someone's house.

---

## 2. The mapping (workspace = household)

| Concept in the product | Core primitive | Notes |
|---|---|---|
| **A customer's home** | a **workspace** (the hard wall) | One SurrealDB namespace + `ws/{home}/**` bus prefix + its own secrets. The unit of isolation. |
| **A person's account** | a **global identity** | One identity, independent of any workspace (README §6.6). The same login is used everywhere. |
| **"My home"** | a workspace I **own / admin** | Created at sign-up; I am its workspace-admin. |
| **A second house I own** | a sub-division *inside* my workspace | A SurrealDB **database** under my namespace (README §6.1), or just a tag (`home:holiday-flat`). Both my houses are mine, one wall. |
| **My AC unit** | a **producer / appliance** | Authenticates with a workspace-bound node token; is itself an access-controlled **resource** (0003 §2). The core never knows "AC" — only series + samples + tags. |
| **Inviting my partner** | `members.add` (permanent) | They become a member of my workspace; my homes appear in their switcher. |
| **A weekend guest** | `members.add` + a **scoped grant** (temporary) | Added as a member but granted only the appliance(s) I choose, by tag (below). |
| **Guest leaves** | `members.remove` / revoke grant | Membership is re-resolved live on every read (tenancy-scope.md), so access drops **instantly**. |

**Daikin itself is not a workspace** in this picture — Daikin is the **hub operator** (the
super-admin running the cloud, README §6.6). It hosts thousands of household workspaces and keeps
them walled. (When Daikin wants to look *across* households — fleet health, a firmware rollout — it
hits the gap in §6.)

---

## 3. End-to-end flow — "an AC in my house, then a weekend at a friend's"

1. **Sign up.** I create an account → a **global identity**. The app provisions my home **workspace**
   `home:alex` and makes me its admin. (Solo/edge or hub-hosted is config, not a code branch — §3
   rule 1; a consumer app is normally hub-hosted with the phone as a `mobile` thin client.)
2. **Add a home + connect the AC.** I register my "holiday-flat" as a sub-division of my workspace.
   The AC joins Wi-Fi via the P1/P2 bridge, authenticates with a workspace-bound node token, and
   **announces presence** → it shows **online** in my app. `sensor-source` emits `Sample`s
   (setpoint, room temp, mode) into ingest; history is a store query, live values stream over the
   bus (§3 motion-vs-state, exactly as 0003).
3. **Control it.** Setting the temperature is a **gated MCP tool call** (`hvac.set` or similar) —
   Gate 1 workspace ✓, Gate 2 capability ✓. The phone, the web app, and (later) an AI agent all
   call the *same* tool the same way (README §6.5/§7).
4. **Invite my partner (permanent).** `members.add(home:alex, jordan)`. Jordan's one identity now
   has a membership in my workspace; my homes appear in **their workspace switcher** alongside
   their own. Full household access.
5. **Get invited to a friend's for the weekend.** My friend Sam runs
   `members.add(home:sam, alex)` and grants me **only the guest-room AC**, by tag:
   `grant(alex → appliances tagged room:guest)`. I switch to `home:sam` in the switcher and can set
   the guest-room AC — but **not** see Sam's main thermostat, security, or energy data. This is the
   second isolation layer *below* the wall (membership + grant-by-tag, tenancy-scope.md / 0003 §6).
6. **The wall holds.** While I hold a membership in both `home:alex` and `home:sam`, I am **never** a
   bridge between them: each tool call carries exactly one `ws` claim; my caps in Sam's home cannot
   name a key in mine, and vice-versa. One identity, two walled contexts.
7. **Weekend ends.** Sam runs `members.remove(home:sam, alex)` (or just lets a time-boxed grant
   expire). Because access is re-resolved on every read, my access to the guest AC is gone on the
   next call — no token to chase down, no stale capability minted into a JWT.

The core never knew "Daikin", "AC", or "guest". It knew identities, workspaces, memberships,
grants, series, samples, tags, presence, and one routed tool namespace.

---

## 4. Why workspace-per-household is the right B2C mapping

There are two ways to model consumers; only one is safe in v1.

- **✅ Workspace per household (this doc).** Each customer's home is its own hard wall. Guest access
  is a membership + a scoped grant. Fits the shipped model with **zero new core concepts** — it is
  literally the Slack pattern (your own workspace + workspaces you're invited to) applied to homes.
- **❌ Daikin as one workspace, every customer a member.** This *breaks the wall*: all customers
  would share one namespace, and the membership graph is **not** a tenant boundary — it is
  within-workspace collaboration. One customer could reach another's home. Do not do this.

The B2C shape is, if anything, a *better* fit for the platform's primitives than B2B, because
"global identity belongs to many workspaces and switches between them" was designed for exactly
"I have my home and I'm a guest in yours."

---

## 5. The one finding: the org tier (brand-wide visibility)

Workspace-per-household solves customer isolation **and** guest invites completely. It does **not**,
by itself, give **Daikin the brand** a view *across* all those households:

- fleet health ("how many units are offline in EMEA?"),
- a **firmware rollout** to every unit,
- model-level analytics, or tier-2 support reaching into a customer's home **with consent**.

That is a level **above** the workspace — precisely the **"org tier" deferred to v2** in README §2
(non-goals) and §7 (entity model). Note this is the *mirror* of B2B's deferral: 0003's KFC wanted an
org *grouping companies*; here Daikin wants an org *grouping households under a brand*. Same missing
tier, approached from the other side.

**Interim options without the org tier (each a deliberate trade-off):**

- **Cross-workspace super-admin tooling** — Daikin operates as hub super-admin and uses a small,
  **heavily-audited** cross-workspace platform extension for fleet/firmware (README §11.5 flags
  cross-workspace extensions as *the* isolation risk — keep this set near-empty). Good enough for
  ops; **not** a per-customer data product.
- **Consented egress** — a household workspace **opts in** to publish anonymised/aggregate samples
  to a Daikin-owned analytics workspace via the **outbox/job egress** pattern (0003 §6's
  TimescaleDB adapter). Keeps the wall intact (data *leaves* by consent, the brand never *reaches
  in*).
- **Pull the org tier forward** — make `org` the new outer isolation boundary with workspaces as
  subdivisions (README §7's stated migration path). The biggest change; only justified when
  brand-wide visibility is a core requirement, not a nice-to-have.

**Recommendation:** ship consumer households on the existing model now (it needs no core work);
treat the org tier as a *scoped* decision driven by whether Daikin's brand-wide visibility is
actually a v1 product requirement. Until then, prefer **consented egress** over cross-workspace
super-admin reach — it keeps the hard wall honest.

---

## 6. Why this is a good design probe

- **Tenancy stress test** — proves the workspace wall holds when tenants are *people*, not
  companies, and when one identity legitimately lives in several workspaces at once.
- **Membership + grant-by-tag** — exercises the second isolation layer (guest sees one appliance,
  not the house) that 0003 introduced at fleet scale, now at household scale.
- **Lifecycle** — invite/scope/revoke as the everyday path, with revocation that bites on the next
  read (no token-chasing).
- **Surfaces the org-tier decision cleanly** — isolates the *one* thing the current model can't do
  (brand-across-households) from the many things it can, so the v2 call is made on evidence.

---

## 7. Related

- `vision/0003-iot-dashboard.md` — the **B2B** sibling (KFC & McDonald's; workspace = company).
- `vision/0002-coding-agent-workplace.md` — the original "all extensions" worked example.
- README §6.6 (global identity, super-admin → workspace-admin → member), §7 (workspace = tenant;
  the deferred org tier), §3.6 (isolation is gate 1).
- `scope/tenancy/tenancy-scope.md` (the wall, structural on every surface; the membership graph as
  the second isolation layer).
- `scope/auth-caps/authz-grants-scope.md` (grant-by-tag — the scoped-guest mechanism).
- `scope/node-roles/node-connection-scope.md` (appliance ↔ hub), `scope/ingest/ingest-scope.md`
  (the `Sample` envelope; "device = a producer, not a registry entry").
