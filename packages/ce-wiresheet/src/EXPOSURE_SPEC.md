# Folder exposure — specification

Status: spec. Defines how folder **exposed-port** records in the `__facets`
property are formatted, computed, and **maintained**. The maintainer is **ce-rest**,
driven by the engine event channel (see §6 for why, and `API_REQUESTS.md §3`).

There are three implementations of the *algorithm* (§4), all verified against the
same golden vectors (`lib/exposure-vectors.json`):

1. **ce-rest** — the authoritative maintainer (this spec, §5–§7).
2. **client JS** (`lib/exposure.ts`) — executable spec + optimistic preview.
3. (legacy) the C++ Folder component, being retired in favour of ce-rest.

The core engine stores `__facets` as an **opaque string** (a `ROLE_FACETS`
property); it does not interpret it. Exposure is a wiresheet *presentation*
projection and lives in the presentation layer (ce-rest), not the runtime.

---

## 1. Terms

- **Component / property / edge** — the engine model. A component owns properties
  (`component → prop`; with the landed owner field, `prop.componentUid` inverts it).
  An edge connects `source.prop → target.prop` and carries `loopBack`/`hidden`.
- **Folder / container** — any component with children (not only `core-extRoot::Folder`;
  a plain `math::add` that nests children is a container too — key on "has children
  + a boundary-crossing edge", never on `type`).
- **Subtree of F** — F's strict descendants (F itself is *not* inside F).
- **Boundary edge of F** — an edge with **exactly one** endpoint inside F's subtree.
- **Exposed port** — a child property F projects as its own input/output port,
  recorded in F's `__facets`. The record **key is the deep property uid**.
- **Chained port** — when the inside endpoint sits below a *child folder* of F, F
  re-projects that child folder's port rather than re-exposing the deep child
  (so the prop is exposed exactly once per ancestor level).

---

## 2. The `__facets` wire format

`__facets` is a control-character-delimited string. No escaping is needed — the
delimiters never appear in user text. Empty string = no facets.

### 2.1 Delimiters

| Name | Byte | Role |
|---|---|---|
| RS | `0x1E` | between property **records** |
| US | `0x1F` | between **fields** within a record |
| GS | `0x1D` | between **alias items** (within an `o` field) |
| FS | `0x1C` | between an alias's **code and label** |

### 2.2 Record layout

```
record   := uid (US field)*
string   := record (RS record)*
```

- The first field of a record is the **property uid** (decimal). For an exposed-port
  record this is the **deep child** prop uid; for a presentation facet it is the
  owning component's own prop uid.
- Each subsequent field is `<tag><value>`, `tag` a single char:

| Tag | Field | Value | Owner |
|---|---|---|---|
| `l` | label | string | **user** |
| `u` | unit | string | user |
| `d` | decimals | int, clamped 0–10 | user |
| `n` | min | number | user |
| `x` | max | number | user |
| `h` | hidden | `1` = hidden | user |
| `r` | order | number (row sort) | user |
| `t` | format | `datetime`\|`date`\|`time` | user |
| `a` | action | string | user |
| `o` | aliases | `code FS label` items, GS-separated (bare `code` ⇒ label = code) | user |
| `e` | **expose** | `o` = output, `i` = input | **maintainer** |
| `c` | **childComponent** | int — the chain **link** (direct child / inner folder) | maintainer |
| `f` | **facetProp** | int — the link's `__facets` prop uid (for live label streaming) | maintainer |
| `k` | **chain** | `1` = chained | maintainer |
| `w` | **owner** | int — the **real** owner component uid (NEW, §7) | maintainer |
| `m` | **name** | string — the owner prop's name (NEW, §7) | maintainer |

- Field order within a record is not significant. Unknown tags are ignored
  (forward-compatible).
- Records with only a uid (no fields) are dropped on serialize.

### 2.3 Exposure record vs presentation record

A record is an **exposure record** iff it carries an `e` field. The maintainer owns
exactly the `e / c / f / k / w / m` fields of exposure records and **must preserve**
every user-owned field and every non-exposure record verbatim (see §5.3). A single
record may be both — a user-labelled exposed port carries `e…` (maintainer) and a
user `l…` (user); the maintainer never writes `l`.

---

## 3. Worked format example

Folder `100027` exposes one direct port and one chained port:

```
1000100 e o  c 100013 f 1000101 w 100013 m out                    (direct: add-4.out)
1000086 l out  e o  c 100012 f 1000098 k 1  w 100009 m out         (chained via folder 100012)
```

Encoded (US shown as `·`, RS as `¶`):

```
1000100·eo·c100013·f1000101·w100013·mout¶1000086·lout·eo·c100012·f1000098·k1·w100009·mout
```

The first record: prop `1000100` is exposed as an **o**utput, link/owner is `100013`
(`add-4`), named `out`. The second: prop `1000086` is exposed as an output **chained**
through child folder `100012`; the chain link `c` is the inner folder, but the
resolved owner `w` is the real deep owner `100009`.

---

## 4. Exposure algorithm (`computeFolderExposure`)

Pure function. Input: a folder `F`, the `parents` map (`uid → parentUid`), the set
of `edges` (`{s, sp, t, tp}`), and `facetProp` (`uid → its __facets prop uid`).
Output: the exposed-port records for `F`, one per exposed prop, sorted by prop uid.

```
records := {}                                  # keyed by deep prop uid
for each edge (s, sp, t, tp):
    sIn := isInside(s, F)                       # strict descendant
    tIn := isInside(t, F)
    if sIn == tIn: continue                     # internal or fully external — skip
    cin  := sIn ? s  : t                        # inside endpoint component
    pin  := sIn ? sp : tp                       # inside endpoint property (the key)
    side := sIn ? output : input
    via  := directChildOfFOnPathTo(cin)         # walk up from cin until parent == F
    if via is undefined: continue               # defensive: broken parent map
    chain := (via != cin)
    link  := chain ? via : cin                  # inner folder for chain, real owner for direct
    records[pin] := { prop: pin, side, childComponent: link,
                      facetProp: facetProp[link], chain,
                      owner: componentUid(pin), name: propName(pin) }   # §7
return sort(records.values by prop)
```

Helpers (both bounded by `MAX_DEPTH = 256` to survive a broken/cyclic parent map):

- `isInside(x, F)` — walk `x`'s ancestors; `true` on reaching `F`, `false` on root.
- `directChildOfFOnPathTo(cin)` — walk up from `cin` until the parent is `F`; return
  that node (returns `cin` itself when `cin`'s parent is already `F`).

### 4.1 Rules that fall out (and matter)

- **Exactly-once per prop.** A prop with several boundary edges (e.g. an output
  fanning to two external inputs) is keyed once → one record. Dedup by `pin`.
- **Internal edges never expose.** Both ends inside `F` (incl. a loopback from a
  deep output back to a deep input in the *same* F) → `sIn == tIn` → skipped. This
  is the loopback case the client otherwise had to special-case.
- **Chained, not deep-exposed.** When `cin` is below a child folder, `via != cin`,
  so `F` records the **child folder** as the link with `chain=1`; the deep child is
  exposed by *that* folder, and `F` re-projects it. Following `c` down each level
  reaches the real owner.
- **Side** is from F's perspective: inside source ⇒ output port; inside target ⇒
  input port.
- **User-facing only.** `pin` must be a normal-role property (skip system roles).

---

## 5. Maintenance (ce-rest, event-driven)

ce-rest subscribes to the engine event channel and keeps every affected folder's
`__facets` correct. Exposure is a pure function of (hierarchy, edges); the channel
delivers every mutation of both, so the maintained value always converges.

### 5.1 Trigger events

Recompute on the **topology** events:

| Event | Affected folders |
|---|---|
| edge added | folders for which the new edge is a boundary edge (§5.2) |
| edge removed | folders for which the removed edge *was* a boundary edge |
| component reparented | boundary set of every edge touching the moved subtree, computed against **both** old and new parent chains |
| component removed | boundary set of edges touching the removed subtree (its records vanish with it; its ancestors lose any port it fed) |

Do **not** trigger on `ROLE_FACETS` (`__facets`) property changes — that is the
maintainer's own write (loop guard, §6).

### 5.2 Affected-folder set for an edge `(s, t)`

The folders for which `(s, t)` is a boundary edge are exactly the ancestors of `s`
**xor** of `t`: walk up from `s` and from `t` to their lowest common ancestor `L`;
the boundary folders are the proper ancestors of `s` up to (not including) `L`,
plus the proper ancestors of `t` up to (not including) `L`. (For each such `F`,
exactly one of `s,t` is inside.) This is `O(depth)`, not a subtree scan.

### 5.3 Recompute + write-back (per affected folder `F`)

1. **Read** `F.__facets`, parse to records.
2. **Recompute** the exposure record set for `F` via §4 (full recompute of `F` is
   always correct and idempotent; an incremental update of just the touched prop's
   record across the affected ancestor chain is a permitted optimization iff it
   yields the identical result).
3. **Merge, preserving user fields:**
   - replace the maintainer-owned fields (`e c f k w m`) of each surviving exposure
     record; keep its user fields (`l u d n x h r t a o`) untouched;
   - **add** records for newly-crossing props;
   - **remove** exposure records whose boundary edge is gone — but only the `e c f k
     w m` fields; if a removed record still carries user fields, drop the whole
     record only when nothing user-owned remains (a bare port row carries none, so
     it is removed entirely).
4. **Write** the re-serialized string back **iff it changed** (no-op writes must not
   emit, to keep the channel quiet and avoid needless client churn).

### 5.4 Atomic group / ungroup (optional, `API_REQUESTS.md §4`)

A server-side `POST /group` / `/ungroup` should compute the new folder's exposure
**and** re-chain the old parent inline, in the same transaction, before responding —
so the client sees a fully-formed result without waiting for the event round-trip.
The reactive path (§5.1) still covers all other mutation sources.

### 5.5 Copy / paste (`API_REQUESTS.md §5`)

`/copy/nodes` clones a subtree and its **internal** edges (external edges dropped)
and returns a `uidMap`. Because the engine copies `__facets` **verbatim**, the
clone's records still reference the **original** uids — a pasted container's ports
would point back at the source. Two ways the maintainer resolves this:

1. **Regeneration (preferred — falls out of §5).** The cloned internal edges publish
   edge-added events, so the maintainer recomputes the copied folders' `__facets`
   from them (§5.1–5.3) with correct new uids. The verbatim original-uid copy is
   transient and overwritten. **With reactive maintenance, `/copy/nodes` does not
   need to remap the string** — the maintainer rebuilds it.

2. **Topology-pure consequence.** A copied folder then exposes exactly what its
   *cloned* edges justify. A port that was driven by an edge to an **external**
   partner (not in the copied set) has no cloned edge → it is **not** regenerated; it
   returns when the copy is rewired. This is the consistent default (exposure =
   f(topology)).

   If **shape-preserving** copy is required instead — retain externally-driven ports
   as dangling, ready-to-wire, so the paste is structurally isomorphic to the source
   — then `/copy/nodes` **must** remap the verbatim `__facets` uid refs (the record
   key = deep prop uid, plus `c` / `f` / `w`) through the `uidMap`, **and** the
   maintainer must not strip retained records that lack a current boundary edge.
   Adopt this only if the UX demands it; topology-pure is simpler and fully
   consistent.

Either way the client's `lib/facet.ts::remapFacetUids` and the facet-remap half of
`lib/paste.ts::planPaste` are deleted — copy exposure becomes the engine/maintainer's
responsibility. (The existing client `remapFacetUids` is the executable reference for
the shape-preserving remap, should you take that route.)

---

## 6. Idempotency, loops, ordering

- **Pure ⇒ idempotent.** Exposure is a function of current topology only. Re-running
  a recompute (e.g. after an undo/redo event re-adds an edge) yields the same record
  set. No history needed.
- **Loop guard.** The write-back in §5.3 emits a `ROLE_FACETS` change event; the
  maintainer must ignore `ROLE_FACETS` events as triggers. Only topology events fire
  recompute. The "write iff changed" rule bounds any residual churn to zero at the
  fixpoint.
- **Coverage is the contract.** This design is correct **iff every** topology
  mutation — REST, SDK, and undo/redo replay — publishes to the channel. (Confirmed:
  the channel sees all and can push all.) If any mutation ever bypasses the channel,
  exposure goes stale and the maintainer must move into the engine (first-class),
  per `API_REQUESTS.md §3`.
- **Ordering.** Recompute is reactive: the topology event is published, then the
  `__facets` write, then its event. Subscribers receive topology-then-exposure.
  Clients tolerate this via a reload generation; for a strict single-frame result,
  use the inline group op (§5.4).

---

## 7. Owner & name resolution (uses the landed `prop.componentUid`)

The property→component owner is now intrinsic (`API_REQUESTS.md §1`), so the
maintainer fills two enrichment fields the client otherwise back-derived:

- **`w` (owner)** — `componentUid(pin)`, the **real** deep owner. For a direct
  record `w == c`; for a chained record `c` is the inner folder (chain link) and `w`
  is the terminal owner. With `w` present, the client wires a new edge to the port
  directly (no `buildPropOwners`), and routing resolves the owner without the
  deep-subtree walk.
- **`m` (name)** — the owner prop's name. Kills the `#<uid>` fallback: an
  engine-written port renders with its real name immediately. `l` (user label, if
  any) still wins for display; `m` is the canonical fallback.

`f` (facetProp) remains — it points at the link's `__facets` so the client can
subscribe to live unit/alias metadata of the deep prop.

---

## 8. Edge cases

- **Fan-out output (incl. loopback).** `out` feeds external `X` (boundary) and
  internal `Y` (internal). Exposed once as output (the boundary edge); the internal
  edge is skipped. A loopback `out → in` within the same F is internal → no port.
- **Delete the external partner.** Removing the *outside* end of a boundary edge
  removes the boundary → F un-exposes that prop (unless another boundary edge keeps
  it). Affected set = ancestors of the inside end.
- **Reparent in / out.** Moving a component changes which edges cross which folders;
  recompute against both old and new parent chains (§5.1).
- **Deep chains.** Each ancestor folder chains via its own direct child; `c` is the
  direct child at that level, `w` the shared terminal owner. Following `c` per level
  reconstructs the chain; `w` short-circuits it.
- **Root.** Root is the top container and exposes nothing upward — never write
  exposure records to root.
- **Cycles / broken parent maps.** Bounded walks (`MAX_DEPTH`) make `isInside` /
  `directChildOfFOnPathTo` total; a defended-against `via == undefined` skips the
  edge rather than emitting a bad record.

---

## 9. Verification

- **Golden vectors:** `lib/exposure-vectors.json` — each case is
  `{ folder, parents, edges, facetProp } → expected records`. Every implementation
  (§0) must reproduce them exactly. ce-rest's maintainer runs the same vectors in
  its test suite.
- **Round-trip:** `parseFacet(serializeFacet(x)) == x` for all field combinations,
  including the new `w` / `m` fields and user fields preserved through a recompute.
- **Idempotency:** recompute(recompute(state)) == recompute(state) for a fixed
  topology (the fixpoint / no-op-write rule).
- **Maintenance invariants (property tests):** after any sequence of edge
  add/remove / reparent / delete events, every folder's `__facets` equals
  `computeFolderExposure` over the resulting topology — the oracle the client model
  tester already uses (`lib/grouping.model.test.ts`).
